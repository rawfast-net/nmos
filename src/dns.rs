use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Clone)]
pub struct Resolver {
    servers: Vec<SocketAddr>,
}

#[derive(Debug, Clone)]
pub struct Answer {
    pub rcode: u8,
    pub records: Vec<Ipv4Addr>,
    pub count: u16,
}

impl Resolver {
    pub fn system() -> Option<Self> {
        let mut servers = Vec::new();
        #[cfg(target_os = "linux")]
        if let Ok(config) = std::fs::read_to_string("/etc/resolv.conf") {
            for line in config.lines() {
                let mut parts = line.split_whitespace();
                if parts.next() == Some("nameserver") {
                    if let Some(server) = parts
                        .next()
                        .and_then(|value| value.parse::<std::net::IpAddr>().ok())
                    {
                        servers.push(SocketAddr::new(server, 53));
                    }
                }
            }
        }
        #[cfg(target_os = "windows")]
        if let Ok(output) = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", "(Get-DnsClientServerAddress -AddressFamily IPv4 | Where-Object { $_.ServerAddresses }).ServerAddresses | Select-Object -Unique"])
            .output()
        {
            for line in String::from_utf8_lossy(&output.stdout).lines() {
                if let Ok(server) = line.trim().parse::<std::net::IpAddr>() {
                    servers.push(SocketAddr::new(server, 53));
                }
            }
        }
        servers.truncate(3);
        if servers.is_empty() {
            None
        } else {
            Some(Self { servers })
        }
    }

    pub fn query(&self, name: &str, record_type: u16) -> Result<Answer, String> {
        let packet = build_query(name, record_type)?;
        for server in &self.servers {
            if let Ok(answer) = exchange(*server, &packet) {
                return Ok(answer);
            }
        }
        Err("DNS-сервер не ответил".into())
    }
}

fn build_query(name: &str, record_type: u16) -> Result<Vec<u8>, String> {
    let id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos() as u16;
    let mut packet = vec![0; 12];
    packet[0..2].copy_from_slice(&id.to_be_bytes());
    packet[2..4].copy_from_slice(&0x0100u16.to_be_bytes());
    packet[4..6].copy_from_slice(&1u16.to_be_bytes());
    for label in name.trim_end_matches('.').split('.') {
        if label.is_empty() || label.len() > 63 || !label.is_ascii() {
            return Err("Неверное DNS-имя".into());
        }
        packet.push(label.len() as u8);
        packet.extend_from_slice(label.as_bytes());
    }
    packet.push(0);
    packet.extend_from_slice(&record_type.to_be_bytes());
    packet.extend_from_slice(&1u16.to_be_bytes());
    Ok(packet)
}

fn exchange(server: SocketAddr, packet: &[u8]) -> Result<Answer, String> {
    let bind = if server.is_ipv4() {
        "0.0.0.0:0"
    } else {
        "[::]:0"
    };
    let socket = UdpSocket::bind(bind).map_err(|_| "Не удалось открыть UDP")?;
    socket
        .set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(|_| "Не удалось установить таймаут")?;
    socket
        .connect(server)
        .map_err(|_| "DNS-сервер недоступен")?;
    socket
        .send(packet)
        .map_err(|_| "Не удалось отправить DNS-запрос")?;
    let mut response = [0u8; 4096];
    let len = socket.recv(&mut response).map_err(|_| "Таймаут DNS")?;
    parse_response(packet, &response[..len])
}

fn parse_response(query: &[u8], response: &[u8]) -> Result<Answer, String> {
    if response.len() < 12 || response[0..2] != query[0..2] {
        return Err("Неверный DNS-ответ".into());
    }
    let flags = u16::from_be_bytes([response[2], response[3]]);
    if flags & 0x8000 == 0 || flags & 0x0200 != 0 {
        return Err("Неполный DNS-ответ".into());
    }
    let questions = u16::from_be_bytes([response[4], response[5]]);
    let count = u16::from_be_bytes([response[6], response[7]]);
    let mut offset = 12usize;
    for _ in 0..questions {
        offset = skip_name(response, offset)?;
        offset = offset
            .checked_add(4)
            .filter(|&end| end <= response.len())
            .ok_or("Некорректный DNS-вопрос")?;
    }
    let mut records = Vec::new();
    for _ in 0..count {
        offset = skip_name(response, offset)?;
        if offset + 10 > response.len() {
            return Err("Некорректный DNS-ответ".into());
        }
        let kind = u16::from_be_bytes([response[offset], response[offset + 1]]);
        let class = u16::from_be_bytes([response[offset + 2], response[offset + 3]]);
        let length = u16::from_be_bytes([response[offset + 8], response[offset + 9]]) as usize;
        offset += 10;
        if offset + length > response.len() {
            return Err("Некорректная DNS-запись".into());
        }
        if kind == 1 && class == 1 && length == 4 {
            records.push(Ipv4Addr::new(
                response[offset],
                response[offset + 1],
                response[offset + 2],
                response[offset + 3],
            ));
        }
        offset += length;
    }
    Ok(Answer {
        rcode: (flags & 15) as u8,
        records,
        count,
    })
}

fn skip_name(packet: &[u8], mut offset: usize) -> Result<usize, String> {
    loop {
        let length = *packet.get(offset).ok_or("Некорректное DNS-имя")?;
        if length & 0xc0 == 0xc0 {
            return if offset + 1 < packet.len() {
                Ok(offset + 2)
            } else {
                Err("Некорректное DNS-сжатие".into())
            };
        }
        if length & 0xc0 != 0 {
            return Err("Некорректное DNS-имя".into());
        }
        offset += 1;
        if length == 0 {
            return Ok(offset);
        }
        offset += length as usize;
        if offset > packet.len() {
            return Err("Некорректное DNS-имя".into());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distinguishes_answer_and_nxdomain() {
        let query = build_query("2.0.0.127.example.org", 1).unwrap();
        let mut response = query.clone();
        response[2..4].copy_from_slice(&0x8180u16.to_be_bytes());
        response[6..8].copy_from_slice(&1u16.to_be_bytes());
        response.extend_from_slice(&[0xc0, 0x0c, 0, 1, 0, 1, 0, 0, 0, 60, 0, 4, 127, 0, 0, 2]);
        assert_eq!(
            parse_response(&query, &response).unwrap().records,
            vec![Ipv4Addr::new(127, 0, 0, 2)]
        );
        response[3] = 0x83;
        response[6..8].copy_from_slice(&0u16.to_be_bytes());
        assert_eq!(parse_response(&query, &response).unwrap().rcode, 3);
    }
}
