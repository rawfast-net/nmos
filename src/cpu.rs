use serde::Serialize;
use std::process::Command;
use sysinfo::System;

#[cfg(target_os = "linux")]
use std::fs;

#[derive(Debug, Clone, Serialize)]
pub struct CpuInfo {
    pub model: String,
    pub vendor: String,
    pub architecture: String,
    pub physical_cores: usize,
    pub logical_cores: usize,
    pub freq_mhz_reported: Option<u64>,
    pub freq_mhz_min: Option<u64>,
    pub freq_mhz_max: Option<u64>,
    pub freq_mhz_current_avg: Option<f64>,
    /// Доп. анализ частоты (VDS часто врёт «ровно 3000»)
    pub freq_analysis: FreqAnalysis,
    pub cache_l1_kb: Option<u64>,
    pub cache_l2_kb: Option<u64>,
    pub cache_l3_kb: Option<u64>,
    pub virtualization: VirtInfo,
    pub flags_sample: Vec<String>,
    pub steal_percent: Option<f64>,
    pub bogomips: Option<f64>,
    pub numa_nodes: Option<usize>,
    pub os: String,
    pub hostname: String,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct FreqAnalysis {
    pub cpufreq_available: bool,
    pub all_cores_same: bool,
    pub sticky_min_eq_max: bool,
    pub round_ghz_suspect: bool,
    pub bogomips_implied_mhz: Option<f64>,
    pub per_core_mhz: Vec<u64>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct VirtInfo {
    pub is_virtual: bool,
    pub hypervisor: Option<String>,
    pub virt_type: String,
    pub evidence: Vec<String>,
}

pub fn collect() -> CpuInfo {
    let mut sys = System::new_all();
    sys.refresh_all();

    let (model, vendor) = detect_model_vendor(&sys);
    let physical = sys.physical_core_count().unwrap_or(0);
    let logical = sys.cpus().len();

    let freqs_raw: Vec<u64> = sys.cpus().iter().map(|c| c.frequency()).collect();
    let freq_current_avg = if freqs_raw.is_empty() {
        None
    } else {
        let sum: u64 = freqs_raw.iter().sum();
        Some(sum as f64 / freqs_raw.len() as f64)
    };
    let freq_reported = freqs_raw.iter().copied().filter(|&f| f > 0).max();

    let (freq_min, freq_max) = read_freq_limits();
    let (l1, l2, l3) = read_cache_sizes();
    let virt = detect_virtualization();
    let flags = read_cpu_flags();
    let steal = read_steal_percent();
    let bogomips = read_bogomips();
    let numa = read_numa_nodes();
    let freq_analysis = analyze_frequency(
        freq_min,
        freq_max,
        freq_reported,
        freq_current_avg,
        bogomips,
        &freqs_raw,
    );

    CpuInfo {
        model,
        vendor,
        architecture: std::env::consts::ARCH.to_string(),
        physical_cores: physical,
        logical_cores: logical,
        freq_mhz_reported: freq_reported,
        freq_mhz_min: freq_min.or(freq_reported),
        freq_mhz_max: freq_max.or(freq_reported),
        freq_mhz_current_avg: freq_current_avg,
        freq_analysis,
        cache_l1_kb: l1,
        cache_l2_kb: l2,
        cache_l3_kb: l3,
        virtualization: virt,
        flags_sample: flags,
        steal_percent: steal,
        bogomips,
        numa_nodes: numa,
        os: format!("{} {}", System::name().unwrap_or_default(), System::os_version().unwrap_or_default()),
        hostname: System::host_name().unwrap_or_else(|| "unknown".into()),
    }
}

fn detect_model_vendor(sys: &System) -> (String, String) {
    let brand = sys
        .cpus()
        .first()
        .map(|c| c.brand().trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Unknown".into());

    #[cfg(target_os = "linux")]
    {
        if let Ok(content) = fs::read_to_string("/proc/cpuinfo") {
            let model = content
                .lines()
                .find(|l| l.starts_with("model name") || l.starts_with("Hardware"))
                .and_then(|l| l.split(':').nth(1))
                .map(|s| s.trim().to_string())
                .unwrap_or(brand.clone());
            let vendor = content
                .lines()
                .find(|l| l.starts_with("vendor_id") || l.starts_with("CPU implementer"))
                .and_then(|l| l.split(':').nth(1))
                .map(|s| s.trim().to_string())
                .unwrap_or_else(|| guess_vendor(&model));
            return (model, vendor);
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Some(model) = wmic_cpu_name() {
            return (model.clone(), guess_vendor(&model));
        }
    }

    (brand.clone(), guess_vendor(&brand))
}

fn guess_vendor(model: &str) -> String {
    let m = model.to_lowercase();
    if m.contains("amd") || m.contains("epyc") || m.contains("ryzen") || m.contains("opteron") || m.contains("threadripper") {
        "AuthenticAMD".into()
    } else if m.contains("intel")
        || m.contains("xeon")
        || m.contains("pentium")
        || m.contains("celeron")
        || m.contains("core i3")
        || m.contains("core i5")
        || m.contains("core i7")
        || m.contains("core i9")
        || m.contains("core(tm)")
    {
        "GenuineIntel".into()
    } else if m.contains("arm") || m.contains("ampere") || m.contains("neoverse") {
        "ARM".into()
    } else {
        "Unknown".into()
    }
}

#[cfg(target_os = "windows")]
fn wmic_cpu_name() -> Option<String> {
    // Prefer PowerShell CIM — wmic deprecated on newer Windows
    if let Ok(out) = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "(Get-CimInstance Win32_Processor | Select-Object -First 1 -ExpandProperty Name)",
        ])
        .output()
    {
        if out.status.success() {
            let v = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !v.is_empty() {
                return Some(v);
            }
        }
    }
    let out = Command::new("wmic")
        .args(["cpu", "get", "Name", "/value"])
        .output()
        .ok()?;
    let s = String::from_utf8_lossy(&out.stdout);
    for line in s.lines() {
        if let Some(v) = line.strip_prefix("Name=") {
            let v = v.trim();
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    None
}

fn analyze_frequency(
    min: Option<u64>,
    max: Option<u64>,
    reported: Option<u64>,
    current_avg: Option<f64>,
    bogomips: Option<f64>,
    per_core: &[u64],
) -> FreqAnalysis {
    let mut notes = Vec::new();
    let cpufreq_available = {
        #[cfg(target_os = "linux")]
        {
            std::path::Path::new("/sys/devices/system/cpu/cpu0/cpufreq").exists()
        }
        #[cfg(not(target_os = "linux"))]
        {
            false
        }
    };
    if !cpufreq_available {
        notes.push(
            "Нет cpufreq в sysfs — в госте VDS частота часто фиктивная (паспорт гипервизора)".into(),
        );
    }

    let nonzero: Vec<u64> = per_core.iter().copied().filter(|&f| f > 0).collect();
    let all_cores_same = nonzero.len() > 1 && nonzero.iter().all(|&f| f == nonzero[0]);
    if all_cores_same {
        notes.push(format!(
            "Все ядра показывают одинаковые {} МГц — типично для виртуалки",
            nonzero[0]
        ));
    }

    let sticky_min_eq_max = match (min, max) {
        (Some(a), Some(b)) if a > 0 && a == b => true,
        _ => false,
    };
    if sticky_min_eq_max {
        notes.push(format!(
            "min=max={} МГц — нет диапазона частот (turbo/P-states скрыты или фейк)",
            min.unwrap()
        ));
    }

    // «Круглые» GHz: 2000/2500/2800/3000/3200/3500 без дробной части из cpuinfo
    let suspect_vals = [2000u64, 2200, 2400, 2500, 2600, 2800, 3000, 3200, 3400, 3500, 3600];
    let round_ghz_suspect = [reported, max, min]
        .into_iter()
        .flatten()
        .any(|f| suspect_vals.contains(&f))
        && sticky_min_eq_max;

    if let Some(f) = reported.or(max) {
        if suspect_vals.contains(&f) {
            notes.push(format!(
                "Частота ровно {f} МГц ({:.2} GHz) — на VDS это почти всегда номинал из паспорта, не замер turbo",
                f as f64 / 1000.0
            ));
        }
    }

    let bogomips_implied_mhz = bogomips.map(|b| b / 2.0);
    if let (Some(imp), Some(rep)) = (bogomips_implied_mhz, reported.or(max)) {
        let ratio = imp / rep as f64;
        if !(0.85..=1.25).contains(&ratio) {
            notes.push(format!(
                "BogoMIPS/2 ≈ {imp:.0} МГц, паспорт {rep} МГц (ratio {ratio:.2}) — рассинхрон калибровки"
            ));
        }
    }

    if let Some(cur) = current_avg {
        if let Some(mx) = max {
            if (cur - mx as f64).abs() < 1.0 && sticky_min_eq_max {
                notes.push(
                    "Текущая = max = min — гость не видит реальные колебания частоты".into(),
                );
            }
        }
    }

    notes.push(
        "Важно: «реальный» CPU определяется по стресс-тесту и каталогу, а не по цифре 3.00 GHz".into(),
    );

    FreqAnalysis {
        cpufreq_available,
        all_cores_same,
        sticky_min_eq_max,
        round_ghz_suspect: round_ghz_suspect
            || reported.map(|f| suspect_vals.contains(&f)).unwrap_or(false),
        bogomips_implied_mhz,
        per_core_mhz: nonzero,
        notes,
    }
}

/// Текущая частота (МГц) из sysfs / cpuinfo — для замера под нагрузкой.
pub fn sample_current_mhz() -> Option<f64> {
    #[cfg(target_os = "linux")]
    {
        // scaling_cur_freq в kHz
        if let Ok(entries) = fs::read_dir("/sys/devices/system/cpu") {
            let mut vals = Vec::new();
            for e in entries.flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                if !name.starts_with("cpu")
                    || !name.chars().nth(3).map(|c| c.is_ascii_digit()).unwrap_or(false)
                {
                    continue;
                }
                if let Ok(s) = fs::read_to_string(e.path().join("cpufreq/scaling_cur_freq")) {
                    if let Ok(khz) = s.trim().parse::<f64>() {
                        vals.push(khz / 1000.0);
                    }
                }
            }
            if !vals.is_empty() {
                return Some(vals.iter().sum::<f64>() / vals.len() as f64);
            }
        }
        // fallback: /proc/cpuinfo
        if let Ok(content) = fs::read_to_string("/proc/cpuinfo") {
            let mut vals = Vec::new();
            for line in content.lines() {
                if line.starts_with("cpu MHz") {
                    if let Some(v) = line.split(':').nth(1) {
                        if let Ok(mhz) = v.trim().parse::<f64>() {
                            vals.push(mhz);
                        }
                    }
                }
            }
            if !vals.is_empty() {
                return Some(vals.iter().sum::<f64>() / vals.len() as f64);
            }
        }
    }
    None
}

fn read_freq_limits() -> (Option<u64>, Option<u64>) {
    #[cfg(target_os = "linux")]
    {
        let min = read_mhz("/sys/devices/system/cpu/cpu0/cpufreq/cpuinfo_min_freq")
            .or_else(|| read_mhz("/sys/devices/system/cpu/cpu0/cpufreq/scaling_min_freq"));
        let max = read_mhz("/sys/devices/system/cpu/cpu0/cpufreq/cpuinfo_max_freq")
            .or_else(|| read_mhz("/sys/devices/system/cpu/cpu0/cpufreq/scaling_max_freq"));
        return (min, max);
    }
    #[cfg(target_os = "windows")]
    {
        let out = Command::new("wmic")
            .args(["cpu", "get", "MaxClockSpeed", "/value"])
            .output()
            .ok();
        if let Some(out) = out {
            let s = String::from_utf8_lossy(&out.stdout);
            for line in s.lines() {
                if let Some(v) = line.strip_prefix("MaxClockSpeed=") {
                    if let Ok(mhz) = v.trim().parse::<u64>() {
                        return (None, Some(mhz));
                    }
                }
            }
        }
    }
    (None, None)
}

#[cfg(target_os = "linux")]
fn read_mhz(path: &str) -> Option<u64> {
    let raw: u64 = fs::read_to_string(path).ok()?.trim().parse().ok()?;
    // sysfs часто в kHz
    Some(if raw > 10_000 { raw / 1000 } else { raw })
}

fn read_cache_sizes() -> (Option<u64>, Option<u64>, Option<u64>) {
    #[cfg(target_os = "linux")]
    {
        let l1 = read_cache_kb("/sys/devices/system/cpu/cpu0/cache/index0/size")
            .or_else(|| read_cache_kb("/sys/devices/system/cpu/cpu0/cache/index1/size"));
        let l2 = read_cache_kb("/sys/devices/system/cpu/cpu0/cache/index2/size");
        let l3 = read_cache_kb("/sys/devices/system/cpu/cpu0/cache/index3/size");
        return (l1, l2, l3);
    }
    (None, None, None)
}

#[cfg(target_os = "linux")]
fn read_cache_kb(path: &str) -> Option<u64> {
    let s = fs::read_to_string(path).ok()?.trim().to_string();
    parse_size_kb(&s)
}

#[cfg(target_os = "linux")]
fn parse_size_kb(s: &str) -> Option<u64> {
    let s = s.trim().to_uppercase();
    if let Some(n) = s.strip_suffix('K') {
        return n.trim().parse().ok();
    }
    if let Some(n) = s.strip_suffix("KB") {
        return n.trim().parse().ok();
    }
    if let Some(n) = s.strip_suffix('M') {
        return n.trim().parse::<u64>().ok().map(|v| v * 1024);
    }
    if let Some(n) = s.strip_suffix("MB") {
        return n.trim().parse::<u64>().ok().map(|v| v * 1024);
    }
    s.parse().ok()
}

fn detect_virtualization() -> VirtInfo {
    let mut evidence = Vec::new();
    let mut hypervisor = None;
    let mut is_virtual = false;
    let mut virt_type = "bare-metal?".to_string();

    #[cfg(target_os = "linux")]
    {
        if let Ok(s) = fs::read_to_string("/sys/class/dmi/id/product_name") {
            let s = s.trim().to_string();
            if !s.is_empty() {
                evidence.push(format!("DMI product: {s}"));
                let low = s.to_lowercase();
                if low.contains("kvm") || low.contains("virtual") || low.contains("bochs")
                    || low.contains("vmware") || low.contains("virtualbox") || low.contains("hvm")
                    || low.contains("droplet") || low.contains("openstack") || low.contains("qemu")
                {
                    is_virtual = true;
                    hypervisor = Some(s.clone());
                    virt_type = "VM".into();
                }
            }
        }
        if let Ok(s) = fs::read_to_string("/sys/class/dmi/id/sys_vendor") {
            let s = s.trim().to_string();
            if !s.is_empty() {
                evidence.push(format!("DMI vendor: {s}"));
                let low = s.to_lowercase();
                if low.contains("qemu") || low.contains("xen") || low.contains("vmware")
                    || low.contains("microsoft") || low.contains("innotek") || low.contains("parallels")
                    || low.contains("amazon") || low.contains("google")
                {
                    is_virtual = true;
                    if hypervisor.is_none() {
                        hypervisor = Some(s);
                    }
                    virt_type = "VM".into();
                }
            }
        }
        if PathExists("/proc/xen").check() {
            is_virtual = true;
            hypervisor = Some("Xen".into());
            virt_type = "VM".into();
            evidence.push("Найден /proc/xen".into());
        }
        if let Ok(cpuinfo) = fs::read_to_string("/proc/cpuinfo") {
            if cpuinfo.contains("hypervisor") {
                is_virtual = true;
                evidence.push("Флаг hypervisor в /proc/cpuinfo".into());
                virt_type = "VM".into();
            }
        }
        if PathExists("/.dockerenv").check() {
            is_virtual = true;
            virt_type = "container".into();
            evidence.push("Обнаружен /.dockerenv".into());
        }
        if let Ok(c) = fs::read_to_string("/proc/1/cgroup") {
            if c.contains("docker") || c.contains("lxc") || c.contains("kubepods") {
                is_virtual = true;
                if virt_type != "VM" {
                    virt_type = "container".into();
                }
                evidence.push("Признаки контейнера в cgroup".into());
            }
        }
        // systemd-detect-virt
        if let Ok(out) = Command::new("systemd-detect-virt").output() {
            let v = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !v.is_empty() && v != "none" {
                is_virtual = true;
                hypervisor = Some(v.clone());
                virt_type = if v == "docker" || v == "lxc" || v == "podman" {
                    "container".into()
                } else {
                    "VM".into()
                };
                evidence.push(format!("systemd-detect-virt: {v}"));
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(out) = Command::new("wmic")
            .args(["computersystem", "get", "Model,Manufacturer", "/value"])
            .output()
        {
            let s = String::from_utf8_lossy(&out.stdout);
            for line in s.lines() {
                if line.starts_with("Manufacturer=") || line.starts_with("Model=") {
                    evidence.push(line.trim().to_string());
                    let low = line.to_lowercase();
                    if low.contains("vmware") || low.contains("virtual") || low.contains("kvm")
                        || low.contains("xen") || low.contains("hyper-v") || low.contains("virtualbox")
                        || low.contains("qemu") || low.contains("parallels")
                    {
                        is_virtual = true;
                        virt_type = "VM".into();
                        if let Some(v) = line.split('=').nth(1) {
                            hypervisor = Some(v.trim().to_string());
                        }
                    }
                }
            }
        }
    }

    if evidence.is_empty() {
        evidence.push("Явных признаков виртуализации не найдено".into());
    }

    VirtInfo {
        is_virtual,
        hypervisor,
        virt_type,
        evidence,
    }
}

#[cfg(target_os = "linux")]
struct PathExists(&'static str);
#[cfg(target_os = "linux")]
impl PathExists {
    fn check(&self) -> bool {
        std::path::Path::new(self.0).exists()
    }
}

fn read_cpu_flags() -> Vec<String> {
    #[cfg(target_os = "linux")]
    {
        if let Ok(content) = fs::read_to_string("/proc/cpuinfo") {
            if let Some(line) = content.lines().find(|l| l.starts_with("flags") || l.starts_with("Features")) {
                if let Some(flags) = line.split(':').nth(1) {
                    let interesting = [
                        "avx", "avx2", "avx512", "sse4_2", "aes", "sha", "vmx", "svm",
                        "hypervisor", "ht", "pdpe1gb", "rdrand", "rdseed",
                    ];
                    return flags
                        .split_whitespace()
                        .filter(|f| {
                            let fl = f.to_lowercase();
                            interesting.iter().any(|i| fl.contains(i))
                        })
                        .take(24)
                        .map(|s| s.to_string())
                        .collect();
                }
            }
        }
    }
    Vec::new()
}

fn read_steal_percent() -> Option<f64> {
    #[cfg(target_os = "linux")]
    {
        let a = read_proc_stat()?;
        std::thread::sleep(std::time::Duration::from_millis(400));
        let b = read_proc_stat()?;
        let idle = (b.idle + b.iowait) - (a.idle + a.iowait);
        let steal = b.steal.saturating_sub(a.steal);
        let total = b.total.saturating_sub(a.total);
        if total == 0 {
            return Some(0.0);
        }
        // steal + недогрузка относительно «честного» idle не нужна — только steal
        let _ = idle;
        return Some((steal as f64 / total as f64) * 100.0);
    }
    None
}

#[cfg(target_os = "linux")]
struct StatSample {
    idle: u64,
    iowait: u64,
    steal: u64,
    total: u64,
}

#[cfg(target_os = "linux")]
fn read_proc_stat() -> Option<StatSample> {
    let line = fs::read_to_string("/proc/stat").ok()?;
    let cpu = line.lines().next()?;
    let parts: Vec<u64> = cpu
        .split_whitespace()
        .skip(1)
        .filter_map(|x| x.parse().ok())
        .collect();
    if parts.len() < 8 {
        return None;
    }
    let idle = parts[3];
    let iowait = parts.get(4).copied().unwrap_or(0);
    let steal = parts.get(7).copied().unwrap_or(0);
    let total: u64 = parts.iter().sum();
    Some(StatSample {
        idle,
        iowait,
        steal,
        total,
    })
}

fn read_bogomips() -> Option<f64> {
    #[cfg(target_os = "linux")]
    {
        let content = fs::read_to_string("/proc/cpuinfo").ok()?;
        for line in content.lines() {
            if line.to_lowercase().starts_with("bogomips") || line.starts_with("BogoMIPS") {
                if let Some(v) = line.split(':').nth(1) {
                    return v.trim().parse().ok();
                }
            }
        }
    }
    None
}

fn read_numa_nodes() -> Option<usize> {
    #[cfg(target_os = "linux")]
    {
        let path = std::path::Path::new("/sys/devices/system/node");
        if !path.exists() {
            return Some(1);
        }
        let count = fs::read_dir(path)
            .ok()?
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with("node")
                    && e.file_name()
                        .to_string_lossy()
                        .chars()
                        .nth(4)
                        .map(|c| c.is_ascii_digit())
                        .unwrap_or(false)
            })
            .count();
        return Some(count.max(1));
    }
    None
}
