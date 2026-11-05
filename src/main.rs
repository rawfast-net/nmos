mod antifraud;
mod catalog;
mod cpu;
mod cpureality;
mod diskio;
mod extras;
mod memcheck;
mod network;
mod oversell;
mod probe;
mod report;
mod stress;
mod system;
mod tech;
mod truth;

use antifraud::ProviderClaims;
use clap::Parser;
use report::FullReport;

#[derive(Parser, Debug)]
#[command(
    name = "nmos",
    about = "nmos (NoMoreOverSell) — VDS: claimed vs measured",
    version
)]
struct Args {
    /// Stress test duration per phase, seconds
    #[arg(short, long, default_value_t = 4.0)]
    duration: f64,

    #[arg(long)]
    skip_stress: bool,

    #[arg(long)]
    skip_speed: bool,

    #[arg(long)]
    skip_disk: bool,

    #[arg(long)]
    skip_mem: bool,

    /// Download test size, MB (10–500)
    #[arg(long, default_value_t = 100)]
    speed_mb: u64,

    /// Disk I/O test size, MB
    #[arg(long, default_value_t = 256)]
    disk_mb: u64,

    /// RAM test size, MB (capped by available)
    #[arg(long, default_value_t = 512)]
    mem_mb: u64,

    /// Speed/ping region: auto | ru | intl | all
    #[arg(long, default_value = "auto")]
    region: String,

    #[arg(long)]
    claim_cores: Option<usize>,

    #[arg(long)]
    claim_ram_gb: Option<f64>,

    #[arg(long)]
    claim_disk_gb: Option<f64>,

    #[arg(long)]
    claim_cpu: Option<String>,

    #[arg(long)]
    claim_net_mbps: Option<f64>,

    #[arg(long)]
    json: bool,

    /// No outbound network
    #[arg(long)]
    offline: bool,

    #[arg(long)]
    no_public_ip: bool,
}

fn main() {
    let args = Args::parse();
    let duration = args.duration.clamp(0.5, 60.0);
    let speed_mb = args.speed_mb.clamp(10, 500);
    let disk_mb = args.disk_mb.clamp(64, 1024);
    let mem_mb = args.mem_mb.clamp(64, 4096);
    let do_speed = !args.skip_speed && !args.offline;
    let do_public = !args.no_public_ip && !args.offline;
    let do_disk = !args.skip_disk;
    let do_mem = !args.skip_mem;
    let region = args.region.to_lowercase();

    let claims = ProviderClaims {
        cores: args.claim_cores,
        ram_gb: args.claim_ram_gb,
        disk_gb: args.claim_disk_gb,
        cpu_model: args.claim_cpu,
        net_mbps: args.claim_net_mbps,
    };

    if !args.json {
        eprintln!("nmos — NoMoreOverSell");
        eprintln!("region={}", region);
        if claims.any() {
            eprintln!(
                "claims: cores={:?} ram={:?}GB disk={:?}GB cpu={:?} net={:?}Mbps",
                claims.cores, claims.ram_gb, claims.disk_gb, claims.cpu_model, claims.net_mbps
            );
        } else {
            eprintln!("hint: pass --claim-* to compare against the plan");
        }
    }

    let total = 11u32;

    step(1, total, "system");
    let sys_info = system::collect();

    step(2, total, "cpu");
    let cpu_info = cpu::collect();

    step(3, total, "cpu flags / tech");
    let tech = tech::collect();

    step(4, total, "dmi / cgroup / cpuid");
    let deep = probe::collect();

    step(5, total, "extras (psi, timers, …)");
    let extras = extras::collect(&sys_info, &cpu_info);

    step(6, total, "cpuid ↔ model");
    let cpu_reality = cpureality::analyze(&cpu_info, &deep, &tech);

    let mem = if do_mem {
        step(7, total, &format!("ram check ~{mem_mb} MB"));
        Some(memcheck::run(
            mem_mb,
            sys_info.ram_total_mb,
            sys_info.swap_total_mb,
        ))
    } else {
        step(7, total, "ram check skipped");
        None
    };

    step(8, total, "network");
    let net_info = network::collect(network::NetOptions {
        fetch_public_ip: do_public,
        run_speed: do_speed,
        speed_size_mb: speed_mb,
        region,
    });
    if !args.json {
        eprintln!("  resolved region: {}", net_info.resolved_region);
    }

    let disk = if do_disk {
        step(9, total, &format!("disk io ~{disk_mb} MB"));
        Some(diskio::run(disk_mb))
    } else {
        step(9, total, "disk skipped");
        None
    };

    let stress = if args.skip_stress {
        step(10, total, "stress skipped");
        None
    } else {
        let n = cpu_info.logical_cores.max(1);
        step(10, total, &format!("cpu stress {duration:.1}s × 2, {n} threads"));
        Some(stress::run(duration, n))
    };

    step(11, total, "verdict");
    let extras = refresh_load_metrics(extras, stress.as_ref());
    let oversell = oversell::analyze(&cpu_info, stress.as_ref());
    let estimate = oversell::estimate_real_cpu(&cpu_info, stress.as_ref(), &oversell);
    let truth = truth::analyze(
        &cpu_info,
        &deep,
        &sys_info,
        &net_info,
        stress.as_ref(),
        disk.as_ref(),
        &tech,
        Some(&extras),
    );
    let antifraud = antifraud::build(
        &claims,
        &cpu_info,
        &sys_info,
        &deep,
        &tech,
        &net_info,
        stress.as_ref(),
        disk.as_ref(),
        mem.as_ref(),
        &truth,
        &cpu_reality,
        Some(&extras),
    );

    let report = FullReport {
        timestamp: chrono::Local::now().to_rfc3339(),
        claims,
        system: sys_info,
        cpu: cpu_info,
        tech,
        cpu_reality,
        deep,
        extras,
        memcheck: mem,
        network: net_info,
        diskio: disk,
        stress,
        oversell,
        estimate,
        truth,
        antifraud,
    };

    if args.json {
        println!("{}", serde_json::to_string_pretty(&report).unwrap());
    } else {
        report::print_human(&report);
    }
}

// PSI / cgroup throttle after stress — pre-load numbers are useless
fn refresh_load_metrics(
    extras: extras::ExtraProbe,
    _stress: Option<&stress::StressResult>,
) -> extras::ExtraProbe {
    #[cfg(not(target_os = "linux"))]
    {
        let _ = _stress;
        return extras;
    }
    #[cfg(target_os = "linux")]
    {
        let mut extras = extras;
        if let Some(p) = std::fs::read_to_string("/proc/pressure/cpu").ok().and_then(|t| {
            t.lines().find(|l| l.starts_with("some")).and_then(|l| {
                l.split_whitespace()
                    .find_map(|p| p.strip_prefix("avg10=")?.parse().ok())
            })
        }) {
            extras.cpu_pressure_some_avg10 = Some(p);
        }
        if let Ok(stat) = std::fs::read_to_string("/sys/fs/cgroup/cpu.stat") {
            for line in stat.lines() {
                let mut sp = line.split_whitespace();
                match (sp.next(), sp.next()) {
                    (Some("nr_throttled"), Some(v)) => {
                        extras.cgroup_nr_throttled = v.parse().ok();
                    }
                    (Some("throttled_usec"), Some(v)) => {
                        extras.cgroup_throttled_usec = v.parse().ok();
                    }
                    _ => {}
                }
            }
        }
        let _ = _stress;
        extras
    }
}

fn step(n: u32, total: u32, msg: &str) {
    eprintln!("[{n}/{total}] {msg}");
}
