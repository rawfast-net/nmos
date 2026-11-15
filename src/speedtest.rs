use crate::network::{SpeedAttempt, SpeedMeasurement, SpeedResult, SpeedSample};
use serde::Deserialize;
use serde_json::Value;
use std::io::{self, IsTerminal, Write};
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

pub struct SpeedOptions {
    pub backend: String,
    pub region: String,
    pub country: Option<String>,
    pub city: Option<String>,
    pub manual_country: bool,
    pub server_id: Option<u64>,
    pub accept_terms: bool,
    pub repeat_suspicious: bool,
}

#[derive(Debug, Clone)]
struct Server {
    id: u64,
    name: String,
    city: String,
    country: String,
    host: Option<String>,
    distance_km: Option<f64>,
}

#[derive(Deserialize)]
struct IperfNode {
    name: String,
    city: String,
    country: String,
    host: String,
    port: u16,
}

fn empty(backend: &str) -> SpeedResult {
    SpeedResult {
        backend: backend.into(),
        status: "error".into(),
        error: None,
        server_name: None,
        server_city: None,
        server_country: None,
        server_id: None,
        ping_ms: None,
        jitter_ms: None,
        packet_loss_percent: None,
        result_url: None,
        tested_at: None,
        attempts: Vec::new(),
        measurements: Vec::new(),
        download_mbps: Vec::new(),
        upload_mbps: None,
        best_download_mbps: None,
        avg_download_mbps: None,
        notes: Vec::new(),
    }
}

pub fn run(opts: SpeedOptions) -> SpeedResult {
    if opts.backend == "iperf3" { return run_iperf(&opts); }
    let mut result = empty("ookla");
    if opts.backend != "ookla" {
        result.error = Some(format!("Неизвестный backend: {}", opts.backend));
        return result;
    }
    if let Err(error) = official_available() {
        result.error = Some(error);
        return fallback_for_ru(result, &opts);
    }
    if let Err(error) = accept_terms(opts.accept_terms) {
        result.error = Some(error);
        return result;
    }

    let candidates = if let Some(id) = opts.server_id {
        vec![Server { id, name: format!("Ookla #{id}"), city: String::new(), country: String::new(), host: None, distance_km: None }]
    } else {
        match list_servers() {
            Ok(servers) => rank_servers(servers, &opts),
            Err(error) => {
                result.error = Some(error);
                return fallback_for_ru(result, &opts);
            }
        }
    };
    if candidates.is_empty() {
        result.error = Some("В списке Ookla нет подходящих серверов для выбранной страны или города".into());
        return fallback_for_ru(result, &opts);
    }
    for server in candidates.into_iter().take(8) {
        match run_ookla(server.id) {
            Ok(mut measured) => {
                result.attempts.push(SpeedAttempt { server_id: Some(server.id), server_name: server.name.clone(), status: "ok".into(), detail: "Проверка завершена".into() });
                measured.attempts = result.attempts;
                measured.notes.extend(result.notes);
                if measured.server_country.is_none() && !server.country.is_empty() { measured.server_country = Some(server.country.clone()); }
                if measured.server_city.is_none() && !server.city.is_empty() { measured.server_city = Some(server.city.clone()); }
                if opts.repeat_suspicious && (measured.best_download_mbps.is_some_and(|v| v < 10.0) || measured.packet_loss_percent.is_some_and(|v| v > 5.0)) {
                    match run_ookla(server.id) {
                        Ok(repeated) => {
                            measured.measurements.extend(repeated.measurements);
                            measured.download_mbps.extend(repeated.download_mbps);
                            measured.best_download_mbps = measured.download_mbps.iter().map(|s| s.mbps).reduce(f64::max);
                            measured.avg_download_mbps = Some(measured.download_mbps.iter().map(|s| s.mbps).sum::<f64>() / measured.download_mbps.len() as f64);
                            measured.attempts.push(SpeedAttempt { server_id: Some(server.id), server_name: server.name.clone(), status: "ok".into(), detail: "Повторная проверка завершена".into() });
                        }
                        Err(error) => measured.attempts.push(SpeedAttempt { server_id: Some(server.id), server_name: server.name.clone(), status: "error".into(), detail: format!("Повторная проверка: {error}") }),
                    }
                }
                return measured;
            }
            Err(error) => result.attempts.push(SpeedAttempt { server_id: Some(server.id), server_name: server.name.clone(), status: "error".into(), detail: error }),
        }
    }
    result.error = Some("Не удалось проверить скорость на доступных ближайших серверах Ookla".into());
    fallback_for_ru(result, &opts)
}

fn fallback_for_ru(mut ookla: SpeedResult, opts: &SpeedOptions) -> SpeedResult {
    if opts.region != "ru" { return ookla; }
    let mut iperf = run_iperf(opts);
    if iperf.status == "ok" {
        iperf.notes.push(format!("Ookla недоступен: {}", ookla.error.unwrap_or_default()));
        return iperf;
    }
    ookla.error = Some(format!("Ookla: {}; iperf3: {}", ookla.error.unwrap_or_default(), iperf.error.unwrap_or_default()));
    ookla
}

fn official_available() -> Result<(), String> {
    let out = Command::new("speedtest").arg("--version").output()
        .map_err(|_| "Официальный Ookla Speedtest CLI не найден. Запустите nmos setup --speedtest".to_string())?;
    let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr)).to_lowercase();
    if !out.status.success() || !text.contains("ookla") {
        return Err("Команда speedtest найдена, но это не официальный CLI Ookla".into());
    }
    Ok(())
}

fn terms_marker() -> Option<PathBuf> {
    let root = std::env::var_os("LOCALAPPDATA")
        .or_else(|| std::env::var_os("XDG_CONFIG_HOME"))
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config").into_os_string()))?;
    Some(PathBuf::from(root).join("nmos").join("ookla-terms-accepted"))
}

fn accept_terms(explicit: bool) -> Result<(), String> {
    let marker = terms_marker();
    if marker.as_ref().is_some_and(|path| path.exists()) { return Ok(()); }
    if !explicit {
        if !io::stdin().is_terminal() {
            return Err("Требуется согласие с условиями Ookla: повторите с --accept-speedtest-terms".into());
        }
        println!("Ookla: https://www.speedtest.net/about/terms и https://www.speedtest.net/about/privacy");
        print!("Принимаете условия Ookla для запуска теста? [y/N]: ");
        io::stdout().flush().map_err(|e| e.to_string())?;
        let mut answer = String::new();
        io::stdin().read_line(&mut answer).map_err(|e| e.to_string())?;
        if !matches!(answer.trim().to_lowercase().as_str(), "y" | "yes" | "да" | "д") {
            return Err("Условия Ookla не приняты; тест не запускался".into());
        }
    }
    if let Some(path) = marker {
        if let Some(parent) = path.parent() { std::fs::create_dir_all(parent).map_err(|e| e.to_string())?; }
        std::fs::write(path, b"accepted\n").map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn run_command(mut command: Command, limit: Duration) -> Result<Output, String> {
    let mut child = command.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().map_err(|e| e.to_string())?;
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return child.wait_with_output().map_err(|e| e.to_string()),
            Ok(None) if started.elapsed() < limit => std::thread::sleep(Duration::from_millis(100)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("Превышен лимит времени {} с", limit.as_secs()));
            }
            Err(error) => return Err(error.to_string()),
        }
    }
}

fn list_servers() -> Result<Vec<Server>, String> {
    let mut command = Command::new("speedtest");
    command.args(["--servers", "--format=json", "--accept-license", "--accept-gdpr"]);
    let output = run_command(command, Duration::from_secs(15))?;
    if output.status.success() {
        let servers = parse_server_json(&output.stdout);
        if !servers.is_empty() { return Ok(servers); }
    }
    let mut command = Command::new("speedtest");
    command.args(["--servers", "--accept-license", "--accept-gdpr"]);
    let output = run_command(command, Duration::from_secs(15))?;
    if !output.status.success() {
        return Err(format!("Ookla не вернул список серверов: {}", String::from_utf8_lossy(&output.stderr).trim()));
    }
    let servers = parse_server_text(&String::from_utf8_lossy(&output.stdout));
    if servers.is_empty() { Err("Не удалось разобрать список серверов Ookla".into()) } else { Ok(servers) }
}

fn parse_server_json(bytes: &[u8]) -> Vec<Server> {
    let Ok(value) = serde_json::from_slice::<Value>(bytes) else { return Vec::new() };
    let Some(items) = value.as_array().or_else(|| value.get("servers").and_then(Value::as_array)) else { return Vec::new() };
    items.iter().filter_map(|item| {
        Some(Server {
            id: item.get("id")?.as_u64().or_else(|| item.get("id")?.as_str()?.parse().ok())?,
            name: item.get("name").or_else(|| item.get("sponsor")).and_then(Value::as_str).unwrap_or("Ookla").into(),
            city: item.get("location").or_else(|| item.get("city")).and_then(Value::as_str).unwrap_or("").into(),
            country: item.get("country").and_then(Value::as_str).unwrap_or("").into(),
            host: item.get("host").and_then(Value::as_str).map(str::to_string),
            distance_km: item.get("distance").and_then(Value::as_f64),
        })
    }).collect()
}

fn parse_server_text(text: &str) -> Vec<Server> {
    text.lines().filter_map(|line| {
        let trimmed = line.trim();
        let id_text = trimmed.split_whitespace().next()?.trim_end_matches([')', ':']);
        let id = id_text.parse::<u64>().ok()?;
        let rest = trimmed[id_text.len()..].trim_start_matches([')', ':', ' ', '\t']).trim();
        if rest.is_empty() { return None; }
        let distance_km = rest.rfind(['[', '(']).and_then(|start| {
            let tail = &rest[start + 1..];
            if tail.contains("km") { tail.split_whitespace().next()?.parse::<f64>().ok() } else { None }
        });
        let location = rest.rfind('(').and_then(|start| {
            let tail = rest[start + 1..].split(')').next()?;
            if tail.contains(',') { Some(tail) } else { None }
        }).or_else(|| rest.rsplit_once(" - ").map(|(_, location)| location)).unwrap_or(rest);
        let city = location.split(',').next().unwrap_or("").trim().to_string();
        let country = location.split_once(',').map(|(_, country)| country.split(['(', '[']).next().unwrap_or("").trim().to_string()).unwrap_or_default();
        Some(Server { id, name: rest.into(), city, country, host: None, distance_km })
    }).collect()
}

fn country_names(code: &str) -> &'static [&'static str] {
    match code.to_lowercase().as_str() {
        "de" => &["germany", "deutschland", "германия"],
        "se" => &["sweden", "sverige", "швеция"],
        "ru" => &["russia", "россия", "russian federation"],
        "fr" => &["france", "франция"],
        "nl" => &["netherlands", "holland", "нидерланды"],
        "fi" => &["finland", "финляндия"],
        "pl" => &["poland", "польша"],
        "gb" => &["united kingdom", "uk", "britain"],
        _ => &[],
    }
}

fn rank_servers(mut servers: Vec<Server>, opts: &SpeedOptions) -> Vec<Server> {
    let country = opts.country.as_deref().or_else(|| if opts.region.len() == 2 { Some(opts.region.as_str()) } else { None });
    let country = country.filter(|value| value.len() == 2);
    let names = country.map(country_names).unwrap_or(&[]);
    if opts.manual_country {
        servers.retain(|server| {
            let text = format!("{} {} {}", server.name, server.city, server.country).to_lowercase();
            names.iter().any(|name| text.contains(name))
                || country.is_some_and(|code| server.country.eq_ignore_ascii_case(code))
        });
    }
    let city = opts.city.as_deref().unwrap_or("").to_lowercase();
    servers.sort_by(|a, b| {
        let score = |server: &Server| {
            let text = format!("{} {} {}", server.name, server.city, server.country).to_lowercase();
            let same_city = !city.is_empty() && text.contains(&city);
            let same_country = names.iter().any(|name| text.contains(name));
            (if same_city { 0 } else if same_country { 1 } else { 2 }, server.distance_km.unwrap_or(f64::INFINITY))
        };
        let a = score(a);
        let b = score(b);
        a.0.cmp(&b.0).then_with(|| a.1.total_cmp(&b.1))
    });
    servers.truncate(12);
    servers
}

fn run_ookla(id: u64) -> Result<SpeedResult, String> {
    let mut command = Command::new("speedtest");
    command.args(["--format=json", "--progress=no", "--accept-license", "--accept-gdpr", "--server-id"]);
    command.arg(id.to_string());
    let output = run_command(command, Duration::from_secs(90))?;
    if !output.status.success() {
        return Err(format!("Ookla: {}", String::from_utf8_lossy(&output.stderr).trim()));
    }
    parse_ookla(&output.stdout, id)
}

fn parse_ookla(bytes: &[u8], requested_id: u64) -> Result<SpeedResult, String> {
    let value: Value = serde_json::from_slice(bytes).map_err(|_| "Некорректный JSON Ookla".to_string())?;
    let download = value.pointer("/download/bandwidth").and_then(Value::as_f64).filter(|v| v.is_finite() && *v > 0.0).ok_or("Ookla не вернул скорость download")? * 8.0 / 1_000_000.0;
    let upload = value.pointer("/upload/bandwidth").and_then(Value::as_f64).filter(|v| v.is_finite() && *v >= 0.0).map(|v| v * 8.0 / 1_000_000.0);
    let id = value.pointer("/server/id").and_then(Value::as_u64).or_else(|| value.pointer("/server/id").and_then(Value::as_str).and_then(|v| v.parse().ok()));
    if id.is_some_and(|actual| actual != requested_id) { return Err("Ookla выбрал другой server ID".into()); }
    let mut result = empty("ookla");
    result.status = "ok".into();
    result.server_id = id.or(Some(requested_id));
    result.server_name = value.pointer("/server/name").or_else(|| value.pointer("/server/host")).and_then(Value::as_str).map(str::to_string);
    result.server_city = value.pointer("/server/location").and_then(Value::as_str).map(str::to_string);
    result.server_country = value.pointer("/server/country").and_then(Value::as_str).map(str::to_string);
    result.ping_ms = value.pointer("/ping/latency").and_then(Value::as_f64);
    result.jitter_ms = value.pointer("/ping/jitter").and_then(Value::as_f64);
    result.packet_loss_percent = value.get("packetLoss").and_then(Value::as_f64);
    result.result_url = value.pointer("/result/url").and_then(Value::as_str).map(str::to_string);
    result.tested_at = value.get("timestamp").and_then(Value::as_str).map(str::to_string);
    result.download_mbps.push(SpeedSample { label: "Ookla download".into(), url: result.result_url.clone().unwrap_or_default(), mbps: download, bytes: 0, seconds: 0.0 });
    result.upload_mbps = upload.map(|mbps| SpeedSample { label: "Ookla upload".into(), url: result.result_url.clone().unwrap_or_default(), mbps, bytes: 0, seconds: 0.0 });
    result.best_download_mbps = Some(download);
    result.avg_download_mbps = Some(download);
    result.measurements.push(SpeedMeasurement { download_mbps: download, upload_mbps: upload, ping_ms: result.ping_ms, jitter_ms: result.jitter_ms, packet_loss_percent: result.packet_loss_percent, tested_at: result.tested_at.clone() });
    Ok(result)
}

fn run_iperf(opts: &SpeedOptions) -> SpeedResult {
    let mut result = empty("iperf3");
    let nodes: Vec<IperfNode> = match serde_json::from_str(include_str!("../data/iperf-nodes.json")) {
        Ok(nodes) => nodes,
        Err(_) => {
            result.error = Some("Некорректная конфигурация узлов NMOS iperf3".into());
            return result;
        }
    };
    let country = opts.country.as_deref().unwrap_or(&opts.region);
    let node = nodes.iter().filter(|node| node.country.eq_ignore_ascii_case(country))
        .find(|node| opts.city.as_deref().is_some_and(|city| node.city.eq_ignore_ascii_case(city)))
        .or_else(|| nodes.iter().find(|node| node.country.eq_ignore_ascii_case(country)));
    let Some(node) = node else {
        result.error = Some("Нет настроенного тестового узла NMOS iperf3 для выбранной страны".into());
        return result;
    };
    let mut download_cmd = Command::new("iperf3");
    download_cmd.args(["-c", &node.host, "-p", &node.port.to_string(), "-J", "-t", "8", "-R"]);
    let download = match run_command(download_cmd, Duration::from_secs(20)).and_then(|output| parse_iperf(&output)) {
        Ok(value) => value,
        Err(error) => {
            result.error = Some(format!("iperf3 download: {error}"));
            return result;
        }
    };
    let mut upload_cmd = Command::new("iperf3");
    upload_cmd.args(["-c", &node.host, "-p", &node.port.to_string(), "-J", "-t", "8"]);
    let upload = match run_command(upload_cmd, Duration::from_secs(20)).and_then(|output| parse_iperf(&output)) {
        Ok(value) => value,
        Err(error) => {
            result.error = Some(format!("iperf3 upload: {error}"));
            return result;
        }
    };
    result.status = "ok".into();
    result.server_name = Some(node.name.clone());
    result.server_city = Some(node.city.clone());
    result.server_country = Some(node.country.clone());
    result.tested_at = Some(chrono::Utc::now().to_rfc3339());
    result.best_download_mbps = Some(download);
    result.avg_download_mbps = Some(download);
    result.measurements.push(SpeedMeasurement { download_mbps: download, upload_mbps: Some(upload), ping_ms: None, jitter_ms: None, packet_loss_percent: None, tested_at: result.tested_at.clone() });
    result.download_mbps.push(SpeedSample { label: "iperf3 download".into(), url: String::new(), mbps: download, bytes: 0, seconds: 8.0 });
    result.upload_mbps = Some(SpeedSample { label: "iperf3 upload".into(), url: String::new(), mbps: upload, bytes: 0, seconds: 8.0 });
    result.notes.push("Контролируемый узел NMOS; методика не сопоставима с Ookla".into());
    result
}

fn parse_iperf(output: &Output) -> Result<f64, String> {
    if !output.status.success() { return Err(String::from_utf8_lossy(&output.stderr).trim().to_string()); }
    let value: Value = serde_json::from_slice(&output.stdout).map_err(|_| "Некорректный JSON iperf3".to_string())?;
    if let Some(error) = value.get("error").and_then(Value::as_str) { return Err(error.into()); }
    let bits = value.pointer("/end/sum_received/bits_per_second").or_else(|| value.pointer("/end/sum_sent/bits_per_second")).and_then(Value::as_f64).filter(|v| v.is_finite() && *v > 0.0).ok_or("Нет измеренной скорости iperf3")?;
    Ok(bits / 1_000_000.0)
}

pub fn setup() -> Result<(), String> {
    if official_available().is_ok() {
        println!("Официальный Ookla Speedtest CLI уже установлен");
        return Ok(());
    }
    #[cfg(not(target_os = "linux"))]
    {
        return Err("Автоустановка доступна для Debian/Ubuntu Linux. Для этой ОС используйте официальный сайт https://www.speedtest.net/apps/cli".into());
    }
    #[cfg(target_os = "linux")]
    {
        setup_linux()
    }
}

pub fn offer_setup() {
    if official_available().is_ok() { return; }
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        eprintln!("Ookla Speedtest не установлен. Автоматическая установка требует подтверждения в терминале.");
        return;
    }
    println!("\nOokla Speedtest не установлен. NMOS может установить его сейчас.");
    if let Err(error) = setup() {
        eprintln!("Не удалось установить Ookla Speedtest: {error}");
        eprintln!("Остальные проверки NMOS продолжатся.");
    }
}

#[cfg(target_os = "linux")]
fn setup_linux() -> Result<(), String> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err("Установка требует интерактивного согласия пользователя".into());
    }
    let root = Command::new("id").arg("-u").output().map_err(|e| e.to_string())?;
    if String::from_utf8_lossy(&root.stdout).trim() != "0" {
        return Err("Для автоматической установки нужны права администратора. Запустите NMOS через sudo".into());
    }
    let release = std::fs::read_to_string("/etc/os-release").map_err(|e| e.to_string())?;
    let field = |name: &str| -> Option<String> {
        release.lines().find_map(|line| line.strip_prefix(&format!("{name}=")))
            .map(|value| value.trim_matches('"').to_lowercase())
    };
    let id = field("ID").unwrap_or_default();
    let distro = if id == "ubuntu" || field("ID_LIKE").as_deref().is_some_and(|value| value.contains("ubuntu")) {
        "ubuntu"
    } else if id == "debian" || field("ID_LIKE").as_deref().is_some_and(|value| value.contains("debian")) {
        "debian"
    } else {
        return Err("Автоустановка поддерживает Debian, Ubuntu и совместимые дистрибутивы".into());
    };
    let codename = if distro == "ubuntu" { field("UBUNTU_CODENAME").or_else(|| field("VERSION_CODENAME")) }
        else { field("VERSION_CODENAME") }.ok_or("Не удалось определить кодовое имя дистрибутива")?;
    if !codename.chars().all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit()) {
        return Err("Недопустимое кодовое имя дистрибутива".into());
    }
    let arch = Command::new("dpkg").arg("--print-architecture").output().map_err(|e| e.to_string())?;
    let arch = String::from_utf8_lossy(&arch.stdout).trim().to_string();
    if !matches!(arch.as_str(), "amd64" | "arm64" | "armhf" | "i386") {
        return Err(format!("Архитектура {arch} не поддерживается официальным репозиторием Ookla"));
    }
    println!("Будет добавлен официальный APT репозиторий Ookla для {distro}/{codename} ({arch}) и установлен пакет speedtest.");
    println!("Источник: https://packagecloud.io/ookla/speedtest-cli");
    print!("Разрешаете изменить системный APT и установить пакет? [y/N]: ");
    io::stdout().flush().map_err(|e| e.to_string())?;
    let mut answer = String::new();
    io::stdin().read_line(&mut answer).map_err(|e| e.to_string())?;
    if !matches!(answer.trim().to_lowercase().as_str(), "y" | "yes" | "да" | "д") {
        return Err("Установка отменена пользователем".into());
    }
    let key_file = std::env::temp_dir().join(format!("nmos-ookla-key-{}", std::process::id()));
    let download = Command::new("curl")
        .args(["-fsSL", "--max-time", "30", "-o", key_file.to_str().ok_or("Путь временного файла не поддерживается")?, "https://packagecloud.io/ookla/speedtest-cli/gpgkey"])
        .status().map_err(|e| e.to_string())?;
    if !download.success() { return Err("Не удалось получить ключ официального репозитория Ookla".into()); }
    let inspected = Command::new("gpg").args(["--show-keys", "--with-colons", key_file.to_str().ok_or("Путь ключа не поддерживается")?])
        .output().map_err(|e| e.to_string())?;
    let keys = String::from_utf8_lossy(&inspected.stdout);
    if !inspected.status.success() || !keys.lines().filter(|line| line.starts_with("fpr:")).any(|line| line.split(':').nth(9).is_some_and(|value| value.ends_with("8E61C2AB9A6D1557"))) {
        let _ = std::fs::remove_file(&key_file);
        return Err("Отпечаток ключа Ookla не совпадает с опубликованным в packagecloud".into());
    }
    let keyring = "/etc/apt/keyrings/nmos-ookla.gpg";
    std::fs::create_dir_all("/etc/apt/keyrings").map_err(|e| e.to_string())?;
    let imported = Command::new("gpg").args(["--dearmor", "--yes", "--output", keyring, key_file.to_str().ok_or("Путь ключа не поддерживается")?])
        .status().map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&key_file);
    if !imported.success() { return Err("Не удалось установить ключ Ookla".into()); }
    let source = format!("deb [arch={arch} signed-by={keyring}] https://packagecloud.io/ookla/speedtest-cli/{distro}/ {codename} main\n");
    std::fs::write("/etc/apt/sources.list.d/nmos-ookla.list", source).map_err(|e| e.to_string())?;
    if !Command::new("apt-get").arg("update").status().map_err(|e| e.to_string())?.success() {
        return Err("APT не смог проверить репозиторий Ookla или обновить индекс".into());
    }
    if !Command::new("apt-get").args(["install", "-y", "speedtest"]).status().map_err(|e| e.to_string())?.success() {
        return Err("Не удалось установить пакет speedtest".into());
    }
    official_available()?;
    println!("Ookla Speedtest CLI установлен. При первом тесте NMOS попросит принять условия Ookla.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(country: &str, city: &str) -> SpeedOptions {
        SpeedOptions { backend: "ookla".into(), region: country.into(), country: Some(country.into()), city: Some(city.into()), manual_country: true, server_id: None, accept_terms: true, repeat_suspicious: false }
    }

    #[test]
    fn ranks_three_local_servers_before_foreign_ones() {
        let servers = vec![
            Server { id: 1, name: "Istanbul, Turkey".into(), city: "Istanbul".into(), country: "Turkey".into(), host: None, distance_km: Some(10.0) },
            Server { id: 2, name: "Frankfurt, Germany".into(), city: "Frankfurt".into(), country: "Germany".into(), host: None, distance_km: Some(20.0) },
            Server { id: 3, name: "Berlin, Germany".into(), city: "Berlin".into(), country: "Germany".into(), host: None, distance_km: Some(30.0) },
            Server { id: 4, name: "Munich, Germany".into(), city: "Munich".into(), country: "Germany".into(), host: None, distance_km: Some(40.0) },
        ];
        let ranked = rank_servers(servers, &opts("de", "Frankfurt"));
        assert_eq!(ranked.iter().map(|server| server.id).collect::<Vec<_>>(), vec![2, 3, 4]);
    }

    #[test]
    fn parses_ookla_json_without_inventing_missing_metrics() {
        let json = br#"{"server":{"id":42,"name":"Test","location":"Stockholm","country":"Sweden"},"download":{"bandwidth":125000000},"upload":{"bandwidth":25000000},"ping":{"latency":2.5,"jitter":0.8},"timestamp":"2026-11-15T10:00:00Z"}"#;
        let result = parse_ookla(json, 42).unwrap();
        assert_eq!(result.best_download_mbps, Some(1000.0));
        assert_eq!(result.upload_mbps.unwrap().mbps, 200.0);
        assert_eq!(result.packet_loss_percent, None);
        assert!(parse_ookla(json, 43).is_err());
    }

    #[test]
    fn missing_owned_iperf_node_is_an_explicit_error() {
        let result = run_iperf(&opts("ru", "Moscow"));
        assert_eq!(result.status, "error");
        assert!(result.best_download_mbps.is_none());
    }

    #[test]
    fn ranks_swedish_servers_and_manual_region_over_bad_geo() {
        let servers = vec![
            Server { id: 1, name: "Frankfurt".into(), city: "Frankfurt".into(), country: "Germany".into(), host: None, distance_km: Some(2.0) },
            Server { id: 2, name: "Stockholm".into(), city: "Stockholm".into(), country: "Sweden".into(), host: None, distance_km: Some(20.0) },
            Server { id: 3, name: "Gothenburg".into(), city: "Gothenburg".into(), country: "Sweden".into(), host: None, distance_km: Some(80.0) },
        ];
        let ranked = rank_servers(servers, &opts("se", "Stockholm"));
        assert_eq!(ranked.iter().map(|server| server.id).collect::<Vec<_>>(), vec![2, 3]);
    }

    #[test]
    fn russian_candidates_stay_in_russia_when_region_is_manual() {
        let servers = vec![
            Server { id: 1, name: "Helsinki".into(), city: "Helsinki".into(), country: "Finland".into(), host: None, distance_km: Some(1.0) },
            Server { id: 2, name: "Moscow".into(), city: "Moscow".into(), country: "Russia".into(), host: None, distance_km: Some(30.0) },
        ];
        let ranked = rank_servers(servers, &opts("ru", "Moscow"));
        assert_eq!(ranked.iter().map(|server| server.id).collect::<Vec<_>>(), vec![2]);
    }

    #[test]
    fn parses_ookla_text_server_list_with_distance() {
        let servers = parse_server_text(" 12345) Provider (Frankfurt, Germany) [12.4 km]\n");
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].city, "Frankfurt");
        assert_eq!(servers[0].country, "Germany");
        assert_eq!(servers[0].distance_km, Some(12.4));
    }
}
