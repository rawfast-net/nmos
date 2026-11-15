use rayon::prelude::*;
use serde::Serialize;
use std::net::{TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize)]
pub struct MailEgressReport {
    pub targets: Vec<MailTarget>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MailTarget {
    pub purpose: &'static str,
    pub host: &'static str,
    pub port: u16,
    pub connected: bool,
    pub latency_ms: Option<f64>,
    pub detail: String,
}

const TARGETS: [(&str, &str, u16); 3] = [
    ("SMTP relay", "smtp.gmail.com", 587),
    ("SMTP relay", "smtp.office365.com", 587),
    ("Прямая отправка", "gmail-smtp-in.l.google.com", 25),
];

pub fn check() -> MailEgressReport {
    MailEgressReport {
        targets: TARGETS
            .par_iter()
            .map(|&(purpose, host, port)| {
                let addresses: Vec<_> = match (host, port).to_socket_addrs() {
                    Ok(addresses) => addresses.take(2).collect(),
                    Err(_) => Vec::new(),
                };
                if addresses.is_empty() {
                    return MailTarget {
                        purpose,
                        host,
                        port,
                        connected: false,
                        latency_ms: None,
                        detail: "DNS-имя не разрешено".into(),
                    };
                }
                let start = Instant::now();
                let connected = addresses.iter().any(|address| {
                    TcpStream::connect_timeout(address, Duration::from_secs(2)).is_ok()
                });
                MailTarget {
                    purpose,
                    host,
                    port,
                    connected,
                    latency_ms: connected.then(|| start.elapsed().as_secs_f64() * 1000.0),
                    detail: if connected {
                        "TCP-подключение успешно"
                    } else {
                        "Таймаут или отказ подключения"
                    }
                    .into(),
                }
            })
            .collect(),
    }
}
