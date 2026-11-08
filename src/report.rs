use crate::antifraud::{AntifraudReport, ProviderClaims};
use crate::cpu::CpuInfo;
use crate::cpureality::CpuRealityReport;
use crate::diskio::DiskIoResult;
use crate::extras::ExtraProbe;
use crate::memcheck::MemCheckResult;
use crate::network::NetworkInfo;
use crate::oversell::{CpuEstimate, OversellAnalysis};
use crate::probe::DeepProbe;
use crate::stress::StressResult;
use crate::system::SystemInfo;
use crate::tech::TechReport;
use crate::truth::TruthReport;
use colored::*;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct DbBenchOutcome {
    pub result: Option<crate::dbbench::DbReport>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct FullReport {
    pub schema_version: u32,
    pub nmos_version: &'static str,
    pub db_bench: Option<DbBenchOutcome>,
    pub timestamp: String,
    pub claims: ProviderClaims,
    pub system: SystemInfo,
    pub cpu: CpuInfo,
    pub tech: TechReport,
    pub cpu_reality: CpuRealityReport,
    pub deep: DeepProbe,
    pub extras: ExtraProbe,
    pub memcheck: Option<MemCheckResult>,
    pub network: NetworkInfo,
    pub diskio: Option<DiskIoResult>,
    pub stress: Option<StressResult>,
    pub oversell: OversellAnalysis,
    #[allow(dead_code)]
    #[serde(skip_serializing)]
    pub estimate: CpuEstimate,
    #[allow(dead_code)]
    #[serde(skip_serializing)]
    pub truth: TruthReport,
    #[allow(dead_code)]
    #[serde(skip_serializing)]
    pub antifraud: AntifraudReport,
}

pub fn print_human(r: &FullReport) {
    banner();
    kv("time", &r.timestamp);
    section("Результат");
    kv("CPU", &r.cpu.model);
    kv("vCPU", &r.cpu.logical_cores.to_string());
    kv("Память", &format!("{} MB", r.system.ram_total_mb));
    kv("CPU под нагрузкой", &r.oversell.oversell_level);
    if let Some(s) = &r.stress {
        if let Some(d) = &s.cgroup_multi {
            kv("cgroup CPU", &format!("периодов: {}, ограничено: {}, throttled: {} мкс", d.nr_periods, d.nr_throttled, d.throttled_usec));
        } else { kv("cgroup CPU", "нет данных за нагрузку"); }
    }
    if let Some(v) = r.claims.cores { kv("Тариф vCPU", &v.to_string()); }
    if let Some(v) = r.claims.ram_gb { kv("Тариф RAM", &format!("{v} GB")); }
    if let Some(v) = &r.claims.cpu_model { kv("Тариф CPU", v); }
    if let Some(v) = r.claims.disk_gb { kv("Тариф диск", &format!("{v} GB")); }
    if let Some(v) = r.claims.net_mbps { kv("Тариф сеть", &format!("{v} Mbps")); }
    println!("  Значения тарифа и измерения приведены отдельно.");

    if let Some(outcome) = &r.db_bench {
        section("SQLite");
        if let Some(error) = &outcome.error { kv("Ошибка", error); }
        if let Some(db) = &outcome.result {
            kv("Каталог", &db.directory);
            kv("Размер базы", &format!("{:.1} MiB", db.database_bytes as f64 / 1048576.0));
            for (name, p) in [("1 поток", &db.single_writer), ("4 потока", &db.four_writers), ("Чтение", &db.random_read)] {
                kv(name, &format!("{:.1} операций/с; p50/p95/p99 {:.2}/{:.2}/{:.2} мс; n={}", p.operations_per_second, p.latency_ms_p50, p.latency_ms_p95, p.latency_ms_p99, p.completed));
                if let Some(steal) = p.steal_percent { kv("  steal", &format!("{steal:.2}%")); }
            }
            for note in &db.notes { println!("  • {note}"); }
        }
    }

    section("system");
    kv("Хост", &r.system.hostname);
    kv("ОС", &r.system.os);
    kv("Ядро", &r.system.kernel);
    kv("Arch", &r.system.arch);
    kv("Uptime", &r.system.uptime_human);
    if let Some(l) = r.system.load_avg {
        kv(
            "Load avg",
            &format!("{:.2} / {:.2} / {:.2}  (1/5/15 мин)", l[0], l[1], l[2]),
        );
    }
    kv("Процессы", &r.system.processes.to_string());
    println!();

    section("memory / disk");
    let ram = format!(
        "{} / {} MB  ({:.0}%)",
        r.system.ram_used_mb, r.system.ram_total_mb, r.system.ram_used_percent
    );
    kv("RAM", &ram);
    bar(r.system.ram_used_percent, ram_color(r.system.ram_used_percent));
    if r.system.swap_total_mb > 0 {
        let sp = if r.system.swap_total_mb > 0 {
            (r.system.swap_used_mb as f64 / r.system.swap_total_mb as f64) * 100.0
        } else {
            0.0
        };
        kv(
            "Swap",
            &format!("{} / {} MB  ({:.0}%)", r.system.swap_used_mb, r.system.swap_total_mb, sp),
        );
        bar(sp, ram_color(sp));
    } else {
        kv("Swap", "нет");
    }
    for d in &r.system.disks {
        kv(
            &format!("Disk {}", d.mount),
            &format!(
                "{:.1} / {:.1} GB free  ({:.0}% used, {})",
                d.available_gb,
                d.total_gb,
                d.used_percent,
                d.fs
            ),
        );
        bar(d.used_percent, ram_color(d.used_percent));
    }
    println!();

    section("cpu");
    kv("Модель", &r.cpu.model);
    kv("Vendor", &r.cpu.vendor);
    kv(
        "Ядра",
        &format!("{} физ. / {} лог.", r.cpu.physical_cores, r.cpu.logical_cores),
    );
    if let Some(n) = r.cpu.numa_nodes {
        kv("NUMA", &n.to_string());
    }
    print_freq(&r.cpu);
    if let Some(b) = r.cpu.bogomips {
        kv("BogoMIPS", &format!("{b:.1}"));
    }
    print_cache(&r.cpu);
    if !r.cpu.flags_sample.is_empty() {
        kv("Флаги", &r.cpu.flags_sample.join(", "));
    }
    println!();

    print_tech(&r.tech);
    print_cpu_reality(&r.cpu_reality);

    if let Some(m) = &r.memcheck {
        section("ram check");
        kv("Тест", &format!("{} MB", m.tested_mb));
        let alloc_s = if m.alloc_ok {
            "OK".green().to_string()
        } else {
            "FAIL".red().bold().to_string()
        };
        kv("Alloc", &alloc_s);
        if let Some(ok) = m.verify_ok {
            let ver = if ok {
                "OK — паттерн цел".green().to_string()
            } else {
                "FAIL — ошибка проверки памяти".red().bold().to_string()
            };
            kv("Verify", &ver);
        }
        if let (Some(w), Some(rd)) = (m.write_gbps, m.read_gbps) {
            kv("Bandwidth", &format!("W {w:.1} / R {rd:.1} GB/s"));
        }
        if let Some(b) = &m.balloon_hint {
            println!("  {} {}", "!".yellow(), b);
        }
        if let Some(s) = &m.swap_abuse_hint {
            println!("  {} {}", "!".yellow(), s);
        }
        for n in &m.notes {
            println!("  {} {}", "•".dimmed(), n);
        }
        println!();
    }

    section("virt");
    let virt_label = if r.cpu.virtualization.is_virtual {
        format!(
            "{} ({})",
            r.cpu.virtualization.virt_type,
            "guest".yellow().bold()
        )
    } else {
        format!(
            "{} ({})",
            r.cpu.virtualization.virt_type,
            "bare-metal?".green()
        )
    };
    kv("Тип", &virt_label);
    if let Some(h) = &r.cpu.virtualization.hypervisor {
        kv("Гипервизор", h);
    }
    for e in &r.cpu.virtualization.evidence {
        println!("  {} {}", "•".dimmed(), e.dimmed());
    }
    if let Some(s) = r.cpu.steal_percent {
        let colored = if s > 10.0 {
            format!("{s:.2}%").red().bold().to_string()
        } else if s > 2.0 {
            format!("{s:.2}%").yellow().to_string()
        } else {
            format!("{s:.2}%").green().to_string()
        };
        kv("CPU steal", &colored);
        bar(
            s.min(100.0),
            if s > 10.0 {
                "red"
            } else if s > 2.0 {
                "yellow"
            } else {
                "green"
            },
        );
    }
    println!();

    print_deep(&r.deep);
    print_extras(&r.extras);

    section("network");
    kv("Hostname", &r.network.hostname);
    if let Some(ip) = &r.network.public_ip {
        kv("Внешний IP", &ip.bright_cyan().to_string());
    }
    if let Some(g) = &r.network.geo_hint {
        kv("Гео / ISP", g);
    }
    kv("Регион теста", &r.network.resolved_region);
    if let Some(d) = &r.network.default_route_guess {
        kv("Основной IF", d);
    }
    if let Some(ms) = r.network.dns_latency_ms {
        kv("DNS", &format!("{ms:.1} мс"));
    }

    if !r.network.latency_targets.is_empty() {
        println!("  {}", "Задержки:".dimmed());
        for t in &r.network.latency_targets {
            let val = match t.latency_ms {
                Some(ms) => latency_color(ms),
                None => "timeout".red().to_string(),
            };
            println!("    {:<12} {:>10}  {}", t.name, val, t.host.dimmed());
        }
    }

    for iface in &r.network.interfaces {
        let v4 = if iface.ipv4.is_empty() {
            "—".into()
        } else {
            iface.ipv4.join(", ")
        };
        let mut extra = String::new();
        if let Some(sp) = iface.speed_mbps {
            extra.push_str(&format!("  link={sp}Mbps"));
        }
        if let Some(m) = iface.mtu {
            extra.push_str(&format!("  mtu={m}"));
        }
        println!(
            "  {} {}  {}{}",
            format!("[{}]", iface.name).bright_white(),
            v4,
            iface.mac.as_deref().unwrap_or(""),
            extra.dimmed()
        );
        if !iface.ipv6.is_empty() {
            println!("         IPv6: {}", iface.ipv6.join(", ").dimmed());
        }
    }

    if let Some(sp) = &r.network.speed {
        println!();
        println!("  {}", "Скорость интернета:".bold());
        for s in &sp.download_mbps {
            let bar_pct = (s.mbps / 1000.0 * 100.0).clamp(0.0, 100.0);
            println!(
                "    {}  {:>8.1} Mbps",
                format!("↓ {}", s.label).green(),
                s.mbps
            );
            bar(bar_pct, "cyan");
        }
        if let Some(u) = &sp.upload_mbps {
            println!(
                "    {}  {:>8.1} Mbps",
                format!("↑ {}", u.label).magenta(),
                u.mbps
            );
            bar((u.mbps / 1000.0 * 100.0).clamp(0.0, 100.0), "magenta");
        }
        if let (Some(best), Some(avg)) = (sp.best_download_mbps, sp.avg_download_mbps) {
            kv(
                "Итого ↓",
                &format!("best {:.0} Mbps · avg {:.0} Mbps", best, avg),
            );
        }
        for n in &sp.notes {
            println!("  {} {}", "!".yellow(), n.dimmed());
        }
    }

    for n in &r.network.notes {
        println!("  {} {}", "!".yellow(), n.dimmed());
    }
    println!();

    if let Some(io) = &r.diskio {
        section("disk io");
        kv("Файл теста", &io.path);
        kv("Размер", &format!("{} MB", io.size_mb));
        if let Some(w) = io.write_mbps {
            kv("Write seq", &format!("{w:.0} MB/s"));
            bar((w / 2000.0 * 100.0).clamp(0.0, 100.0), "cyan");
        }
        if let Some(rd) = io.read_mbps {
            kv("Read seq", &format!("{rd:.0} MB/s"));
            bar((rd / 2000.0 * 100.0).clamp(0.0, 100.0), "cyan");
        }
        if let Some(lat) = io.fsync_latency_us_avg {
            kv("fsync avg", &format!("{lat:.0} µs (n={})", io.fsync_samples));
        }
        if let Some(iops) = io.rand_read_iops {
            let p99 = io
                .rand_read_lat_us_p99
                .map(|v| format!(" · p99 {v:.0} µs"))
                .unwrap_or_default();
            kv("Random 4K", &format!("{iops:.0} IOPS{p99}"));
        }
        if let Some(ok) = io.verify_ok {
            let cs = if ok {
                "OK".green().bold().to_string()
            } else {
                "FAIL".red().bold().to_string()
            };
            kv("Checksum", &cs);
        }
        if io.looks_like_ramdisk {
            println!(
                "  {} {}",
                "✗".red().bold(),
                "looks like ramdisk/tmpfs — don't trust the disk size".red()
            );
        }
        for n in &io.notes {
            println!("  {} {}", "!".yellow(), n.dimmed());
        }
        println!();
    }

    if let Some(s) = &r.stress {
        section("cpu stress");
        kv("Длительность", &format!("{:.1} с / фаза", s.duration_sec));
        kv("Потоки", &s.threads.to_string());
        kv(
            "Single-thread",
            &format!("{:.0} ops/s   score {}", s.single_ops_per_sec, format!("{:.1}", s.score_single).bright_white()),
        );
        kv(
            "Multi-thread",
            &format!("{:.0} ops/s   score {}", s.multi_ops_per_sec, format!("{:.1}", s.score_multi).bright_white()),
        );
        let sc = if s.scaling_efficiency >= 80.0 {
            format!("{:.1}%", s.scaling_efficiency).green().to_string()
        } else if s.scaling_efficiency >= 55.0 {
            format!("{:.1}%", s.scaling_efficiency).yellow().to_string()
        } else {
            format!("{:.1}%", s.scaling_efficiency).red().bold().to_string()
        };
        kv("Scaling", &format!("{sc} от идеального ST×N"));
        bar(s.scaling_efficiency.min(100.0), if s.scaling_efficiency >= 80.0 { "green" } else if s.scaling_efficiency >= 55.0 { "yellow" } else { "red" });
        if s.freq_idle_mhz.is_some() || s.freq_load_mhz.is_some() {
            kv(
                "Freq idle→load",
                &format!(
                    "{:.0} → {:.0} МГц  (Δ {:+.0})",
                    s.freq_idle_mhz.unwrap_or(0.0),
                    s.freq_load_mhz.unwrap_or(0.0),
                    s.freq_delta_mhz.unwrap_or(0.0)
                ),
            );
        }
        println!();
    }

    section("CPU: признаки ограничений");
    kv("Статус", &r.oversell.oversell_level);
    kv("Данные", &r.oversell.confidence);
    for reason in &r.oversell.reasons {
        println!("  {} {}", "•".dimmed(), reason);
    }
    println!();

    println!();
}

fn banner() {
    println!();
    println!("{}", "nmos — NoMoreOverSell".bright_cyan().bold());
    println!("{}", "claimed vs measured on this VDS".dimmed());
    println!();
}

fn section(title: &str) {
    println!();
    println!("== {} ==", title.bold());
}

fn kv(key: &str, val: &str) {
    println!("  {}  {}", pad(key), val);
}

fn pad(key: &str) -> ColoredString {
    format!("{:<14}", key).dimmed()
}

fn bar(pct: f64, color: &str) {
    let width = 28usize;
    let filled = ((pct / 100.0) * width as f64).round() as usize;
    let filled = filled.min(width);
    let empty = width - filled;
    let body = format!("{}{}", "█".repeat(filled), "░".repeat(empty));
    let colored = match color {
        "red" => body.red().to_string(),
        "yellow" => body.yellow().to_string(),
        "green" => body.green().to_string(),
        "magenta" => body.magenta().to_string(),
        "cyan" => body.cyan().to_string(),
        _ => body,
    };
    println!("                [{}] {:5.1}%", colored, pct);
}

fn ram_color(pct: f64) -> &'static str {
    if pct >= 90.0 {
        "red"
    } else if pct >= 70.0 {
        "yellow"
    } else {
        "green"
    }
}

fn latency_color(ms: f64) -> String {
    let s = format!("{ms:.1} мс");
    if ms < 40.0 {
        s.green().to_string()
    } else if ms < 100.0 {
        s.yellow().to_string()
    } else {
        s.red().to_string()
    }
}

fn print_freq(cpu: &CpuInfo) {
    let cur = cpu
        .freq_mhz_current_avg
        .map(|v| format!("{v:.0}"))
        .unwrap_or_else(|| "—".into());
    let min = cpu.freq_mhz_min.map(|v| v.to_string()).unwrap_or_else(|| "—".into());
    let max = cpu.freq_mhz_max.map(|v| v.to_string()).unwrap_or_else(|| "—".into());
    kv(
        "Частота",
        &format!("≈{cur} МГц  (min {min} / max {max})"),
    );
    let fa = &cpu.freq_analysis;
    kv(
        "Freq check",
        &format!(
            "cpufreq={}  sticky={}  round_suspect={}  bogomips/2≈{}",
            if fa.cpufreq_available { "yes" } else { "no" },
            if fa.sticky_min_eq_max { "yes" } else { "no" },
            if fa.round_ghz_suspect { "YES" } else { "no" },
            fa.bogomips_implied_mhz
                .map(|v| format!("{v:.0}"))
                .unwrap_or_else(|| "—".into())
        ),
    );
    for n in &fa.notes {
        println!("  {} {}", "•".yellow(), n);
    }
}

fn print_cache(cpu: &CpuInfo) {
    let fmt = |v: Option<u64>| {
        v.map(|x| {
            if x >= 1024 {
                format!("{:.1} MB", x as f64 / 1024.0)
            } else {
                format!("{x} KB")
            }
        })
        .unwrap_or_else(|| "—".into())
    };
    if cpu.cache_l1_kb.is_some() || cpu.cache_l2_kb.is_some() || cpu.cache_l3_kb.is_some() {
        kv(
            "Кэш",
            &format!(
                "L1={}  L2={}  L3={}",
                fmt(cpu.cache_l1_kb),
                fmt(cpu.cache_l2_kb),
                fmt(cpu.cache_l3_kb)
            ),
        );
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let t: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{t}…")
    }
}

fn print_deep(d: &crate::probe::DeepProbe) {
    section("linux probe");

    if let Some(v) = &d.dmi.sys_vendor {
        kv("DMI vendor", v);
    }
    if let Some(p) = &d.dmi.product_name {
        kv("DMI product", p);
    }
    if let Some(b) = &d.dmi.bios_vendor {
        kv(
            "BIOS",
            &format!(
                "{} {}",
                b,
                d.dmi.bios_version.as_deref().unwrap_or("")
            ),
        );
    }

    let ids = &d.cpu_ids;
    if ids.family.is_some() || ids.model.is_some() {
        kv(
            "CPUID",
            &format!(
                "family={} model={} stepping={} ucode={}",
                ids.family.map(|x| x.to_string()).unwrap_or_else(|| "?".into()),
                ids.model.map(|x| x.to_string()).unwrap_or_else(|| "?".into()),
                ids.stepping.map(|x| x.to_string()).unwrap_or_else(|| "?".into()),
                ids.microcode.as_deref().unwrap_or("?")
            ),
        );
    }
    if let Some(mhz) = ids.cpu_mhz_cpuinfo {
        kv("cpuinfo MHz", &format!("{mhz:.0}"));
    }
    kv(
        "ISA",
        &format!(
            "AVX={} AVX2={} AVX512={} AES={} SHA={} HV={}",
            yn(ids.has_avx),
            yn(ids.has_avx2),
            yn(ids.has_avx512),
            yn(ids.has_aes),
            yn(ids.has_sha),
            yn(ids.has_hypervisor_flag)
        ),
    );
    if !ids.bugs.is_empty() {
        kv("CPU bugs", &ids.bugs.join(", "));
    }

    let t = &d.topology;
    kv(
        "Topology",
        &format!(
            "sockets={} cores/sock≈{} thr/core≈{} online={}",
            t.sockets.map(|x| x.to_string()).unwrap_or_else(|| "?".into()),
            t.cores_per_socket
                .map(|x| x.to_string())
                .unwrap_or_else(|| "?".into()),
            t.threads_per_core
                .map(|x| x.to_string())
                .unwrap_or_else(|| "?".into()),
            t.online_cpus.as_deref().unwrap_or("?")
        ),
    );
    if let Some(g) = &d.governor {
        kv(
            "Governor",
            &format!(
                "{}  turbo={}",
                g,
                d.turbo_enabled
                    .map(|x| if x { "on" } else { "off" })
                    .unwrap_or("?")
            ),
        );
    }

    let cg = &d.cgroup;
    if let Some(v) = &cg.version {
        let mut s = format!("cgroup {v}");
        if let Some(q) = cg.cpu_quota_cores {
            s.push_str(&format!(" · CPU quota≈{q:.2}"));
        }
        if let Some(m) = cg.memory_max_mb {
            s.push_str(&format!(" · mem.max={m}MB"));
        }
        if let Some(c) = &cg.cpuset_cpus {
            s.push_str(&format!(" · cpuset={c}"));
        }
        kv("Cgroup", &s);
    }
    for n in &cg.notes {
        println!("  {} {}", "•".dimmed(), n.dimmed());
    }

    if let Some(cmd) = &d.cmdline {
        kv("cmdline", &truncate(cmd, 70));
    }

    if !d.virtio_devices.is_empty() {
        kv(
            "virtio",
            &truncate(&d.virtio_devices.join(", "), 70),
        );
    }
    if !d.block_devices.is_empty() {
        println!("  {}", "Block devices:".dimmed());
        for b in d.block_devices.iter().take(8) {
            let kind = match b.rota {
                Some(true) => "HDD",
                Some(false) if b.name.starts_with("vd") || b.name.starts_with("xvd") => "virtio/SSD?",
                Some(false) => "SSD/NVMe?",
                None => "?",
            };
            println!(
                "    {}  {:.0} GB  {}  {}  {}",
                b.name.bright_white(),
                b.size_gb.unwrap_or(0.0),
                kind,
                b.model.as_deref().unwrap_or(""),
                b.sched.as_deref().unwrap_or("")
            );
        }
    }

    if !d.vulnerabilities.is_empty() {
        println!("  {}", "Spectre/Meltdown и др.:".dimmed());
        for v in d.vulnerabilities.iter().take(10) {
            let st = if v.status.contains("Vulnerable") {
                v.status.red().to_string()
            } else if v.status.contains("Mitigation") {
                v.status.yellow().to_string()
            } else {
                v.status.green().to_string()
            };
            println!("    {:<18} {}", v.name, st);
        }
    }

    let md = &d.mem_detail;
    if md.mem_available_mb.is_some() || md.committed_as_mb.is_some() {
        kv(
            "MemDetail",
            &format!(
                "avail={}MB dirty={}KB commit={}/{}MB hugepages={}",
                md.mem_available_mb.unwrap_or(0),
                md.dirty_kb.unwrap_or(0),
                md.committed_as_mb.unwrap_or(0),
                md.commit_limit_mb.unwrap_or(0),
                md.hugepages_total.unwrap_or(0)
            ),
        );
    }

    if let Some(l) = &d.lscpu_summary {
        println!("  {} {}", "lscpu".dimmed(), truncate(l, 90).dimmed());
    }
    for e in &d.evidence_extra {
        println!("  {} {}", "•".dimmed(), e.dimmed());
    }
    println!();
}

fn print_extras(x: &crate::extras::ExtraProbe) {
    section("extras");
    if let Some(cs) = &x.clocksource {
        kv("Clocksource", cs);
    }
    if let (Some(a), Some(p)) = (x.sleep_jitter_avg_us, x.sleep_jitter_p99_us) {
        kv("Timer jitter", &format!("avg {a:.0} · p99 {p:.0} µs"));
    }
    if let Some(e) = x.entropy_avail {
        kv("Entropy", &format!("{e}"));
    }
    if let Some(p) = x.cpu_pressure_some_avg10 {
        kv("PSI CPU", &format!("avg10 {p:.1}%"));
    }
    if let Some(p) = x.io_pressure_some_avg10 {
        kv("PSI IO", &format!("avg10 {p:.1}%"));
    }
    if let Some(p) = x.mem_pressure_some_avg10 {
        kv("PSI Mem", &format!("avg10 {p:.1}%"));
    }
    if let Some(n) = x.cgroup_nr_throttled {
        kv(
            "CPU throttle",
            &format!(
                "nr={n} usec={}",
                x.cgroup_throttled_usec.unwrap_or(0)
            ),
        );
    }
    if let Some(w) = x.cgroup_cpu_weight {
        kv("cpu.weight", &w.to_string());
    }
    if let Some(fs) = &x.root_fstype {
        kv(
            "Root FS",
            &format!("{fs} ({})", x.root_source.as_deref().unwrap_or("?")),
        );
    }
    if let Some(thp) = &x.thp_enabled {
        kv("THP", thp);
    }
    if let Some(oc) = &x.overcommit_memory {
        kv("overcommit", oc);
    }
    if x.zram_present {
        kv("zram", "yes");
    }
    if x.ksm_run == Some(true) {
        kv("KSM", "on");
    }
    if !x.modules_virt.is_empty() {
        kv("Virt modules", &truncate(&x.modules_virt.join(", "), 70));
    }
    if !x.net_drivers.is_empty() {
        kv("NIC drivers", &truncate(&x.net_drivers.join(", "), 70));
    }
    if !x.net_link_speeds.is_empty() {
        kv("Link speed", &x.net_link_speeds.join(", "));
    }
    if let Some(hv) = &x.hypervisor_vendor {
        kv("HV vendor", hv);
    }
    if let (Some(on), Some(pr)) = (x.online_cpu_count, x.present_cpu_count) {
        kv("CPU online/present", &format!("{on}/{pr}"));
    }
    if let Some(c) = x.cpuset_count {
        kv("cpuset CPUs", &c.to_string());
    }
    for h in &x.container_hints {
        println!("  {} {}", "!".yellow(), h.yellow());
    }
    for n in &x.notes {
        println!("  {} {}", "•".dimmed(), n.dimmed());
    }
    println!();
}

fn yn(v: bool) -> &'static str {
    if v {
        "yes"
    } else {
        "no"
    }
}

fn print_cpu_reality(r: &CpuRealityReport) {
    section("cpuid vs model");
    let cons = match r.consistency.as_str() {
        "ok" => "ok".green().bold().to_string(),
        "warn" => "warn".yellow().bold().to_string(),
        _ => "fail".red().bold().to_string(),
    };
    kv("consistency", &cons);
    kv("confidence", &r.confidence);
    if let Some(u) = &r.decoded_uarch {
        kv("uarch", &u.bright_white().to_string());
    }
    kv("from", &r.decoded_from);
    if let Some(c) = &r.catalog_match {
        kv(
            "catalog",
            &format!("{} ({})", c, r.catalog_gen.as_deref().unwrap_or("?")),
        );
    }
    for t in &r.expected_tech {
        let mark = if t.present {
            "+".green().to_string()
        } else if t.required {
            "x".red().bold().to_string()
        } else {
            "?".yellow().to_string()
        };
        let req = if t.required { "must" } else { "should" };
        println!("    {mark} [{req}] {}", t.title);
    }
    for m in &r.mismatches {
        println!("  {} {}", "x".red().bold(), m.red());
    }
    for n in &r.notes {
        println!("  - {}", n.dimmed());
    }
    println!();
}

fn print_tech(t: &crate::tech::TechReport) {
    section("cpu tech");
    if !t.available {
        for n in &t.notes {
            println!("  ! {}", n);
        }
        println!();
        return;
    }
    kv("flags", &t.total_flags.to_string());
    kv("isa", &t.isa_level.bright_white().to_string());
    kv("gen", &t.generation_hint);
    if !t.present_highlights.is_empty() {
        kv("have", &t.present_highlights.join(", ").green().to_string());
    }
    if !t.missing_important.is_empty() {
        kv("missing", &t.missing_important.join(", ").red().to_string());
    }

    for cat in &t.categories {
        println!(
            "  {} [{}]",
            cat.name.bold(),
            cat.score.dimmed()
        );
        let mut col = 0;
        let mut line = String::new();
        for item in &cat.items {
            let token = if item.present {
                format!("+{:<10}", item.title).green().to_string()
            } else {
                format!("-{:<10}", item.title).dimmed().to_string()
            };
            if col == 0 {
                line = format!("    {token}");
            } else {
                line.push(' ');
                line.push_str(&token);
            }
            col += 1;
            if col == 4 {
                println!("{line}");
                line.clear();
                col = 0;
            }
        }
        if col > 0 {
            println!("{line}");
        }
    }

    if !t.raw_interesting.is_empty() {
        kv("other", &t.raw_interesting.join(" "));
    }
    for n in &t.notes {
        println!("  - {}", n.dimmed());
    }
    println!();
}
