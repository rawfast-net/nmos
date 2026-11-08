use crate::report::FullReport;
use serde::Serialize;
use std::io::{self, IsTerminal, Write};

#[derive(Debug, Serialize)]
struct PublicResult {
    schema: u32,
    provider: String,
    region: String,
    plan: String,
    tested_at: String,
    nmos_version: &'static str,
    cpu_model: String,
    vcpu: usize,
    ram_mb: u64,
    cpu_single_ops_per_sec: Option<f64>,
    cpu_multi_ops_per_sec: Option<f64>,
    cpu_benchmark_version: Option<&'static str>,
    steal_percent_load: Option<f64>,
    disk_random_iops: Option<f64>,
    disk_random_p99_us: Option<f64>,
    disk_read_method: String,
    network_download_mbps: Option<f64>,
    sqlite_single_tps: Option<f64>,
    sqlite_four_tps: Option<f64>,
    sqlite_read_p99_ms: Option<f64>,
}

fn prompt(label: &str) -> io::Result<String> {
    print!("{label}");
    io::stdout().flush()?;
    let mut input = String::new();
    if io::stdin().read_line(&mut input)? == 0 { return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Ввод завершён")); }
    Ok(input.trim().chars().filter(|c| !c.is_control()).take(80).collect())
}

fn encode(value: &str) -> String {
    let mut out = String::new();
    for b in value.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') { out.push(b as char); }
        else { out.push_str(&format!("%{b:02X}")); }
    }
    out
}

fn build(report: &FullReport, provider: String, region: String, plan: String) -> PublicResult {
    let disk = report.diskio.as_ref();
    let db = report.db_bench.as_ref().and_then(|x| x.result.as_ref());
    PublicResult {
        schema: 1, provider, region, plan,
        tested_at: report.timestamp.chars().take(10).collect(),
        nmos_version: env!("CARGO_PKG_VERSION"),
        cpu_model: report.cpu.model.chars().take(100).collect(),
        vcpu: report.cpu.logical_cores,
        ram_mb: report.system.ram_total_mb,
        cpu_single_ops_per_sec: report.stress.as_ref().map(|s| s.single_ops_per_sec),
        cpu_multi_ops_per_sec: report.stress.as_ref().map(|s| s.multi_ops_per_sec),
        cpu_benchmark_version: report.stress.as_ref().map(|s| s.benchmark_version),
        steal_percent_load: report.stress.as_ref().and_then(|s| s.steal_percent_load),
        disk_random_iops: disk.and_then(|d| d.rand_read_iops),
        disk_random_p99_us: disk.and_then(|d| d.rand_read_lat_us_p99),
        disk_read_method: disk.map(|d| if d.notes.iter().any(|n| n.contains("O_DIRECT (")) { "O_DIRECT" } else { "buffered_or_unknown" }).unwrap_or("not_tested").into(),
        network_download_mbps: report.network.speed.as_ref().and_then(|s| s.best_download_mbps),
        sqlite_single_tps: db.map(|d| d.single_writer.operations_per_second),
        sqlite_four_tps: db.map(|d| d.four_writers.operations_per_second),
        sqlite_read_p99_ms: db.map(|d| d.random_read.latency_ms_p99),
    }
}

pub fn offer(report: &FullReport) -> io::Result<()> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() { return Ok(()); }
    let answer = prompt("\nПодготовить результат для публикации? [y/N]: ")?;
    if !matches!(answer.to_lowercase().as_str(), "y" | "yes" | "д" | "да") { return Ok(()); }
    let provider = prompt("Провайдер: ")?;
    if provider.is_empty() { println!("Провайдер не указан. Отменено."); return Ok(()); }
    let region = prompt("Регион (необязательно): ")?;
    let plan = prompt("Тариф (необязательно): ")?;
    let public = build(report, provider, region, plan);
    let json = serde_json::to_string_pretty(&public).map_err(io::Error::other)?;
    println!("\nДанные для публикации:\n{json}\n");
    println!("Проверьте текст: поля провайдера, тарифа и модели CPU могут содержать введённые вами данные.");
    println!("Issue в публичном репозитории будет виден всем сразу после отправки.");
    let answer = prompt("Создать ссылку на черновик Issue? [y/N]: ")?;
    if !matches!(answer.to_lowercase().as_str(), "y" | "yes" | "д" | "да") { println!("Отменено. Данные не отправлены."); return Ok(()); }
    let title = format!("VDS результат: {}", public.provider);
    let body = format!("Результат nmos для проверки перед добавлением на страницу.\n\n```json\n{json}\n```\n\nЯ проверил данные и согласен на их публичное размещение.");
    println!("\nОткройте ссылку, проверьте текст и отправьте Issue вручную:\nhttps://github.com/rawfast-net/nmos/issues/new?title={}&body={}\n", encode(&title), encode(&body));

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::encode;
    #[test]
    fn encodes_utf8_and_reserved_characters() { assert_eq!(encode("да & #"), "%D0%B4%D0%B0%20%26%20%23"); }
}
