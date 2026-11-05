use crate::catalog;
use crate::cpu::CpuInfo;
use crate::stress::StressResult;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct OversellAnalysis {
    pub oversell_factor: f64,
    pub oversell_level: String,
    pub effective_cores: f64,
    pub performance_ratio: f64,
    pub steal_impact: Option<f64>,
    pub scaling_penalty: Option<f64>,
    pub reasons: Vec<String>,
    pub confidence: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CpuEstimate {
    pub advertised_model: String,
    pub catalog_match: Option<String>,
    pub estimated_equivalent: String,
    pub estimated_tier: String,
    pub score_single: Option<f64>,
    pub score_multi: Option<f64>,
    pub nearest_matches: Vec<CpuMatch>,
    pub commentary: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CpuMatch {
    pub name: String,
    pub cores: usize,
    pub score_single: f64,
    pub score_multi: f64,
    pub distance: f64,
}

pub fn analyze(cpu: &CpuInfo, stress: Option<&StressResult>) -> OversellAnalysis {
    let mut reasons = Vec::new();
    // Оверселл = делёжка ядер с соседями (steal / плохое MT scaling).
    // ST ниже паспорта — это underclock/VM-tax/подмена модели, НЕ оверселл ядер.
    let mut factor = 1.0;
    let mut performance_ratio = 1.0;
    let mut scaling_penalty = None;

    if cpu.virtualization.is_virtual {
        reasons.push(format!(
            "Обнаружена виртуализация ({}) — само по себе не оверселл",
            cpu.virtualization.virt_type
        ));
    }

    if let Some(steal) = cpu.steal_percent {
        if steal > 8.0 {
            reasons.push(format!(
                "CPU steal ≈ {steal:.1}% — сильный oversell на хосте"
            ));
            factor *= 1.0 + (steal / 100.0) * 2.5;
        } else if steal > 3.0 {
            reasons.push(format!(
                "CPU steal ≈ {steal:.1}% — заметный shared/oversell"
            ));
            factor *= 1.0 + (steal / 100.0) * 1.8;
        } else if steal > 1.0 {
            reasons.push(format!("CPU steal ≈ {steal:.1}% — лёгкий шум соседей"));
            factor *= 1.0 + (steal / 100.0) * 1.2;
        } else {
            reasons.push(format!("CPU steal низкий ({steal:.2}%) — хост не отбирает время"));
        }
    }

    if let Some(s) = stress {
        let eff = s.scaling_efficiency;
        scaling_penalty = Some((100.0 - eff).max(0.0));

        if eff < 50.0 {
            reasons.push(format!(
                "Масштабирование потоков слабое ({eff:.0}%) — сильный contention/оверселл ядер"
            ));
            // 40% → ~2.0×, 50% → ~1.7×
            factor *= (95.0 / eff.max(25.0)).clamp(1.15, 4.0);
        } else if eff < 70.0 {
            reasons.push(format!(
                "Масштабирование снижено ({eff:.0}%) — возможен shared CPU"
            ));
            factor *= (90.0 / (eff + 5.0)).clamp(1.05, 2.2);
        } else if eff < 85.0 {
            reasons.push(format!(
                "Масштабирование чуть ниже идеала ({eff:.0}%) — слабый contention"
            ));
            factor *= 1.0 + (85.0 - eff) / 200.0; // max ~1.075
        } else {
            reasons.push(format!(
                "Масштабирование хорошее ({eff:.0}%) — vCPU скорее выделенные / мало соседей"
            ));
        }

        // Perf ratio — отдельно, почти не крутит oversell-factor
        if let Some(cat) = catalog::match_model(&cpu.model) {
            let guest_tax = if cpu.virtualization.is_virtual { 0.82 } else { 1.0 };
            let expect = cat.expected_st * guest_tax;
            performance_ratio = s.score_single / expect.max(1.0);
            let low = 1.0 - cat.tol_low;
            if performance_ratio < low {
                reasons.push(format!(
                    "ST ниже каталога {} (ожидание {:.0}{}, факт {:.0}, ratio {:.2}) — \
                     это не оверселл ядер, а слабее паспорт / VM-tax / cap",
                    cat.name,
                    expect,
                    if guest_tax < 1.0 { " с учётом VM" } else { "" },
                    s.score_single,
                    performance_ratio
                ));
                // Только если ещё и scaling плохой — чуть усилим фактор
                if eff < 70.0 && performance_ratio < 0.55 {
                    factor *= 1.0 + (0.55 - performance_ratio).clamp(0.0, 0.35);
                    reasons.push(
                        "ST сильно просел вместе с плохим scaling — похоже на перегруженную ноду"
                            .into(),
                    );
                }
            } else {
                reasons.push(format!(
                    "ST соответствует каталогу {} (ratio {:.2}{})",
                    cat.name,
                    performance_ratio,
                    if guest_tax < 1.0 { ", VM-tax" } else { "" }
                ));
            }
        } else if let Some(mhz) = cpu.freq_mhz_max.or(cpu.freq_mhz_reported) {
            let expected_st = (mhz as f64 / 3500.0) * 125.0;
            performance_ratio = s.score_single / expected_st.max(1.0);
            if performance_ratio < 0.55 {
                reasons.push(format!(
                    "Однопоток ниже ожидаемого для {mhz} МГц (ratio {performance_ratio:.2}) — \
                     смотрите steal/scaling, не только МГц"
                ));
            } else if performance_ratio < 0.8 {
                reasons.push(format!(
                    "Однопоток чуть ниже номинала частоты (ratio {performance_ratio:.2})"
                ));
            } else {
                reasons.push(format!(
                    "Однопоток соответствует частоте (ratio {performance_ratio:.2})"
                ));
            }
        }

        let model_l = cpu.model.to_lowercase();
        if (model_l.contains("xeon") || model_l.contains("epyc"))
            && s.score_single < 55.0
            && eff < 75.0
        {
            reasons.push(
                "Серверный паспорт + низкий ST + слабый scaling — типичный shared VPS".into(),
            );
            factor *= 1.12;
        }

        // Итог: хорошие ядра → не раздуваем
        if eff >= 90.0 && cpu.steal_percent.unwrap_or(0.0) < 2.0 {
            factor = factor.min(1.12);
            if factor <= 1.12 {
                reasons.push(
                    "Steal низкий и scaling ≥90% → оверселл ядер минимальный (ST отдельно)"
                        .into(),
                );
            }
        }
    } else {
        reasons.push("Стресс-тест пропущен — оверселл только по steal (если есть)".into());
        if cpu.virtualization.is_virtual && cpu.steal_percent.is_none() {
            reasons.push("Без стресса и steal фактор не завышаем".into());
        }
    }

    factor = factor.clamp(1.0, 12.0);
    let effective_cores = cpu.logical_cores as f64 / factor;

    let oversell_level = match factor {
        f if f < 1.12 => "минимальный / отсутствует".into(),
        f if f < 1.35 => "лёгкий".into(),
        f if f < 2.0 => "средний".into(),
        f if f < 3.5 => "высокий".into(),
        _ => "критический".into(),
    };

    let confidence = if stress.is_some() {
        if cpu.steal_percent.is_some() {
            "средняя–высокая"
        } else {
            "средняя"
        }
    } else {
        "низкая"
    }
    .to_string();

    OversellAnalysis {
        oversell_factor: factor,
        oversell_level,
        effective_cores,
        performance_ratio,
        steal_impact: cpu.steal_percent,
        scaling_penalty,
        reasons,
        confidence,
    }
}

pub fn estimate_real_cpu(
    cpu: &CpuInfo,
    stress: Option<&StressResult>,
    oversell: &OversellAnalysis,
) -> CpuEstimate {
    let advertised = cpu.model.clone();
    let catalog_match = catalog::match_model(&cpu.model).map(|c| c.name.to_string());

    let (st, mt) = match stress {
        Some(s) => (Some(s.score_single), Some(s.score_multi)),
        None => (None, None),
    };

    let mut nearest = Vec::new();
    let estimated_equivalent;
    let estimated_tier;
    let commentary;

    if let (Some(st), Some(mt)) = (st, mt) {
        let mut scored: Vec<CpuMatch> = catalog::benchmark_refs()
            .into_iter()
            .map(|(name, cores, rst, rmt)| {
                let r_mt_per = rmt / cores.max(1) as f64;
                let m_mt_per = mt / cpu.logical_cores.max(1) as f64;
                // На VDS с 2 vCPU не штрафуем за несовпадение 16-ядерного паспорта
                let core_pen = if cpu.logical_cores <= 4 && cores >= 8 {
                    0.05
                } else {
                    0.35
                };
                let dist = (rst - st).abs() * 2.0
                    + (r_mt_per - m_mt_per).abs() * 1.5
                    + (cores as f64 - cpu.logical_cores as f64).abs() * core_pen;
                CpuMatch {
                    name: name.to_string(),
                    cores,
                    score_single: rst,
                    score_multi: rmt,
                    distance: dist,
                }
            })
            .collect();
        scored.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap());
        nearest = scored.into_iter().take(8).collect();

        // Если паспорт совпал с каталогом и ST в допуске (с VM-tax) — это и есть эквивалент
        let cat = catalog::match_model(&cpu.model);
        let guest_tax = if cpu.virtualization.is_virtual { 0.82 } else { 1.0 };
        let best = if let Some(c) = cat {
            let expect = c.expected_st * guest_tax;
            let ratio = st / expect.max(1.0);
            if ratio >= (1.0 - c.tol_low) {
                Some(CpuMatch {
                    name: format!("{} (vCPU slice)", c.name),
                    cores: cpu.logical_cores.max(1),
                    score_single: expect,
                    score_multi: expect * cpu.logical_cores as f64 * 0.9,
                    distance: 0.0,
                })
            } else if ratio >= 0.55 {
                // Флаги/имя похожи, ST чуть ниже — не скатываемся в Zen3 только из-за 2 ядер
                Some(CpuMatch {
                    name: format!("{} (слабее паспорта / VM)", c.name),
                    cores: cpu.logical_cores.max(1),
                    score_single: expect,
                    score_multi: st * cpu.logical_cores as f64,
                    distance: 0.5,
                })
            } else {
                nearest.first().cloned()
            }
        } else {
            nearest.first().cloned()
        };

        // поднимем каталожный матч в топ списка
        if let Some(ref b) = best {
            if !nearest.iter().any(|n| n.name == b.name) {
                nearest.insert(0, b.clone());
                nearest.truncate(8);
            } else {
                nearest.retain(|n| n.name != b.name);
                nearest.insert(0, b.clone());
            }
        }
        estimated_equivalent = best
            .as_ref()
            .map(|m| {
                format!(
                    "{} (≈ {:.0}% ST / эфф. ядер ≈ {:.1})",
                    m.name,
                    (st / m.score_single.max(1.0)) * 100.0,
                    oversell.effective_cores
                )
            })
            .unwrap_or_else(|| "недостаточно данных".into());

        estimated_tier = tier_from_scores(st, mt, cpu.logical_cores);
        commentary = format!(
            "st={st:.0} mt={mt:.0} oversell={factor:.2}x ({level}); \
             advertised={advertised}{cat} → {eq}",
            factor = oversell.oversell_factor,
            level = oversell.oversell_level,
            cat = catalog_match
                .as_ref()
                .map(|c| format!(" [{c}]"))
                .unwrap_or_default(),
            eq = best.map(|m| m.name).unwrap_or_else(|| "?".into()),
        );
    } else {
        estimated_equivalent = if cpu.virtualization.is_virtual {
            format!("virtual ({advertised}), need stress")
        } else {
            advertised.clone()
        };
        estimated_tier = "unknown (no stress)".into();
        commentary = "drop --skip-stress to estimate cpu equivalent".into();
    }

    CpuEstimate {
        advertised_model: advertised,
        catalog_match,
        estimated_equivalent,
        estimated_tier,
        score_single: st,
        score_multi: mt,
        nearest_matches: nearest,
        commentary,
    }
}

fn tier_from_scores(st: f64, mt: f64, threads: usize) -> String {
    let per = mt / threads.max(1) as f64;
    if st < 40.0 || per < 35.0 {
        "бюджетный shared / устаревший серверный".into()
    } else if st < 70.0 {
        "средний VPS / старый Xeon-класс".into()
    } else if st < 100.0 {
        "хороший выделенный vCPU (EPYC/современный Xeon)".into()
    } else if st < 118.0 {
        "десктоп-класс (Ryzen 3000 / Comet Lake)".into()
    } else if st < 138.0 {
        "высокий класс (Zen3–Zen5 / Ryzen 5000+ на VDS)".into()
    } else if st < 160.0 {
        "топ-класс VDS (Zen4/Zen5 / Alder Lake+)".into()
    } else {
        "ультра топ (desktop-level ST)".into()
    }
}
