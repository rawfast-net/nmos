use serde::Serialize;
use rayon::prelude::*;
use std::net::{TcpStream, ToSocketAddrs};
use std::process::Command;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize)]
pub struct NetworkInfo {
    pub hostname: String,
    pub interfaces: Vec<Iface>,
    pub default_route_guess: Option<String>,
    pub public_ip: Option<String>,
    pub geo_hint: Option<String>,
    
    pub resolved_region: String,
    pub dns_latency_ms: Option<f64>,
    pub latency_targets: Vec<LatencySample>,
    pub quality: Vec<QualitySample>,
    pub ipv6_egress: Option<Ipv6Egress>,
    pub speed: Option<SpeedResult>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Iface {
    pub name: String,
    pub ipv4: Vec<String>,
    pub ipv6: Vec<String>,
    pub mac: Option<String>,
    pub speed_mbps: Option<u64>,
    pub mtu: Option<u64>,
    pub rx_bytes: Option<u64>,
    pub tx_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LatencySample {
    pub name: String,
    pub host: String,
    pub latency_ms: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct QualitySample {
    pub name: String,
    pub host: String,
    pub attempts: usize,
    pub successful: usize,
    pub median_ms: Option<f64>,
    pub p95_ms: Option<f64>,
    pub jitter_ms: Option<f64>,
    pub loss_percent: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Ipv6Egress {
    pub connected: bool,
    pub local_address: Option<String>,
    pub latency_ms: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SpeedResult {
    pub backend: String,
    pub status: String,
    pub error: Option<String>,
    pub server_name: Option<String>,
    pub server_city: Option<String>,
    pub server_country: Option<String>,
    pub server_id: Option<u64>,
    pub ping_ms: Option<f64>,
    pub jitter_ms: Option<f64>,
    pub packet_loss_percent: Option<f64>,
    pub result_url: Option<String>,
    pub tested_at: Option<String>,
    pub attempts: Vec<SpeedAttempt>,
    pub measurements: Vec<SpeedMeasurement>,
    pub download_mbps: Vec<SpeedSample>,
    pub upload_mbps: Option<SpeedSample>,
    pub best_download_mbps: Option<f64>,
    pub avg_download_mbps: Option<f64>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SpeedMeasurement {
    pub download_mbps: f64,
    pub upload_mbps: Option<f64>,
    pub ping_ms: Option<f64>,
    pub jitter_ms: Option<f64>,
    pub packet_loss_percent: Option<f64>,
    pub tested_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SpeedAttempt {
    pub server_id: Option<u64>,
    pub server_name: String,
    pub status: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SpeedSample {
    pub label: String,
    pub url: String,
    pub mbps: f64,
    pub bytes: u64,
    pub seconds: f64,
}

pub struct NetOptions {
    pub fetch_public_ip: bool,
    pub run_speed: bool,
    
    pub region: String,
    pub run_quality: bool,
    pub offline: bool,
    pub speedtest_backend: String,
    pub speedtest_server_id: Option<u64>,
    pub speedtest_country: Option<String>,
    pub speedtest_city: Option<String>,
    pub speedtest_manual_country: bool,
    pub speedtest_accept_terms: bool,
    pub speedtest_repeat: bool,
}

pub fn collect(opts: NetOptions) -> NetworkInfo {
    let mut notes = Vec::new();
    let interfaces = collect_interfaces(&mut notes);
    let default_route_guess = guess_default(&interfaces);

    let public_ip = if opts.fetch_public_ip {
        fetch_public().or_else(|| {
            notes.push("Не удалось определить внешний IP".into());
            None
        })
    } else {
        notes.push("Проверка внешнего IP отключена".into());
        None
    };

    let geo = if opts.fetch_public_ip {
        fetch_geo_full()
    } else {
        None
    };
    let geo_hint = geo.as_ref().map(|g| g.hint.clone());
    let country_code = geo.as_ref().and_then(|g| g.country_code.clone());

    let region = if opts.offline && opts.region == "auto" {
        notes.push("Offline: выбран регион intl без сетевого определения".into());
        "intl".into()
    } else {
        resolve_region(&opts.region, country_code.as_deref(), &mut notes)
    };

    let dns_host = if region == "ru" {
        "ya.ru"
    } else {
        "cloudflare.com"
    };
    let dns_latency_ms = if opts.run_quality { measure_dns(dns_host) } else { None };
    let latency_targets = if opts.run_quality { measure_latencies(&region) } else { Vec::new() };
    let quality = if opts.run_quality { measure_quality(&region) } else { Vec::new() };
    let ipv6_egress = if opts.run_quality { Some(measure_ipv6_egress()) } else { None };

    let speed = if opts.run_speed {
        Some(crate::speedtest::run(crate::speedtest::SpeedOptions {
            backend: opts.speedtest_backend,
            region: region.clone(),
            country: opts.speedtest_country.or_else(|| {
                if opts.speedtest_manual_country && region.len() == 2 { Some(region.clone()) }
                else { geo.as_ref().and_then(|g| g.country_code.clone()) }
            }),
            city: opts.speedtest_city.or_else(|| {
                if opts.speedtest_manual_country { None }
                else { geo.as_ref().and_then(|g| g.city.clone()) }
            }),
            manual_country: opts.speedtest_manual_country,
            server_id: opts.speedtest_server_id,
            accept_terms: opts.speedtest_accept_terms,
            repeat_suspicious: opts.speedtest_repeat,
        }))
    } else {
        None
    };

    NetworkInfo {
        hostname: sysinfo::System::host_name().unwrap_or_else(|| "unknown".into()),
        interfaces,
        default_route_guess,
        public_ip,
        geo_hint,
        resolved_region: region,
        dns_latency_ms,
        latency_targets,
        quality,
        ipv6_egress,
        speed,
        notes,
    }
}

struct GeoInfo {
    hint: String,
    country_code: Option<String>,
    city: Option<String>,
}


fn resolve_region(requested: &str, country_code: Option<&str>, notes: &mut Vec<String>) -> String {
    let requested = requested.trim().to_lowercase();
    match requested.as_str() {
        "auto" => {
            if let Some(country) = country_code.filter(|value| value.len() == 2) {
                notes.push(format!("Страна по IP-геолокации: {country}; при ошибке укажите --region"));
                country.to_lowercase()
            } else {
                notes.push("IP-геолокация недоступна; выбор сервера ограничен списком Ookla".into());
                "intl".into()
            }
        }
        "russia" | "rf" => "ru".into(),
        "international" | "eu" | "us" | "all" | "both" | "intl" => "intl".into(),
        value if value.len() == 2 && value.chars().all(|ch| ch.is_ascii_alphabetic()) => {
            notes.push(format!("Страна задана вручную: {}", value.to_uppercase()));
            value.into()
        }
        _ => {
            notes.push(format!("Неизвестный регион «{requested}»; страна не ограничена"));
            "intl".into()
        }
    }
}
fn collect_interfaces(notes: &mut Vec<String>) -> Vec<Iface> {
    #[cfg(target_os = "linux")]
    {
        return collect_linux(notes);
    }
    #[cfg(target_os = "windows")]
    {
        return collect_windows(notes);
    }
    #[allow(unreachable_code)]
    {
        notes.push("Сбор интерфейсов для этой ОС ограничен".into());
        Vec::new()
    }
}

#[cfg(target_os = "linux")]
fn collect_linux(notes: &mut Vec<String>) -> Vec<Iface> {
    use std::fs;
    use std::path::Path;

    let mut out = Vec::new();
    let net = Path::new("/sys/class/net");
    let entries = match fs::read_dir(net) {
        Ok(e) => e,
        Err(e) => {
            notes.push(format!("Не удалось прочитать /sys/class/net: {e}"));
            return out;
        }
    };

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name == "lo" {
            continue;
        }
        let path = entry.path();
        let mac = fs::read_to_string(path.join("address"))
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty() && s != "00:00:00:00:00:00");

        let speed_mbps = fs::read_to_string(path.join("speed"))
            .ok()
            .and_then(|s| s.trim().parse::<i64>().ok())
            .filter(|&v| v > 0)
            .map(|v| v as u64);

        let mtu = fs::read_to_string(path.join("mtu"))
            .ok()
            .and_then(|s| s.trim().parse().ok());

        let rx_bytes = fs::read_to_string(path.join("statistics/rx_bytes"))
            .ok()
            .and_then(|s| s.trim().parse().ok());
        let tx_bytes = fs::read_to_string(path.join("statistics/tx_bytes"))
            .ok()
            .and_then(|s| s.trim().parse().ok());

        let (ipv4, ipv6) = addrs_for(&name);
        out.push(Iface {
            name,
            ipv4,
            ipv6,
            mac,
            speed_mbps,
            mtu,
            rx_bytes,
            tx_bytes,
        });
    }
    out
}

#[cfg(target_os = "windows")]
fn collect_windows(notes: &mut Vec<String>) -> Vec<Iface> {
    let mut out = Vec::new();
    let cmd = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            r#"Get-NetIPAddress -AddressFamily IPv4,IPv6 | Where-Object { $_.InterfaceAlias -notmatch 'Loopback' } | Select-Object InterfaceAlias,IPAddress,AddressFamily | ConvertTo-Json -Compress"#,
        ])
        .output();

    match cmd {
        Ok(o) if o.status.success() => {
            let s = String::from_utf8_lossy(&o.stdout);
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&s) {
                let arr = if val.is_array() {
                    val.as_array().cloned().unwrap_or_default()
                } else {
                    vec![val]
                };
                use std::collections::BTreeMap;
                let mut map: BTreeMap<String, Iface> = BTreeMap::new();
                for item in arr {
                    let name = item
                        .get("InterfaceAlias")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_string();
                    let ip = item
                        .get("IPAddress")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let family = item
                        .get("AddressFamily")
                        .and_then(|v| v.as_u64().or_else(|| v.as_str().and_then(|s| s.parse().ok())))
                        .unwrap_or(0);
                    let e = map.entry(name.clone()).or_insert(Iface {
                        name,
                        ipv4: Vec::new(),
                        ipv6: Vec::new(),
                        mac: None,
                        speed_mbps: None,
                        mtu: None,
                        rx_bytes: None,
                        tx_bytes: None,
                    });
                    if family == 2 || ip.contains('.') {
                        e.ipv4.push(ip);
                    } else {
                        e.ipv6.push(ip);
                    }
                }
                out.extend(map.into_values());
            } else {
                notes.push("Не удалось разобрать вывод Get-NetIPAddress".into());
            }
        }
        _ => notes.push("PowerShell Get-NetIPAddress недоступен".into()),
    }
    out
}

#[cfg(target_os = "linux")]
fn addrs_for(iface: &str) -> (Vec<String>, Vec<String>) {
    let mut v4 = Vec::new();
    let mut v6 = Vec::new();
    if let Ok(out) = Command::new("ip")
        .args(["-o", "addr", "show", "dev", iface])
        .output()
    {
        let s = String::from_utf8_lossy(&out.stdout);
        for line in s.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if let Some(pos) = parts.iter().position(|p| *p == "inet" || *p == "inet6") {
                if let Some(addr) = parts.get(pos + 1) {
                    let ip = addr.split('/').next().unwrap_or(addr).to_string();
                    if parts[pos] == "inet" {
                        v4.push(ip);
                    } else if !ip.starts_with("fe80:") {
                        v6.push(ip);
                    }
                }
            }
        }
    }
    (v4, v6)
}

fn guess_default(ifaces: &[Iface]) -> Option<String> {
    let is_tunnel = |name: &str| {
        let n = name.to_lowercase();
        n.contains("vpn")
            || n.contains("tap")
            || n.contains("tun")
            || n.contains("outline")
            || n.contains("wireguard")
            || n.contains("wsl")
            || n.contains("vethernet")
            || n.contains("docker")
            || n.contains("br-")
            || n.contains("veth")
    };
    let score = |iface: &Iface| -> i32 {
        let mut s = 0;
        if is_tunnel(&iface.name) {
            s -= 50;
        }
        if iface.name == "eth0" || iface.name == "ens3" || iface.name.starts_with("enp") {
            s += 20;
        }
        for ip in &iface.ipv4 {
            if ip.starts_with("127.") {
                s -= 100;
            } else if ip.starts_with("169.254.") {
                s -= 20;
            } else if ip.starts_with("10.") || ip.starts_with("192.168.") || ip.starts_with("172.") {
                s += 25;
            } else {
                s += 50; 
            }
        }
        s
    };
    ifaces
        .iter()
        .filter(|i| !i.ipv4.is_empty())
        .max_by_key(|i| score(i))
        .map(|i| format!("{} ({})", i.name, i.ipv4[0]))
}

fn fetch_public() -> Option<String> {
    #[cfg(target_os = "windows")]
    {
        if let Ok(out) = Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                "(Invoke-RestMethod -Uri 'https://api.ipify.org' -TimeoutSec 5).ToString()",
            ])
            .output()
        {
            if out.status.success() {
                let ip = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !ip.is_empty() && ip.len() < 64 {
                    return Some(ip);
                }
            }
        }
    }

    
    for args in [
        vec!["-4", "-fsS", "--max-time", "5", "https://ifconfig.me/ip"],
        vec!["-4", "-fsS", "--max-time", "5", "https://icanhazip.com"],
        vec!["-4", "-fsS", "--max-time", "5", "https://api.ip.sb/ip"],
        vec!["-4", "-fsS", "--max-time", "5", "https://api.ipify.org"],
        vec!["-fsS", "--max-time", "5", "http://ip-api.com/line/?fields=query"],
    ] {
        if let Ok(out) = Command::new("curl").args(&args).output() {
            if out.status.success() {
                let ip = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !ip.is_empty() && ip.len() < 64 && !ip.contains('<') && ip.contains('.') {
                    return Some(ip);
                }
            }
        }
    }
    None
}

fn fetch_geo_full() -> Option<GeoInfo> {
    let out = Command::new("curl")
        .args([
            "-fsS",
            "--max-time",
            "6",
            "http://ip-api.com/json/?fields=status,country,countryCode,city,isp,org,as,query",
        ])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
    if v.get("status").and_then(|s| s.as_str()) != Some("success") {
        return None;
    }
    let country = v.get("country").and_then(|x| x.as_str()).unwrap_or("?");
    let city = v.get("city").and_then(|x| x.as_str()).unwrap_or("?");
    let isp = v
        .get("isp")
        .or_else(|| v.get("org"))
        .and_then(|x| x.as_str())
        .unwrap_or("?");
    let asn = v.get("as").and_then(|x| x.as_str()).unwrap_or("");
    let country_code = v
        .get("countryCode")
        .and_then(|x| x.as_str())
        .map(|s| s.to_string());
    Some(GeoInfo {
        hint: format!("{city}, {country} · {isp} {asn}").trim().to_string(),
        country_code,
        city: if city == "?" { None } else { Some(city.to_string()) },
    })
}

fn measure_dns(host: &str) -> Option<f64> {
    let start = Instant::now();
    let res = format!("{host}:443").to_socket_addrs().ok()?;
    let _ = res.collect::<Vec<_>>();
    Some(start.elapsed().as_secs_f64() * 1000.0)
}

fn measure_tcp(addr: &str) -> Option<f64> {
    let start = Instant::now();
    let parsed = addr.parse().ok()?;
    let _ = TcpStream::connect_timeout(&parsed, Duration::from_secs(3)).ok()?;
    Some(start.elapsed().as_secs_f64() * 1000.0)
}

fn measure_latencies(region: &str) -> Vec<LatencySample> {
    let ru = [
        ("Yandex DNS", "77.88.8.8:443"),
        ("Yandex", "ya.ru:443"),
        ("VK", "vk.com:443"),
        ("Selectel", "selectel.ru:443"),
        ("Mail.ru", "mail.ru:443"),
        ("Sber", "sberbank.ru:443"),
    ];
    let intl = [
        ("Cloudflare", "1.1.1.1:443"),
        ("Google DNS", "8.8.8.8:443"),
        ("Quad9", "9.9.9.9:443"),
        ("GitHub", "github.com:443"),
    ];

    let targets: Vec<(&str, &str)> = match region {
        "ru" => ru.to_vec(),
        "all" => {
            let mut v = ru.to_vec();
            v.extend(intl);
            v
        }
        _ => intl.to_vec(),
    };

    targets
        .iter()
        .map(|(name, host)| {
            let latency_ms = if host.chars().next().unwrap().is_ascii_digit() {
                measure_tcp(host)
            } else {
                let start = Instant::now();
                let ok = host
                    .to_socket_addrs()
                    .ok()
                    .and_then(|mut a| a.next())
                    .and_then(|addr| {
                        TcpStream::connect_timeout(&addr, Duration::from_secs(3)).ok()
                    });
                ok.map(|_| start.elapsed().as_secs_f64() * 1000.0)
            };
            LatencySample {
                name: (*name).into(),
                host: (*host).into(),
                latency_ms,
            }
        })
        .collect()
}

fn measure_quality(region: &str) -> Vec<QualitySample> {
    let targets = if region == "ru" {
        [("Yandex", "ya.ru:443"), ("Selectel", "selectel.ru:443"), ("Cloudflare", "1.1.1.1:443")]
    } else {
        [("Cloudflare", "1.1.1.1:443"), ("Google", "8.8.8.8:443"), ("GitHub", "github.com:443")]
    };
    targets.par_iter().map(|&(name, host)| {
        let resolved: Vec<_> = host.to_socket_addrs().map(|a| a.collect()).unwrap_or_default();
        let mut samples = Vec::new();
        for _ in 0..5 {
            let start = Instant::now();
            if resolved.iter().any(|addr| TcpStream::connect_timeout(addr, Duration::from_millis(1200)).is_ok()) {
                samples.push(start.elapsed().as_secs_f64() * 1000.0);
            }
        }
        let successful = samples.len();
        let jitter_ms = if successful > 1 {
            Some(samples.windows(2).map(|w| (w[1] - w[0]).abs()).sum::<f64>() / (successful - 1) as f64)
        } else { None };
        samples.sort_by(|a, b| a.total_cmp(b));
        let median_ms = if successful > 0 { Some(samples[successful / 2]) } else { None };
        let p95_ms = if successful > 0 { Some(samples[((successful as f64 * 0.95).ceil() as usize).saturating_sub(1)]) } else { None };
        QualitySample { name: name.into(), host: host.into(), attempts: 5, successful, median_ms, p95_ms, jitter_ms, loss_percent: (5 - successful) as f64 * 20.0 }
    }).collect()
}

fn measure_ipv6_egress() -> Ipv6Egress {
    let target = "[2606:4700:4700::1111]:443".parse().expect("valid IPv6 target");
    let start = Instant::now();
    match TcpStream::connect_timeout(&target, Duration::from_secs(2)) {
        Ok(stream) => Ipv6Egress {
            connected: true,
            local_address: stream.local_addr().ok().map(|addr| addr.ip().to_string()),
            latency_ms: Some(start.elapsed().as_secs_f64() * 1000.0),
        },
        Err(_) => Ipv6Egress { connected: false, local_address: None, latency_ms: None },
    }
}
