use crate::cpu;
use rayon::prelude::*;
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize)]
pub struct StressResult {
    pub duration_sec: f64,
    pub threads: usize,
    pub single_ops: u64,
    pub multi_ops: u64,
    pub single_ops_per_sec: f64,
    pub multi_ops_per_sec: f64,
    pub scaling_efficiency: f64,
    pub score_single: f64,
    pub score_multi: f64,
    pub latency_ns_estimate: f64,
    /// Частота до/во время/после нагрузки (МГц)
    pub freq_idle_mhz: Option<f64>,
    pub freq_load_mhz: Option<f64>,
    pub freq_after_mhz: Option<f64>,
    pub freq_delta_mhz: Option<f64>,
    pub freq_load_samples: Vec<f64>,
}

/// Интенсивный целочисленный + FP workload (детерминированный).
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
    let duration = Duration::from_secs_f64(duration_sec);
    let freq_idle = cpu::sample_current_mhz();

    // Single-thread + sampling частоты
    let stop = Arc::new(AtomicBool::new(false));
    let samples = Arc::new(std::sync::Mutex::new(Vec::new()));
    let stop_c = Arc::clone(&stop);
    let handle = std::thread::spawn(move || run_worker(stop_c));
    let stop_s = Arc::clone(&stop);
    let samples_c = Arc::clone(&samples);
    let sampler = std::thread::spawn(move || sample_freq_while(stop_s, samples_c));
    std::thread::sleep(duration);
    stop.store(true, Ordering::Relaxed);
    let single_ops = handle.join().unwrap_or(0);
    let _ = sampler.join();
    let single_ops_per_sec = single_ops as f64 / duration_sec;

    let load_samples: Vec<f64> = samples.lock().map(|v| v.clone()).unwrap_or_default();
    let freq_load = if load_samples.is_empty() {
        None
    } else {
        Some(load_samples.iter().sum::<f64>() / load_samples.len() as f64)
    };

    // Multi-thread
    let stop = Arc::new(AtomicBool::new(false));
    let stop_flag = Arc::clone(&stop);
    let multi_handle = std::thread::spawn(move || {
        (0..threads)
            .into_par_iter()
            .map(|_| run_worker(Arc::clone(&stop_flag)))
            .sum::<u64>()
    });
    std::thread::sleep(duration);
    stop.store(true, Ordering::Relaxed);
    let multi_ops = multi_handle.join().unwrap_or(0);
    let multi_ops_per_sec = multi_ops as f64 / duration_sec;

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
        duration_sec,
        threads,
        single_ops,
        multi_ops,
        single_ops_per_sec,
        multi_ops_per_sec,
        scaling_efficiency,
        score_single,
        score_multi,
        latency_ns_estimate: latency_ns,
        freq_idle_mhz: freq_idle,
        freq_load_mhz: freq_load,
        freq_after_mhz: freq_after,
        freq_delta_mhz: freq_delta,
        freq_load_samples: load_samples,
    }
}

/// Короткий калибровочный замер: ops за фиксированное время vs ожидаемое по МГц.
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
