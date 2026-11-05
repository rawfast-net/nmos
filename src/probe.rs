
use serde::Serialize;

#[cfg(target_os = "linux")]
use std::collections::BTreeMap;
#[cfg(target_os = "linux")]
use std::fs;
#[cfg(target_os = "linux")]
use std::path::Path;
#[cfg(target_os = "linux")]
use std::process::Command;

#[derive(Debug, Clone, Serialize, Default)]
pub struct DeepProbe {
    pub dmi: DmiInfo,
    pub cpu_ids: CpuIds,
    pub topology: Topology,
    pub governor: Option<String>,
    pub turbo_enabled: Option<bool>,
    pub cmdline: Option<String>,
    pub cgroup: CgroupLimits,
    pub vulnerabilities: Vec<VulnStatus>,
    pub mem_detail: MemDetail,
    pub virtio_devices: Vec<String>,
    pub block_devices: Vec<BlockDev>,
    pub lscpu_summary: Option<String>,
    pub evidence_extra: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct DmiInfo {
    pub sys_vendor: Option<String>,
    pub product_name: Option<String>,
    pub product_version: Option<String>,
    pub board_vendor: Option<String>,
    pub board_name: Option<String>,
    pub bios_vendor: Option<String>,
    pub bios_version: Option<String>,
    pub chassis_vendor: Option<String>,
    pub chassis_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct CpuIds {
    pub family: Option<u32>,
    pub model: Option<u32>,
    pub stepping: Option<u32>,
    pub microcode: Option<String>,
    pub cpu_mhz_cpuinfo: Option<f64>,
    pub cache_size_cpuinfo: Option<String>,
    pub address_sizes: Option<String>,
    pub bugs: Vec<String>,
    pub flags_count: usize,
    pub has_avx: bool,
    pub has_avx2: bool,
    pub has_avx512: bool,
    pub has_aes: bool,
    pub has_sha: bool,
    pub has_hypervisor_flag: bool,
    pub has_svm_or_vmx: bool,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct Topology {
    pub sockets: Option<usize>,
    pub cores_per_socket: Option<usize>,
    pub threads_per_core: Option<usize>,
    pub online_cpus: Option<String>,
    pub present_cpus: Option<String>,
    pub packages: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct CgroupLimits {
    pub version: Option<String>,
    pub cpu_quota_cores: Option<f64>,
    pub cpu_period_us: Option<u64>,
    pub cpu_max_raw: Option<String>,
    pub memory_max_mb: Option<u64>,
    pub memory_current_mb: Option<u64>,
    pub cpuset_cpus: Option<String>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct VulnStatus {
    pub name: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct MemDetail {
    pub hugepages_total: Option<u64>,
    pub hugepage_size_kb: Option<u64>,
    pub anon_huge_pages_kb: Option<u64>,
    pub dirty_kb: Option<u64>,
    pub commit_limit_mb: Option<u64>,
    pub committed_as_mb: Option<u64>,
    pub mem_available_mb: Option<u64>,
    pub buffers_mb: Option<u64>,
    pub cached_mb: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BlockDev {
    pub name: String,
    pub size_gb: Option<f64>,
    pub model: Option<String>,
    pub rota: Option<bool>,
    pub sched: Option<String>,
}

pub fn collect() -> DeepProbe {
    #[cfg(not(target_os = "linux"))]
    {
        let mut p = DeepProbe::default();
        p.evidence_extra
            .push("Глубокий Linux-зонд доступен только на Linux".into());
        return p;
    }

    #[cfg(target_os = "linux")]
    {
        let mut evidence = Vec::new();
        let dmi = read_dmi();
        let cpu_ids = read_cpu_ids();
        let topology = read_topology();
        let governor = read_first_line("/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor");
        let turbo = read_turbo();
        let cmdline = fs::read_to_string("/proc/cmdline")
            .ok()
            .map(|s| s.trim().to_string());
        let cgroup = read_cgroup();
        let vulnerabilities = read_vulns();
        let mem_detail = read_mem_detail();
        let virtio_devices = read_virtio(&mut evidence);
        let block_devices = read_block_devices();
        let lscpu_summary = run_lscpu_brief();

        if cpu_ids.has_hypervisor_flag {
            evidence.push("Флаг hypervisor в CPUID/flags".into());
        }
        if let Some(ref cmd) = cmdline {
            if cmd.contains("console=hvc") || cmd.contains("xen") {
                evidence.push("cmdline намекает на Xen/PV".into());
            }
        }

        DeepProbe {
            dmi,
            cpu_ids,
            topology,
            governor,
            turbo_enabled: turbo,
            cmdline,
            cgroup,
            vulnerabilities,
            mem_detail,
            virtio_devices,
            block_devices,
            lscpu_summary,
            evidence_extra: evidence,
        }
    }
}

#[cfg(target_os = "linux")]
fn read_dmi() -> DmiInfo {
    let r = |name: &str| {
        fs::read_to_string(format!("/sys/class/dmi/id/{name}"))
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty() && s != "None")
    };
    DmiInfo {
        sys_vendor: r("sys_vendor"),
        product_name: r("product_name"),
        product_version: r("product_version"),
        board_vendor: r("board_vendor"),
        board_name: r("board_name"),
        bios_vendor: r("bios_vendor"),
        bios_version: r("bios_version"),
        chassis_vendor: r("chassis_vendor"),
        chassis_type: r("chassis_type"),
    }
}

#[cfg(target_os = "linux")]
fn read_cpu_ids() -> CpuIds {
    let content = fs::read_to_string("/proc/cpuinfo").unwrap_or_default();
    let get_u32 = |key: &str| -> Option<u32> {
        content
            .lines()
            .find(|l| l.starts_with(key))
            .and_then(|l| l.split(':').nth(1))
            .and_then(|v| v.trim().parse().ok())
    };
    let get_str = |key: &str| -> Option<String> {
        content
            .lines()
            .find(|l| l.starts_with(key))
            .and_then(|l| l.split(':').nth(1))
            .map(|v| v.trim().to_string())
            .filter(|s| !s.is_empty())
    };
    let get_f64 = |key: &str| -> Option<f64> {
        content
            .lines()
            .find(|l| l.starts_with(key))
            .and_then(|l| l.split(':').nth(1))
            .and_then(|v| v.trim().parse().ok())
    };

    let flags_line = content
        .lines()
        .find(|l| l.starts_with("flags") || l.starts_with("Features"))
        .and_then(|l| l.split(':').nth(1))
        .unwrap_or("")
        .to_lowercase();
    let flags: Vec<&str> = flags_line.split_whitespace().collect();

    let bugs = content
        .lines()
        .find(|l| l.starts_with("bugs"))
        .and_then(|l| l.split(':').nth(1))
        .map(|v| {
            v.split_whitespace()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    CpuIds {
        family: get_u32("cpu family"),
        model: get_u32("model").or_else(|| get_u32("CPU part")),
        stepping: get_u32("stepping"),
        microcode: get_str("microcode"),
        cpu_mhz_cpuinfo: get_f64("cpu MHz"),
        cache_size_cpuinfo: get_str("cache size"),
        address_sizes: get_str("address sizes"),
        bugs,
        flags_count: flags.len(),
        has_avx: flags.iter().any(|f| *f == "avx"),
        has_avx2: flags.iter().any(|f| *f == "avx2"),
        has_avx512: flags.iter().any(|f| f.starts_with("avx512")),
        has_aes: flags.iter().any(|f| *f == "aes"),
        has_sha: flags.iter().any(|f| f.starts_with("sha")),
        has_hypervisor_flag: flags.iter().any(|f| *f == "hypervisor"),
        has_svm_or_vmx: flags.iter().any(|f| *f == "svm" || *f == "vmx"),
    }
}

#[cfg(target_os = "linux")]
fn read_topology() -> Topology {
    let online = read_first_line("/sys/devices/system/cpu/online");
    let present = read_first_line("/sys/devices/system/cpu/present");

    let mut packages = Vec::new();
    if let Ok(entries) = fs::read_dir("/sys/devices/system/cpu") {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if !name.starts_with("cpu") || !name.chars().nth(3).map(|c| c.is_ascii_digit()).unwrap_or(false)
            {
                continue;
            }
            if let Ok(pkg) = fs::read_to_string(e.path().join("topology/physical_package_id")) {
                let pkg = pkg.trim().to_string();
                if !packages.contains(&pkg) {
                    packages.push(pkg);
                }
            }
        }
    }
    packages.sort();

    // threads/cores из cpu0 topology
    let thread_siblings = fs::read_to_string(
        "/sys/devices/system/cpu/cpu0/topology/thread_siblings_list",
    )
    .ok();
    let core_siblings =
        fs::read_to_string("/sys/devices/system/cpu/cpu0/topology/core_siblings_list").ok();

    let threads_per_core = thread_siblings.as_ref().map(|s| {
        s.trim()
            .split([',', '-'])
            .filter(|x| !x.is_empty())
            .count()
            .max(1)
    });

    // грубо: cores in package from core_siblings / threads
    let cores_per_socket = match (&core_siblings, threads_per_core) {
        (Some(cs), Some(t)) if t > 0 => {
            let n = parse_cpu_list_count(cs.trim());
            Some((n / t).max(1))
        }
        _ => None,
    };

    Topology {
        sockets: if packages.is_empty() {
            Some(1)
        } else {
            Some(packages.len())
        },
        cores_per_socket,
        threads_per_core,
        online_cpus: online,
        present_cpus: present,
        packages,
    }
}

#[cfg(target_os = "linux")]
fn parse_cpu_list_count(s: &str) -> usize {
    let mut n = 0usize;
    for part in s.split(',') {
        if let Some((a, b)) = part.split_once('-') {
            if let (Ok(a), Ok(b)) = (a.parse::<usize>(), b.parse::<usize>()) {
                n += b.saturating_sub(a) + 1;
            }
        } else if part.parse::<usize>().is_ok() {
            n += 1;
        }
    }
    n.max(1)
}

#[cfg(target_os = "linux")]
fn read_turbo() -> Option<bool> {
    // Intel
    if let Ok(v) = fs::read_to_string("/sys/devices/system/cpu/intel_pstate/no_turbo") {
        return Some(v.trim() == "0");
    }
    // AMD cpufreq boost
    if let Ok(v) = fs::read_to_string("/sys/devices/system/cpu/cpufreq/boost") {
        return Some(v.trim() == "1");
    }
    None
}

#[cfg(target_os = "linux")]
fn read_cgroup() -> CgroupLimits {
    let mut notes = Vec::new();
    let mut out = CgroupLimits {
        notes: Vec::new(),
        ..Default::default()
    };

    // cgroup v2?
    if Path::new("/sys/fs/cgroup/cgroup.controllers").exists() {
        out.version = Some("v2".into());
        // текущий cgroup процесса
        let cg_path = current_cgroup_v2_path();
        if let Some(ref base) = cg_path {
            let cpu_max = fs::read_to_string(format!("{base}/cpu.max")).ok();
            if let Some(raw) = cpu_max {
                let raw = raw.trim().to_string();
                out.cpu_max_raw = Some(raw.clone());
                let mut parts = raw.split_whitespace();
                if let (Some(quota), Some(period)) = (parts.next(), parts.next()) {
                    if let Ok(period_u) = period.parse::<u64>() {
                        out.cpu_period_us = Some(period_u);
                        if quota != "max" {
                            if let Ok(q) = quota.parse::<f64>() {
                                out.cpu_quota_cores = Some(q / period_u as f64);
                            }
                        } else {
                            notes.push("cpu.max = max (квота не ограничена)".into());
                        }
                    }
                }
            }
            if let Ok(mem) = fs::read_to_string(format!("{base}/memory.max")) {
                let mem = mem.trim();
                if mem != "max" {
                    if let Ok(b) = mem.parse::<u64>() {
                        out.memory_max_mb = Some(b / 1024 / 1024);
                    }
                }
            }
            if let Ok(cur) = fs::read_to_string(format!("{base}/memory.current")) {
                if let Ok(b) = cur.trim().parse::<u64>() {
                    out.memory_current_mb = Some(b / 1024 / 1024);
                }
            }
            if let Ok(cpus) = fs::read_to_string(format!("{base}/cpuset.cpus")) {
                let cpus = cpus.trim().to_string();
                if !cpus.is_empty() {
                    out.cpuset_cpus = Some(cpus);
                }
            }
        } else {
            notes.push("Не удалось определить путь cgroup v2 процесса".into());
        }
    } else if Path::new("/sys/fs/cgroup/cpu").exists() {
        out.version = Some("v1".into());
        // упрощённо: /sys/fs/cgroup/cpu/cpu.cfs_quota_us
        let quota = read_first_line("/sys/fs/cgroup/cpu/cpu.cfs_quota_us")
            .and_then(|s| s.parse::<i64>().ok());
        let period = read_first_line("/sys/fs/cgroup/cpu/cpu.cfs_period_us")
            .and_then(|s| s.parse::<u64>().ok());
        if let (Some(q), Some(p)) = (quota, period) {
            out.cpu_period_us = Some(p);
            if q > 0 {
                out.cpu_quota_cores = Some(q as f64 / p as f64);
            } else if q == -1 {
                notes.push("cfs_quota = -1 (без лимита)".into());
            }
        }
        if let Ok(mem) = fs::read_to_string("/sys/fs/cgroup/memory/memory.limit_in_bytes") {
            if let Ok(b) = mem.trim().parse::<u64>() {
                // огромный лимит = «без лимита»
                if b < (1u64 << 60) {
                    out.memory_max_mb = Some(b / 1024 / 1024);
                }
            }
        }
    } else {
        notes.push("cgroup не обнаружен".into());
    }

    out.notes = notes;
    out
}

#[cfg(target_os = "linux")]
fn current_cgroup_v2_path() -> Option<String> {
    let cg = fs::read_to_string("/proc/self/cgroup").ok()?;
    // формат v2: 0::/user.slice/...
    for line in cg.lines() {
        if let Some(path) = line.strip_prefix("0::") {
            let path = path.trim();
            let full = if path == "/" {
                "/sys/fs/cgroup".to_string()
            } else {
                format!("/sys/fs/cgroup{path}")
            };
            if Path::new(&full).exists() {
                return Some(full);
            }
        }
    }
    Some("/sys/fs/cgroup".into())
}

#[cfg(target_os = "linux")]
fn read_vulns() -> Vec<VulnStatus> {
    let dir = Path::new("/sys/devices/system/cpu/vulnerabilities");
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return out;
    };
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if let Ok(status) = fs::read_to_string(e.path()) {
            out.push(VulnStatus {
                name,
                status: status.trim().to_string(),
            });
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

#[cfg(target_os = "linux")]
fn read_mem_detail() -> MemDetail {
    let content = fs::read_to_string("/proc/meminfo").unwrap_or_default();
    let map: BTreeMap<String, u64> = content
        .lines()
        .filter_map(|l| {
            let mut p = l.split_whitespace();
            let k = p.next()?.trim_end_matches(':').to_string();
            let v: u64 = p.next()?.parse().ok()?;
            Some((k, v))
        })
        .collect();
    let kb = |k: &str| map.get(k).copied();
    let mb = |k: &str| kb(k).map(|v| v / 1024);

    MemDetail {
        hugepages_total: kb("HugePages_Total"),
        hugepage_size_kb: kb("Hugepagesize"),
        anon_huge_pages_kb: kb("AnonHugePages"),
        dirty_kb: kb("Dirty"),
        commit_limit_mb: mb("CommitLimit"),
        committed_as_mb: mb("Committed_AS"),
        mem_available_mb: mb("MemAvailable"),
        buffers_mb: mb("Buffers"),
        cached_mb: mb("Cached"),
    }
}

#[cfg(target_os = "linux")]
fn read_virtio(evidence: &mut Vec<String>) -> Vec<String> {
    let mut out = Vec::new();
    // /sys/bus/virtio/devices
    if let Ok(entries) = fs::read_dir("/sys/bus/virtio/devices") {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            let virtio = fs::read_to_string(e.path().join("device"))
                .ok()
                .map(|s| s.trim().to_string())
                .unwrap_or_default();
            out.push(format!("{name} ({virtio})"));
        }
    }
    if !out.is_empty() {
        evidence.push(format!("virtio устройств: {}", out.len()));
    }
    // pci vendor hints
    if let Ok(outp) = Command::new("lspci").arg("-nn").output() {
        let s = String::from_utf8_lossy(&outp.stdout);
        for key in ["Virtio", "QEMU", "Red Hat", "Xen", "VMware", "Amazon", "Google"] {
            if s.contains(key) {
                evidence.push(format!("lspci: найден «{key}»"));
            }
        }
    }
    out
}

#[cfg(target_os = "linux")]
fn read_block_devices() -> Vec<BlockDev> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir("/sys/block") else {
        return out;
    };
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if name.starts_with("loop") || name.starts_with("ram") || name.starts_with("dm-") {
            continue;
        }
        let size_sectors = fs::read_to_string(e.path().join("size"))
            .ok()
            .and_then(|s| s.trim().parse::<u64>().ok());
        let size_gb = size_sectors.map(|s| (s as f64 * 512.0) / 1e9);
        let model = fs::read_to_string(e.path().join("device/model"))
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let rota = fs::read_to_string(e.path().join("queue/rotational"))
            .ok()
            .map(|s| s.trim() == "1");
        // virtio/vd*/xvd* часто врёт rotational=1 при SSD/NVMe на хосте
        let rota = if name.starts_with("vd")
            || name.starts_with("xvd")
            || name.starts_with("nvme")
            || model
                .as_ref()
                .map(|m| {
                    let m = m.to_lowercase();
                    m.contains("virtio") || m.contains("qemu") || m.contains("nvme")
                })
                .unwrap_or(false)
        {
            Some(false)
        } else {
            rota
        };
        let sched = fs::read_to_string(e.path().join("queue/scheduler"))
            .ok()
            .map(|s| {
                // [mq-deadline] none
                s.split_whitespace()
                    .find(|x| x.starts_with('[') && x.ends_with(']'))
                    .map(|x| x.trim_matches(|c| c == '[' || c == ']').to_string())
                    .unwrap_or_else(|| s.trim().to_string())
            });
        out.push(BlockDev {
            name,
            size_gb,
            model,
            rota,
            sched,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

#[cfg(target_os = "linux")]
fn run_lscpu_brief() -> Option<String> {
    let out = Command::new("lscpu").output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout);
    let keys = [
        "Architecture",
        "CPU(s)",
        "Thread(s) per core",
        "Core(s) per socket",
        "Socket(s)",
        "Model name",
        "CPU max MHz",
        "CPU min MHz",
        "Virtualization",
        "Hypervisor vendor",
        "Vulnerability",
    ];
    let mut lines = Vec::new();
    for line in s.lines() {
        if keys.iter().any(|k| line.starts_with(k)) {
            lines.push(line.trim().to_string());
        }
    }
    if lines.is_empty() {
        None
    } else {
        Some(lines.join(" | "))
    }
}

#[cfg(target_os = "linux")]
fn read_first_line(path: &str) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}
