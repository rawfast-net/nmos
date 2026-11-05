
use serde::Serialize;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::time::Instant;

#[derive(Debug, Clone, Serialize)]
pub struct DiskIoResult {
    pub path: String,
    pub size_mb: u64,
    pub write_mbps: Option<f64>,
    pub read_mbps: Option<f64>,
    pub fsync_latency_us_avg: Option<f64>,
    pub fsync_samples: u32,
    pub verify_ok: Option<bool>,
    pub checksum_expected: Option<u64>,
    pub checksum_got: Option<u64>,
    /// Слишком быстрый read+идеальный write без реальной задержки — подозрение на tmpfs/RAM
    pub looks_like_ramdisk: bool,
    pub rand_read_iops: Option<f64>,
    pub rand_read_lat_us_p99: Option<f64>,
    pub notes: Vec<String>,
}

pub fn run(size_mb: u64) -> DiskIoResult {
    let size_mb = size_mb.clamp(64, 1024);
    let mut notes = Vec::new();

    let path = work_file();
    let path_str = path.display().to_string();

    // Паттерн-чанк с seed
    let seed = 0xDEAD_BEEF_CAFE_u64;
    let chunk = patterned_chunk(1024 * 1024, seed);

    let write_mbps = match sequential_write_patterned(&path, &chunk, size_mb as usize) {
        Ok(mbps) => Some(mbps),
        Err(e) => {
            notes.push(format!("write error: {e}"));
            None
        }
    };

    // fsync latency
    let (fsync_avg, fsync_n) = match fsync_latency(&path, 32) {
        Ok((avg, n)) => (Some(avg), n),
        Err(e) => {
            notes.push(format!("fsync error: {e}"));
            (None, 0)
        }
    };

    // Закрыли/открыли заново + checksum verify (не из того же буфера)
    let (verify_ok, checksum_expected, checksum_got) = match verify_file_checksum(&path, size_mb as usize, seed)
    {
        Ok((ok, exp, got)) => {
            if ok {
                notes.push("Checksum после reopen+fsync: OK — данные реально на ФС".into());
            } else {
                notes.push(format!(
                    "Checksum FAIL exp={exp:#x} got={got:#x} — диск/кэш врёт"
                ));
            }
            (Some(ok), Some(exp), Some(got))
        }
        Err(e) => {
            notes.push(format!("verify error: {e}"));
            (None, None, None)
        }
    };

    let bytes = (size_mb as usize) * 1024 * 1024;
    let read_mbps = match sequential_read(&path, bytes) {
        Ok(mbps) => Some(mbps),
        Err(e) => {
            notes.push(format!("read error: {e}"));
            None
        }
    };

    // Эвристика RAM-disk: путь в tmpfs или безумно быстрый write (>3000) + fsync ~0
    let on_tmpfs = is_tmpfs_path(&path);
    let looks_like_ramdisk = on_tmpfs
        || (write_mbps.unwrap_or(0.0) > 3500.0
            && fsync_avg.unwrap_or(9999.0) < 50.0
            && path_str.contains("tmp"));

    if on_tmpfs {
        notes.push("Рабочий путь на tmpfs — для теста диска это не считается".into());
    }
    if looks_like_ramdisk && !on_tmpfs {
        notes.push("I/O профиль похож на RAM-кэш/недиск".into());
    }
    if let (Some(w), Some(r)) = (write_mbps, read_mbps) {
        if r > w * 4.0 && r > 3000.0 {
            notes.push(format!(
                "Read {:.0} MB/s ≫ Write {:.0} — скорее page cache, не «сырой» диск",
                r, w
            ));
        }
    }

    let (rand_read_iops, rand_read_lat_us_p99) = match random_read_4k(&path, size_mb as usize, &mut notes)
    {
        Ok((iops, p99)) => {
            notes.push(format!("Random 4K read ≈ {iops:.0} IOPS (p99 {p99:.0} µs)"));
            (Some(iops), Some(p99))
        }
        Err(e) => {
            notes.push(format!("random 4K: {e}"));
            (None, None)
        }
    };

    let _ = fs::remove_file(&path);

    DiskIoResult {
        path: path_str,
        size_mb,
        write_mbps,
        read_mbps,
        fsync_latency_us_avg: fsync_avg,
        fsync_samples: fsync_n,
        verify_ok,
        checksum_expected,
        checksum_got,
        looks_like_ramdisk,
        rand_read_iops,
        rand_read_lat_us_p99,
        notes,
    }
}

fn patterned_chunk(len: usize, seed: u64) -> Vec<u8> {
    let mut v = vec![0u8; len];
    for i in 0..(len / 8) {
        let val = seed
            .wrapping_mul(0x9E3779B97F4A7C15)
            .wrapping_add(i as u64);
        v[i * 8..i * 8 + 8].copy_from_slice(&val.to_le_bytes());
    }
    v
}

fn file_checksum(path: &PathBuf, mb: usize, seed: u64) -> std::io::Result<u64> {
    let mut f = File::open(path)?;
    let mut buf = vec![0u8; 1024 * 1024];
    let mut hash = seed;
    for i in 0..mb {
        f.read_exact(&mut buf)?;
        // быстрый checksum
        for (j, chunk) in buf.chunks_exact(8).enumerate() {
            let mut a = [0u8; 8];
            a.copy_from_slice(chunk);
            let n = u64::from_le_bytes(a);
            hash ^= n.wrapping_add((i as u64) << 32).wrapping_add(j as u64);
            hash = hash.rotate_left(7).wrapping_mul(0x100000001B3);
        }
    }
    Ok(hash)
}

fn expected_checksum(mb: usize, seed: u64) -> u64 {
    let chunk = patterned_chunk(1024 * 1024, seed);
    let mut hash = seed;
    for i in 0..mb {
        for (j, c) in chunk.chunks_exact(8).enumerate() {
            let mut a = [0u8; 8];
            a.copy_from_slice(c);
            let n = u64::from_le_bytes(a);
            hash ^= n.wrapping_add((i as u64) << 32).wrapping_add(j as u64);
            hash = hash.rotate_left(7).wrapping_mul(0x100000001B3);
        }
    }
    hash
}

fn verify_file_checksum(
    path: &PathBuf,
    mb: usize,
    seed: u64,
) -> std::io::Result<(bool, u64, u64)> {
    // убеждаемся что данные сброшены
    {
        let f = OpenOptions::new().write(true).open(path)?;
        f.sync_all()?;
    }
    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("sh")
            .args(["-c", "sync; echo 3 > /proc/sys/vm/drop_caches 2>/dev/null"])
            .status();
    }
    let exp = expected_checksum(mb, seed);
    let got = file_checksum(path, mb, seed)?;
    Ok((exp == got, exp, got))
}

fn work_file() -> PathBuf {
    // предпочитаем /var/tmp (обычно диск), не /tmp (часто tmpfs)
    let candidates = [
        PathBuf::from("/var/tmp"),
        PathBuf::from("."),
        std::env::temp_dir(),
        PathBuf::from("/tmp"),
    ];
    for dir in candidates {
        if dir.exists() {
            return dir.join(format!(
                "nmos-io-{}-{}.bin",
                std::process::id(),
                chrono::Local::now().timestamp_millis()
            ));
        }
    }
    PathBuf::from("nmos-io.bin")
}

fn is_tmpfs_path(path: &PathBuf) -> bool {
    #[cfg(target_os = "linux")]
    {
        if let Ok(out) = std::process::Command::new("df")
            .args(["-T", path.to_str().unwrap_or("/var/tmp")])
            .output()
        {
            let s = String::from_utf8_lossy(&out.stdout).to_lowercase();
            return s.contains("tmpfs") || s.contains("ramfs");
        }
    }
    let _ = path;
    false
}

fn sequential_write_patterned(path: &PathBuf, chunk: &[u8], mb: usize) -> std::io::Result<f64> {
    let mut f = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)?;
    let t0 = Instant::now();
    for _ in 0..mb {
        f.write_all(chunk)?;
    }
    f.sync_all()?;
    let secs = t0.elapsed().as_secs_f64().max(1e-6);
    Ok((mb as f64) / secs)
}

fn sequential_read(path: &PathBuf, bytes: usize) -> std::io::Result<f64> {
    let mut f = File::open(path)?;
    let mut buf = vec![0u8; 1024 * 1024];
    let mut left = bytes;
    let t0 = Instant::now();
    while left > 0 {
        let n = f.read(&mut buf)?.min(left);
        if n == 0 {
            break;
        }
        left -= n;
        std::hint::black_box(&buf[..n]);
    }
    let secs = t0.elapsed().as_secs_f64().max(1e-6);
    let read_mb = (bytes - left) as f64 / (1024.0 * 1024.0);
    Ok(read_mb / secs)
}

fn fsync_latency(path: &PathBuf, samples: u32) -> std::io::Result<(f64, u32)> {
    let mut f = OpenOptions::new().create(true).append(true).open(path)?;
    let block = vec![0x5Au8; 4096];
    let mut total_us = 0.0f64;
    let mut n = 0u32;
    for _ in 0..samples {
        let t0 = Instant::now();
        f.write_all(&block)?;
        f.sync_data()?;
        total_us += t0.elapsed().as_secs_f64() * 1_000_000.0;
        n += 1;
    }
    let _ = f.seek(SeekFrom::End(0));
    Ok((total_us / n as f64, n))
}

/// Random 4K reads — предпочитаем O_DIRECT, иначе drop_caches + buffered.
fn random_read_4k(
    path: &PathBuf,
    size_mb: usize,
    notes: &mut Vec<String>,
) -> std::io::Result<(f64, f64)> {
    let file_bytes = (size_mb as u64) * 1024 * 1024;
    let blocks = (file_bytes / 4096).max(1);
    let samples = 512u32.min(blocks as u32).max(64);

    #[repr(C, align(4096))]
    struct Aligned4k([u8; 4096]);

    let mut buf = Aligned4k([0u8; 4096]);
    let mut f = open_for_cold_read(path, notes)?;

    let mut lats = Vec::with_capacity(samples as usize);
    let t_all = Instant::now();
    let mut seed = 0xC0FFEE_u64;
    for _ in 0..samples {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1);
        let off = (seed % blocks) * 4096;
        let t0 = Instant::now();
        f.seek(SeekFrom::Start(off))?;
        let n = f.read(&mut buf.0)?;
        std::hint::black_box(&buf.0[..n]);
        lats.push(t0.elapsed().as_secs_f64() * 1_000_000.0);
    }
    let secs = t_all.elapsed().as_secs_f64().max(1e-6);
    let iops = samples as f64 / secs;
    lats.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let idx = ((lats.len() as f64) * 0.99) as usize;
    let p99 = lats[idx.min(lats.len() - 1)];
    Ok((iops, p99))
}

fn open_for_cold_read(path: &PathBuf, notes: &mut Vec<String>) -> std::io::Result<File> {
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::OpenOptionsExt;
        // O_DIRECT (linux x86/arm common value)
        const O_DIRECT: i32 = 0o40000;
        match OpenOptions::new()
            .read(true)
            .custom_flags(O_DIRECT)
            .open(path)
        {
            Ok(f) => {
                notes.push("random 4K: O_DIRECT (без page cache)".into());
                return Ok(f);
            }
            Err(e) => {
                notes.push(format!(
                    "random 4K: O_DIRECT недоступен ({e}) — drop_caches + buffered"
                ));
            }
        }
        let _ = std::process::Command::new("sh")
            .args(["-c", "sync; echo 3 > /proc/sys/vm/drop_caches 2>/dev/null"])
            .status();
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = notes;
    }
    File::open(path)
}
