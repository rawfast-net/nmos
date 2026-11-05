
use serde::Serialize;
use std::time::Instant;

#[derive(Debug, Clone, Serialize)]
pub struct MemCheckResult {
    pub tested_mb: u64,
    pub alloc_ok: bool,
    pub verify_ok: Option<bool>,
    pub write_gbps: Option<f64>,
    pub read_gbps: Option<f64>,
    pub balloon_hint: Option<String>,
    pub swap_abuse_hint: Option<String>,
    pub notes: Vec<String>,
}

pub fn run(target_mb: u64, ram_total_mb: u64, swap_total_mb: u64) -> MemCheckResult {
    let mut notes = Vec::new();

    // Не валим OOM: берём min(target, 60% RAM, 2048), минимум 128
    let safe_cap = ((ram_total_mb as f64) * 0.55) as u64;
    let tested_mb = target_mb
        .max(128)
        .min(safe_cap.max(128))
        .min(2048)
        .max(64);

    if tested_mb + 256 > ram_total_mb && ram_total_mb > 0 {
        notes.push(format!(
            "Тест урезан до {tested_mb} MB (всего RAM {ram_total_mb} MB)"
        ));
    }

    let balloon_hint = detect_balloon();
    let swap_abuse_hint = if swap_total_mb > ram_total_mb.saturating_mul(2) && ram_total_mb > 0 {
        Some(format!(
            "Swap {swap_total_mb} MB ≫ RAM {ram_total_mb} MB — иногда маскируют нехватку памяти"
        ))
    } else {
        None
    };

    let bytes = (tested_mb as usize).saturating_mul(1024 * 1024);
    let mut buf = match try_alloc(bytes) {
        Some(b) => b,
        None => {
            notes.push(format!(
                "Не удалось выделить {tested_mb} MB — RAM меньше паспорта или сильный balloon/cgroup"
            ));
            return MemCheckResult {
                tested_mb,
                alloc_ok: false,
                verify_ok: None,
                write_gbps: None,
                read_gbps: None,
                balloon_hint,
                swap_abuse_hint,
                notes,
            };
        }
    };

    // WRITE pattern
    let t0 = Instant::now();
    fill_pattern(&mut buf);
    let write_secs = t0.elapsed().as_secs_f64().max(1e-6);
    let write_gbps = (bytes as f64 / write_secs) / 1e9;

    // VERIFY
    let t1 = Instant::now();
    let ok = verify_pattern(&buf);
    let read_secs = t1.elapsed().as_secs_f64().max(1e-6);
    let read_gbps = (bytes as f64 / read_secs) / 1e9;

    if !ok {
        notes.push("Паттерн в RAM повреждён после записи — критично (баг/переподписка памяти)".into());
    } else {
        notes.push(format!(
            "Выделено и проверено {tested_mb} MB OK · write {write_gbps:.1} GB/s · read {read_gbps:.1} GB/s"
        ));
    }

    // Очень низкая «RAM bandwidth» может значить что пошли в swap
    if write_gbps < 1.5 && tested_mb >= 256 {
        notes.push(
            "Скорость записи в «RAM» подозрительно низкая — возможно ушли в swap/balloon".into(),
        );
    }

    // drop buffer
    drop(buf);

    MemCheckResult {
        tested_mb,
        alloc_ok: true,
        verify_ok: Some(ok),
        write_gbps: Some(write_gbps),
        read_gbps: Some(read_gbps),
        balloon_hint,
        swap_abuse_hint,
        notes,
    }
}

fn try_alloc(bytes: usize) -> Option<Vec<u8>> {
    let mut v = Vec::new();
    if v.try_reserve_exact(bytes).is_err() {
        return None;
    }
    v.resize(bytes, 0);
    Some(v)
}

fn fill_pattern(buf: &mut [u8]) {
    // 64-bit паттерн по страницам: index ^ magic
    let magic = 0xC0FFEE_u64;
    let chunks = buf.len() / 8;
    for i in 0..chunks {
        let val = (i as u64).wrapping_mul(0x9E3779B97F4A7C15) ^ magic;
        let off = i * 8;
        buf[off..off + 8].copy_from_slice(&val.to_le_bytes());
    }
}

fn verify_pattern(buf: &[u8]) -> bool {
    let magic = 0xC0FFEE_u64;
    let chunks = buf.len() / 8;
    for i in 0..chunks {
        let expected = (i as u64).wrapping_mul(0x9E3779B97F4A7C15) ^ magic;
        let off = i * 8;
        let mut got = [0u8; 8];
        got.copy_from_slice(&buf[off..off + 8]);
        if u64::from_le_bytes(got) != expected {
            return false;
        }
        // лёгкий black_box
        std::hint::black_box(got);
    }
    true
}

fn detect_balloon() -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        use std::fs;
        use std::path::Path;
        // virtio-balloon
        if Path::new("/sys/bus/virtio/drivers/virtio_balloon").exists() {
            return Some(
                "virtio_balloon есть (типично для KVM) — сдувание RAM возможно, но не доказано"
                    .into(),
            );
        }
        if let Ok(entries) = fs::read_dir("/sys/bus/virtio/devices") {
            for e in entries.flatten() {
                if let Ok(name) = fs::read_to_string(e.path().join("device")) {
                    // balloon device id often 0x05
                    if name.trim() == "0x05" || name.to_lowercase().contains("balloon") {
                        return Some(
                            "Virtio balloon device присутствует — следите за avail RAM под нагрузкой"
                                .into(),
                        );
                    }
                }
            }
        }
        // xen balloon
        if Path::new("/sys/devices/system/xen_memory").exists()
            || Path::new("/proc/xen").exists()
        {
            if Path::new("/sys/devices/system/xen_memory/xen_memory0/target_kb").exists() {
                return Some("Xen balloon target найден — память может сжиматься".into());
            }
        }
    }
    None
}
