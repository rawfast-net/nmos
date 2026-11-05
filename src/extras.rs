
use crate::cpu::CpuInfo;
use crate::diskio::DiskIoResult;
use crate::network::NetworkInfo;
use crate::probe::DeepProbe;
use crate::stress::StressResult;
use crate::system::SystemInfo;
use serde::Serialize;

#[cfg(target_os = "linux")]
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize)]
pub struct ExtraFinding {
    pub severity: String,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct ExtraProbe {
    pub clocksource: Option<String>,
    pub clocksource_available: Vec<String>,
    pub entropy_avail: Option<u64>,
    pub entropy_poolsize: Option<u64>,
    pub thp_enabled: Option<String>,
    pub overcommit_memory: Option<String>,
    pub overcommit_ratio: Option<String>,
    pub ksm_run: Option<bool>,
    pub zram_present: bool,
    pub zswap_enabled: Option<bool>,
    pub cpu_pressure_some_avg10: Option<f64>,
    pub io_pressure_some_avg10: Option<f64>,
    pub mem_pressure_some_avg10: Option<f64>,
    pub cgroup_nr_throttled: Option<u64>,
    pub cgroup_throttled_usec: Option<u64>,
    pub cgroup_cpu_weight: Option<u64>,
    pub cgroup_memory_high_mb: Option<u64>,
    pub cgroup_io_max: Option<String>,
    pub cpuset_count: Option<usize>,
    pub online_cpu_count: Option<usize>,
    pub present_cpu_count: Option<usize>,
    pub root_fstype: Option<String>,
    pub root_source: Option<String>,
    pub modules_virt: Vec<String>,
    pub net_drivers: Vec<String>,
    pub net_link_speeds: Vec<String>,
    pub sleep_jitter_p99_us: Option<f64>,
    pub sleep_jitter_avg_us: Option<f64>,
    pub product_uuid: Option<String>,
    pub hypervisor_vendor: Option<String>,
    pub container_hints: Vec<String>,
    pub load_vs_cores: Option<f64>,
    pub notes: Vec<String>,
}

pub fn collect(sys: &SystemInfo, cpu: &CpuInfo) -> ExtraProbe {
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (sys, cpu);
        let mut e = ExtraProbe::default();
        e.notes.push("Расширенный зонд только на Linux".into());
        return e;
    }

    #[cfg(target_os = "linux")]
    {
        use std::fs;
        use std::path::Path;

        let mut notes = Vec::new();
        let clocksource = read_trim(
            "/sys/devices/system/clocksource/clocksource0/current_clocksource",
        );
        let clocksource_available = read_trim(
            "/sys/devices/system/clocksource/clocksource0/available_clocksource",
        )
        .map(|s| s.split_whitespace().map(|x| x.to_string()).collect())
        .unwrap_or_default();

        let entropy_avail = read_u64("/proc/sys/kernel/random/entropy_avail");
        let entropy_poolsize = read_u64("/proc/sys/kernel/random/poolsize");
        let thp_enabled = read_trim("/sys/kernel/mm/transparent_hugepage/enabled")
            .map(|s| {
                // [always] madvise never → extract bracket or whole
                s.split_whitespace()
                    .find(|x| x.starts_with('[') && x.ends_with(']'))
                    .map(|x| x.trim_matches(|c| c == '[' || c == ']').to_string())
                    .unwrap_or(s)
            });
        let overcommit_memory = read_trim("/proc/sys/vm/overcommit_memory");
        let overcommit_ratio = read_trim("/proc/sys/vm/overcommit_ratio");
        let ksm_run = read_trim("/sys/kernel/mm/ksm/run").map(|s| s.trim() == "1");
        let zram_present = Path::new("/sys/block")
            .read_dir()
            .map(|rd| {
                rd.flatten()
                    .any(|e| e.file_name().to_string_lossy().starts_with("zram"))
            })
            .unwrap_or(false);
        let zswap_enabled = read_trim("/sys/module/zswap/parameters/enabled")
            .or_else(|| read_trim("/sys/module/zswap/parameters/zswap_enabled"))
            .map(|s| s == "Y" || s == "1");

        let cpu_pressure_some_avg10 = parse_psi_some("/proc/pressure/cpu");
        let io_pressure_some_avg10 = parse_psi_some("/proc/pressure/io");
        let mem_pressure_some_avg10 = parse_psi_some("/proc/pressure/memory");

        let (cgroup_nr_throttled, cgroup_throttled_usec, cgroup_cpu_weight, cgroup_memory_high_mb, cgroup_io_max) =
            read_cgroup_extras();

        let cpuset_count = count_cpu_list(
            &fs::read_to_string("/sys/fs/cgroup/cpuset.cpus.effective")
                .or_else(|_| fs::read_to_string("/sys/fs/cgroup/cpuset.cpus"))
                .or_else(|_| fs::read_to_string("/proc/self/cpuset"))
                .unwrap_or_default(),
        );
        let online_cpu_count = count_cpu_list(
            &fs::read_to_string("/sys/devices/system/cpu/online").unwrap_or_default(),
        );
        let present_cpu_count = count_cpu_list(
            &fs::read_to_string("/sys/devices/system/cpu/present").unwrap_or_default(),
        );

        let (root_fstype, root_source) = read_root_fs();
        let modules_virt = read_virt_modules();
        let (net_drivers, net_link_speeds) = read_net_drivers();
        let (sleep_jitter_avg_us, sleep_jitter_p99_us) = measure_sleep_jitter();
        let product_uuid = read_trim("/sys/class/dmi/id/product_uuid");
        let hypervisor_vendor = parse_lscpu_hypervisor();
        let container_hints = detect_container_hints();

        let load_vs_cores = sys
            .load_avg
            .map(|l| l[0] / cpu.logical_cores.max(1) as f64);

        if clocksource.is_none() {
            notes.push("clocksource недоступен".into());
        }

        ExtraProbe {
            clocksource,
            clocksource_available,
            entropy_avail,
            entropy_poolsize,
            thp_enabled,
            overcommit_memory,
            overcommit_ratio,
            ksm_run,
            zram_present,
            zswap_enabled,
            cpu_pressure_some_avg10,
            io_pressure_some_avg10,
            mem_pressure_some_avg10,
            cgroup_nr_throttled,
            cgroup_throttled_usec,
            cgroup_cpu_weight,
            cgroup_memory_high_mb,
            cgroup_io_max,
            cpuset_count,
            online_cpu_count,
            present_cpu_count,
            root_fstype,
            root_source,
            modules_virt,
            net_drivers,
            net_link_speeds,
            sleep_jitter_p99_us,
            sleep_jitter_avg_us,
            product_uuid,
            hypervisor_vendor,
            container_hints,
            load_vs_cores,
            notes,
        }
    }
}

/// Скоринг расширенных сигналов → findings для Trust.
pub fn analyze(
    x: &ExtraProbe,
    cpu: &CpuInfo,
    deep: &DeepProbe,
    sys: &SystemInfo,
    net: &NetworkInfo,
    disk: Option<&DiskIoResult>,
    stress: Option<&StressResult>,
) -> Vec<ExtraFinding> {
    let mut f = Vec::new();

    if let Some(ref cs) = x.clocksource {
        let cs_l = cs.to_lowercase();
        if cs_l.contains("kvm") || cs_l.contains("xen") || cs_l.contains("hyperv") {
            f.push(info(
                "clock_paravirt",
                format!("clocksource={cs} — паравирт таймер (типично для VM)"),
            ));
        } else if cs_l == "tsc" {
            f.push(info("clock_tsc", "clocksource=tsc".into()));
        } else {
            f.push(warn(
                "clock_odd",
                format!("Необычный clocksource={cs}"),
            ));
        }
        if !x.clocksource_available.is_empty() {
            f.push(info(
                "clock_available",
                format!("available: {}", x.clocksource_available.join(", ")),
            ));
        }
    }

    if let (Some(avg), Some(p99)) = (x.sleep_jitter_avg_us, x.sleep_jitter_p99_us) {
        f.push(info(
            "timer_jitter",
            format!("nanosleep jitter avg={avg:.0}µs p99={p99:.0}µs"),
        ));
        if p99 > 2500.0 {
            f.push(warn(
                "timer_jitter_high",
                format!("p99 sleep jitter {p99:.0}µs — шумные соседи / steal / плохие таймеры"),
            ));
        }
    }

    if let Some(e) = x.entropy_avail {
        if e < 64 {
            f.push(warn(
                "entropy_low",
                format!("entropy_avail={e} — пул энтропии низкий"),
            ));
        } else {
            f.push(info("entropy_ok", format!("entropy_avail={e}")));
        }
    }

    if let Some(p) = x.cpu_pressure_some_avg10 {
        if p > 30.0 {
            f.push(crit(
                "psi_cpu_high",
                format!("CPU PSI some avg10={p:.1}% — сильный contention"),
            ));
        } else if p > 10.0 {
            f.push(warn(
                "psi_cpu_mid",
                format!("CPU PSI some avg10={p:.1}% — нагрузка/соседи"),
            ));
        } else {
            f.push(info("psi_cpu_ok", format!("CPU PSI avg10={p:.1}%")));
        }
    }
    if let Some(p) = x.io_pressure_some_avg10 {
        if p > 20.0 {
            f.push(warn(
                "psi_io_high",
                format!("IO PSI some avg10={p:.1}% — диск/соседи давят"),
            ));
        } else {
            f.push(info("psi_io", format!("IO PSI avg10={p:.1}%")));
        }
    }
    if let Some(p) = x.mem_pressure_some_avg10 {
        if p > 15.0 {
            f.push(warn(
                "psi_mem_high",
                format!("Memory PSI some avg10={p:.1}% — reclaim/balloon"),
            ));
        } else {
            f.push(info("psi_mem", format!("Memory PSI avg10={p:.1}%")));
        }
    }

    if let Some(n) = x.cgroup_nr_throttled {
        let usec = x.cgroup_throttled_usec.unwrap_or(0);
        // nr_throttled — lifetime счётчик; без usec/uptime нельзя считать «сейчас режут»
        if usec >= 5_000_000 {
            f.push(warn(
                "cgroup_throttled",
                format!(
                    "cpu.stat throttled_usec={usec} (nr={n}) — квота реально съела ≥5с CPU"
                ),
            ));
        } else if n > 0 {
            f.push(info(
                "cgroup_throttled_hist",
                format!(
                    "cpu.stat nr_throttled={n} usec={usec} — lifetime история, не обязательно активный throttle"
                ),
            ));
        } else {
            f.push(info("cgroup_throttle_ok", "cpu.stat: throttling нет".into()));
        }
    }
    if let Some(w) = x.cgroup_cpu_weight {
        if w < 50 {
            f.push(warn(
                "cgroup_cpu_weight_low",
                format!("cpu.weight={w} — низкий приоритет относительно соседей"),
            ));
        } else {
            f.push(info("cgroup_cpu_weight", format!("cpu.weight={w}")));
        }
    }
    if let Some(h) = x.cgroup_memory_high_mb {
        f.push(warn(
            "cgroup_memory_high",
            format!("memory.high≈{h} MB — soft reclaim до OOM"),
        ));
    }
    if let Some(ref io) = x.cgroup_io_max {
        if io != "max" && !io.is_empty() {
            f.push(warn(
                "cgroup_io_max",
                format!("io.max ограничен: {io}"),
            ));
        }
    }

    if let (Some(set), Some(online)) = (x.cpuset_count, x.online_cpu_count) {
        if set + 0 < online && set < cpu.logical_cores {
            f.push(warn(
                "cpuset_narrow",
                format!("cpuset≈{set} CPU, online={online}, logical={} — ядра урезаны cpuset", cpu.logical_cores),
            ));
        } else {
            f.push(info(
                "cpuset_ok",
                format!("cpuset≈{set}, online={online}"),
            ));
        }
    }
    if let (Some(on), Some(pres)) = (x.online_cpu_count, x.present_cpu_count) {
        if on < pres {
            f.push(warn(
                "cpu_hotplug_offline",
                format!("online={on} < present={pres} — часть CPU offline"),
            ));
        }
    }
    if let (Some(sock), Some(cps), Some(tpc)) = (
        deep.topology.sockets,
        deep.topology.cores_per_socket,
        deep.topology.threads_per_core,
    ) {
        let topo = sock.saturating_mul(cps).saturating_mul(tpc);
        f.push(info(
            "topo_smt",
            format!("topology sockets={sock} cores/sock≈{cps} thr/core≈{tpc} (≈{topo})"),
        ));
        if tpc > 1 && cpu.logical_cores <= 2 {
            f.push(info(
                "topo_smt_slice",
                "SMT>1 на хосте, гостю мало потоков — нормальный vCPU slice".into(),
            ));
        }
    }

    let vendor = deep.dmi.sys_vendor.clone().unwrap_or_default();
    let product = deep.dmi.product_name.clone().unwrap_or_default();
    let bios = deep
        .dmi
        .bios_vendor
        .clone()
        .unwrap_or_default()
        .to_lowercase();
    let vl = vendor.to_lowercase();
    let pl = product.to_lowercase();
    if vl.contains("qemu") || pl.contains("qemu") || pl.contains("kvm") || bios.contains("seabios")
    {
        f.push(info(
            "dmi_kvm",
            format!("DMI {vendor} / {product} — KVM/QEMU гость"),
        ));
    }
    if (vl.contains("google") || vl.contains("amazon") || vl.contains("microsoft") || vl.contains("ovh") || vl.contains("hetzner") || vl.contains("aeza") || vl.contains("selectel"))
        && !cpu.virtualization.is_virtual
    {
        f.push(warn(
            "dmi_cloud_no_virt",
            format!("DMI vendor «{vendor}», но виртуализация не детектится — странно"),
        ));
    }
    let model_l = cpu.model.to_lowercase();
    if (model_l.contains("epyc") || model_l.contains("xeon") || model_l.contains("ryzen"))
        && (pl.contains("standard pc") || pl.contains("qemu") || pl.is_empty())
        && cpu.virtualization.is_virtual
    {
        f.push(info(
            "dmi_generic_product",
            format!("Паспорт CPU «дорогой», DMI product «{product}» — норма для VDS"),
        ));
    }
    if let Some(ref uuid) = x.product_uuid {
        f.push(info("dmi_uuid", format!("product_uuid={uuid}")));
        if uuid.chars().all(|c| c == '0' || c == '-') {
            f.push(warn("dmi_uuid_zero", "product_uuid нулевой — шаблон/клон".into()));
        }
    }
    if let Some(ref hv) = x.hypervisor_vendor {
        f.push(info("hv_vendor", format!("Hypervisor vendor: {hv}")));
    }

    if !x.modules_virt.is_empty() {
        f.push(info(
            "modules_virt",
            format!("virt-модули: {}", x.modules_virt.join(", ")),
        ));
    }
    if x.modules_virt.iter().any(|m| m.contains("vz") || m.contains("ploop") || m.contains("ve_")) {
        f.push(crit(
            "openvz_like",
            "Модули OpenVZ/ploop — это контейнерный стек, не классический KVM VDS".into(),
        ));
    }
    for h in &x.container_hints {
        f.push(warn("container_hint", h.clone()));
    }

    if let Some(ref fs) = x.root_fstype {
        let fs_l = fs.to_lowercase();
        if fs_l.contains("nfs") || fs_l.contains("cifs") || fs_l.contains("fuse") || fs_l.contains("gluster") || fs_l.contains("ceph")
        {
            f.push(crit(
                "root_network_fs",
                format!("Корень на {fs} ({}) — не локальный диск", x.root_source.as_deref().unwrap_or("?")),
            ));
        } else if fs_l.contains("tmpfs") || fs_l.contains("overlay") {
            f.push(warn(
                "root_unusual_fs",
                format!("Корень fstype={fs} — необычно для VDS"),
            ));
        } else {
            f.push(info(
                "root_fs_ok",
                format!("root={fs} src={}", x.root_source.as_deref().unwrap_or("?")),
            ));
        }
    }

    if let Some(ref thp) = x.thp_enabled {
        f.push(info("thp", format!("THP={thp}")));
    }
    if let Some(hp) = deep.mem_detail.hugepages_total {
        if hp > 0 {
            f.push(info(
                "hugepages",
                format!(
                    "HugePages_Total={hp} size≈{:?} KB",
                    deep.mem_detail.hugepage_size_kb
                ),
            ));
        } else {
            f.push(info("hugepages_none", "HugePages_Total=0".into()));
        }
    }
    if x.ksm_run == Some(true) {
        f.push(warn(
            "ksm_on",
            "KSM включён — память может схлопываться между VM/процессами".into(),
        ));
    }
    if x.zram_present {
        f.push(warn("zram", "Обнаружен zram — сжатый RAM как swap".into()));
    }
    if x.zswap_enabled == Some(true) {
        f.push(info("zswap", "zswap включён".into()));
    }
    if let Some(ref oc) = x.overcommit_memory {
        if oc == "1" {
            f.push(warn(
                "overcommit_always",
                "vm.overcommit_memory=1 (always) — можно «продать» больше RAM чем есть".into(),
            ));
        } else {
            f.push(info("overcommit", format!("vm.overcommit_memory={oc}")));
        }
    }

    if !x.net_drivers.is_empty() {
        f.push(info(
            "net_drivers",
            format!("NIC: {}", x.net_drivers.join(", ")),
        ));
    }
    for s in &x.net_link_speeds {
        if let Some(mbps) = s
            .split('=')
            .nth(1)
            .and_then(|v| v.replace("Mbps", "").trim().parse::<u64>().ok())
        {
            if mbps > 0 && mbps < 100 {
                f.push(warn(
                    "net_link_slow",
                    format!("Линк {s} — очень узкий канал (<100 Mbps)"),
                ));
            } else if mbps > 0 && mbps < 1000 {
                f.push(info(
                    "net_link",
                    format!("{s} — не гигабит (часто норма для VPS, не fraud)"),
                ));
            } else {
                f.push(info("net_link", s.clone()));
            }
        } else {
            f.push(info("net_link", s.clone()));
        }
    }
    // claim-like: iface speed vs measured download
    if let Some(best) = net.speed.as_ref().and_then(|s| s.best_download_mbps) {
        let max_link = x
            .net_link_speeds
            .iter()
            .filter_map(|s| {
                s.split('=')
                    .nth(1)
                    .and_then(|v| v.replace("Mbps", "").trim().parse::<f64>().ok())
            })
            .fold(0.0_f64, f64::max);
        if max_link >= 100.0 && best > max_link * 1.5 {
            f.push(warn(
                "net_speed_gt_link",
                format!("Download {best:.0} > link {max_link:.0} Mbps — странно (или link неверн)"),
            ));
        }
    }

    if let Some(d) = disk {
        if let Some(iops) = d.rand_read_iops {
            f.push(info(
                "disk_rand_iops",
                format!("Random 4K read ≈ {iops:.0} IOPS"),
            ));
            if iops < 500.0 {
                f.push(warn(
                    "disk_rand_slow",
                    format!("Random 4K {iops:.0} IOPS — слабо / throttle"),
                ));
            } else if iops >= 20000.0 {
                f.push(info(
                    "disk_rand_fast",
                    format!("Random 4K {iops:.0} IOPS — похоже на NVMe/хороший virtio"),
                ));
            }
        }
        if let Some(lat) = d.rand_read_lat_us_p99 {
            f.push(info(
                "disk_rand_lat",
                format!("Random 4K p99 latency ≈ {lat:.0} µs"),
            ));
            if lat > 5000.0 {
                f.push(warn(
                    "disk_rand_lat_high",
                    format!("p99 {lat:.0}µs — высокий latency диска"),
                ));
            }
        }
    }

    if let Some(root) = sys.disks.iter().find(|d| d.mount == "/") {
        let blk: f64 = deep
            .block_devices
            .iter()
            .filter_map(|b| b.size_gb)
            .fold(0.0, f64::max);
        if blk > 1.0 && root.total_gb > 1.0 && root.total_gb > blk * 1.35 {
            f.push(warn(
                "disk_thin_provision",
                format!(
                    "ФС / ≈ {:.0} GB, block max ≈ {blk:.0} GB — thin provision / рост диска",
                    root.total_gb
                ),
            ));
        }
    }

    if let Some(ref g) = deep.governor {
        f.push(info("governor", format!("cpufreq governor={g}")));
        if g == "powersave" {
            f.push(warn(
                "governor_powersave",
                "governor=powersave — частота может быть занижена".into(),
            ));
        }
    }
    if let Some(false) = deep.turbo_enabled {
        f.push(warn("turbo_off", "Turbo/boost выключен в sysfs".into()));
    } else if let Some(true) = deep.turbo_enabled {
        f.push(info("turbo_on", "Turbo/boost включён (sysfs)".into()));
    }

    if let Some(r) = x.load_vs_cores {
        if r > 2.0 && sys.uptime_sec > 120 {
            f.push(warn(
                "load_high_idle",
                format!("load/cores≈{r:.2} — нода шумная даже без нашего теста"),
            ));
        } else {
            f.push(info("load_ok", format!("load/cores≈{r:.2}")));
        }
    }

    let vul_mit = deep
        .vulnerabilities
        .iter()
        .filter(|v| v.status.to_lowercase().contains("mitigation"))
        .count();
    let vul_aff = deep
        .vulnerabilities
        .iter()
        .filter(|v| {
            let s = v.status.to_lowercase();
            s.contains("vulnerable") && !s.contains("not affected")
        })
        .count();
    f.push(info(
        "vulns_summary",
        format!(
            "CPU vulns: {} записей, mitigation≈{vul_mit}, vulnerable≈{vul_aff}",
            deep.vulnerabilities.len()
        ),
    ));

    if let Some(s) = stress {
        if let Some(n) = x.cgroup_nr_throttled {
            if n > 10 && s.scaling_efficiency < 80.0 {
                f.push(crit(
                    "throttle_vs_scaling",
                    format!(
                        "Throttle×{n} и scaling {:.0}% — ядра режут квотой под нагрузкой",
                        s.scaling_efficiency
                    ),
                ));
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        if std::path::Path::new("/dev/kvm").exists() {
            f.push(info(
                "nested_kvm_dev",
                "/dev/kvm есть — nested virtualization возможна".into(),
            ));
        } else if deep.cpu_ids.has_svm_or_vmx && cpu.virtualization.is_virtual {
            f.push(info(
                "nested_no_dev",
                "VT-x/AMD-V в госте есть, /dev/kvm нет — nested не отдали в userspace".into(),
            ));
        }
    }

    let _ = sys;
    f
}

fn info(code: &str, message: String) -> ExtraFinding {
    ExtraFinding {
        severity: "info".into(),
        code: code.into(),
        message,
    }
}
fn warn(code: &str, message: String) -> ExtraFinding {
    ExtraFinding {
        severity: "warn".into(),
        code: code.into(),
        message,
    }
}
fn crit(code: &str, message: String) -> ExtraFinding {
    ExtraFinding {
        severity: "critical".into(),
        code: code.into(),
        message,
    }
}

#[cfg(target_os = "linux")]
fn read_trim(path: &str) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

#[cfg(target_os = "linux")]
fn read_u64(path: &str) -> Option<u64> {
    read_trim(path)?.parse().ok()
}

#[cfg(target_os = "linux")]
fn parse_psi_some(path: &str) -> Option<f64> {
    let s = std::fs::read_to_string(path).ok()?;
    // some avg10=0.00 avg60=0.00 avg300=0.00 total=...
    for line in s.lines() {
        if line.starts_with("some") {
            for part in line.split_whitespace() {
                if let Some(v) = part.strip_prefix("avg10=") {
                    return v.parse().ok();
                }
            }
        }
    }
    None
}

#[cfg(target_os = "linux")]
fn read_cgroup_extras() -> (
    Option<u64>,
    Option<u64>,
    Option<u64>,
    Option<u64>,
    Option<String>,
) {
    use std::fs;
    let mut nr = None;
    let mut usec = None;
    let mut weight = None;
    let mut mem_high = None;
    let mut io_max = None;

    // v2 unified
    if let Ok(stat) = fs::read_to_string("/sys/fs/cgroup/cpu.stat") {
        for line in stat.lines() {
            let mut sp = line.split_whitespace();
            match (sp.next(), sp.next()) {
                (Some("nr_throttled"), Some(v)) => nr = v.parse().ok(),
                (Some("throttled_usec"), Some(v)) => usec = v.parse().ok(),
                _ => {}
            }
        }
    }
    weight = read_u64("/sys/fs/cgroup/cpu.weight");
    if let Some(raw) = read_trim("/sys/fs/cgroup/memory.high") {
        if raw != "max" {
            if let Ok(b) = raw.parse::<u64>() {
                mem_high = Some(b / (1024 * 1024));
            }
        }
    }
    io_max = read_trim("/sys/fs/cgroup/io.max");

    // try nested self path
    if nr.is_none() {
        if let Ok(cg) = fs::read_to_string("/proc/self/cgroup") {
            for line in cg.lines() {
                if let Some(path) = line.split(':').nth(2) {
                    let base = if path == "/" {
                        "/sys/fs/cgroup".to_string()
                    } else {
                        format!("/sys/fs/cgroup{path}")
                    };
                    if let Ok(stat) = fs::read_to_string(format!("{base}/cpu.stat")) {
                        for l in stat.lines() {
                            let mut sp = l.split_whitespace();
                            match (sp.next(), sp.next()) {
                                (Some("nr_throttled"), Some(v)) => nr = v.parse().ok(),
                                (Some("throttled_usec"), Some(v)) => usec = v.parse().ok(),
                                _ => {}
                            }
                        }
                    }
                    if weight.is_none() {
                        weight = read_u64(&format!("{base}/cpu.weight"));
                    }
                    if io_max.is_none() {
                        io_max = read_trim(&format!("{base}/io.max"));
                    }
                    break;
                }
            }
        }
    }

    (nr, usec, weight, mem_high, io_max)
}

#[cfg(target_os = "linux")]
fn count_cpu_list(s: &str) -> Option<usize> {
    let s = s.trim();
    if s.is_empty() || s == "/" {
        return None;
    }
    // strip cpuset path prefix like /0-3
    let s = s.rsplit('/').next().unwrap_or(s);
    let mut n = 0usize;
    for part in s.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if let Some((a, b)) = part.split_once('-') {
            let a: usize = a.parse().ok()?;
            let b: usize = b.parse().ok()?;
            n += b.saturating_sub(a) + 1;
        } else {
            let _: usize = part.parse().ok()?;
            n += 1;
        }
    }
    if n == 0 {
        None
    } else {
        Some(n)
    }
}

#[cfg(target_os = "linux")]
fn read_root_fs() -> (Option<String>, Option<String>) {
    let mounts = std::fs::read_to_string("/proc/mounts").unwrap_or_default();
    for line in mounts.lines() {
        let mut sp = line.split_whitespace();
        let src = sp.next().unwrap_or("");
        let dst = sp.next().unwrap_or("");
        let fstype = sp.next().unwrap_or("");
        if dst == "/" {
            return (Some(fstype.into()), Some(src.into()));
        }
    }
    (None, None)
}

#[cfg(target_os = "linux")]
fn read_virt_modules() -> Vec<String> {
    let mods = std::fs::read_to_string("/proc/modules").unwrap_or_default();
    let watch = [
        "virtio", "kvm", "vhost", "xen", "vz", "ploop", "openvz", "ve_", "hyperv", "vmw_", "hv_",
        "xen_blk", "xen_net", "balloon",
    ];
    let mut out = Vec::new();
    for line in mods.lines() {
        let name = line.split_whitespace().next().unwrap_or("");
        if watch.iter().any(|w| name.contains(w)) {
            out.push(name.to_string());
        }
    }
    out.sort();
    out.dedup();
    out
}

#[cfg(target_os = "linux")]
fn read_net_drivers() -> (Vec<String>, Vec<String>) {
    use std::fs;
    use std::path::Path;
    let mut drivers = Vec::new();
    let mut speeds = Vec::new();
    let Ok(rd) = fs::read_dir("/sys/class/net") else {
        return (drivers, speeds);
    };
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if name == "lo" {
            continue;
        }
        let drv_path = e.path().join("device/driver");
        if let Ok(link) = fs::read_link(&drv_path) {
            let d = link
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| link.display().to_string());
            drivers.push(format!("{name}:{d}"));
        } else if Path::new("/sys/devices/virtual/net").join(&name).exists() {
            drivers.push(format!("{name}:virtual"));
        }
        if let Ok(sp) = fs::read_to_string(e.path().join("speed")) {
            let sp = sp.trim();
            if sp != "-1" && !sp.is_empty() {
                speeds.push(format!("{name}={sp}Mbps"));
            }
        }
    }
    (drivers, speeds)
}

#[cfg(target_os = "linux")]
fn measure_sleep_jitter() -> (Option<f64>, Option<f64>) {
    let n = 80usize;
    let mut samples = Vec::with_capacity(n);
    for _ in 0..n {
        let t0 = Instant::now();
        std::thread::sleep(Duration::from_millis(1));
        let us = t0.elapsed().as_secs_f64() * 1_000_000.0;
        samples.push(us);
    }
    if samples.is_empty() {
        return (None, None);
    }
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let avg = samples.iter().sum::<f64>() / samples.len() as f64;
    let idx = ((samples.len() as f64) * 0.99) as usize;
    let p99 = samples[idx.min(samples.len() - 1)];
    (Some(avg), Some(p99))
}

#[cfg(target_os = "linux")]
fn parse_lscpu_hypervisor() -> Option<String> {
    let out = std::process::Command::new("lscpu").output().ok()?;
    let s = String::from_utf8_lossy(&out.stdout);
    for line in s.lines() {
        if line.to_lowercase().starts_with("hypervisor vendor") {
            return line.split(':').nth(1).map(|x| x.trim().to_string());
        }
    }
    None
}

#[cfg(target_os = "linux")]
fn detect_container_hints() -> Vec<String> {
    use std::fs;
    use std::path::Path;
    let mut h = Vec::new();
    if Path::new("/.dockerenv").exists() {
        h.push("Найден /.dockerenv — Docker-контейнер".into());
    }
    if let Ok(cg) = fs::read_to_string("/proc/1/cgroup") {
        if cg.contains("docker") || cg.contains("kubepods") || cg.contains("containerd") {
            h.push("PID1 cgroup похож на контейнер (docker/k8s)".into());
        }
    }
    if let Ok(comm) = fs::read_to_string("/proc/1/comm") {
        let c = comm.trim();
        if c != "systemd" && c != "init" && !c.is_empty() {
            h.push(format!("PID1={c} (не systemd) — возможен контейнер"));
        }
    }
    if Path::new("/proc/vz").exists() {
        h.push("/proc/vz — OpenVZ/Virtuozzo".into());
    }
    h
}
