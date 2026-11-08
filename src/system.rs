use serde::Serialize;
use sysinfo::{Disks, System};

#[derive(Debug, Clone, Serialize)]
pub struct SystemInfo {
    pub hostname: String,
    pub os: String,
    pub kernel: String,
    pub arch: String,
    pub uptime_sec: u64,
    pub uptime_human: String,
    pub load_avg: Option<[f64; 3]>,
    pub ram_total_mb: u64,
    pub ram_used_mb: u64,
    pub ram_available_mb: u64,
    pub ram_used_percent: f64,
    pub swap_total_mb: u64,
    pub swap_used_mb: u64,
    pub disks: Vec<DiskInfo>,
    pub processes: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiskInfo {
    pub mount: String,
    pub fs: String,
    pub total_gb: f64,
    pub available_gb: f64,
    pub used_percent: f64,
}

pub fn collect() -> SystemInfo {
    let mut sys = System::new_all();
    sys.refresh_all();

    let ram_total = sys.total_memory() / 1024 / 1024;
    let ram_used = sys.used_memory() / 1024 / 1024;
    let ram_avail = sys.available_memory() / 1024 / 1024;
    let ram_pct = if ram_total > 0 {
        (ram_used as f64 / ram_total as f64) * 100.0
    } else {
        0.0
    };

    let swap_total = sys.total_swap() / 1024 / 1024;
    let swap_used = sys.used_swap() / 1024 / 1024;

    let uptime = System::uptime();
    let load = read_loadavg();

    let disks_sys = Disks::new_with_refreshed_list();
    let mut disks = Vec::new();
    for d in disks_sys.list() {
        let mount = d.mount_point().to_string_lossy().to_string();
        
        let total = d.total_space();
        if total < 512 * 1024 * 1024 {
            continue;
        }
        #[cfg(target_os = "linux")]
        {
            if mount.starts_with("/snap")
                || mount.starts_with("/boot/efi")
                || mount.starts_with("/run")
                || mount.starts_with("/sys")
                || mount.starts_with("/dev")
            {
                continue;
            }
        }
        let avail = d.available_space();
        let used_pct = if total > 0 {
            ((total - avail) as f64 / total as f64) * 100.0
        } else {
            0.0
        };
        disks.push(DiskInfo {
            mount,
            fs: d.file_system().to_string_lossy().to_string(),
            total_gb: total as f64 / 1e9,
            available_gb: avail as f64 / 1e9,
            used_percent: used_pct,
        });
    }
    disks.sort_by(|a, b| a.mount.cmp(&b.mount));

    SystemInfo {
        hostname: System::host_name().unwrap_or_else(|| "unknown".into()),
        os: format!(
            "{} {}",
            System::name().unwrap_or_default(),
            System::os_version().unwrap_or_default()
        ),
        kernel: System::kernel_version().unwrap_or_else(|| "unknown".into()),
        arch: std::env::consts::ARCH.to_string(),
        uptime_sec: uptime,
        uptime_human: format_uptime(uptime),
        load_avg: load,
        ram_total_mb: ram_total,
        ram_used_mb: ram_used,
        ram_available_mb: ram_avail,
        ram_used_percent: ram_pct,
        swap_total_mb: swap_total,
        swap_used_mb: swap_used,
        disks,
        processes: sys.processes().len(),
    }
}

fn format_uptime(sec: u64) -> String {
    let days = sec / 86400;
    let hours = (sec % 86400) / 3600;
    let mins = (sec % 3600) / 60;
    if days > 0 {
        format!("{days}д {hours}ч {mins}м")
    } else if hours > 0 {
        format!("{hours}ч {mins}м")
    } else {
        format!("{mins}м")
    }
}

fn read_loadavg() -> Option<[f64; 3]> {
    #[cfg(target_os = "linux")]
    {
        let s = std::fs::read_to_string("/proc/loadavg").ok()?;
        let mut parts = s.split_whitespace();
        let a: f64 = parts.next()?.parse().ok()?;
        let b: f64 = parts.next()?.parse().ok()?;
        let c: f64 = parts.next()?.parse().ok()?;
        return Some([a, b, c]);
    }
    #[cfg(not(target_os = "linux"))]
    {
        let l = System::load_average();
        if l.one == 0.0 && l.five == 0.0 && l.fifteen == 0.0 {
            None
        } else {
            Some([l.one, l.five, l.fifteen])
        }
    }
}
