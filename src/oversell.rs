use crate::catalog;
use crate::cpu::CpuInfo;
use crate::stress::StressResult;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct OversellAnalysis {
    pub oversell_level: String,
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

pub fn analyze(_cpu: &CpuInfo, stress: Option<&StressResult>) -> OversellAnalysis {
    let mut reasons = vec!["Коэффициент перепродажи и число физических ядер недоступны из гостевой ОС".into()];
    let (level, confidence, steal, scaling) = if let Some(s) = stress {
        if let Some(v) = s.steal_percent_load {
            reasons.push(format!("Steal под многопоточной нагрузкой: {v:.2}% (среднее по vCPU гостя)"));
        } else { reasons.push("Steal под нагрузкой недоступен".into()); }
        reasons.push(format!("Масштабирование: {:.1}%; на него влияют SMT, частоты, квоты и характер теста", s.scaling_efficiency));
        let level = if s.steal_percent_load.map_or(false, |v| v > 3.0) { "обнаружены признаки задержки CPU" } else if s.steal_percent_load.is_some() { "признаки задержки CPU по steal не обнаружены в этом тесте" } else { "недостаточно данных о steal" };
        (level, "один краткий замер", s.steal_percent_load, Some((100.0-s.scaling_efficiency).max(0.0)))
    } else {
        reasons.push("Стресс-тест пропущен; данных о нагрузке недостаточно".into());
        ("недостаточно данных", "недостаточно данных", None, None)
    };
    OversellAnalysis { oversell_level: level.into(), reasons, confidence: confidence.into(), steal_impact: steal, scaling_penalty: scaling }
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

        
        let cat = catalog::match_model(&cpu.model).filter(|c| c.expected_st.is_some());
        let guest_tax = if cpu.virtualization.is_virtual { 0.82 } else { 1.0 };
        let best = if catalog::match_model(&cpu.model).is_some_and(|c| c.expected_st.is_none()) {
            nearest.clear();
            None
        } else if let Some(c) = cat {
            let expect = c.expected_st.unwrap_or_default() * guest_tax;
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
                
                Some(CpuMatch {
                    name: format!("{} (ниже каталожного ориентира)", c.name),
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
                    "{} (≈ {:.0}% ST по этому тесту)",
                    m.name,
                    (st / m.score_single.max(1.0)) * 100.0
                )
            })
            .unwrap_or_else(|| "недостаточно данных".into());

        estimated_tier = tier_from_scores(st, mt, cpu.logical_cores);
        commentary = format!(
            "st={st:.0} mt={mt:.0} статус={level}; \
             advertised={advertised}{cat} → {eq}",
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
        "низкий результат теста".into()
    } else if st < 70.0 {
        "результат ниже среднего".into()
    } else if st < 100.0 {
        "средний результат теста".into()
    } else if st < 118.0 {
        "результат выше среднего".into()
    } else if st < 138.0 {
        "высокий результат теста".into()
    } else if st < 160.0 {
        "высокий результат теста".into()
    } else {
        "высокий результат теста".into()
    }
}
