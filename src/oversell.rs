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
            reasons.push(format!("Steal под многопоточной нагрузкой: {v:.2}% (медиана трёх замеров)"));
        } else { reasons.push("Steal под нагрузкой недоступен".into()); }
        reasons.push(format!("Масштабирование: {:.1}%; на него влияют SMT, частоты, квоты и характер теста", s.scaling_efficiency));
        reasons.push(format!("Разброс: один поток {:.1}%, все потоки {:.1}%", s.single_variation_percent, s.multi_variation_percent));
        let throttle = s.cgroup_multi.as_ref().and_then(|d| {
            if d.nr_periods == 0 { None } else { Some(d.nr_throttled as f64 / d.nr_periods as f64 * 100.0) }
        });
        if let Some(v) = throttle { reasons.push(format!("Ограничение cgroup сработало в {v:.1}% периодов многопоточного теста")); }
        let unstable = s.single_variation_percent > 15.0 || s.multi_variation_percent > 15.0;
        let level = if throttle.is_some_and(|v| v > 10.0) {
            "обнаружено ограничение CPU по cgroup"
        } else if s.steal_percent_load.is_some_and(|v| v > 5.0) {
            "есть признаки задержек CPU по steal"
        } else if unstable {
            "результат нестабилен; повторите проверку"
        } else if s.steal_percent_load.is_some() || throttle.is_some() {
            "явных задержек CPU в этом запуске нет"
        } else { "недостаточно данных о задержках CPU" };
        let confidence = if unstable {
            "низкая: большой разброс между повторами"
        } else {
            "ограниченная: три повтора в одном временном интервале"
        };
        (level, confidence, s.steal_percent_load, Some((100.0-s.scaling_efficiency).max(0.0)))
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

    let estimated_equivalent = "не определяется по тесту внутри гостевой ОС".into();
    let estimated_tier = "нет проверенного эталона".into();
    let commentary = match (st, mt) {
        (Some(st), Some(mt)) => format!(
            "ST={st:.0}, MT={mt:.0}; {}. Для сравнения нужны замеры той же версии теста с известной конфигурацией.",
            oversell.oversell_level
        ),
        _ => "Нагрузочный тест пропущен; эквивалент физического CPU определить нельзя".into(),
    };

    CpuEstimate {
        advertised_model: advertised,
        catalog_match,
        estimated_equivalent,
        estimated_tier,
        score_single: st,
        score_multi: mt,
        nearest_matches: Vec::new(),
        commentary,
    }
}
