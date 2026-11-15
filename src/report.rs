use crate::antifraud::{AntifraudReport, ProviderClaims};
use crate::cpu::CpuInfo;
use crate::cpureality::CpuRealityReport;
use crate::diskio::DiskIoResult;
use crate::extras::ExtraProbe;
use crate::memcheck::MemCheckResult;
use crate::mail::MailEgressReport;
use crate::network::{NetworkInfo, SpeedResult};
use crate::oversell::{CpuEstimate, OversellAnalysis};
use crate::probe::DeepProbe;
use crate::reputation::ReputationReport;
use crate::stress::StressResult;
use crate::system::SystemInfo;
use crate::tech::TechReport;
use crate::truth::TruthReport;
use colored::*;
use comfy_table::{presets::NOTHING, Attribute, Cell, Color, ContentArrangement, Table};
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Serialize)]
pub struct DbBenchOutcome {
    pub result: Option<crate::dbbench::DbReport>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct FullReport {
    pub schema_version: u32,
    pub nmos_version: &'static str,
    pub db_bench: Option<DbBenchOutcome>,
    pub timestamp: String,
    pub claims: ProviderClaims,
    pub system: SystemInfo,
    pub cpu: CpuInfo,
    pub tech: TechReport,
    pub cpu_reality: CpuRealityReport,
    pub deep: DeepProbe,
    pub extras: ExtraProbe,
    pub memcheck: Option<MemCheckResult>,
    pub network: NetworkInfo,
    pub reputation: Option<ReputationReport>,
    pub mail_egress: Option<MailEgressReport>,
    pub diskio: Option<DiskIoResult>,
    pub stress: Option<StressResult>,
    pub oversell: OversellAnalysis,
    #[allow(dead_code)]
    #[serde(skip_serializing)]
    pub estimate: CpuEstimate,
    #[allow(dead_code)]
    #[serde(skip_serializing)]
    pub truth: TruthReport,
    #[allow(dead_code)]
    #[serde(skip_serializing)]
    pub antifraud: AntifraudReport,
}

pub fn print_human(r: &FullReport) {
    println!();
    println!("  {}", "nmos".cyan().bold());
    println!("  {}", "Диагностика сервера и качества VDS".dimmed());
    println!();

    let mut overview = new_table();
    overview.add_row(vec!["Время", &r.timestamp]);
    overview.add_row(vec!["Процессор", &r.cpu.model]);
    overview.add_row(vec!["Доступно vCPU", &r.cpu.logical_cores.to_string()]);
    overview.add_row(vec!["Память", &format!("{} MiB", r.system.ram_total_mb)]);
    overview.add_row(vec!["CPU под нагрузкой", &r.oversell.oversell_level]);
    overview.add_row(vec!["Согласованность данных", &r.truth.verdict]);
    if let Some(rep) = &r.reputation {
        overview.add_row(vec!["Спам-базы", &format!("{} записей, {} неясных", rep.listed, rep.inconclusive)]);
    }
    print_table("Обзор", overview);

    print_group("Тариф и заявленные параметры", &r.claims);
    print_group("Система", &r.system);
    print_group("Процессор", &r.cpu);
    print_group("Технологии CPU", &r.tech);
    print_group("Данные CPU", &r.cpu_reality);
    print_group("Нагрузочный тест CPU", &r.stress);
    print_group("Задержки CPU", &r.oversell);
    print_group("Оценка CPU", &r.estimate);
    print_group("Память", &r.memcheck);
    print_group("Диск", &r.diskio);
    let mut network = serde_json::to_value(&r.network).unwrap_or(Value::Null);
    if let Value::Object(map) = &mut network { map.remove("speed"); }
    print_group("Сеть", &network);
    print_speed(r.network.speed.as_ref());
    print_reputation(r.reputation.as_ref());
    print_group("Исходящая почта", &r.mail_egress);
    print_group("Глубокие проверки", &r.deep);
    print_group("Дополнительные проверки", &r.extras);
    print_group("Согласованность характеристик", &r.truth);
    print_group("Проверка тарифа", &r.antifraud);
    print_group("SQLite", &r.db_bench);
}

fn new_table() -> Table {
    let mut table = Table::new();
    table.load_preset(NOTHING);
    table.set_content_arrangement(ContentArrangement::Dynamic);
    table.set_header(vec![
        Cell::new("Показатель").add_attribute(Attribute::Bold).fg(Color::Cyan),
        Cell::new("Результат").add_attribute(Attribute::Bold).fg(Color::Cyan),
    ]);
    table
}

fn print_table(title: &str, table: Table) {
    println!("  {}", title.cyan().bold());
    println!("{table}");
    println!();
}

fn print_group<T: Serialize>(title: &str, data: &T) {
    let value = serde_json::to_value(data).unwrap_or(Value::Null);
    let mut rows = Vec::new();
    if value.as_object().is_some_and(|map| map.values().all(Value::is_null)) {
        rows.push(("Статус".to_string(), "Не указано".to_string()));
    } else {
        collect_rows("", &value, &mut rows);
    }
    if rows.is_empty() {
        rows.push(("Статус".to_string(), "Нет данных или тест пропущен".to_string()));
    }
    let mut table = new_table();
    for (key, value) in rows {
        table.add_row(vec![Cell::new(key).fg(Color::Cyan), status_cell(&wrap(&value, 100))]);
    }
    print_table(title, table);
}

fn collect_rows(path: &str, value: &Value, rows: &mut Vec<(String, String)>) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                let label = field_label(key);
                let next = if path.is_empty() { label } else { format!("{path} / {label}") };
                collect_rows(&next, child, rows);
            }
        }
        Value::Array(items) => {
            if items.is_empty() {
                rows.push((path.to_string(), "—".to_string()));
            } else if items.iter().all(|item| !item.is_object() && !item.is_array()) {
                rows.push((path.to_string(), items.iter().map(format_scalar).collect::<Vec<_>>().join(", ")));
            } else {
                for (i, item) in items.iter().enumerate() {
                    collect_rows(&format!("{path} / {}", i + 1), item, rows);
                }
            }
        }
        _ => rows.push((path.to_string(), format_scalar(value))),
    }
}

fn format_scalar(value: &Value) -> String {
    match value {
        Value::Null => "—".to_string(),
        Value::String(s) => s.clone(),
        Value::Number(n) if n.is_f64() => {
            let value = n.as_f64().unwrap_or_default();
            if value != 0.0 && value.abs() < 0.01 {
                format!("{value:.4}")
            } else {
                format!("{value:.2}")
            }
        }
        _ => value.to_string(),
    }
}

fn field_label(key: &str) -> String {
    let label = match key {
        "arch" => "Архитектура",
        "hostname" => "Хост",
        "os" => "ОС",
        "kernel" => "Ядро ОС",
        "disks" => "Диски",
        "mount" => "Том",
        "fs" => "Файловая система",
        "total_gb" => "Объём, ГиБ",
        "available_gb" => "Свободно, ГиБ",
        "used_percent" => "Занято, %",
        "ram_total_mb" => "Память всего, МиБ",
        "ram_used_mb" => "Память занята, МиБ",
        "ram_available_mb" => "Память доступна, МиБ",
        "ram_used_percent" => "Память занята, %",
        "swap_total_mb" => "Подкачка всего, МиБ",
        "swap_used_mb" => "Подкачка занята, МиБ",
        "cores" => "vCPU",
        "cpu_model" => "Модель CPU",
        "ram_gb" => "Память, ГиБ",
        "disk_gb" => "Диск, ГиБ",
        "net_mbps" => "Сеть, Мбит/с",
        "benchmark_version" => "Версия теста",
        "repetitions" => "Повторов",
        "single_variation_percent" => "Разброс одного потока, %",
        "multi_variation_percent" => "Разброс всех потоков, %",
        "single_ops_per_sec" => "Один поток, оп/с",
        "multi_ops_per_sec" => "Все потоки, оп/с",
        "steal_percent_load" => "CPU steal под нагрузкой, %",
        "scaling_efficiency" => "Масштабирование, %",
        "oversell_level" => "Вывод",
        "confidence" => "Надёжность вывода",
        "reasons" => "Основания",
        "verdict" => "Вывод",
        "trust_score" => "Баллы правил",
        "findings" => "Наблюдения",
        "message" => "Пояснение",
        "severity" => "Важность",
        "status" => "Статус",
        "notes" => "Примечания",
        _ => return key.replace('_', " "),
    };
    label.to_string()
}

fn wrap(value: &str, width: usize) -> String {
    let mut out = String::new();
    let mut count = 0;
    for ch in value.chars() {
        if ch == '\n' {
            count = 0;
        } else if count >= width {
            out.push('\n');
            count = 0;
        }
        out.push(ch);
        count += 1;
    }
    out
}

fn status_cell(value: &str) -> Cell {
    let lower = value.to_lowercase();
    let cell = Cell::new(value);
    if lower.contains("critical") || lower.contains("ошибка") || lower.contains("listed") {
        cell.fg(Color::Red)
    } else if lower.contains("warn") || lower.contains("неяс") || lower.contains("inconclusive") {
        cell.fg(Color::Yellow)
    } else if lower == "ok" || lower == "clear" || lower.contains("успеш") {
        cell.fg(Color::Green)
    } else {
        cell
    }
}

fn print_reputation(report: Option<&ReputationReport>) {
    let Some(report) = report else {
        print_group("Спам-базы", &Option::<String>::None);
        return;
    };
    let mut summary = new_table();
    summary.add_row(vec!["Проверяемый IP", &report.ip]);
    summary.add_row(vec!["Spam / Policy / другие", &format!("{} / {} / {}", report.listed, report.policy, report.other_flags)]);
    summary.add_row(vec!["Чисто / неясно / недоступно", &format!("{} / {} / {}", report.clear, report.inconclusive, report.unsupported)]);
    print_table("Репутация IP", summary);

    let mut table = Table::new();
    table.load_preset(NOTHING);
    table.set_content_arrangement(ContentArrangement::Dynamic);
    table.set_header(vec![
        Cell::new("База").add_attribute(Attribute::Bold).fg(Color::Cyan),
        Cell::new("Статус").add_attribute(Attribute::Bold).fg(Color::Cyan),
        Cell::new("Ответ и пояснение").add_attribute(Attribute::Bold).fg(Color::Cyan),
    ]);
    for check in &report.checks {
        let detail = if check.answers.is_empty() {
            check.detail.clone()
        } else {
            format!("{} — {}", check.answers.join(", "), check.detail)
        };
        table.add_row(vec![
            Cell::new(check.name).fg(Color::Cyan),
            status_cell(check.status),
            Cell::new(wrap(&detail, 80)),
        ]);
    }
    print_table("Спам-базы", table);
}

fn print_speed(speed: Option<&SpeedResult>) {
    let Some(speed) = speed else {
        print_group("Скорость сети", &Option::<String>::None);
        return;
    };
    let mut table = new_table();
    table.add_row(vec!["Метод", &speed.backend]);
    table.add_row(vec!["Статус", &speed.status]);
    if let Some(error) = &speed.error { table.add_row(vec!["Причина ошибки", error]); }
    if let Some(name) = &speed.server_name { table.add_row(vec!["Сервер", name]); }
    if let Some(city) = &speed.server_city { table.add_row(vec!["Город", city]); }
    if let Some(country) = &speed.server_country { table.add_row(vec!["Страна", country]); }
    if let Some(id) = speed.server_id { table.add_row(vec!["ID сервера", &id.to_string()]); }
    if let Some(value) = speed.best_download_mbps { table.add_row(vec!["Download", &format!("{value:.1} Мбит/с · {:.3} Гбит/с", value / 1000.0)]); }
    if let Some(value) = speed.upload_mbps.as_ref().map(|sample| sample.mbps) { table.add_row(vec!["Upload", &format!("{value:.1} Мбит/с · {:.3} Гбит/с", value / 1000.0)]); }
    if let Some(value) = speed.ping_ms { table.add_row(vec!["Ping", &format!("{value:.2} мс")]); }
    if let Some(value) = speed.jitter_ms { table.add_row(vec!["Jitter", &format!("{value:.2} мс")]); }
    if let Some(value) = speed.packet_loss_percent { table.add_row(vec!["Потери", &format!("{value:.2} %")]); }
    if let Some(value) = &speed.result_url { table.add_row(vec!["Результат", value]); }
    if let Some(value) = &speed.tested_at { table.add_row(vec!["Время", value]); }
    print_table("Скорость сети", table);
    if !speed.attempts.is_empty() { print_group("Попытки Speedtest", &speed.attempts); }
    if speed.measurements.len() > 1 { print_group("Повторные измерения", &speed.measurements); }
    if !speed.notes.is_empty() { print_group("Примечания Speedtest", &speed.notes); }
}
