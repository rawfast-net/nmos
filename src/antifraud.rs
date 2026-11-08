use crate::cpu::CpuInfo;
use crate::cpureality::CpuRealityReport;
use crate::diskio::DiskIoResult;
use crate::memcheck::MemCheckResult;
use crate::network::NetworkInfo;
use crate::probe::DeepProbe;
use crate::stress::StressResult;
use crate::system::SystemInfo;
use crate::tech::TechReport;
use crate::truth::TruthReport;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, Default)]
pub struct ProviderClaims {
    pub cores: Option<usize>,
    pub ram_gb: Option<f64>,
    pub disk_gb: Option<f64>,
    pub cpu_model: Option<String>,
    pub net_mbps: Option<f64>,
}

impl ProviderClaims {
    pub fn any(&self) -> bool {
        self.cores.is_some()
            || self.ram_gb.is_some()
            || self.disk_gb.is_some()
            || self.cpu_model.is_some()
            || self.net_mbps.is_some()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ChecklistItem {
    pub area: String,
    pub status: String, 
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AntifraudReport {
    pub honesty_score: f64,
    pub grade: String,
    pub headline: String,
    pub checklist: Vec<ChecklistItem>,
    pub claims_used: bool,
    pub advice: Vec<String>,
}

pub fn build(
    claims: &ProviderClaims,
    cpu: &CpuInfo,
    sys: &SystemInfo,
    deep: &DeepProbe,
    tech: &TechReport,
    net: &NetworkInfo,
    stress: Option<&StressResult>,
    disk: Option<&DiskIoResult>,
    mem: Option<&MemCheckResult>,
    truth: &TruthReport,
    cpu_reality: &CpuRealityReport,
    extras: Option<&crate::extras::ExtraProbe>,
) -> AntifraudReport {
    let mut checklist = Vec::new();
    let mut score: f64 = 100.0;
    let claims_used = claims.any();

    if let Some(ref claimed) = claims.cpu_model {
        let ok = crate::catalog::claim_matches_actual(claimed, &cpu.model);
        if ok {
            checklist.push(item("CPU модель", "pass", format!("«{claimed}» ≈ «{}»", cpu.model)));
        } else {
            checklist.push(item(
                "CPU модель",
                "fail",
                format!("Продали «{claimed}», в системе «{}»", cpu.model),
            ));
            score -= 25.0;
        }
    } else {
        checklist.push(item(
            "CPU модель",
            "skip",
            format!("Факт: {} (укажите --claim-cpu)", cpu.model),
        ));
    }

    if let Some(claim_c) = claims.cores {
        let fact = cpu.logical_cores;
        let quota = deep.cgroup.cpu_quota_cores;
        if let Some(q) = quota {
            if q + 0.2 < claim_c as f64 {
                checklist.push(item(
                    "Ядра / quota",
                    "fail",
                    format!("Продали {claim_c}, cgroup quota ≈ {q:.2}, видно {fact}"),
                ));
                score -= 30.0;
            } else if fact < claim_c {
                checklist.push(item(
                    "Ядра / quota",
                    "fail",
                    format!("Продали {claim_c}, видно только {fact}"),
                ));
                score -= 25.0;
            } else {
                checklist.push(item(
                    "Ядра / quota",
                    "pass",
                    format!("Продали {claim_c}, видно {fact}, quota≈{q:.2}"),
                ));
            }
        } else if fact < claim_c {
            checklist.push(item(
                "Ядра",
                "fail",
                format!("Продали {claim_c} vCPU, система видит {fact}"),
            ));
            score -= 25.0;
        } else {
            checklist.push(item(
                "Ядра",
                "pass",
                format!("Продали {claim_c}, видно {fact}"),
            ));
        }
    } else {
        let q = deep
            .cgroup
            .cpu_quota_cores
            .map(|v| format!(", quota≈{v:.2}"))
            .unwrap_or_default();
        checklist.push(item(
            "Ядра",
            "skip",
            format!("{} лог.{} (укажите --claim-cores)", cpu.logical_cores, q),
        ));
    }

    match cpu_reality.consistency.as_str() {
        "fail" => {
            checklist.push(item(
                "CPU reality",
                "fail",
                format!(
                    "{} · mismatches: {}",
                    cpu_reality.decoded_uarch.clone().unwrap_or_else(|| "?".into()),
                    cpu_reality.mismatches.len()
                ),
            ));
            score -= 25.0;
        }
        "warn" => {
            checklist.push(item(
                "CPU reality",
                "warn",
                format!(
                    "{} · {}",
                    cpu_reality.decoded_uarch.clone().unwrap_or_else(|| "?".into()),
                    cpu_reality.mismatches.first().cloned().unwrap_or_default()
                ),
            ));
            score -= 10.0;
        }
        _ => {
            checklist.push(item(
                "CPU reality",
                "pass",
                format!(
                    "{} · ISA/tech согласованы ({})",
                    cpu_reality.decoded_uarch.clone().unwrap_or_else(|| "n/a".into()),
                    cpu_reality.confidence
                ),
            ));
        }
    }
    for m in cpu_reality.mismatches.iter().take(3) {
        checklist.push(item("CPU mismatch", "fail", m.clone()));
        score -= 5.0;
    }

    for f in truth.findings.iter().filter(|f| {
        matches!(
            f.code.as_str(),
            "vendor_lie_amd_on_intel"
                | "vendor_lie_intel_on_amd"
                | "vmx_on_amd"
                | "svm_on_intel"
                | "flags_missing_avx2"
                | "flags_missing_aes"
                | "cpuid_family_mismatch"
                | "cpuid_family_old"
                | "old_name_new_silicon"
        )
    }) {
        let st = if f.severity == "critical" {
            "fail"
        } else if f.severity == "warn" {
            "warn"
        } else {
            "info"
        };
        checklist.push(item("CPU fingerprint", st, f.message.clone()));
        
        if !matches!(truth.verdict_level.as_str(), "lie" | "suspect") {
            if st == "fail" {
                score -= 10.0;
            } else if st == "warn" {
                score -= 4.0;
            }
        }
    }

    match truth.verdict_level.as_str() {
        "lie" => {
            checklist.push(item("CPU: сравнение с ориентиром", "fail", truth.verdict.clone()));
            score -= 30.0;
        }
        "suspect" => {
            checklist.push(item("CPU: сравнение с ориентиром", "warn", truth.verdict.clone()));
            score -= 12.0;
        }
        _ => {
            checklist.push(item(
                "CPU: сравнение с ориентиром",
                "pass",
                format!("Trust {:.0}/100 · {}", truth.trust_score, truth.verdict),
            ));
        }
    }

    if let Some(s) = stress {
        if s.scaling_efficiency < 55.0 {
            checklist.push(item(
                "CPU scaling",
                "warn",
                format!("{:.0}% — низкое масштабирование; причины неоднозначны", s.scaling_efficiency),
            ));
        } else if s.scaling_efficiency < 75.0 {
            checklist.push(item(
                "CPU scaling",
                "warn",
                format!("{:.0}% — сниженное масштабирование", s.scaling_efficiency),
            ));
        } else {
            checklist.push(item(
                "CPU scaling",
                "pass",
                format!("{:.0}% от ST×N", s.scaling_efficiency),
            ));
        }
    } else {
        checklist.push(item("CPU scaling", "skip", "стресс-тест пропущен".into()));
    }

    if tech.available {
        if tech.missing_important.is_empty() {
            checklist.push(item(
                "Технологии CPU",
                "pass",
                format!("{} · ключевых: {}", tech.isa_level, tech.present_highlights.join(", ")),
            ));
        } else {
            checklist.push(item(
                "Технологии CPU",
                "warn",
                format!("Нет: {}", tech.missing_important.join(", ")),
            ));
            score -= 8.0;
        }
    } else {
        checklist.push(item("Технологии CPU", "skip", "flags недоступны".into()));
    }

    if let Some(st) = cpu.steal_percent {
        if st > 10.0 {
            checklist.push(item("CPU steal", "fail", format!("{st:.1}%")));
            score -= 20.0;
        } else if st > 3.0 {
            checklist.push(item("CPU steal", "warn", format!("{st:.1}%")));
            score -= 8.0;
        } else {
            checklist.push(item("CPU steal", "pass", format!("{st:.2}%")));
        }
    } else {
        checklist.push(item("CPU steal", "skip", "только Linux guest".into()));
    }

    if let Some(claim_ram) = claims.ram_gb {
        let fact_gb = sys.ram_total_mb as f64 / 1024.0;
        let ratio = fact_gb / claim_ram.max(0.01);
        if ratio < 0.85 {
            checklist.push(item(
                "RAM объём",
                "fail",
                format!("Продали {claim_ram:.1} GB, MemTotal ≈ {fact_gb:.2} GB"),
            ));
            score -= 25.0;
        } else if ratio < 0.95 {
            checklist.push(item(
                "RAM объём",
                "warn",
                format!("Продали {claim_ram:.1} GB, факт ≈ {fact_gb:.2} GB"),
            ));
            score -= 8.0;
        } else {
            checklist.push(item(
                "RAM объём",
                "pass",
                format!("Продали {claim_ram:.1} GB, факт ≈ {fact_gb:.2} GB"),
            ));
        }
    } else {
        checklist.push(item(
            "RAM объём",
            "skip",
            format!("{:.2} GB (укажите --claim-ram-gb)", sys.ram_total_mb as f64 / 1024.0),
        ));
    }

    if let Some(m) = mem {
        if !m.alloc_ok {
            checklist.push(item("Проверка RAM", "fail", "не удалось выделить тестовый буфер".into()));
            score -= 20.0;
        } else if m.verify_ok == Some(false) {
            checklist.push(item("Проверка RAM", "fail", "паттерн повреждён — ошибка проверки памяти".into()));
            score -= 30.0;
        } else {
            let bw = m.write_gbps.map(|v| format!("{v:.1} GB/s")).unwrap_or_else(|| "—".into());
            checklist.push(item(
                "Проверка RAM",
                "pass",
                format!("verify OK · {} MB · write {bw}", m.tested_mb),
            ));
        }
        if let Some(ref b) = m.balloon_hint {
            
            checklist.push(item("RAM balloon", "info", b.clone()));
        }
        if let Some(ref s) = m.swap_abuse_hint {
            checklist.push(item("Swap", "warn", s.clone()));
            score -= 5.0;
        }
    } else {
        checklist.push(item("Проверка RAM", "skip", "memcheck пропущен".into()));
    }

    let root_gb = sys
        .disks
        .iter()
        .find(|d| d.mount == "/" || d.mount == "C:\\")
        .map(|d| d.total_gb)
        .or_else(|| sys.disks.first().map(|d| d.total_gb));

    if let (Some(claim_d), Some(fact)) = (claims.disk_gb, root_gb) {
        let ratio = fact / claim_d.max(0.01);
        if ratio < 0.80 {
            checklist.push(item(
                "Диск объём",
                "fail",
                format!("Продали {claim_d:.0} GB, факт ≈ {fact:.1} GB"),
            ));
            score -= 20.0;
        } else if ratio < 0.92 {
            checklist.push(item(
                "Диск объём",
                "warn",
                format!("Продали {claim_d:.0} GB, факт ≈ {fact:.1} GB"),
            ));
            score -= 6.0;
        } else {
            checklist.push(item(
                "Диск объём",
                "pass",
                format!("Продали {claim_d:.0} GB, факт ≈ {fact:.1} GB"),
            ));
        }
    } else if let Some(fact) = root_gb {
        checklist.push(item(
            "Диск объём",
            "skip",
            format!("{fact:.1} GB (укажите --claim-disk-gb)"),
        ));
    }

    if let Some(d) = disk {
        if d.verify_ok == Some(false) {
            checklist.push(item(
                "Диск целостность",
                "fail",
                "checksum после fsync не совпал — контрольная сумма не совпала".into(),
            ));
            score -= 30.0;
        } else if d.verify_ok == Some(true) {
            let w = d.write_mbps.map(|v| format!("{v:.0}")).unwrap_or_else(|| "?".into());
            let r = d.read_mbps.map(|v| format!("{v:.0}")).unwrap_or_else(|| "?".into());
            checklist.push(item(
                "Диск целостность",
                "pass",
                format!("checksum OK · W {w} / R {r} MB/s"),
            ));
        }
        if let Some(w) = d.write_mbps {
            if w < 40.0 {
                checklist.push(item("Диск скорость", "warn", format!("запись {w:.0} MB/s — слабо для SSD")));
                score -= 8.0;
            } else {
                checklist.push(item("Диск скорость", "pass", format!("запись {w:.0} MB/s")));
            }
        }
        if d.looks_like_ramdisk {
            checklist.push(item(
                "Диск тип",
                "fail",
                "поведение похоже на RAM-disk / суперкэш — данные могут быть не на диске".into(),
            ));
            score -= 25.0;
        }
    } else {
        checklist.push(item("Диск", "skip", "I/O тест пропущен".into()));
    }

    if let Some(claim_net) = claims.net_mbps {
        let best = net.speed.as_ref().and_then(|s| s.best_download_mbps);
        if let Some(b) = best {
            let ratio = b / claim_net.max(1.0);
            if ratio < 0.35 {
                checklist.push(item(
                    "Сеть",
                    "fail",
                    format!("Продали {claim_net:.0} Mbps, best ↓ {b:.0}"),
                ));
                score -= 15.0;
            } else if ratio < 0.6 {
                checklist.push(item(
                    "Сеть",
                    "warn",
                    format!("Продали {claim_net:.0} Mbps, best ↓ {b:.0}"),
                ));
                score -= 7.0;
            } else {
                checklist.push(item(
                    "Сеть",
                    "pass",
                    format!("Продали {claim_net:.0} Mbps, best ↓ {b:.0}"),
                ));
            }
        } else {
            checklist.push(item("Сеть", "skip", "speed-тест не дал цифр".into()));
        }
    } else if let Some(b) = net.speed.as_ref().and_then(|s| s.best_download_mbps) {
        checklist.push(item(
            "Сеть",
            "skip",
            format!("best ↓ {b:.0} Mbps (укажите --claim-net-mbps)"),
        ));
    } else {
        checklist.push(item("Сеть", "skip", "нет данных".into()));
    }

    if cpu.virtualization.is_virtual {
        checklist.push(item(
            "Виртуализация",
            "pass",
            format!(
                "{} / {}",
                cpu.virtualization.virt_type,
                cpu.virtualization
                    .hypervisor
                    .clone()
                    .unwrap_or_else(|| "unknown".into())
            ),
        ));
    } else {
        checklist.push(item(
            "Виртуализация",
            "warn",
            "виртуализация не определена".into(),
        ));
    }

    if let Some(x) = extras {
        if let Some(cs) = &x.clocksource {
            checklist.push(item("Clocksource", "pass", cs.clone()));
        }
        if let Some(p99) = x.sleep_jitter_p99_us {
            if p99 > 2500.0 {
                checklist.push(item(
                    "Таймеры",
                    "warn",
                    format!("sleep jitter p99 {p99:.0}µs"),
                ));
                score -= 5.0;
            } else {
                checklist.push(item(
                    "Таймеры",
                    "pass",
                    format!("jitter p99 {p99:.0}µs"),
                ));
            }
        }
        if let Some(p) = x.cpu_pressure_some_avg10 {
            if p > 30.0 {
                checklist.push(item("PSI CPU", "fail", format!("avg10 {p:.1}%")));
                score -= 12.0;
            } else if p > 10.0 {
                checklist.push(item("PSI CPU", "warn", format!("avg10 {p:.1}%")));
                score -= 5.0;
            } else {
                checklist.push(item("PSI CPU", "pass", format!("avg10 {p:.1}%")));
            }
        }
        if let Some(n) = x.cgroup_nr_throttled {
            if n > 0 {
                checklist.push(item(
                    "CPU throttle",
                    "warn",
                    format!("nr_throttled={n}"),
                ));
                score -= 8.0;
            } else {
                checklist.push(item("CPU throttle", "pass", "нет".into()));
            }
        }
        if let Some(fs) = &x.root_fstype {
            let bad = ["nfs", "cifs", "fuse", "ceph", "gluster"];
            if bad.iter().any(|b| fs.to_lowercase().contains(b)) {
                checklist.push(item("Root FS", "fail", fs.clone()));
                score -= 20.0;
            } else {
                checklist.push(item("Root FS", "pass", fs.clone()));
            }
        }
        if !x.container_hints.is_empty() {
            checklist.push(item(
                "Контейнер?",
                "warn",
                x.container_hints.join("; "),
            ));
            score -= 10.0;
        }
        if x.modules_virt.iter().any(|m| m.contains("vz") || m.contains("ploop")) {
            checklist.push(item("Стек", "fail", "OpenVZ/ploop модули".into()));
            score -= 25.0;
        }
        if !x.net_drivers.is_empty() {
            checklist.push(item(
                "NIC driver",
                "info",
                x.net_drivers.join(", "),
            ));
        }
        if !x.net_link_speeds.is_empty() {
            let slow = x.net_link_speeds.iter().any(|s| {
                s.split('=')
                    .nth(1)
                    .and_then(|v| v.replace("Mbps", "").trim().parse::<u64>().ok())
                    .map(|m| m > 0 && m < 1000)
                    .unwrap_or(false)
            });
            checklist.push(item(
                "Link speed",
                if slow { "warn" } else { "pass" },
                x.net_link_speeds.join(", "),
            ));
            if slow {
                score -= 5.0;
            }
        }
        if x.zram_present {
            checklist.push(item("zram", "warn", "сжатый RAM как swap".into()));
            score -= 4.0;
        }
        if x.ksm_run == Some(true) {
            checklist.push(item("KSM", "warn", "включён".into()));
            score -= 3.0;
        }
        if let Some(oc) = &x.overcommit_memory {
            if oc == "1" {
                checklist.push(item("overcommit", "warn", "always (1)".into()));
                score -= 4.0;
            } else {
                checklist.push(item("overcommit", "pass", oc.clone()));
            }
        }
    }

    
    for fnd in truth.findings.iter().filter(|f| {
        matches!(
            f.code.as_str(),
            "psi_cpu_high"
                | "cgroup_throttled"
                | "root_network_fs"
                | "openvz_like"
                | "disk_rand_slow"
                | "timer_jitter_high"
                | "throttle_vs_scaling"
                | "cpu_hotplug_offline"
                | "cpuset_narrow"
        )
    }) {
        let st = if fnd.severity == "critical" {
            "fail"
        } else if fnd.severity == "warn" {
            "warn"
        } else {
            "info"
        };
        if !checklist.iter().any(|c| c.detail == fnd.message) {
            checklist.push(item("Extra check", st, fnd.message.clone()));
            if st == "fail" {
                score -= 8.0;
            } else if st == "warn" {
                score -= 3.0;
            }
        }
    }

    if let Some(d) = disk {
        if let Some(iops) = d.rand_read_iops {
            if iops < 500.0 {
                checklist.push(item(
                    "Диск IOPS",
                    "warn",
                    format!("random 4K ≈ {iops:.0} IOPS"),
                ));
                score -= 6.0;
            } else {
                checklist.push(item(
                    "Диск IOPS",
                    "pass",
                    format!("random 4K ≈ {iops:.0} IOPS"),
                ));
            }
        }
    }

    score = score.clamp(0.0, 100.0);
    let (grade, headline) = grade(score, &checklist);
    let mut advice = Vec::new();
    if !claims_used {
        advice.push(
            "re-run with --claim-cores/--claim-ram-gb/--claim-disk-gb/--claim-cpu/--claim-net-mbps"
                .into(),
        );
    }
    if checklist.iter().any(|c| c.status == "fail") {
        advice.push("fails present — ask the hoster for a node swap or refund".into());
    }
    if let Some(s) = stress {
        if s.scaling_efficiency < 70.0 {
            advice.push("retest at different times; low scaling alone does not establish overselling".into());
        }
    }
    advice.push("trust=cpu passport; honesty=plan claims + checklist".into());

    AntifraudReport {
        honesty_score: score,
        grade,
        headline,
        checklist,
        claims_used,
        advice,
    }
}

fn item(area: &str, status: &str, detail: String) -> ChecklistItem {
    ChecklistItem {
        area: area.into(),
        status: status.into(),
        detail,
    }
}

fn grade(score: f64, checklist: &[ChecklistItem]) -> (String, String) {
    
    let hard_fails = checklist
        .iter()
        .filter(|c| {
            c.status == "fail"
                && !matches!(
                    c.area.as_str(),
                    "Extra check" | "Диск IOPS" | "Сеть" | "Технологии CPU"
                )
        })
        .count();
    let fails = checklist.iter().filter(|c| c.status == "fail").count();
    if hard_fails >= 2 || score < 40.0 {
        ("F".into(), "plan vs reality: broken".into())
    } else if (hard_fails >= 1 && score < 70.0) || fails >= 3 || score < 55.0 {
        ("D".into(), "plan vs reality: mismatch".into())
    } else if fails >= 1 || score < 75.0 {
        ("C".into(), "usable, with caveats".into())
    } else if score < 90.0 {
        ("B".into(), "mostly honest".into())
    } else {
        ("A".into(), "looks honest".into())
    }
}
