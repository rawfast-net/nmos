use serde::Serialize;
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
pub struct SpeedResult {
    pub download_mbps: Vec<SpeedSample>,
    pub upload_mbps: Option<SpeedSample>,
    pub best_download_mbps: Option<f64>,
    pub avg_download_mbps: Option<f64>,
    pub notes: Vec<String>,
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
    pub speed_size_mb: u64,
    
    pub region: String,
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

    let region = resolve_region(&opts.region, country_code.as_deref(), &mut notes);

    let dns_host = if region == "intl" {
        "cloudflare.com"
    } else {
        "ya.ru"
    };
    let dns_latency_ms = measure_dns(dns_host);
    let latency_targets = measure_latencies(&region);

    let speed = if opts.run_speed {
        Some(run_speed_test(opts.speed_size_mb, &region, &mut notes))
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
        speed,
        notes,
    }
}

struct GeoInfo {
    hint: String,
    country_code: Option<String>,
}


fn resolve_region(requested: &str, country_code: Option<&str>, notes: &mut Vec<String>) -> String {
    let req = requested.to_lowercase();
    match req.as_str() {
        "intl" | "international" | "eu" | "us" => {
            notes.push("Регион сети задан вручную: intl".into());
            "intl".into()
        }
        "all" | "both" => {
            notes.push("Регион сети задан вручную: all".into());
            "all".into()
        }
        "ru" | "rf" | "russia" => {
            notes.push("Регион сети задан вручную: ru (Selectel)".into());
            "ru".into()
        }
        _ => {
            
            if is_cis_ru(country_code) {
                notes.push(format!(
                    "Авто-регион: ru (Selectel) — geo country={}",
                    country_code.unwrap_or("?")
                ));
                "ru".into()
            } else if country_code.is_some() {
                notes.push(format!(
                    "Авто-регион: intl (speedtest.net / зеркала) — сервер не в РФ (country={})",
                    country_code.unwrap_or("?")
                ));
                "intl".into()
            } else {
                
                let ya = measure_tcp_host("ya.ru:443");
                let cf = measure_tcp_host("1.1.1.1:443");
                match (ya, cf) {
                    (Some(ru_ms), Some(intl_ms)) if ru_ms + 40.0 < intl_ms => {
                        notes.push(format!(
                            "Авто-регион: ru — ya.ru {ru_ms:.0}мс < CF {intl_ms:.0}мс (geo недоступен)"
                        ));
                        "ru".into()
                    }
                    (Some(ru_ms), Some(intl_ms)) => {
                        notes.push(format!(
                            "Авто-регион: intl — CF {intl_ms:.0}мс ≤ ya.ru {ru_ms:.0}мс (geo недоступен)"
                        ));
                        "intl".into()
                    }
                    _ => {
                        notes.push(
                            "Авто-регион: intl по умолчанию (нет geo/latency) — для РФ укажите --region ru"
                                .into(),
                        );
                        "intl".into()
                    }
                }
            }
        }
    }
}

fn is_cis_ru(code: Option<&str>) -> bool {
    matches!(
        code.map(|c| c.to_uppercase()).as_deref(),
        Some("RU") | Some("BY") | Some("KZ") | Some("UA") | Some("UZ") | Some("AM") | Some("KG") | Some("TJ") | Some("MD") | Some("AZ")
    )
}

fn measure_tcp_host(host: &str) -> Option<f64> {
    if host.chars().next()?.is_ascii_digit() {
        return measure_tcp(host);
    }
    let start = Instant::now();
    let ok = host
        .to_socket_addrs()
        .ok()
        .and_then(|mut a| a.next())
        .and_then(|addr| TcpStream::connect_timeout(&addr, Duration::from_secs(3)).ok());
    ok.map(|_| start.elapsed().as_secs_f64() * 1000.0)
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
        "intl" => intl.to_vec(),
        "all" => {
            let mut v = ru.to_vec();
            v.extend(intl);
            v
        }
        _ => ru.to_vec(), 
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

fn run_speed_test(size_mb: u64, region: &str, notes: &mut Vec<String>) -> SpeedResult {
    let size_mb = size_mb.clamp(10, 500);
    notes.push(format!("Регион speed-теста: {region}"));

    
    if region == "intl" || region == "all" {
        if let Some(r) = try_banal_speedtest(notes) {
            if region == "intl" {
                return r;
            }
            
            let mut combined = r;
            notes.push("ALL: + Selectel download".into());
            let sel = run_mirror_downloads(size_mb, "ru", notes);
            combined.download_mbps.extend(sel.download_mbps);
            combined.notes.extend(sel.notes);
            let best = combined
                .download_mbps
                .iter()
                .map(|s| s.mbps)
                .fold(None, |acc: Option<f64>, v| {
                    Some(acc.map_or(v, |a| a.max(v)))
                });
            let avg = if combined.download_mbps.is_empty() {
                None
            } else {
                Some(
                    combined.download_mbps.iter().map(|s| s.mbps).sum::<f64>()
                        / combined.download_mbps.len() as f64,
                )
            };
            combined.best_download_mbps = best;
            combined.avg_download_mbps = avg;
            return combined;
        }
        notes.push(
            "speedtest не найден — fallback на wget/curl зеркала (Cloudflare/Hetzner/OVH)"
                .into(),
        );
        notes.push(
            "Поставьте: curl -s https://packagecloud.io/install/repositories/ookla/speedtest-cli/script.deb.sh | bash && apt install speedtest"
                .into(),
        );
        notes.push("или: apt install speedtest-cli / pip install speedtest-cli".into());
    }

    if region == "ru" || region == "all" {
        notes.push("RU: Selectel через wget -O /dev/null (как в доке Selectel)".into());
    }

    if !has_curl() && !has_wget() {
        notes.push("Нужен wget/curl или speedtest для замера скорости".into());
        return SpeedResult {
            download_mbps: vec![],
            upload_mbps: None,
            best_download_mbps: None,
            avg_download_mbps: None,
            notes: vec!["Установите speedtest (зарубеж) или wget (РФ/Selectel)".into()],
        };
    }

    run_mirror_downloads(size_mb, region, notes)
}

fn run_mirror_downloads(size_mb: u64, region: &str, _notes: &mut Vec<String>) -> SpeedResult {
    let downloads = speed_mirrors(size_mb, region);
    let mut samples = Vec::new();
    let mut speed_notes = Vec::new();

    for (label, url) in &downloads {
        eprint!("    ↓ {label} ... ");
        let is_selectel = url.contains("speedtest.selectel.ru");
        let result = if is_selectel && has_wget() {
            wget_download_mbps(url)
        } else {
            curl_download_mbps(url).or_else(|| {
                if has_wget() {
                    wget_download_mbps(url)
                } else {
                    None
                }
            })
        };
        match result {
            Some(s) => {
                eprintln!("{:.1} Mbps", s.mbps);
                samples.push(SpeedSample {
                    label: label.clone(),
                    url: url.clone(),
                    mbps: s.mbps,
                    bytes: s.bytes,
                    seconds: s.seconds,
                });
            }
            None => {
                eprintln!("ошибка");
                speed_notes.push(format!("{label}: не удалось скачать"));
            }
        }
    }

    let upload = if region == "ru" {
        eprint!("    ↑ Upload ... ");
        eprintln!("пропуск (Selectel — только download)");
        speed_notes.push("Upload в ru: ориентируйтесь на ↓ Selectel".into());
        None
    } else if region == "intl" || region == "all" {
        eprint!("    ↑ Upload ... ");
        let upload = curl_upload_mbps(10, "intl");
        match &upload {
            Some(s) => eprintln!("{}: {:.1} Mbps", s.label, s.mbps),
            None => {
                eprintln!("ошибка / недоступно");
                speed_notes.push("Upload: не удалось измерить".into());
            }
        }
        upload
    } else {
        None
    };

    let best = samples
        .iter()
        .map(|s| s.mbps)
        .fold(None, |acc: Option<f64>, v| Some(acc.map_or(v, |a| a.max(v))));
    let avg = if samples.is_empty() {
        None
    } else {
        Some(samples.iter().map(|s| s.mbps).sum::<f64>() / samples.len() as f64)
    };

    SpeedResult {
        download_mbps: samples,
        upload_mbps: upload,
        best_download_mbps: best,
        avg_download_mbps: avg,
        notes: speed_notes,
    }
}


fn try_banal_speedtest(notes: &mut Vec<String>) -> Option<SpeedResult> {
    if let Some(r) = run_ookla_speedtest() {
        notes.push("INTL: Ookla speedtest CLI".into());
        return Some(r);
    }
    if let Some(r) = run_speedtest_cli() {
        notes.push("INTL: speedtest-cli".into());
        return Some(r);
    }
    None
}

fn run_ookla_speedtest() -> Option<SpeedResult> {
    
    
    let probe = Command::new("speedtest")
        .args(["--version"])
        .output()
        .ok()?;
    let ver = format!(
        "{}{}",
        String::from_utf8_lossy(&probe.stdout),
        String::from_utf8_lossy(&probe.stderr)
    )
    .to_lowercase();
    
    let looks_ookla = ver.contains("ookla") || ver.contains("speedtest by");
    let looks_python = ver.contains("speedtest-cli") || ver.contains("python");
    if looks_python && !looks_ookla {
        return None;
    }

    eprint!("    ↓ Ookla speedtest ... ");
    let out = Command::new("speedtest")
        .args([
            "--format=json",
            "--accept-license",
            "--accept-gdpr",
            "--progress=no",
        ])
        .output()
        .ok()?;
    let out = if out.status.success()
        && serde_json::from_slice::<serde_json::Value>(&out.stdout)
            .ok()
            .and_then(|v| v.pointer("/download/bandwidth").cloned())
            .is_some()
    {
        out
    } else {
        Command::new("speedtest")
            .args(["-f", "json", "--accept-license", "--accept-gdpr"])
            .output()
            .ok()?
    };
    if !out.status.success() {
        eprintln!("ошибка");
        return None;
    }
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
    if v.pointer("/download/bandwidth").is_none() {
        eprintln!("не Ookla JSON");
        return None;
    }
    
    let down_bps = v.pointer("/download/bandwidth").and_then(|x| x.as_f64())?;
    let up_bps = v.pointer("/upload/bandwidth").and_then(|x| x.as_f64());
    let down_mbps = down_bps * 8.0 / 1_000_000.0;
    let up_mbps = up_bps.map(|b| b * 8.0 / 1_000_000.0);
    let server = v
        .pointer("/server/name")
        .and_then(|x| x.as_str())
        .or_else(|| v.pointer("/server/host").and_then(|x| x.as_str()))
        .unwrap_or("speedtest.net");
    let loc = v
        .pointer("/server/location")
        .and_then(|x| x.as_str())
        .unwrap_or("");
    let ping = v
        .pointer("/ping/latency")
        .and_then(|x| x.as_f64())
        .map(|p| format!(" ping {p:.0}ms"))
        .unwrap_or_default();
    eprintln!("{down_mbps:.1} Mbps ({server} {loc}{ping})");

    let sample = SpeedSample {
        label: format!("speedtest.net ↓ ({server})"),
        url: "ookla://speedtest".into(),
        mbps: down_mbps,
        bytes: 0,
        seconds: 0.0,
    };
    let upload = up_mbps.map(|mbps| {
        eprint!("    ↑ Ookla upload ... ");
        eprintln!("{mbps:.1} Mbps");
        SpeedSample {
            label: format!("speedtest.net ↑ ({server})"),
            url: "ookla://speedtest".into(),
            mbps,
            bytes: 0,
            seconds: 0.0,
        }
    });

    Some(SpeedResult {
        download_mbps: vec![sample],
        upload_mbps: upload,
        best_download_mbps: Some(down_mbps),
        avg_download_mbps: Some(down_mbps),
        notes: vec![format!("Сервер: {server} {loc}{ping}")],
    })
}

fn run_speedtest_cli() -> Option<SpeedResult> {
    
    let candidates = ["speedtest-cli", "speedtest"];
    for bin in candidates {
        let probe = Command::new(bin).args(["--version"]).output();
        let Ok(probe) = probe else { continue };
        let ver = format!(
            "{}{}",
            String::from_utf8_lossy(&probe.stdout),
            String::from_utf8_lossy(&probe.stderr)
        )
        .to_lowercase();
        
        if ver.contains("ookla") {
            continue;
        }
        
        eprint!("    ↓ speedtest-cli ({bin}) ... ");
        let out = Command::new(bin)
            .args(["--json", "--secure"])
            .output()
            .ok();
        let out = match out {
            Some(o) if o.status.success() => o,
            _ => match Command::new(bin).args(["--json"]).output().ok() {
                Some(o) if o.status.success() => o,
                _ => {
                    eprintln!("ошибка");
                    continue;
                }
            },
        };
        if let Some(r) = parse_speedtest_cli_json(&out.stdout) {
            return Some(r);
        }
        eprintln!("не JSON speedtest-cli");
    }
    None
}

fn parse_speedtest_cli_json(stdout: &[u8]) -> Option<SpeedResult> {
    let v: serde_json::Value = serde_json::from_slice(stdout).ok()?;
    
    if v.pointer("/download/bandwidth").is_some() {
        return None;
    }
    let down_bits = v.get("download").and_then(|x| x.as_f64())?;
    let up_bits = v.get("upload").and_then(|x| x.as_f64());
    let down_mbps = down_bits / 1_000_000.0;
    let up_mbps = up_bits.map(|b| b / 1_000_000.0);
    let sponsor = v
        .pointer("/server/sponsor")
        .and_then(|x| x.as_str())
        .unwrap_or("speedtest.net");
    let name = v
        .pointer("/server/name")
        .and_then(|x| x.as_str())
        .unwrap_or("");
    eprintln!("{down_mbps:.1} Mbps ({sponsor} {name})");
    if let Some(u) = up_mbps {
        eprint!("    ↑ speedtest-cli upload ... ");
        eprintln!("{u:.1} Mbps");
    }

    Some(SpeedResult {
        download_mbps: vec![SpeedSample {
            label: format!("speedtest-cli ↓ ({sponsor})"),
            url: "speedtest-cli://".into(),
            mbps: down_mbps,
            bytes: 0,
            seconds: 0.0,
        }],
        upload_mbps: up_mbps.map(|mbps| SpeedSample {
            label: format!("speedtest-cli ↑ ({sponsor})"),
            url: "speedtest-cli://".into(),
            mbps,
            bytes: 0,
            seconds: 0.0,
        }),
        best_download_mbps: Some(down_mbps),
        avg_download_mbps: Some(down_mbps),
        notes: vec![format!("Сервер: {sponsor} / {name}")],
    })
}


fn speed_mirrors(size_mb: u64, region: &str) -> Vec<(String, String)> {
    
    let mut selectel = vec![
        (
            "Selectel 10MB".into(),
            "https://speedtest.selectel.ru/10MB".into(),
        ),
        (
            "Selectel 100MB".into(),
            "https://speedtest.selectel.ru/100MB".into(),
        ),
    ];
    if size_mb >= 200 {
        selectel.push((
            "Selectel 1GB".into(),
            "https://speedtest.selectel.ru/1GB".into(),
        ));
    }

    let bytes_cf = size_mb * 1024 * 1024;
    let hetzner_file = if size_mb >= 200 { "1GB.bin" } else { "100MB.bin" };
    
    let intl: Vec<(String, String)> = vec![
        (
            "Cloudflare".into(),
            format!("https://speed.cloudflare.com/__down?bytes={bytes_cf}"),
        ),
        (
            "Cachefly".into(),
            "http://cachefly.cachefly.net/100mb.test".into(),
        ),
        (
            "Hetzner NBG".into(),
            format!("https://nbg1-speed.hetzner.com/{hetzner_file}"),
        ),
        (
            "ThinkBroadband 100MB".into(),
            "http://ipv4.download.thinkbroadband.com/100MB.zip".into(),
        ),
        (
            "OVH".into(),
            "https://proof.ovh.net/files/100Mb.dat".into(),
        ),
    ];

    match region {
        "intl" => intl,
        "all" => {
            let mut v = selectel;
            v.extend(intl);
            v
        }
        _ => selectel, 
    }
}

fn has_curl() -> bool {
    Command::new("curl")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn has_wget() -> bool {
    Command::new("wget")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

struct CurlSpeed {
    mbps: f64,
    bytes: u64,
    seconds: f64,
}


fn wget_download_mbps(url: &str) -> Option<CurlSpeed> {
    let sink = null_sink();
    let t0 = Instant::now();
    let out = Command::new("wget")
        .args([
            "-O",
            sink,
            "--timeout=60",
            "--tries=1",
            "-q", 
            url,
        ])
        .output()
        .ok()?;
    let secs = t0.elapsed().as_secs_f64().max(1e-6);
    if !out.status.success() {
        
        let t1 = Instant::now();
        let out2 = Command::new("wget")
            .args(["-O", sink, "--timeout=60", "--tries=1", url])
            .output()
            .ok()?;
        let secs2 = t1.elapsed().as_secs_f64().max(1e-6);
        if !out2.status.success() {
            return None;
        }
        let err = String::from_utf8_lossy(&out2.stderr);
        if let Some(mbps) = parse_wget_speed(&err) {
            return Some(CurlSpeed {
                mbps,
                bytes: 0,
                seconds: secs2,
            });
        }
        
        let bytes = selectel_size_bytes(url).unwrap_or(10 * 1024 * 1024);
        return Some(CurlSpeed {
            mbps: (bytes as f64 * 8.0) / secs2 / 1_000_000.0,
            bytes,
            seconds: secs2,
        });
    }
    let bytes = selectel_size_bytes(url).unwrap_or(0);
    let mbps = if bytes > 0 {
        (bytes as f64 * 8.0) / secs / 1_000_000.0
    } else {
        
        return None;
    };
    Some(CurlSpeed {
        mbps,
        bytes,
        seconds: secs,
    })
}

fn selectel_size_bytes(url: &str) -> Option<u64> {
    if url.ends_with("/10MB") {
        Some(10 * 1024 * 1024)
    } else if url.ends_with("/100MB") {
        Some(100 * 1024 * 1024)
    } else if url.ends_with("/1GB") {
        Some(1024 * 1024 * 1024)
    } else if url.ends_with("/10GB") {
        Some(10 * 1024 * 1024 * 1024)
    } else {
        None
    }
}

fn parse_wget_speed(stderr: &str) -> Option<f64> {
    
    let re_like = stderr.replace(',', ".");
    for part in re_like.split_whitespace() {
        let p = part.to_lowercase();
        if let Some(n) = p.strip_suffix("mb/s") {
            if let Ok(v) = n.parse::<f64>() {
                return Some(v * 8.0); 
            }
        }
        if let Some(n) = p.strip_suffix("kb/s") {
            if let Ok(v) = n.parse::<f64>() {
                return Some(v * 8.0 / 1000.0);
            }
        }
    }
    None
}

fn curl_download_mbps(url: &str) -> Option<CurlSpeed> {
    
    let (url, range) = if let Some((base, rest)) = url.split_once("#range=") {
        let n: u64 = rest.parse().unwrap_or(0);
        if n > 0 {
            (base, Some(format!("0-{}", n.saturating_sub(1))))
        } else {
            (base, None)
        }
    } else {
        (url, None)
    };

    let mut args = vec![
        "-4".to_string(),
        "-L".to_string(),
        "-fsS".to_string(),
        "--max-time".to_string(),
        "90".to_string(),
        "-o".to_string(),
        null_sink().to_string(),
        "-w".to_string(),
        "%{speed_download} %{size_download} %{time_total}".to_string(),
    ];
    if let Some(r) = range {
        args.push("-r".to_string());
        args.push(r);
    }
    args.push(url.to_string());

    let out = Command::new("curl").args(&args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    parse_curl_speed(&String::from_utf8_lossy(&out.stdout))
}

fn curl_upload_mbps(size_mb: u64, region: &str) -> Option<SpeedSample> {
    if region != "intl" && region != "all" {
        return None;
    }
    curl_upload_cloudflare(size_mb)
}

fn curl_upload_cloudflare(size_mb: u64) -> Option<SpeedSample> {
    let url = "https://speed.cloudflare.com/__up".to_string();

    #[cfg(target_os = "linux")]
    {
        let cmd = format!(
            "dd if=/dev/zero bs=1M count={size_mb} status=none 2>/dev/null | \
             curl -4 -L -fsS --max-time 25 -X POST \
             -H 'Content-Type: application/octet-stream' \
             --data-binary @- -o /dev/null \
             -w '%{{speed_upload}} %{{size_upload}} %{{time_total}}' \
             '{url}'"
        );
        let out = Command::new("sh").args(["-c", &cmd]).output().ok()?;
        if !out.status.success() {
            return None;
        }
        let s = parse_curl_speed(&String::from_utf8_lossy(&out.stdout))?;
        return Some(SpeedSample {
            label: "Cloudflare CDN".into(),
            url,
            mbps: s.mbps,
            bytes: s.bytes,
            seconds: s.seconds,
        });
    }

    #[cfg(target_os = "windows")]
    {
        let bytes = size_mb * 1024 * 1024;
        let tmp = std::env::temp_dir().join("nmos-up.bin");
        let path = tmp.to_string_lossy().replace('\'', "''");
        let ps = format!(
            "$b = New-Object byte[] ({bytes}); [IO.File]::WriteAllBytes('{path}', $b)"
        );
        let _ = Command::new("powershell")
            .args(["-NoProfile", "-Command", &ps])
            .output();
        let out = Command::new("curl")
            .args([
                "-4",
                "-L",
                "-fsS",
                "--max-time",
                "25",
                "-X",
                "POST",
                "-H",
                "Content-Type: application/octet-stream",
                "--data-binary",
                &format!("@{}", tmp.display()),
                "-o",
                "NUL",
                "-w",
                "%{speed_upload} %{size_upload} %{time_total}",
                &url,
            ])
            .output()
            .ok();
        let _ = std::fs::remove_file(&tmp);
        let out = out?;
        if !out.status.success() {
            return None;
        }
        let s = parse_curl_speed(&String::from_utf8_lossy(&out.stdout))?;
        return Some(SpeedSample {
            label: "Cloudflare CDN".into(),
            url,
            mbps: s.mbps,
            bytes: s.bytes,
            seconds: s.seconds,
        });
    }

    #[allow(unreachable_code)]
    None
}

fn parse_curl_speed(s: &str) -> Option<CurlSpeed> {
    let parts: Vec<&str> = s.trim().split_whitespace().collect();
    if parts.len() < 3 {
        return None;
    }
    let speed_bps: f64 = parts[0].parse().ok()?;
    let bytes: f64 = parts[1].parse().ok()?;
    let seconds: f64 = parts[2].parse().ok()?;
    if speed_bps <= 0.0 || seconds <= 0.0 {
        return None;
    }
    Some(CurlSpeed {
        mbps: (speed_bps * 8.0) / 1_000_000.0,
        bytes: bytes as u64,
        seconds,
    })
}

fn null_sink() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "NUL"
    }
    #[cfg(not(target_os = "windows"))]
    {
        "/dev/null"
    }
}
