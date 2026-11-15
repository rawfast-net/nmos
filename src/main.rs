mod antifraud;
mod catalog;
mod cgroup;
mod cpu;
mod cpureality;
mod diskio;
mod dbbench;
mod dns;
mod extras;
mod memcheck;
mod mail;
mod monitor;
mod network;
mod oversell;
mod probe;
mod report;
mod reputation;
mod share;
mod speedtest;
mod stress;
mod system;
mod tech;
mod truth;
mod ui;

use antifraud::ProviderClaims;
use clap::{Parser, Subcommand};
use report::FullReport;

#[derive(Parser, Debug)]
#[command(
    name = "nmos",
    about = "nmos (NoMoreOverSell) — VDS: claimed vs measured",
    version
)]
struct Args {
    #[command(subcommand)]
    command: Option<NmosCommand>,
    #[arg(long, help = "Do not offer to prepare a public result")]
    no_share_prompt: bool,
    #[arg(long, help = "Record passive CPU counters as JSON lines")]
    monitor: bool,

    #[arg(long, default_value_t = 96, value_parser = clap::value_parser!(u32).range(1..=100000))]
    monitor_count: u32,

    #[arg(long, default_value_t = 900, value_parser = clap::value_parser!(u64).range(1..=86400))]
    monitor_interval: u64,
    #[arg(long, help = "Run SQLite storage tests")]
    db_bench: bool,

    #[arg(long, default_value = ".", help = "Directory for the temporary SQLite database")]
    db_dir: std::path::PathBuf,

    #[arg(long, default_value_t = 500, value_parser = clap::value_parser!(u64).range(1..=4096))]
    db_mb: u64,

    #[arg(long, default_value_t = 5000, value_parser = clap::value_parser!(u64).range(1..=1000000))]
    db_operations: u64,
    
    #[arg(short, long, default_value_t = 4.0)]
    duration: f64,

    #[arg(long)]
    skip_stress: bool,

    #[arg(long)]
    skip_speed: bool,

    #[arg(long, conflicts_with = "no_speedtest")]
    speedtest: bool,

    #[arg(long)]
    no_speedtest: bool,

    #[arg(long, default_value = "ookla", value_parser = ["ookla", "iperf3"])]
    speedtest_backend: String,

    #[arg(long)]
    server_id: Option<u64>,

    #[arg(long)]
    country: Option<String>,

    #[arg(long)]
    city: Option<String>,

    #[arg(long)]
    accept_speedtest_terms: bool,

    #[arg(long)]
    speedtest_repeat: bool,

    #[arg(long)]
    skip_disk: bool,

    #[arg(long)]
    skip_mem: bool,

    
    
    #[arg(long, default_value_t = 256)]
    disk_mb: u64,

    
    #[arg(long, default_value_t = 512)]
    mem_mb: u64,

    
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

    
    #[arg(long)]
    offline: bool,

    #[arg(long)]
    no_public_ip: bool,

    #[arg(long, help = "Skip public IP DNS blocklist checks")]
    skip_reputation: bool,

    #[arg(long, help = "Skip repeated TCP latency and loss checks")]
    skip_quality: bool,

    #[arg(long, help = "Skip outbound SMTP connectivity checks")]
    skip_mail: bool,

}

#[derive(Debug, Subcommand)]
enum NmosCommand {
    Setup {
        #[arg(long)]
        speedtest: bool,
    },
}

fn main() {
    let args = Args::parse();
    if let Some(NmosCommand::Setup { speedtest }) = &args.command {
        if !speedtest {
            eprintln!("Укажите nmos setup --speedtest");
            std::process::exit(2);
        }
        if let Err(error) = speedtest::setup() {
            eprintln!("Подготовка Speedtest: {error}");
            std::process::exit(1);
        }
        return;
    }
    if args.monitor {
        if let Err(e) = monitor::run(args.monitor_count, args.monitor_interval) {
            eprintln!("Ошибка мониторинга: {e}");
            std::process::exit(1);
        }
        return;
    }
    if !args.json { speedtest::offer_setup(); }
    let duration = args.duration.clamp(0.5, 60.0);
    let disk_mb = args.disk_mb.clamp(64, 1024);
    let mem_mb = args.mem_mb.clamp(64, 4096);
    let do_speed = !args.skip_speed && !args.no_speedtest && !args.offline;
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

    let total = 13u32;

    step(1, total, "Система");
    let sys_info = system::collect();

    step(2, total, "Процессор");
    let cpu_info = cpu::collect();

    step(3, total, "Инструкции CPU");
    let tech = tech::collect();

    step(4, total, "DMI, cgroup, CPUID");
    let deep = probe::collect();

    step(5, total, "Дополнительные показатели");
    let extras = extras::collect(&sys_info, &cpu_info);

    step(6, total, "Сверка модели CPU");
    let cpu_reality = cpureality::analyze(&cpu_info, &deep, &tech);

    let mem = if do_mem {
        step(7, total, &format!("Проверка памяти, {mem_mb} MiB"));
        Some(memcheck::run(
            mem_mb,
            sys_info.ram_total_mb,
            sys_info.swap_total_mb,
        ))
    } else {
        step(7, total, "Память пропущена");
        None
    };

    step(8, total, "Сеть");
    let net_info = network::collect(network::NetOptions {
        fetch_public_ip: do_public,
        run_speed: do_speed,
        region,
        run_quality: !args.offline && !args.skip_quality,
        offline: args.offline,
        speedtest_backend: args.speedtest_backend,
        speedtest_server_id: args.server_id,
        speedtest_country: args.country.clone(),
        speedtest_city: args.city,
        speedtest_manual_country: args.country.is_some() || (args.region.len() == 2 && args.region != "auto"),
        speedtest_accept_terms: args.accept_speedtest_terms,
        speedtest_repeat: args.speedtest_repeat,
    });
    step(9, total, "Репутация IP");
    let reputation = if args.offline || args.skip_reputation {
        None
    } else {
        net_info.public_ip.as_deref().and_then(reputation::check)
    };

    step(10, total, "Исходящая почта");
    let mail_egress = if args.offline || args.skip_mail { None } else { Some(mail::check()) };

    let disk = if do_disk {
        step(11, total, &format!("Диск, {disk_mb} MiB"));
        Some(diskio::run(disk_mb))
    } else {
        step(11, total, "Диск пропущен");
        None
    };

    let stress = if args.skip_stress {
        step(12, total, "Нагрузка CPU пропущена");
        None
    } else {
        let n = cpu_info.logical_cores.max(1);
        step(12, total, &format!("Нагрузка CPU, 3 повтора по {duration:.1} с, {n} потоков"));
        Some(stress::run(duration, n))
    };

    step(13, total, "Анализ результатов");
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

    let db_bench = if args.db_bench {
        eprintln!("SQLite: тест в {} ({} MiB)", args.db_dir.display(), args.db_mb);
        Some(match dbbench::run(&args.db_dir, args.db_mb, args.db_operations as usize) {
            Ok(result) => report::DbBenchOutcome { result: Some(result), error: None },
            Err(error) => report::DbBenchOutcome { result: None, error: Some(error) },
        })
    } else { None };

    let report = FullReport {
        schema_version: 3,
        nmos_version: env!("CARGO_PKG_VERSION"),
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
        reputation,
        mail_egress,
        diskio: disk,
        stress,
        oversell,
        estimate,
        truth,
        antifraud,
        db_bench,
    };

    ui::finish();
    if args.json {
        println!("{}", serde_json::to_string_pretty(&report).unwrap());
    } else {
        report::print_human(&report);
    }
    if report.db_bench.as_ref().is_some_and(|outcome| outcome.error.is_some()) {
        std::process::exit(1);
    }
    if !args.json && !args.offline && !args.no_share_prompt {
        if let Err(e) = share::offer(&report) { eprintln!("Публикация отменена: {e}"); }
    }
}


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
    ui::step(n, total, msg);
}
