
use crate::catalog::{self, CatalogCpu};
use crate::cpu::CpuInfo;
use crate::diskio::DiskIoResult;
use crate::network::NetworkInfo;
use crate::probe::DeepProbe;
use crate::stress::StressResult;
use crate::system::SystemInfo;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct TruthReport {
    pub verdict: String,
    pub verdict_level: String,
    pub trust_score: f64,
    pub matched_catalog: Option<String>,
    pub expected_st: Option<f64>,
    pub measured_st: Option<f64>,
    pub st_ratio: Option<f64>,
    pub findings: Vec<Finding>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Finding {
    pub severity: String,
    pub code: String,
    pub message: String,
}

pub fn analyze(
    cpu: &CpuInfo,
    deep: &DeepProbe,
    sys: &SystemInfo,
    net: &NetworkInfo,
    stress: Option<&StressResult>,
    diskio: Option<&DiskIoResult>,
    tech: &crate::tech::TechReport,
    extras: Option<&crate::extras::ExtraProbe>,
) -> TruthReport {
    let mut findings = Vec::new();
    let mut trust: f64 = 100.0;

    let matched = catalog::match_model(&cpu.model);
    if let Some(c) = matched {
        findings.push(Finding {
            severity: "info".into(),
            code: "catalog_match".into(),
            message: format!(
                "Модель «{}» → каталог: {} ({})",
                cpu.model, c.name, c.gen
            ),
        });
        if c.expected_st.is_none() {
            findings.push(Finding {
                severity: "info".into(),
                code: "catalog_no_benchmark".into(),
                message: "Для этой модели нет эталонного результата nmos".into(),
            });
        }
    } else {
        findings.push(Finding {
            severity: "warn".into(),
            code: "catalog_unknown".into(),
            message: format!("Модель «{}» не в каталоге — сверка по имени ограничена", cpu.model),
        });
        trust -= 5.0;
    }

    let (expected_st, measured_st, st_ratio) = if let (Some(c), Some(s)) = (matched.filter(|c| c.expected_st.is_some()), stress) {
        let guest_tax = if cpu.virtualization.is_virtual { 0.82 } else { 1.0 };
        let expect = c.expected_st.unwrap_or_default() * guest_tax;
        let ratio = s.score_single / expect.max(1.0);
        check_perf(c, s, guest_tax, &mut findings, &mut trust);
        (Some(expect), Some(s.score_single), Some(ratio))
    } else if let Some(s) = stress {
        (None, Some(s.score_single), None)
    } else {
        findings.push(Finding {
            severity: "warn".into(),
            code: "no_stress".into(),
            message: "Без стресс-теста нет данных о производительности CPU".into(),
        });
        trust -= 15.0;
        (None, None, None)
    };

    if let Some(c) = matched {
        check_flags(c, deep, tech, &mut findings, &mut trust);
        check_cpu_fingerprint(cpu, c, deep, tech, stress, &mut findings, &mut trust);
    } else {
        check_tech_standalone(tech, &mut findings, &mut trust);
        check_cpu_fingerprint_unknown(cpu, deep, tech, stress, &mut findings, &mut trust);
    }
    check_frequency(cpu, deep, stress, &mut findings, &mut trust);
    check_cgroup(cpu, deep, sys, &mut findings, &mut trust);
    check_virt(cpu, deep, &mut findings, &mut trust);
    check_memory(sys, deep, &mut findings, &mut trust);

    if let Some(io) = diskio {
        check_disk(io, &mut findings, &mut trust);
    }

    if let Some(sp) = &net.speed {
        if let Some(best) = sp.best_download_mbps {
            if best < 50.0 {
                findings.push(Finding {
                    severity: "warn".into(),
                    code: "net_slow".into(),
                    message: format!(
                        "Download best ≈ {best:.0} Mbps — слабее типичного 100–1000M VDS"
                    ),
                });
                trust -= 8.0;
            } else if best >= 800.0 {
                findings.push(Finding {
                    severity: "info".into(),
                    code: "net_fast".into(),
                    message: format!("Download best ≈ {best:.0} Mbps — похоже на гигабит"),
                });
            }
        }
    }

    let ml = cpu.model.to_lowercase();
    if ml.contains("qemu") || ml.contains("common kvm") || ml.contains("virtual cpu") {
        findings.push(Finding {
            severity: "warn".into(),
            code: "generic_model".into(),
            message: "Модель CPU не указана гостевой системой".into(),
        });
        trust -= 10.0;
    }

    if tech.available {
        findings.push(Finding {
            severity: "info".into(),
            code: "isa_level".into(),
            message: format!("ISA: {} · {}", tech.isa_level, tech.generation_hint),
        });
        for m in &tech.missing_important {
            findings.push(Finding {
                severity: "warn".into(),
                code: "tech_missing".into(),
                message: format!("Нет технологии: {m}"),
            });
            trust -= 4.0;
        }
    }

    if let Some(ex) = extras {
        let extra_findings = crate::extras::analyze(ex, cpu, deep, sys, net, diskio, stress);
        let mut extras_penalty = 0.0_f64;
        const EXTRAS_PENALTY_CAP: f64 = 18.0;
        for ef in extra_findings {
            let add: f64 = match ef.severity.as_str() {
                "critical" => 8.0,
                "warn" => 2.5,
                _ => 0.0,
            };
            if add > 0.0 && extras_penalty < EXTRAS_PENALTY_CAP {
                let room = EXTRAS_PENALTY_CAP - extras_penalty;
                let take = add.min(room);
                extras_penalty += take;
                trust -= take;
            }
            findings.push(Finding {
                severity: ef.severity,
                code: ef.code,
                message: ef.message,
            });
        }
        if extras_penalty > 0.0 {
            findings.push(Finding {
                severity: "info".into(),
                code: "extras_penalty_cap".into(),
                message: format!(
                    "Штраф extras за Trust ограничен: −{extras_penalty:.0} (cap {EXTRAS_PENALTY_CAP})"
                ),
            });
        }
    }

    
    dedupe_st_stack(&mut findings, &mut trust);

    trust = trust.clamp(0.0, 100.0);
    let (verdict, verdict_level) = verdict_from(trust, &findings);
    let summary = build_summary(&verdict, matched.map(|c| c.name), st_ratio, trust, &findings);

    TruthReport {
        verdict,
        verdict_level,
        trust_score: trust,
        matched_catalog: matched.map(|c| c.name.to_string()),
        expected_st,
        measured_st,
        st_ratio,
        findings,
        summary,
    }
}

fn check_perf(
    c: &CatalogCpu,
    s: &StressResult,
    guest_tax: f64,
    findings: &mut Vec<Finding>,
    trust: &mut f64,
) {
    let expect = c.expected_st.unwrap_or_default() * guest_tax;
    let ratio = s.score_single / expect.max(1.0);
    let low = 1.0 - c.tol_low;
    let tax_note = if guest_tax < 1.0 { " (VM-tax)" } else { "" };

    if ratio < low * 0.55 {
        findings.push(Finding {
            severity: "critical".into(),
            code: "cpu_lie_severe".into(),
            message: format!(
                "ST {:.0} vs ожидание {:.0}{tax_note} для {} (ratio {:.0}%) — результат ниже ориентира; причина не установлена",
                s.score_single, expect, c.name, ratio * 100.0
            ),
        });
        *trust -= 45.0;
    } else if ratio < low {
        findings.push(Finding {
            severity: "critical".into(),
            code: "cpu_underspec".into(),
            message: format!(
                "ST {:.0} ниже эталона {:.0}{tax_note} для {} (ratio {:.0}%); причины неоднозначны",
                s.score_single, expect, c.name, ratio * 100.0
            ),
        });
        *trust -= 28.0;
    } else if ratio < 0.9 {
        findings.push(Finding {
            severity: "warn".into(),
            code: "cpu_soft_underspec".into(),
            message: format!(
                "ST {:.0} чуть ниже ожидаемого {:.0}{tax_note} для {} (ratio {:.0}%)",
                s.score_single, expect, c.name, ratio * 100.0
            ),
        });
        *trust -= 8.0;
    } else {
        findings.push(Finding {
            severity: "info".into(),
            code: "cpu_ok".into(),
            message: format!(
                "ST {:.0} ≈ каталог {} (ожидание {:.0}{tax_note}, ratio {:.0}%)",
                s.score_single, c.name, expect, ratio * 100.0
            ),
        });
    }

    if s.scaling_efficiency < 50.0 {
        findings.push(Finding {
            severity: "warn".into(),
            code: "mt_contention".into(),
            message: format!(
                "MT scaling {:.0}% — низкое масштабирование; причины неоднозначны",
                s.scaling_efficiency
            ),
        });
    } else if s.scaling_efficiency < 70.0 {
        findings.push(Finding {
            severity: "warn".into(),
            code: "mt_soft".into(),
            message: format!("MT scaling {:.0}% — сниженное масштабирование", s.scaling_efficiency),
        });
    }
}

fn check_flags(
    c: &CatalogCpu,
    deep: &DeepProbe,
    tech: &crate::tech::TechReport,
    findings: &mut Vec<Finding>,
    trust: &mut f64,
) {
    if !tech.available && deep.cpu_ids.flags_count == 0 {
        findings.push(Finding {
            severity: "info".into(),
            code: "flags_unavailable".into(),
            message: "CPU flags недоступны на этой ОС — проверка ISA пропущена".into(),
        });
        return;
    }

    let has = |id: &str| {
        crate::tech::has_flag(tech, id)
            || match id {
                "aes" => deep.cpu_ids.has_aes,
                "avx" => deep.cpu_ids.has_avx,
                "avx2" => deep.cpu_ids.has_avx2,
                "avx512f" => deep.cpu_ids.has_avx512,
                "sha_ni" => deep.cpu_ids.has_sha,
                _ => false,
            }
    };

    let gen = c.gen.to_lowercase();
    let modern = gen.contains("zen")
        || gen.contains("ice lake")
        || gen.contains("sapphire")
        || gen.contains("emerald")
        || gen.contains("alder")
        || gen.contains("raptor")
        || gen.contains("arrow")
        || gen.contains("cascade")
        || gen.contains("comet")
        || gen.contains("rocket")
        || gen.contains("coffee");

    if modern {
        if !has("aes") {
            findings.push(Finding {
                severity: "critical".into(),
                code: "flags_missing_aes".into(),
                message: "Нет AES-NI при современном CPU — флаги не представлены гостевой системе".into(),
            });
            *trust -= 25.0;
        }
        if !has("avx2")
            && (gen.contains("zen")
                || gen.contains("ice")
                || gen.contains("sapphire")
                || gen.contains("emerald")
                || gen.contains("alder")
                || gen.contains("raptor")
                || gen.contains("arrow")
                || gen.contains("rocket"))
        {
            findings.push(Finding {
                severity: "critical".into(),
                code: "flags_missing_avx2".into(),
                message: format!("Нет AVX2 при поколении «{}»", c.gen),
            });
            *trust -= 30.0;
        }
        if !has("fma")
            && (gen.contains("zen")
                || gen.contains("haswell")
                || gen.contains("broadwell")
                || gen.contains("alder")
                || gen.contains("raptor")
                || gen.contains("ice")
                || gen.contains("sapphire"))
        {
            findings.push(Finding {
                severity: "warn".into(),
                code: "flags_missing_fma".into(),
                message: format!("Флаг FMA3 отсутствует для модели «{}»", c.gen),
            });
            *trust -= 10.0;
        }
        if !has("bmi2")
            && (gen.contains("zen")
                || gen.contains("haswell")
                || gen.contains("alder")
                || gen.contains("raptor")
                || gen.contains("ice")
                || gen.contains("sapphire"))
        {
            findings.push(Finding {
                severity: "warn".into(),
                code: "flags_missing_bmi2".into(),
                message: "Нет BMI2 — у современного x86 почти всегда есть".into(),
            });
            *trust -= 8.0;
        }
    }

    
    if (gen.contains("zen2")
        || gen.contains("zen3")
        || gen.contains("zen4")
        || gen.contains("zen5")
        || gen.contains("rome")
        || gen.contains("milan")
        || gen.contains("genoa")
        || gen.contains("ice lake")
        || gen.contains("sapphire")
        || gen.contains("alder")
        || gen.contains("raptor")
        || gen.contains("arrow"))
        && !has("sha_ni")
        && !has("sha")
    {
        findings.push(Finding {
            severity: "warn".into(),
            code: "flags_missing_sha".into(),
            message: format!("Флаг SHA-NI отсутствует для «{}»", c.gen),
        });
        *trust -= 8.0;
    }

    
    let expects_avx512 = gen.contains("zen4")
        || gen.contains("zen5")
        || gen.contains("genoa")
        || gen.contains("bergamo")
        || gen.contains("turin")
        || (gen.contains("ice lake") && (c.name.contains("Xeon") || c.name.contains("EPYC")))
        || gen.contains("sapphire")
        || gen.contains("emerald");
    
    let zen_desktop_avx512 = (gen.contains("zen4") || gen.contains("zen5"))
        && (c.name.contains("Ryzen") || c.name.contains("EPYC") || c.name.contains("Threadripper"));
    if (expects_avx512 || zen_desktop_avx512) && !has("avx512f") {
        let guest = deep.cpu_ids.has_hypervisor_flag;
        findings.push(Finding {
            severity: if guest { "warn".into() } else { "critical".into() },
            code: "flags_no_avx512".into(),
            message: format!(
                "Модель «{}» ({}): флаг AVX-512 отсутствует{}",
                c.name,
                c.gen,
                if guest {
                    " (возможна маскировка гипервизором)"
                } else {
                    " — возможно маскирование"
                }
            ),
        });
        *trust -= if guest { 6.0 } else { 18.0 };
    }

    
    if (gen.contains("alder") || gen.contains("raptor") || gen.contains("arrow"))
        && c.name.contains("Core")
        && has("avx512f")
    {
        findings.push(Finding {
            severity: "info".into(),
            code: "flags_avx512_on_client".into(),
            message: "Флаг AVX-512 доступен при указанной модели CPU".into(),
        });
    }

    
    if tech.available {
        let isa = tech.isa_level.to_lowercase();
        if gen.contains("zen3") || gen.contains("zen4") || gen.contains("zen5") || gen.contains("alder") || gen.contains("raptor")
        {
            if !isa.contains("v3") && !isa.contains("v4") && !isa.contains("avx2") {
                findings.push(Finding {
                    severity: "critical".into(),
                    code: "isa_too_old".into(),
                    message: format!(
                        "Модель {}, набор инструкций гостевой системы: {}",
                        c.gen, tech.isa_level
                    ),
                });
                *trust -= 20.0;
            }
        }
        if (gen.contains("zen4") || gen.contains("zen5") || gen.contains("sapphire"))
            && !isa.contains("v4")
            && has("avx2")
        {
            let guest = deep.cpu_ids.has_hypervisor_flag;
            findings.push(Finding {
                severity: if guest { "info".into() } else { "warn".into() },
                code: "isa_not_v4".into(),
                message: format!(
                    "Ожидался x86-64-v4 для {}, факт: {}{}",
                    c.gen,
                    tech.isa_level,
                    if guest { " (гость — AVX-512 могли срезать)" } else { "" }
                ),
            });
            if !guest {
                *trust -= 8.0;
            }
        }
    }
}


fn check_cpu_fingerprint(
    cpu: &CpuInfo,
    c: &CatalogCpu,
    deep: &DeepProbe,
    tech: &crate::tech::TechReport,
    stress: Option<&StressResult>,
    findings: &mut Vec<Finding>,
    trust: &mut f64,
) {
    check_vendor_vs_name(cpu, findings, trust);
    check_virt_ext_vs_vendor(cpu, tech, findings, trust);
    check_named_cores(cpu, c, findings, trust);
    check_l3_vs_catalog(cpu, c, findings, trust);
    check_st_vs_isa(cpu, c, tech, stress, findings, trust);
    check_cpuid_vs_catalog(cpu, c, deep, findings, trust);
}

fn check_cpu_fingerprint_unknown(
    cpu: &CpuInfo,
    deep: &DeepProbe,
    tech: &crate::tech::TechReport,
    stress: Option<&StressResult>,
    findings: &mut Vec<Finding>,
    trust: &mut f64,
) {
    check_vendor_vs_name(cpu, findings, trust);
    check_virt_ext_vs_vendor(cpu, tech, findings, trust);
    
    if let Some(named) = parse_cores_from_model(&cpu.model) {
        if cpu.logical_cores > 0 && named > cpu.logical_cores * 2 {
            findings.push(Finding {
                severity: if cpu.virtualization.is_virtual {
                    "info".into()
                } else {
                    "warn".into()
                },
                code: "named_cores_slice".into(),
                message: format!(
                    "В названии указано {} ядер; гостевой системе доступно {} логических процессоров",
                    named, cpu.logical_cores
                ),
            });
        }
    }
    let _ = (deep, stress);
}

fn check_vendor_vs_name(cpu: &CpuInfo, findings: &mut Vec<Finding>, trust: &mut f64) {
    let m = cpu.model.to_lowercase();
    let v = cpu.vendor.to_lowercase();
    let name_amd = m.contains("amd") || m.contains("ryzen") || m.contains("epyc") || m.contains("threadripper");
    let name_intel = m.contains("intel") || m.contains("xeon") || m.contains("core i") || m.contains("core ultra");
    let vend_amd = v.contains("amd") || v.contains("authenticamd");
    let vend_intel = v.contains("intel") || v.contains("genuineintel");

    if name_amd && vend_intel {
        findings.push(Finding {
            severity: "critical".into(),
            code: "vendor_lie_amd_on_intel".into(),
            message: format!(
                "Имя «{}», а vendor={} — расхождение в данных CPU",
                cpu.model, cpu.vendor
            ),
        });
        *trust -= 40.0;
    } else if name_intel && vend_amd {
        findings.push(Finding {
            severity: "critical".into(),
            code: "vendor_lie_intel_on_amd".into(),
            message: format!(
                "Имя «{}», а vendor={} — расхождение в данных CPU",
                cpu.model, cpu.vendor
            ),
        });
        *trust -= 40.0;
    }
}

fn check_virt_ext_vs_vendor(
    cpu: &CpuInfo,
    tech: &crate::tech::TechReport,
    findings: &mut Vec<Finding>,
    trust: &mut f64,
) {
    let v = cpu.vendor.to_lowercase();
    let has_svm = crate::tech::has_flag(tech, "svm");
    let has_vmx = crate::tech::has_flag(tech, "vmx");
    if (v.contains("amd") || v.contains("authenticamd")) && has_vmx && !has_svm {
        findings.push(Finding {
            severity: "critical".into(),
            code: "vmx_on_amd".into(),
            message: "Vendor AMD, но есть VT-x (vmx) без SVM — флаги/имя врут".into(),
        });
        *trust -= 25.0;
    }
    if (v.contains("intel") || v.contains("genuineintel")) && has_svm && !has_vmx {
        findings.push(Finding {
            severity: "critical".into(),
            code: "svm_on_intel".into(),
            message: "Vendor Intel, но есть AMD-V (svm) без VMX — флаги/имя врут".into(),
        });
        *trust -= 25.0;
    }
}

fn parse_cores_from_model(model: &str) -> Option<usize> {
    let m = model.to_lowercase();
    
    for (i, _) in m.match_indices("-core") {
        let prefix = &m[..i];
        let num: String = prefix
            .chars()
            .rev()
            .take_while(|c| c.is_ascii_digit())
            .collect::<String>()
            .chars()
            .rev()
            .collect();
        if let Ok(n) = num.parse::<usize>() {
            if (2..=256).contains(&n) {
                return Some(n);
            }
        }
    }
    None
}

fn check_named_cores(
    cpu: &CpuInfo,
    c: &CatalogCpu,
    findings: &mut Vec<Finding>,
    trust: &mut f64,
) {
    let named = parse_cores_from_model(&cpu.model).unwrap_or(c.cores_typical);
    let logical = cpu.logical_cores.max(1);
    if named >= 8 && logical * 3 < named {
        let sev = if cpu.virtualization.is_virtual {
            "info"
        } else {
            "warn"
        };
        findings.push(Finding {
            severity: sev.into(),
            code: "vcpu_slice".into(),
            message: format!(
                "В названии указано {named} ядер (каталог {}); гостевой системе доступно {} логических процессоров",
                c.name, logical
            ),
        });
        if !cpu.virtualization.is_virtual {
            *trust -= 8.0;
        }
    }
}

fn check_l3_vs_catalog(
    cpu: &CpuInfo,
    c: &CatalogCpu,
    findings: &mut Vec<Finding>,
    trust: &mut f64,
) {
    let Some(l3_kb) = cpu.cache_l3_kb else {
        return;
    };
    let l3_mb = l3_kb as f64 / 1024.0;
    let gen = c.gen.to_lowercase();
    let name = c.name.to_lowercase();
    let guest = cpu.virtualization.is_virtual;

    
    let min_mb = if guest {
        if name.contains("epyc") || name.contains("xeon") || name.contains("ryzen 9") {
            8.0
        } else if name.contains("ryzen") {
            4.0
        } else {
            return;
        }
    } else if name.contains("9950") || name.contains("7950") || name.contains("5950") {
        32.0
    } else if name.contains("ryzen 9") || name.contains("threadripper") {
        24.0
    } else if name.contains("ryzen 7") || name.contains("x3d") {
        16.0
    } else if name.contains("epyc") && (gen.contains("zen3") || gen.contains("zen4") || gen.contains("zen5"))
    {
        64.0
    } else if name.contains("xeon") && (gen.contains("ice") || gen.contains("sapphire")) {
        24.0
    } else {
        return;
    };

    if l3_mb + 0.5 < min_mb * 0.45 {
        findings.push(Finding {
            severity: if guest { "info".into() } else { "warn".into() },
            code: "l3_too_small".into(),
            message: format!(
                "L3 ≈ {l3_mb:.0} MB для модели {} (ориентир ≥{:.0} MB){}",
                c.name,
                min_mb,
                if guest {
                    " — гостевой системе может быть доступна часть кэша"
                } else {
                    " — shared LLC / урезанный"
                }
            ),
        });
        if !guest {
            *trust -= 10.0;
        }
    } else if l3_mb + 0.5 < min_mb {
        findings.push(Finding {
            severity: "info".into(),
            code: "l3_partial".into(),
            message: format!(
                "L3 ≈ {l3_mb:.0} MB < типичных {:.0} MB для {}",
                min_mb, c.name
            ),
        });
    } else {
        findings.push(Finding {
            severity: "info".into(),
            code: "l3_ok".into(),
            message: format!("L3 ≈ {l3_mb:.0} MB согласуется с классом {}", c.name),
        });
    }
}

fn check_st_vs_isa(
    cpu: &CpuInfo,
    c: &CatalogCpu,
    tech: &crate::tech::TechReport,
    stress: Option<&StressResult>,
    findings: &mut Vec<Finding>,
    trust: &mut f64,
) {
    let Some(s) = stress else { return };
    let gen = c.gen.to_lowercase();
    let modern_isa = tech.isa_level.to_lowercase().contains("v4")
        || tech.isa_level.to_lowercase().contains("v3")
        || crate::tech::has_flag(tech, "avx512f");

    
    if modern_isa && s.score_single < 45.0 && (gen.contains("zen") || gen.contains("alder") || gen.contains("raptor") || gen.contains("ice") || gen.contains("sapphire"))
    {
        findings.push(Finding {
            severity: "critical".into(),
            code: "isa_st_mismatch".into(),
            message: format!(
                "ISA современный ({}), а ST всего {:.0} — результат ниже ориентира; причина не установлена",
                tech.isa_level, s.score_single
            ),
        });
        *trust -= 20.0;
    }

    
    if (gen.contains("ivy") || gen.contains("haswell") || gen.contains("broadwell"))
        && crate::tech::has_flag(tech, "avx512f")
        && s.score_single > 90.0
    {
        findings.push(Finding {
            severity: "critical".into(),
            code: "old_name_new_silicon".into(),
            message: format!(
                "Модель «{}» ({}), AVX-512 доступен, ST {:.0}; данные отличаются от каталога",
                c.name, c.gen, s.score_single
            ),
        });
        *trust -= 18.0;
    }

    let _ = cpu;
}

fn check_cpuid_vs_catalog(
    cpu: &CpuInfo,
    c: &CatalogCpu,
    deep: &DeepProbe,
    findings: &mut Vec<Finding>,
    trust: &mut f64,
) {
    let Some(family) = deep.cpu_ids.family else {
        return;
    };
    let gen = c.gen.to_lowercase();
    let v = cpu.vendor.to_lowercase();

    if v.contains("amd") || v.contains("authenticamd") {
        
        if gen.contains("zen5") && family != 0x1A {
            findings.push(Finding {
                severity: "critical".into(),
                code: "cpuid_family_mismatch".into(),
                message: format!(
                    "Каталог Zen5, но CPUID family={family:#x} (ждали 1Ah) — расхождение в данных CPU",
                ),
            });
            *trust -= 30.0;
        } else if (gen.contains("zen4") || gen.contains("zen3") || gen.contains("milan") || gen.contains("genoa"))
            && family != 0x19
            && family != 0x1A
        {
            
            if family == 0x17 || family < 0x17 {
                findings.push(Finding {
                    severity: "critical".into(),
                    code: "cpuid_family_old".into(),
                    message: format!(
                        "Модель «{}», CPUID family={family:#x} — данные отличаются от каталога",
                        c.name
                    ),
                });
                *trust -= 28.0;
            }
        } else if gen.contains("zen2") && family != 0x17 && family < 0x17 {
            findings.push(Finding {
                severity: "warn".into(),
                code: "cpuid_family_unexpected".into(),
                message: format!("Zen2 в каталоге, family={family:#x}"),
            });
            *trust -= 10.0;
        } else if gen.contains("zen5") && family == 0x1A {
            findings.push(Finding {
                severity: "info".into(),
                code: "cpuid_family_ok".into(),
                message: "CPUID family 1Ah согласован с Zen5".into(),
            });
        }
    }

    if v.contains("intel") || v.contains("genuineintel") {
        if let Some(model) = deep.cpu_ids.model {
            let ok = match () {
                _ if gen.contains("alder") => matches!(model, 0x97 | 0x9A | 0xBE),
                _ if gen.contains("raptor") && gen.contains("refresh") => {
                    matches!(model, 0xB7 | 0xBA | 0xC6 | 0xBD | 0xBF)
                }
                _ if gen.contains("raptor") => matches!(model, 0xB7 | 0xBA | 0xBF),
                _ if gen.contains("sapphire") => model == 0x8F,
                _ if gen.contains("ice lake") => matches!(model, 0x6A | 0x6C),
                _ if gen.contains("cascade") || gen.contains("skylake") => model == 0x55,
                _ if gen.contains("arrow") => matches!(model, 0xC6 | 0xBD | 0xA6 | 0xC5),
                _ => true,
            };
            if !ok && (gen.contains("alder") || gen.contains("raptor") || gen.contains("sapphire") || gen.contains("ice lake"))
            {
                findings.push(Finding {
                    severity: "warn".into(),
                    code: "intel_model_id_mismatch".into(),
                    message: format!(
                        "Поколение «{}», CPUID model={model:#x} — данные могут отличаться",
                        c.gen
                    ),
                });
                *trust -= 12.0;
            }
        }
    }
}

fn check_tech_standalone(
    tech: &crate::tech::TechReport,
    findings: &mut Vec<Finding>,
    trust: &mut f64,
) {
    if !tech.available {
        return;
    }
    for n in &tech.notes {
        if n.contains("подозрительно") || n.contains("AVX-512 доступен, а флаг AVX2 отсутствует") || n.contains("поврежд") {
            findings.push(Finding {
                severity: "warn".into(),
                code: "tech_anomaly".into(),
                message: n.clone(),
            });
            *trust -= 6.0;
        }
    }
}

fn check_frequency(
    cpu: &CpuInfo,
    deep: &DeepProbe,
    stress: Option<&StressResult>,
    findings: &mut Vec<Finding>,
    trust: &mut f64,
) {
    let fa = &cpu.freq_analysis;
    let guest = cpu.virtualization.is_virtual || deep.cpu_ids.has_hypervisor_flag;

    if !fa.cpufreq_available {
        findings.push(Finding {
            severity: if guest { "info".into() } else { "warn".into() },
            code: "no_cpufreq".into(),
            message: if guest {
                "Нет cpufreq в гостевой системе; частота может быть недоступна"
                    .into()
            } else {
                "Нет cpufreq; частота процессора не подтверждена".into()
            },
        });
        if !guest {
            *trust -= 5.0;
        }
    }

    if fa.round_ghz_suspect || fa.sticky_min_eq_max {
        findings.push(Finding {
            severity: if guest { "info".into() } else { "warn".into() },
            code: "freq_fake_nominal".into(),
            message: format!(
                "Частота «липкая»/круглая (min={:?} max={:?} cur={:?}) — {}",
                cpu.freq_mhz_min,
                cpu.freq_mhz_max,
                cpu.freq_mhz_current_avg,
                if guest {
                    "значение, представленное гипервизором"
                } else {
                    "источник частоты не обнаружен"
                }
            ),
        });
        if !guest {
            *trust -= 10.0;
        }
    }

    if fa.all_cores_same {
        findings.push(Finding {
            severity: "info".into(),
            code: "freq_all_same".into(),
            message: "Все vCPU показывают одну и ту же МГц — норма для VDS, не значит «выделенный»"
                .into(),
        });
    }

    if let (Some(max), Some(cur_info)) = (cpu.freq_mhz_max, deep.cpu_ids.cpu_mhz_cpuinfo) {
        if cur_info > max as f64 * 1.25 {
            findings.push(Finding {
                severity: "warn".into(),
                code: "freq_inconsistent".into(),
                message: format!(
                    "cpuinfo MHz ({cur_info:.0}) > max ({max}) — странная калибровка"
                ),
            });
            *trust -= 5.0;
        }
    }

    if let Some(max) = cpu.freq_mhz_max {
        if max > 0 && max < 800 {
            findings.push(Finding {
                severity: "warn".into(),
                code: "freq_too_low".into(),
                message: format!("Max {max} МГц — слишком низко для серверного CPU"),
            });
            *trust -= 10.0;
        }
    }

    
    
    if let Some(s) = stress {
        if let (Some(idle), Some(load)) = (s.freq_idle_mhz, s.freq_load_mhz) {
            let delta = (load - idle).abs();
            findings.push(Finding {
                severity: "info".into(),
                code: "freq_load_sample".into(),
                message: format!(
                    "Частота idle≈{idle:.0} → load≈{load:.0} МГц (Δ {delta:.0})",
                ),
            });
            if delta < 15.0 && fa.sticky_min_eq_max {
                findings.push(Finding {
                    severity: "info".into(),
                    code: "freq_no_turbo".into(),
                    message: if guest {
                        "Под нагрузкой МГц не выросли — в госте turbo часто не виден; смотрите ST score"
                            .into()
                    } else {
                        "Под нагрузкой частота не выросла — turbo не виден или его нет".into()
                    },
                });
            }
        } else {
            findings.push(Finding {
                severity: "info".into(),
                code: "freq_unreadable_load".into(),
                message: "Не удалось прочитать МГц под нагрузкой — опирайтесь на ST score".into(),
            });
        }

        
        if let Some(mhz) = cpu.freq_mhz_max.or(cpu.freq_mhz_reported) {
            if mhz >= 2800 {
                let expected_floor = if cpu.model.to_lowercase().contains("epyc")
                    || cpu.model.to_lowercase().contains("xeon")
                {
                    55.0
                } else {
                    70.0
                };
                if s.score_single < expected_floor * 0.7 {
                    findings.push(Finding {
                        severity: "critical".into(),
                        code: "freq_perf_mismatch".into(),
                        message: format!(
                            "Указанная частота ≈{mhz} МГц; результат ST {:.0} ниже ориентира",
                            s.score_single
                        ),
                    });
                    *trust -= 20.0;
                }
            }
        }
    }

    for n in &fa.notes {
        if n.starts_with("Важно:") {
            findings.push(Finding {
                severity: "info".into(),
                code: "freq_note".into(),
                message: n.clone(),
            });
        }
    }
}

fn check_cgroup(
    cpu: &CpuInfo,
    deep: &DeepProbe,
    sys: &SystemInfo,
    findings: &mut Vec<Finding>,
    trust: &mut f64,
) {
    if let Some(q) = deep.cgroup.cpu_quota_cores {
        let logical = cpu.logical_cores as f64;
        if q + 0.15 < logical {
            findings.push(Finding {
                severity: "critical".into(),
                code: "cgroup_cpu_lie".into(),
                message: format!(
                    "{} лог. CPU, но cgroup quota ≈ {:.2} — CPU режут квотой",
                    cpu.logical_cores, q
                ),
            });
            *trust -= 35.0;
        } else {
            findings.push(Finding {
                severity: "info".into(),
                code: "cgroup_cpu_ok".into(),
                message: format!("cgroup CPU quota ≈ {q:.2} ядер"),
            });
        }
    }

    if let Some(mem_max) = deep.cgroup.memory_max_mb {
        let total = sys.ram_total_mb;
        if mem_max + 64 < total && mem_max < total * 90 / 100 {
            findings.push(Finding {
                severity: "warn".into(),
                code: "cgroup_mem_cap".into(),
                message: format!(
                    "cgroup memory.max ≈ {mem_max} MB при MemTotal {total} MB"
                ),
            });
            *trust -= 8.0;
        }
    }

    if let Some(ref cpus) = deep.cgroup.cpuset_cpus {
        findings.push(Finding {
            severity: "info".into(),
            code: "cpuset".into(),
            message: format!("cpuset.cpus = {cpus}"),
        });
    }
}

fn check_virt(cpu: &CpuInfo, deep: &DeepProbe, findings: &mut Vec<Finding>, trust: &mut f64) {
    if let Some(steal) = cpu.steal_percent {
        if steal > 15.0 {
            findings.push(Finding {
                severity: "critical".into(),
                code: "steal_high".into(),
                message: format!("CPU steal {steal:.1}% — хост перегружен"),
            });
            *trust -= 25.0;
        } else if steal > 5.0 {
            findings.push(Finding {
                severity: "warn".into(),
                code: "steal_mid".into(),
                message: format!("CPU steal {steal:.1}% — задержка vCPU за интервал замера"),
            });
            *trust -= 12.0;
        }
    }

    if !deep.virtio_devices.is_empty() {
        findings.push(Finding {
            severity: "info".into(),
            code: "virtio".into(),
            message: format!("virtio устройств: {}", deep.virtio_devices.len()),
        });
    }

    for e in &deep.evidence_extra {
        findings.push(Finding {
            severity: "info".into(),
            code: "deep_evidence".into(),
            message: e.clone(),
        });
    }
}

fn check_memory(sys: &SystemInfo, deep: &DeepProbe, findings: &mut Vec<Finding>, trust: &mut f64) {
    if let (Some(commit), Some(limit)) = (
        deep.mem_detail.committed_as_mb,
        deep.mem_detail.commit_limit_mb,
    ) {
        if limit > 0 && commit as f64 > limit as f64 * 0.95 {
            findings.push(Finding {
                severity: "warn".into(),
                code: "mem_overcommit".into(),
                message: format!(
                    "Committed_AS {commit} ≈ CommitLimit {limit} MB — overcommit"
                ),
            });
            *trust -= 6.0;
        }
    }
    if sys.ram_total_mb < 256 {
        findings.push(Finding {
            severity: "critical".into(),
            code: "ram_tiny".into(),
            message: format!("Объём RAM: {} MB", sys.ram_total_mb),
        });
        *trust -= 15.0;
    }
}

fn check_disk(io: &DiskIoResult, findings: &mut Vec<Finding>, trust: &mut f64) {
    if let Some(w) = io.write_mbps {
        if w < 30.0 {
            findings.push(Finding {
                severity: "warn".into(),
                code: "disk_slow_write".into(),
                message: format!("Запись ≈ {w:.0} MB/s — низкая скорость записи в этом тесте"),
            });
            *trust -= 10.0;
        } else if w >= 400.0 {
            findings.push(Finding {
                severity: "info".into(),
                code: "disk_fast".into(),
                message: format!("Запись ≈ {w:.0} MB/s — SSD/NVMe"),
            });
        }
    }
    if let Some(lat) = io.fsync_latency_us_avg {
        if lat > 5000.0 {
            findings.push(Finding {
                severity: "warn".into(),
                code: "fsync_slow".into(),
                message: format!("fsync ≈ {lat:.0} µs — медленные sync-записи"),
            });
            *trust -= 8.0;
        }
    }
}

fn verdict_from(trust: f64, findings: &[Finding]) -> (String, String) {
    let hard_lie = findings.iter().any(|f| {
        matches!(
            f.code.as_str(),
            "cpu_lie_severe"
                | "cgroup_cpu_lie"
                | "flags_missing_avx2"
                | "flags_missing_aes"
                | "vendor_lie_amd_on_intel"
                | "vendor_lie_intel_on_amd"
                | "root_network_fs"
                | "openvz_like"
        )
    });
    
    let hard_critical = findings.iter().any(|f| {
        f.severity == "critical"
            && !matches!(
                f.code.as_str(),
                "flags_no_avx512" | "isa_not_v4" | "l3_too_small" | "tech_missing"
            )
    });

    if hard_lie || (hard_critical && trust < 40.0) {
        (
            "Обнаружены отклонения измерений от ориентиров".into(),
            "lie".into(),
        )
    } else if trust < 55.0 || (hard_critical && trust < 70.0) {
        (
            "Обнаружены отклонения от ориентиров".into(),
            "suspect".into(),
        )
    } else if trust < 80.0 {
        ("В целом похоже на правду, с оговорками".into(), "ok".into())
    } else {
        ("Существенных отклонений от ориентиров не обнаружено".into(), "ok".into())
    }
}


fn dedupe_st_stack(findings: &mut Vec<Finding>, trust: &mut f64) {
    let has_main = findings.iter().any(|f| {
        matches!(
            f.code.as_str(),
            "cpu_underspec" | "cpu_lie_severe" | "cpu_soft_underspec"
        )
    });
    if !has_main {
        return;
    }
    let stack = ["isa_st_mismatch", "freq_perf_mismatch"];
    let mut refund = 0.0;
    for f in findings.iter_mut() {
        if stack.contains(&f.code.as_str()) && f.severity != "info" {
            if f.code == "isa_st_mismatch" {
                refund += 12.0;
            } else {
                refund += 15.0;
            }
            f.severity = "info".into();
            f.message = format!("{} (учтено в ST vs каталог)", f.message);
        }
    }
    *trust = (*trust + refund).min(100.0);
}

fn build_summary(
    verdict: &str,
    matched: Option<&str>,
    st_ratio: Option<f64>,
    trust: f64,
    findings: &[Finding],
) -> String {
    let crit = findings.iter().filter(|f| f.severity == "critical").count();
    let warn = findings.iter().filter(|f| f.severity == "warn").count();
    format!(
        "{verdict}. Trust {:.0}/100. Каталог: {}. ST ratio: {}. Critical: {crit}, warn: {warn}.",
        trust,
        matched.unwrap_or("—"),
        st_ratio
            .map(|r| format!("{:.0}%", r * 100.0))
            .unwrap_or_else(|| "—".into())
    )
}
