use crate::cpu;
use rayon::prelude::*;
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize)]
pub struct StressResult {
    pub benchmark_version: &'static str,
    pub duration_sec: f64,
    pub steal_percent_load: Option<f64>,
    pub cgroup_single: Option<crate::cgroup::CpuDelta>,
    pub cgroup_multi: Option<crate::cgroup::CpuDelta>,
    pub threads: usize,
    pub single_ops: u64,
    pub multi_ops: u64,
    pub single_ops_per_sec: f64,
    pub multi_ops_per_sec: f64,
    pub scaling_efficiency: f64,
    pub score_single: f64,
    pub score_multi: f64,
    pub latency_ns_estimate: f64,
    pub repetitions: usize,
    pub single_samples_ops_per_sec: Vec<f64>,
    pub multi_samples_ops_per_sec: Vec<f64>,
    pub single_variation_percent: f64,
    pub multi_variation_percent: f64,
    
    pub freq_idle_mhz: Option<f64>,
    pub freq_load_mhz: Option<f64>,
    pub freq_after_mhz: Option<f64>,
    pub freq_delta_mhz: Option<f64>,
    pub freq_load_samples: Vec<f64>,
}


#[inline(never)]
fn burn_iteration(state: u64) -> u64 {
    let mut acc = state;
    let mut f = state as f64 * 0.5 + 1.0;
    for i in 0..256u64 {
        acc = acc
            .wrapping_mul(6364136223846793005)
            .wrapping_add(i.wrapping_mul(1442695040888963407));
        acc ^= acc >> 17;
        acc = acc.wrapping_mul(0x9E3779B97F4A7C15);
        f = f.sin().cos().mul_add(1.0000001, (acc as f64) * 1e-18);
        acc = acc.wrapping_add(f.to_bits());
    }
    acc ^ f.to_bits()
}

fn run_worker(stop: Arc<AtomicBool>) -> u64 {
    let mut state = 0x1234_5678_9ABC_DEF0u64;
    let mut ops = 0u64;
    while !stop.load(Ordering::Relaxed) {
        state = burn_iteration(state);
        ops += 1;
        std::hint::black_box(state);
    }
    ops.wrapping_add(state & 1)
}

fn sample_freq_while(stop: Arc<AtomicBool>, out: Arc<std::sync::Mutex<Vec<f64>>>) {
    while !stop.load(Ordering::Relaxed) {
        if let Some(mhz) = cpu::sample_current_mhz() {
            if let Ok(mut v) = out.lock() {
                v.push(mhz);
            }
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

pub fn run(duration_sec: f64, threads: usize) -> StressResult {
    let samples: Vec<_> = (0..3).map(|_| run_once(duration_sec, threads)).collect();
    let single_rates: Vec<f64> = samples.iter().map(|s| s.single_ops_per_sec).collect();
    let multi_rates: Vec<f64> = samples.iter().map(|s| s.multi_ops_per_sec).collect();
    let single = median(&single_rates);
    let multi = median(&multi_rates);
    let mut result = samples[1].clone();
    result.duration_sec = samples.iter().map(|s| s.duration_sec).sum();
    result.single_ops = samples.iter().map(|s| s.single_ops).sum();
    result.multi_ops = samples.iter().map(|s| s.multi_ops).sum();
    result.single_ops_per_sec = single;
    result.multi_ops_per_sec = multi;
    result.score_single = single / 1_000.0;
    result.score_multi = multi / 1_000.0;
    result.latency_ns_estimate = if single > 0.0 { 1e9 / single } else { 0.0 };
    result.scaling_efficiency = if single > 0.0 {
        multi / (single * threads.max(1) as f64) * 100.0
    } else { 0.0 };
    result.steal_percent_load = {
        let values: Vec<_> = samples.iter().filter_map(|s| s.steal_percent_load).collect();
        if values.is_empty() { None } else { Some(median(&values)) }
    };
    result.cgroup_single = sum_deltas(samples.iter().filter_map(|s| s.cgroup_single.as_ref()));
    result.cgroup_multi = sum_deltas(samples.iter().filter_map(|s| s.cgroup_multi.as_ref()));
    result.freq_load_samples = samples.iter().flat_map(|s| s.freq_load_samples.iter().copied()).collect();
    result.freq_load_mhz = if result.freq_load_samples.is_empty() { None } else {
        Some(result.freq_load_samples.iter().sum::<f64>() / result.freq_load_samples.len() as f64)
    };
    result.freq_delta_mhz = result.freq_idle_mhz.zip(result.freq_load_mhz).map(|(idle, load)| load - idle);
    result.repetitions = samples.len();
    result.single_samples_ops_per_sec = single_rates.clone();
    result.multi_samples_ops_per_sec = multi_rates.clone();
    result.single_variation_percent = variation(&single_rates);
    result.multi_variation_percent = variation(&multi_rates);
    result
}

fn median(values: &[f64]) -> f64 {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    sorted[sorted.len() / 2]
}

fn variation(values: &[f64]) -> f64 {
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    if mean <= 0.0 { return 0.0; }
    let variance = values.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / values.len() as f64;
    variance.sqrt() / mean * 100.0
}

fn sum_deltas<'a>(values: impl Iterator<Item = &'a crate::cgroup::CpuDelta>) -> Option<crate::cgroup::CpuDelta> {
    let mut count = 0;
    let mut result = crate::cgroup::CpuDelta { nr_periods: 0, nr_throttled: 0, throttled_usec: 0 };
    for value in values {
        count += 1;
        result.nr_periods += value.nr_periods;
        result.nr_throttled += value.nr_throttled;
        result.throttled_usec += value.throttled_usec;
    }
    if count == 0 { None } else { Some(result) }
}

fn run_once(duration_sec: f64, threads: usize) -> StressResult {
    let duration = Duration::from_secs_f64(duration_sec);
    let freq_idle = cpu::sample_current_mhz();
    let cgroup_single_before = crate::cgroup::sample();

    
    let stop = Arc::new(AtomicBool::new(false));
    let samples = Arc::new(std::sync::Mutex::new(Vec::new()));
    let stop_c = Arc::clone(&stop);
    let handle = std::thread::spawn(move || run_worker(stop_c));
    let stop_s = Arc::clone(&stop);
    let samples_c = Arc::clone(&samples);
    let sampler = std::thread::spawn(move || sample_freq_while(stop_s, samples_c));
    let single_started = Instant::now();
    std::thread::sleep(duration);
    stop.store(true, Ordering::Relaxed);
    let single_ops = handle.join().unwrap_or(0);
    let single_elapsed = single_started.elapsed().as_secs_f64();
    let cgroup_single_after = crate::cgroup::sample();
    let _ = sampler.join();
    let single_ops_per_sec = single_ops as f64 / single_elapsed;

    let load_samples: Vec<f64> = samples.lock().map(|v| v.clone()).unwrap_or_default();
    let freq_load = if load_samples.is_empty() {
        None
    } else {
        Some(load_samples.iter().sum::<f64>() / load_samples.len() as f64)
    };

    
    let before = steal_sample();
    let cgroup_multi_before = crate::cgroup::sample();
    
    let stop = Arc::new(AtomicBool::new(false));
    let stop_flag = Arc::clone(&stop);
    let multi_handle = std::thread::spawn(move || {
        (0..threads)
            .into_par_iter()
            .map(|_| run_worker(Arc::clone(&stop_flag)))
            .sum::<u64>()
    });
    let multi_started = Instant::now();
    std::thread::sleep(duration);
    stop.store(true, Ordering::Relaxed);
    let multi_ops = multi_handle.join().unwrap_or(0);
    let multi_elapsed = multi_started.elapsed().as_secs_f64();
    let multi_ops_per_sec = multi_ops as f64 / multi_elapsed;

    let after = steal_sample();
    let cgroup_multi_after = crate::cgroup::sample();
    let steal_percent_load = before.zip(after).and_then(|((a, at), (b, bt))| {
        let total = bt.checked_sub(at)?;
        if total == 0 { return None; }
        Some(b.checked_sub(a)? as f64 * 100.0 / total as f64)
    });
    let freq_after = cpu::sample_current_mhz();
    let freq_delta = match (freq_idle, freq_load) {
        (Some(a), Some(b)) => Some(b - a),
        _ => None,
    };

    let ideal = single_ops_per_sec * threads as f64;
    let scaling_efficiency = if ideal > 0.0 {
        (multi_ops_per_sec / ideal) * 100.0
    } else {
        0.0
    };

    let score_single = single_ops_per_sec / 1_000.0;
    let score_multi = multi_ops_per_sec / 1_000.0;
    let latency_ns = if single_ops_per_sec > 0.0 {
        1e9 / single_ops_per_sec
    } else {
        0.0
    };

    StressResult {
        benchmark_version: "nmos-cpu-2",
        duration_sec: single_elapsed + multi_elapsed,
        steal_percent_load,
        cgroup_single: cgroup_single_before.zip(cgroup_single_after).and_then(|(a,b)| crate::cgroup::delta(&a, &b)),
        cgroup_multi: cgroup_multi_before.zip(cgroup_multi_after).and_then(|(a,b)| crate::cgroup::delta(&a, &b)),
        threads,
        single_ops,
        multi_ops,
        single_ops_per_sec,
        multi_ops_per_sec,
        scaling_efficiency,
        score_single,
        score_multi,
        latency_ns_estimate: latency_ns,
        repetitions: 1,
        single_samples_ops_per_sec: vec![single_ops_per_sec],
        multi_samples_ops_per_sec: vec![multi_ops_per_sec],
        single_variation_percent: 0.0,
        multi_variation_percent: 0.0,
        freq_idle_mhz: freq_idle,
        freq_load_mhz: freq_load,
        freq_after_mhz: freq_after,
        freq_delta_mhz: freq_delta,
        freq_load_samples: load_samples,
    }
}

pub(crate) fn steal_sample() -> Option<(u64, u64)> {
    #[cfg(target_os = "linux")]
    {
        let raw = std::fs::read_to_string("/proc/stat").ok()?;
        let values: Vec<u64> = raw.lines().next()?.split_whitespace().skip(1)
            .map(str::parse).collect::<Result<_, _>>().ok()?;
        if values.len() < 8 { return None; }
        return Some((values[7], values.iter().take(8).sum()));
    }
    #[cfg(not(target_os = "linux"))]
    None
}


#[allow(dead_code)]
pub fn calibrate_brief() -> (f64, Option<f64>) {
    let t0 = Instant::now();
    let stop = Arc::new(AtomicBool::new(false));
    let s = Arc::clone(&stop);
    let h = std::thread::spawn(move || run_worker(s));
    std::thread::sleep(Duration::from_millis(800));
    stop.store(true, Ordering::Relaxed);
    let ops = h.join().unwrap_or(0) as f64;
    let secs = t0.elapsed().as_secs_f64().max(1e-6);
    (ops / secs / 1000.0, cpu::sample_current_mhz())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn median_and_variation_reflect_repeated_runs() {
        let samples = [100.0, 90.0, 110.0];
        assert_eq!(median(&samples), 100.0);
        assert!((variation(&samples) - 8.1649658).abs() < 0.0001);
        assert_eq!(variation(&[50.0, 50.0, 50.0]), 0.0);
    }
}
