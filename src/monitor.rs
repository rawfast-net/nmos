use serde::Serialize;
use std::time::{Duration, Instant};

#[derive(Serialize)]
struct Sample {
    kind: &'static str,
    timestamp: String,
    elapsed_seconds: f64,
    steal_percent: Option<f64>,
    cgroup: Option<crate::cgroup::CpuDelta>,
}

#[derive(Serialize)]
struct Summary {
    kind: &'static str,
    samples: u32,
    steal_available: usize,
    steal_over_3_percent: usize,
    cgroup_available: u32,
    cgroup_throttled_intervals: u32,
}

pub fn run(count: u32, interval: u64) -> Result<(), String> {
    use std::io::{self, Write};
    let mut out = io::BufWriter::new(io::stdout().lock());
    let mut previous_steal = crate::stress::steal_sample();
    let mut previous_cgroup = crate::cgroup::sample();
    let mut steal_available = 0;
    let mut steal_over_3_percent = 0;
    let mut cgroup_available = 0;
    let mut cgroup_throttled_intervals = 0;
    for _ in 0..count {
        let start = Instant::now();
        std::thread::sleep(Duration::from_secs(interval));
        let elapsed = start.elapsed().as_secs_f64();
        let steal = crate::stress::steal_sample();
        let cgroup = crate::cgroup::sample();
        let steal_percent = previous_steal.zip(steal).and_then(|((a,at),(b,bt))| {
            let total = bt.checked_sub(at)?;
            if total == 0 { return None; }
            Some(b.checked_sub(a)? as f64 * 100.0 / total as f64)
        });
        let cgroup_delta = previous_cgroup.zip(cgroup).and_then(|(a,b)| crate::cgroup::delta(&a,&b));
        if let Some(value) = steal_percent { steal_available += 1; if value > 3.0 { steal_over_3_percent += 1; } }
        if let Some(ref value) = cgroup_delta { cgroup_available += 1; if value.nr_throttled > 0 { cgroup_throttled_intervals += 1; } }
        let sample = Sample { kind: "sample", timestamp: chrono::Local::now().to_rfc3339(), elapsed_seconds: elapsed, steal_percent, cgroup: cgroup_delta };
        serde_json::to_writer(&mut out, &sample).map_err(|e| e.to_string())?;
        writeln!(&mut out).and_then(|_| out.flush()).map_err(|e| e.to_string())?;
        previous_steal = steal;
        previous_cgroup = cgroup;
    }
    let summary = Summary { kind: "summary", samples: count, steal_available, steal_over_3_percent, cgroup_available, cgroup_throttled_intervals };
    serde_json::to_writer(&mut out, &summary).map_err(|e| e.to_string())?;
    writeln!(&mut out).and_then(|_| out.flush()).map_err(|e| e.to_string())?;
    Ok(())
}
