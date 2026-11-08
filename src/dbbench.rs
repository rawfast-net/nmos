use rusqlite::{params, Connection};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Barrier};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[derive(Debug, Serialize)]
pub struct DbReport {
    pub directory: String,
    pub sqlite_version: String,
    pub transaction_count: usize,
    pub database_bytes: u64,
    pub single_writer: Phase,
    pub four_writers: Phase,
    pub random_read: Phase,
    pub notes: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct Phase {
    pub completed: usize,
    pub elapsed_seconds: f64,
    pub operations_per_second: f64,
    pub latency_ms_p50: f64,
    pub latency_ms_p95: f64,
    pub latency_ms_p99: f64,
    pub steal_percent: Option<f64>,
}

struct WorkDir(PathBuf);
impl Drop for WorkDir {
    fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); }
}

fn work_dir(base: &Path) -> Result<WorkDir, String> {
    let base = base.canonicalize().map_err(|e| format!("Каталог: {e}"))?;
    for n in 0..100u32 {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|e| e.to_string())?.as_nanos();
        let path = base.join(format!("nmos-db-{}-{stamp}-{n}", std::process::id()));
        match std::fs::create_dir(&path) {
            Ok(()) => return Ok(WorkDir(path)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(format!("Создание временного каталога: {e}")),
        }
    }
    Err("Не удалось создать временный каталог".into())
}

fn connect(path: &Path) -> Result<Connection, String> {
    let db = Connection::open(path).map_err(|e| e.to_string())?;
    db.busy_timeout(Duration::from_secs(30)).map_err(|e| e.to_string())?;
    db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; PRAGMA wal_autocheckpoint=1000;").map_err(|e| e.to_string())?;
    Ok(db)
}

fn phase(mut samples: Vec<f64>, elapsed: f64, before: Option<(u64,u64)>, after: Option<(u64,u64)>) -> Phase {
    let steal_percent = before.zip(after).and_then(|((a, at), (b, bt))| { let total = bt.checked_sub(at)?; if total == 0 { return None; } Some(b.checked_sub(a)? as f64 * 100.0 / total as f64) });
    samples.sort_by(|a,b| a.total_cmp(b));
    let percentile = |p: f64| -> f64 {
        if samples.is_empty() { 0.0 } else { samples[((samples.len() - 1) as f64 * p).ceil() as usize] }
    };
    Phase { completed: samples.len(), elapsed_seconds: elapsed, operations_per_second: samples.len() as f64 / elapsed.max(1e-9), latency_ms_p50: percentile(0.50), latency_ms_p95: percentile(0.95), latency_ms_p99: percentile(0.99), steal_percent }
}

pub fn run(base: &Path, size_mb: u64, count: usize) -> Result<DbReport, String> {
    let dir = work_dir(base)?;
    let path = dir.0.join("bench.sqlite");
    let db = connect(&path)?;
    db.execute_batch("CREATE TABLE writes (id INTEGER PRIMARY KEY, payload BLOB NOT NULL); CREATE TABLE reads (id INTEGER PRIMARY KEY, payload BLOB NOT NULL);").map_err(|e| e.to_string())?;
    let mut times = Vec::with_capacity(count);
    let before = crate::stress::steal_sample();
    let start = Instant::now();
    for _ in 0..count {
        let t = Instant::now();
        db.execute("INSERT INTO writes(payload) VALUES (zeroblob(128))", []).map_err(|e| format!("Запись: {e}"))?;
        times.push(t.elapsed().as_secs_f64()*1000.0);
    }
    let single_writer = phase(times, start.elapsed().as_secs_f64(), before, crate::stress::steal_sample());
    drop(db);
    let barrier = Arc::new(Barrier::new(5));
    let mut handles = Vec::new();
    let connections: Vec<Connection> = (0..4).map(|_| connect(&path)).collect::<Result<_, _>>()?;
    for (worker, db) in connections.into_iter().enumerate() {
        let barrier = Arc::clone(&barrier);
        handles.push(std::thread::spawn(move || -> Result<Vec<f64>, String> {
            barrier.wait();
            let mut samples = Vec::new();
            for _ in (worker..count).step_by(4) {
                let t = Instant::now();
                db.execute("INSERT INTO writes(payload) VALUES (zeroblob(128))", []).map_err(|e| format!("Поток записи: {e}"))?;
                samples.push(t.elapsed().as_secs_f64()*1000.0);
            }
            Ok(samples)
        }));
    }
    let before = crate::stress::steal_sample();
    let start = Instant::now();
    barrier.wait();
    let mut times = Vec::with_capacity(count);
    let mut failure = None;
    for h in handles {
        match h.join() {
            Ok(Ok(values)) => times.extend(values),
            Ok(Err(error)) => failure = Some(error),
            Err(_) => failure = Some("Поток записи завершился с ошибкой".into()),
        }
    }
    if let Some(error) = failure { return Err(error); }
    let four_writers = phase(times, start.elapsed().as_secs_f64(), before, crate::stress::steal_sample());
    let mut db = connect(&path)?;
    let rows = (size_mb * 1024 * 1024 / 8192).max(1);
    let mut seed = 0x39a3_7e51_2b19_8e61u64;
    let mut payload = vec![0u8; 8192];
    for begin in (0..rows).step_by(256) {
        let tx = db.transaction().map_err(|e| e.to_string())?;
        {
            let mut insert = tx.prepare("INSERT INTO reads(id,payload) VALUES (?1,?2)").map_err(|e| e.to_string())?;
            for id in begin..(begin+256).min(rows) {
                for block in payload.chunks_exact_mut(8) {
                    seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17;
                    block.copy_from_slice(&seed.to_le_bytes());
                }
                insert.execute(params![id as i64, &payload]).map_err(|e| format!("Подготовка базы: {e}"))?;
            }
        }
        tx.commit().map_err(|e| e.to_string())?;
    }
    db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);").map_err(|e| e.to_string())?;
    drop(db);
    let database_bytes = std::fs::metadata(&path).map_err(|e| e.to_string())?.len();
    let db = connect(&path)?;
    db.execute_batch("PRAGMA cache_size=-2048;").map_err(|e| e.to_string())?;
    let mut query = db.prepare("SELECT payload FROM reads WHERE id=?1").map_err(|e| e.to_string())?;
    let mut times = Vec::with_capacity(count);
    let before = crate::stress::steal_sample();
    let start = Instant::now();
    for _ in 0..count {
        seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17;
        let id = (seed % rows) as i64;
        let t = Instant::now();
        let data: Vec<u8> = query.query_row([id], |r| r.get(0)).map_err(|e| format!("Чтение: {e}"))?;
        std::hint::black_box(data);
        times.push(t.elapsed().as_secs_f64()*1000.0);
    }
    let random_read = phase(times, start.elapsed().as_secs_f64(), before, crate::stress::steal_sample());
    drop(query); drop(db);
    Ok(DbReport { directory: base.display().to_string(), sqlite_version: rusqlite::version().into(), transaction_count: count, database_bytes, single_writer, four_writers, random_read, notes: vec![
        "SQLite WAL, synchronous=FULL; каждый INSERT выполняется отдельной транзакцией. Checkpoint включён.".into(),
        "SQLite допускает одного писателя одновременно; блокировки влияют на результат четырёх потоков.".into(),
        "Случайное чтение буферизовано: кэш ОС и хранилища не сбрасывается; это не измерение O_DIRECT.".into(),
        "TPS и задержки описывают систему целиком и не определяют тип диска или оверселлинг.".into(),
    ] })
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sqlite_phases_complete_and_remove_files() {
        let base = work_dir(&std::env::temp_dir()).unwrap();
        let result = run(&base.0, 1, 12).unwrap();
        assert_eq!(result.single_writer.completed, 12);
        assert_eq!(result.four_writers.completed, 12);
        assert_eq!(result.random_read.completed, 12);
        assert!(result.database_bytes >= 1024 * 1024);
        assert!(std::fs::read_dir(&base.0).unwrap().next().is_none());
    }

    #[test]
    fn missing_directory_is_reported() {
        let base = work_dir(&std::env::temp_dir()).unwrap();
        assert!(run(&base.0.join("missing"), 1, 1).is_err());
    }
}
