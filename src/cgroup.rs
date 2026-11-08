use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, Serialize, Default)]
pub struct CpuCounters { pub nr_periods: u64, pub nr_throttled: u64, pub throttled_usec: u64 }

#[derive(Debug, Clone, Serialize)]
pub struct CpuDelta { pub nr_periods: u64, pub nr_throttled: u64, pub throttled_usec: u64 }

pub fn delta(a: &CpuCounters, b: &CpuCounters) -> Option<CpuDelta> {
    Some(CpuDelta { nr_periods: b.nr_periods.checked_sub(a.nr_periods)?, nr_throttled: b.nr_throttled.checked_sub(a.nr_throttled)?, throttled_usec: b.throttled_usec.checked_sub(a.throttled_usec)? })
}

fn parse_stat(text: &str) -> Option<CpuCounters> {
    let mut periods = None; let mut throttled = None; let mut usec = None;
    for line in text.lines() {
        let mut parts = line.split_whitespace();
        match (parts.next(), parts.next()) {
            (Some("nr_periods"), Some(v)) => periods = v.parse().ok(),
            (Some("nr_throttled"), Some(v)) => throttled = v.parse().ok(),
            (Some("throttled_usec"), Some(v)) => usec = v.parse().ok(),
            _ => {}
        }
    }
    Some(CpuCounters { nr_periods: periods?, nr_throttled: throttled?, throttled_usec: usec? })
}

fn unescape(s: &str) -> String { s.replace("\\040", " ").replace("\\011", "\t").replace("\\012", "\n").replace("\\134", "\\") }

fn resolve(cgroups: &str, mounts: &str) -> Option<PathBuf> {
    let group = cgroups.lines().find_map(|line| line.strip_prefix("0::"))?;
    let group = Path::new(group);
    for line in mounts.lines() {
        let (head, tail) = line.split_once(" - ")?;
        if tail.split_whitespace().next() != Some("cgroup2") { continue; }
        let parts: Vec<_> = head.split_whitespace().collect();
        if parts.len() < 5 { continue; }
        let root = unescape(parts[3]);
        let mount = unescape(parts[4]);
        if let Ok(relative) = group.strip_prefix(Path::new(&root)) {
            if relative.components().any(|c| matches!(c, std::path::Component::ParentDir)) { return None; }
            return Some(Path::new(&mount).join(relative).join("cpu.stat"));
        }
    }
    None
}

pub fn sample() -> Option<CpuCounters> {
    #[cfg(target_os="linux")]
    {
        let group = std::fs::read_to_string("/proc/self/cgroup").ok()?;
        let mount = std::fs::read_to_string("/proc/self/mountinfo").ok()?;
        let path = resolve(&group, &mount)?;
        parse_stat(&std::fs::read_to_string(path).ok()?)
    }
    #[cfg(not(target_os="linux"))]
    { None }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resolves_nested_cgroup_and_mount_root() {
        let mount = "27 22 0:25 /tenant /sys/fs/cgroup rw - cgroup2 cgroup rw";
        assert_eq!(resolve("0::/tenant/work", mount), Some(PathBuf::from("/sys/fs/cgroup/work/cpu.stat")));
        assert!(resolve("0::/other", mount).is_none());
    }
    #[test]
    fn counters_use_deltas_and_reject_resets() {
        let a = parse_stat("nr_periods 10\nnr_throttled 4\nthrottled_usec 100\n").unwrap();
        let b = parse_stat("nr_periods 15\nnr_throttled 6\nthrottled_usec 250\n").unwrap();
        assert_eq!(delta(&a, &b).unwrap().throttled_usec, 150);
        assert!(delta(&b, &a).is_none());
        assert!(parse_stat("nr_periods 10\n").is_none());
    }
}
