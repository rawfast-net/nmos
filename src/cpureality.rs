
use crate::catalog;
use crate::cpu::CpuInfo;
use crate::probe::DeepProbe;
use crate::tech::{self, TechReport};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct CpuRealityReport {
    pub decoded_uarch: Option<String>,
    pub decoded_from: String,
    pub catalog_match: Option<String>,
    pub catalog_gen: Option<String>,
    pub consistency: String, 
    pub expected_tech: Vec<TechExpect>,
    pub mismatches: Vec<String>,
    pub matches: Vec<String>,
    pub notes: Vec<String>,
    pub confidence: String,
    pub verdict: String,
    pub confirmed: Vec<String>,
    pub contradictions: Vec<String>,
    pub masked: Vec<String>,
    pub unknown: Vec<String>,
    pub effective_cpu_cores: Option<f64>,
    pub cpuset_count: Option<usize>,
    pub quota_cores: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TechExpect {
    pub id: String,
    pub title: String,
    pub required: bool,
    pub present: bool,
}

struct UarchProfile {
    name: &'static str,
    
    must: &'static [&'static str],
    
    should: &'static [&'static str],
}

pub fn analyze(cpu: &CpuInfo, deep: &DeepProbe, tech: &TechReport) -> CpuRealityReport {
    let virtualized = cpu.virtualization.is_virtual || deep.cpu_ids.has_hypervisor_flag;
    let mut confirmed = Vec::new();
    let mut contradictions = Vec::new();
    let mut masked = Vec::new();
    let mut unknown = Vec::new();
    let mut notes = Vec::new();
    let mut signals = 0usize;

    let raw_vendor = deep.cpu_ids.raw_vendor.as_deref().unwrap_or(&cpu.vendor);
    let family = deep.cpu_ids.raw_family.or(deep.cpu_ids.family);
    let model = deep.cpu_ids.raw_model.or(deep.cpu_ids.model);
    let decoded = decode_uarch(family, model, raw_vendor);
    let decoded_from = format!(
        "CPUID vendor={} family={:?} model={:?} stepping={:?}; /proc family={:?} model={:?}",
        raw_vendor, family, model,
        deep.cpu_ids.raw_stepping.or(deep.cpu_ids.stepping),
        deep.cpu_ids.family, deep.cpu_ids.model
    );
    let cat = catalog::match_model(&cpu.model);
    let catalog_match = cat.map(|c| c.name.to_string());
    let catalog_gen = cat.map(|c| c.gen.to_string());

    if let Some(c) = cat {
        confirmed.push(format!("Название гостевого CPU распознано как {}", c.name));
    } else {
        unknown.push("Точная модель не найдена в локальном каталоге".into());
    }

    if let Some(vendor) = &deep.cpu_ids.raw_vendor {
        let name_amd = cpu.model.to_lowercase().contains("amd")
            || cpu.model.to_lowercase().contains("epyc")
            || cpu.model.to_lowercase().contains("ryzen");
        let name_intel = cpu.model.to_lowercase().contains("intel")
            || cpu.model.to_lowercase().contains("xeon");
        let raw_amd = vendor.eq_ignore_ascii_case("AuthenticAMD");
        let raw_intel = vendor.eq_ignore_ascii_case("GenuineIntel");
        if (name_amd && raw_intel) || (name_intel && raw_amd) {
            signals += 1;
            contradictions.push(format!("Название CPU «{}» противоречит CPUID vendor={vendor}", cpu.model));
        } else if (name_amd && raw_amd) || (name_intel && raw_intel) {
            confirmed.push(format!("Производитель в названии согласуется с CPUID vendor={vendor}"));
        } else {
            unknown.push("Производитель в названии и CPUID не удалось сопоставить".into());
        }
    } else {
        unknown.push("Прямое чтение CPUID недоступно".into());
    }

    if let (Some(raw_family), Some(raw_model), Some(proc_family), Some(proc_model)) = (
        deep.cpu_ids.raw_family, deep.cpu_ids.raw_model, deep.cpu_ids.family, deep.cpu_ids.model
    ) {
        if (raw_family, raw_model) != (proc_family, proc_model) {
            signals += 1;
            contradictions.push(format!(
                "CPUID instruction: family {raw_family:#x}, model {raw_model:#x}; /proc/cpuinfo: family {proc_family:#x}, model {proc_model:#x}"
            ));
        } else {
            confirmed.push("Прямой CPUID и /proc/cpuinfo согласуются".into());
        }
    }

    if let (Some(uarch), Some(c)) = (decoded.as_ref(), cat) {
        if uarch.contains("Family") || uarch.contains("family") {
            unknown.push(format!("CPUID не уточняет микроархитектуру модели {}", c.name));
        } else if uarch_aligned(c.gen, uarch) {
            confirmed.push(format!("Поколение {} согласуется с CPUID {}", c.gen, uarch));
        } else {
            signals += 1;
            contradictions.push(format!("Каталог: {}, CPUID: {}", c.gen, uarch));
        }
    } else if decoded.is_none() {
        unknown.push("Микроархитектура по CPUID не определена".into());
    }

    let profile = decoded.as_ref()
        .filter(|u| !u.contains("Family") && !u.contains("family"))
        .and_then(|u| profile_by_name(u))
        .or_else(|| cat.and_then(|c| profile_by_gen(c.gen)));
    let mut expected_tech = Vec::new();
    if let Some(profile) = profile {
        let mut missing_exposed = Vec::new();
        for id in profile.must.iter().chain(profile.should.iter()) {
            let required = profile.must.contains(id);
            let available = flag_observable(tech, deep, id);
            let present = tech::has_flag(tech, id) || flag_fallback(tech, deep, id);
            expected_tech.push(TechExpect {
                id: (*id).into(), title: title_of(id), required, present
            });
            if present {
                if required { confirmed.push(format!("Доступна инструкция {}", title_of(id))); }
            } else if !available {
                unknown.push(format!("Не удалось проверить инструкцию {}", title_of(id)));
            } else if virtualized {
                masked.push(format!("Инструкция {} не предоставлена гостевой системе", title_of(id)));
            } else if required {
                missing_exposed.push(title_of(id));
            } else {
                notes.push(format!("Дополнительная инструкция {} недоступна", title_of(id)));
            }
        }
        if !missing_exposed.is_empty() {
            signals += 1;
            contradictions.push(format!(
                "Для профиля {} отсутствуют ожидаемые инструкции: {}",
                profile.name, missing_exposed.join(", ")
            ));
        }
    } else {
        unknown.push("Нет достоверного профиля инструкций для этого CPU".into());
    }

    if let Some(bogomips) = cpu.bogomips {
        notes.push(format!("BogoMIPS={bogomips:.0}; этот показатель не измеряет производительность CPU"));
    }
    if virtualized {
        notes.push("Гипервизор может менять видимые CPUID, частоту, кэш и набор инструкций".into());
    }

    let quota_cores = deep.cgroup.cpu_quota_cores.filter(|v| v.is_finite() && *v > 0.0);
    let cpuset_count = deep.cgroup.cpuset_effective_count;
    let effective_cpu_cores = {
        let mut value = cpu.logical_cores as f64;
        if let Some(quota) = quota_cores { value = value.min(quota); }
        if let Some(count) = cpuset_count { value = value.min(count as f64); }
        if cpu.logical_cores == 0 { None } else { Some(value) }
    };
    if let Some(quota) = quota_cores {
        confirmed.push(format!("Квота CPU доступна для измерения: {quota:.2} ядра"));
        if quota + 0.05 < cpu.logical_cores as f64 {
            notes.push("Квота ограничивает суммарное время CPU, хотя гостю видны дополнительные логические CPU".into());
        }
    } else {
        unknown.push("Квота CPU не установлена или не доступна для чтения".into());
    }
    if let Some(count) = cpuset_count {
        confirmed.push(format!("Эффективный cpuset разрешает {count} CPU"));
    } else {
        unknown.push("Эффективный cpuset не доступен для чтения".into());
    }

    let consistency = if signals >= 2 && !virtualized { "fail" }
        else if signals == 1 { "warn" }
        else if signals >= 2 { "warn" }
        else if !masked.is_empty() { "masked" }
        else if cat.is_none() && decoded.is_none() { "unknown" }
        else { "ok" }.to_string();
    let confidence = if virtualized {
        "ограниченная: гипервизор управляет видимыми признаками"
    } else if deep.cpu_ids.raw_vendor.is_some() && cat.is_some() && tech.available {
        "высокая для видимой идентичности"
    } else {
        "средняя или низкая: часть источников недоступна"
    }.to_string();
    let verdict = match consistency.as_str() {
        "fail" => "Несколько независимых признаков противоречат видимой модели CPU",
        "warn" => "Есть одно противоречие; требуется дополнительная проверка",
        "masked" => "Признаки CPU частично скрыты гипервизором",
        "unknown" => "Недостаточно данных для проверки модели CPU",
        _ => "Противоречий в доступных данных CPU не найдено",
    }.to_string();

    CpuRealityReport {
        decoded_uarch: decoded, decoded_from, catalog_match, catalog_gen,
        consistency, expected_tech, mismatches: contradictions.clone(),
        matches: confirmed.clone(), notes, confidence, verdict,
        confirmed, contradictions, masked, unknown, effective_cpu_cores,
        cpuset_count, quota_cores,
    }
}

fn flag_observable(tech: &TechReport, deep: &DeepProbe, id: &str) -> bool {
    if tech.available { return true; }
    match id {
        "aes" => deep.cpu_ids.raw_aes.is_some(),
        "avx" => deep.cpu_ids.raw_avx.is_some(),
        "avx2" => deep.cpu_ids.raw_avx2.is_some(),
        "avx512f" => deep.cpu_ids.raw_avx512f.is_some(),
        _ => false,
    }
}
fn flag_fallback(tech: &TechReport, deep: &DeepProbe, id: &str) -> bool {
    if tech::has_flag(tech, id) {
        return true;
    }
    match id {
        "aes" => deep.cpu_ids.has_aes || deep.cpu_ids.raw_aes == Some(true),
        "avx" => deep.cpu_ids.has_avx || deep.cpu_ids.raw_avx == Some(true),
        "avx2" => deep.cpu_ids.has_avx2 || deep.cpu_ids.raw_avx2 == Some(true),
        "avx512f" => deep.cpu_ids.has_avx512 || deep.cpu_ids.raw_avx512f == Some(true),
        "sha_ni" => deep.cpu_ids.has_sha,
        "hypervisor" => deep.cpu_ids.has_hypervisor_flag,
        "vmx" | "svm" => deep.cpu_ids.has_svm_or_vmx,
        _ => false,
    }
}

fn title_of(id: &str) -> String {
    match id {
        "sse4_2" => "SSE4.2".into(),
        "avx" => "AVX".into(),
        "avx2" => "AVX2".into(),
        "avx512f" => "AVX-512F".into(),
        "fma" => "FMA3".into(),
        "aes" => "AES-NI".into(),
        "sha_ni" => "SHA-NI".into(),
        "bmi1" => "BMI1".into(),
        "bmi2" => "BMI2".into(),
        "popcnt" => "POPCNT".into(),
        "nx" => "NX".into(),
        "lm" => "Long Mode".into(),
        "pdpe1gb" => "1GB pages".into(),
        "rdrand" => "RDRAND".into(),
        "rdseed" => "RDSEED".into(),
        "constant_tsc" => "constant_tsc".into(),
        "amx_tile" => "AMX".into(),
        "vaes" => "VAES".into(),
        "sse2" => "SSE2".into(),
        _ => id.to_string(),
    }
}

fn profile_by_gen(gen: &str) -> Option<&'static UarchProfile> {
    let g = gen.to_lowercase();
    if g.contains("zen5") {
        return Some(&ZEN5);
    }
    if g.contains("zen4") || g.contains("genoa") || g.contains("bergamo") {
        return Some(&ZEN4);
    }
    if g.contains("zen3") || g.contains("milan") {
        return Some(&ZEN3);
    }
    if g.contains("zen2") || g.contains("rome") {
        return Some(&ZEN2);
    }
    if g.contains("sapphire") || g.contains("emerald") {
        return Some(&SAPPHIRE);
    }
    if g.contains("granite") {
        return Some(&GRANITE);
    }
    if g.contains("ice lake") {
        return Some(&ICELAKE);
    }
    if g.contains("arrow") || g.contains("meteor") || g.contains("alder") || g.contains("raptor") {
        return Some(&ALDER);
    }
    if g.contains("comet") || g.contains("rocket") || g.contains("coffee") {
        return Some(&COMET);
    }
    if g.contains("cascade") || g.contains("skylake") {
        return Some(&CASCADE);
    }
    if g.contains("broadwell") {
        return Some(&BROADWELL);
    }
    if g.contains("haswell") {
        return Some(&HASWELL);
    }
    if g.contains("ivy") {
        return Some(&IVY);
    }
    None
}

fn profile_by_name(name: &str) -> Option<&'static UarchProfile> {
    let n = name.to_lowercase();
    if n.contains("zen 5") {
        return Some(&ZEN5);
    }
    if n.contains("zen 4") {
        return Some(&ZEN4);
    }
    if n.contains("zen 3") {
        return Some(&ZEN3);
    }
    if n.contains("zen 2") {
        return Some(&ZEN2);
    }
    if n.contains("sapphire") {
        return Some(&SAPPHIRE);
    }
    if n.contains("granite") {
        return Some(&GRANITE);
    }
    if n.contains("ice lake") {
        return Some(&ICELAKE);
    }
    if n.contains("cascade") || n.contains("skylake") {
        return Some(&CASCADE);
    }
    if n.contains("broadwell") {
        return Some(&BROADWELL);
    }
    if n.contains("haswell") {
        return Some(&HASWELL);
    }
    if n.contains("ivy") {
        return Some(&IVY);
    }
    None
}

static IVY: UarchProfile = UarchProfile {
    name: "Ivy Bridge-EP",
    must: &["sse4_2", "avx", "aes", "popcnt", "nx", "lm"],
    should: &["pdpe1gb", "rdrand", "constant_tsc"],
};
static HASWELL: UarchProfile = UarchProfile {
    name: "Haswell-EP",
    must: &["sse4_2", "avx", "avx2", "fma", "aes", "bmi1", "bmi2", "popcnt"],
    should: &["pdpe1gb", "rdrand", "constant_tsc"],
};
static BROADWELL: UarchProfile = UarchProfile {
    name: "Broadwell-EP",
    must: &["sse4_2", "avx", "avx2", "fma", "aes", "bmi2", "rdseed", "popcnt"],
    should: &["pdpe1gb", "rdrand", "constant_tsc", "sha_ni"],
};
static CASCADE: UarchProfile = UarchProfile {
    name: "Skylake/Cascade Lake",
    must: &["sse4_2", "avx", "avx2", "fma", "aes", "bmi2", "popcnt"],
    should: &["avx512f", "pdpe1gb", "rdrand", "rdseed", "constant_tsc"],
};
static ICELAKE: UarchProfile = UarchProfile {
    name: "Ice Lake",
    must: &["avx2", "avx512f", "aes", "sha_ni", "bmi2", "fma", "popcnt"],
    should: &["vaes", "pdpe1gb", "rdrand", "constant_tsc"],
};
static SAPPHIRE: UarchProfile = UarchProfile {
    name: "Sapphire Rapids",
    must: &["avx2", "avx512f", "aes", "sha_ni", "amx_tile", "bmi2", "fma"],
    should: &["pdpe1gb", "rdrand", "constant_tsc"],
};
static GRANITE: UarchProfile = UarchProfile {
    name: "Granite Rapids",
    must: &["avx2", "aes", "fma", "bmi2"],
    should: &["avx512f", "amx_tile"],
};
static ALDER: UarchProfile = UarchProfile {
    name: "Alder/Raptor/Arrow Lake",
    must: &["avx2", "fma", "aes", "bmi2", "popcnt", "sse4_2"],
    should: &["sha_ni", "rdrand", "vaes"],
};
static COMET: UarchProfile = UarchProfile {
    name: "Comet/Rocket/Coffee Lake",
    must: &["avx2", "fma", "aes", "bmi2", "popcnt", "sse4_2"],
    should: &["rdrand", "pdpe1gb"],
};
static ZEN2: UarchProfile = UarchProfile {
    name: "AMD Zen 2 (Rome)",
    must: &["avx2", "fma", "aes", "sha_ni", "bmi2", "popcnt", "sse4_2"],
    should: &["pdpe1gb", "rdrand", "rdseed", "constant_tsc"],
};
static ZEN3: UarchProfile = UarchProfile {
    name: "AMD Zen 3 (Milan)",
    must: &["avx2", "fma", "aes", "sha_ni", "bmi2", "popcnt", "sse4_2"],
    should: &["pdpe1gb", "rdrand", "rdseed", "constant_tsc", "vaes"],
};
static ZEN4: UarchProfile = UarchProfile {
    name: "AMD Zen 4 (Genoa)",
    must: &["avx2", "avx512f", "fma", "aes", "sha_ni", "bmi2", "popcnt"],
    should: &["pdpe1gb", "rdrand", "vaes"],
};
static ZEN5: UarchProfile = UarchProfile {
    name: "AMD Zen 5",
    must: &["avx2", "avx512f", "fma", "aes", "sha_ni", "bmi2", "popcnt"],
    
    should: &["pdpe1gb", "rdrand", "vaes", "rdseed"],
};


fn uarch_aligned(gen: &str, uarch: &str) -> bool {
    let gen_l = gen.to_lowercase();
    let u_l = uarch.to_lowercase();
    let gen_n = normalize_arch(&gen_l);
    let u_n = normalize_arch(&u_l);

    if !gen_n.is_empty() && (u_n.contains(&gen_n) || gen_n.contains(&u_n)) {
        return true;
    }
    
    for n in 1..=5 {
        let a = format!("zen{n}");
        let b = format!("zen {n}");
        if (gen_l.contains(&a) || gen_l.contains(&b)) && (u_l.contains(&a) || u_l.contains(&b) || u_n.contains(&a))
        {
            return true;
        }
    }
    (gen_l.contains("milan") && u_l.contains("zen 3"))
        || (gen_l.contains("rome") && u_l.contains("zen 2"))
        || (gen_l.contains("genoa") && (u_l.contains("zen 4") || u_n.contains("zen4")))
        || (gen_l.contains("turin") && (u_l.contains("zen 5") || u_n.contains("zen5")))
        || (gen_l.contains("ice lake") && u_l.contains("ice lake"))
        || (gen_l.contains("cascade") && u_l.contains("cascade"))
        || (gen_l.contains("broadwell") && u_l.contains("broadwell"))
        || (gen_l.contains("haswell") && u_l.contains("haswell"))
        || (gen_l.contains("ivy") && u_l.contains("ivy"))
        || (gen_l.contains("alder") && u_l.contains("alder"))
        || (gen_l.contains("raptor") && u_l.contains("raptor"))
        || (gen_l.contains("sapphire") && u_l.contains("sapphire"))
}

fn normalize_arch(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_lowercase()
}


fn decode_uarch(family: Option<u32>, model: Option<u32>, vendor: &str) -> Option<String> {
    let family = family?;
    let model = model?;
    let v = vendor.to_lowercase();

    if v.contains("amd") || v.contains("authenticamd") {
        
        
        
        
        match family {
            0x17 => {
                if (0x30..=0x3F).contains(&model) {
                    return Some("AMD Zen 2 / Rome".into());
                }
                if (0x60..=0x7F).contains(&model) {
                    return Some("AMD Zen 2 / Renoir-class".into());
                }
                return Some("AMD Family 17h (Zen/Zen+/Zen2)".into());
            }
            0x19 => {
                if (0x00..=0x0F).contains(&model) {
                    return Some("AMD Zen 3 / Milan".into());
                }
                if (0x10..=0x1F).contains(&model) {
                    return Some("AMD Zen 4 / Genoa".into());
                }
                if (0x20..=0x2F).contains(&model) {
                    return Some("AMD Zen 3 / Vermeer-class".into());
                }
                if (0x40..=0x4F).contains(&model) {
                    return Some("AMD Zen 3+ / Rembrandt-class".into());
                }
                if (0x60..=0x6F).contains(&model) {
                    return Some("AMD Zen 4 / Raphael-class".into());
                }
                if (0x70..=0x7F).contains(&model) {
                    return Some("AMD Zen 4 / Phoenix-class".into());
                }
                if (0xA0..=0xAF).contains(&model) {
                    return Some("AMD Zen 4c / Bergamo-class".into());
                }
                return Some("AMD Family 19h (Zen3/Zen4)".into());
            }
            0x1A => {
                
                if (0x40..=0x4F).contains(&model) {
                    return Some("AMD Zen 5 / Granite Ridge (desktop)".into());
                }
                if (0x10..=0x1F).contains(&model) {
                    return Some("AMD Zen 5 / Turin (EPYC)".into());
                }
                return Some("AMD Zen 5 / Family 1Ah".into());
            }
            0x15 => return Some("AMD Bulldozer/Piledriver-era".into()),
            0x16 => return Some("AMD Jaguar/Puma".into()),
            _ => return Some(format!("AMD Family {family:#x} Model {model:#x}")),
        }
    }

    if v.contains("intel") || v.contains("genuineintel") {
        
        if family == 6 {
            return Some(match model {
                0x3E => "Intel Ivy Bridge-EP".into(),
                0x3F => "Intel Haswell-EP".into(),
                0x4F | 0x56 => "Intel Broadwell-EP/EX".into(),
                0x55 => "Intel Skylake/Cascade Lake-SP".into(),
                0x6A | 0x6C => "Intel Ice Lake-SP".into(),
                0x8F => "Intel Sapphire Rapids".into(),
                0xCF => "Intel Emerald Rapids".into(),
                0xAD | 0xAE => "Intel Granite Rapids".into(),
                0x5E | 0x4E => "Intel Skylake client".into(),
                0x9E | 0x8E => "Intel Kaby/Coffee Lake".into(),
                0xA5 => "Intel Comet Lake".into(),
                0xA7 => "Intel Rocket Lake".into(),
                0x97 | 0x9A | 0xBE => "Intel Alder Lake".into(),
                0xB7 | 0xBA | 0xBF => "Intel Raptor Lake".into(),
                0xC6 | 0xBD => "Intel Raptor Lake Refresh".into(),
                0xC5 | 0xC4 => "Intel Arrow Lake".into(),
                0xAA | 0xAC => "Intel Meteor Lake".into(),
                0xA6 => "Intel Comet Lake / RPL-ish".into(),
                _ => format!("Intel Family 6 Model {model:#x}"),
            });
        }
        return Some(format!("Intel Family {family} Model {model:#x}"));
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cpu::{FreqAnalysis, VirtInfo};
    use crate::tech::{TechCategory, TechItem};

    fn cpu(virtualized: bool) -> CpuInfo {
        CpuInfo {
            model: "AMD EPYC 9655 96-Core Processor".into(),
            vendor: "AuthenticAMD".into(),
            architecture: "x86_64".into(),
            physical_cores: 96,
            logical_cores: 4,
            freq_mhz_reported: None,
            freq_mhz_min: None,
            freq_mhz_max: None,
            freq_mhz_current_avg: None,
            freq_analysis: FreqAnalysis::default(),
            cache_l1_kb: None,
            cache_l2_kb: None,
            cache_l3_kb: None,
            virtualization: VirtInfo {
                is_virtual: virtualized,
                hypervisor: None,
                virt_type: String::new(),
                evidence: Vec::new(),
            },
            flags_sample: Vec::new(),
            steal_percent: None,
            bogomips: None,
            numa_nodes: None,
            os: "Linux".into(),
            hostname: String::new(),
        }
    }

    fn probe() -> DeepProbe {
        let mut deep = DeepProbe::default();
        deep.cpu_ids.raw_vendor = Some("AuthenticAMD".into());
        deep.cpu_ids.raw_family = Some(0x1a);
        deep.cpu_ids.raw_model = Some(0x10);
        deep.cpu_ids.family = Some(0x1a);
        deep.cpu_ids.model = Some(0x10);
        deep.cpu_ids.raw_aes = Some(true);
        deep.cpu_ids.raw_avx2 = Some(true);
        deep.cpu_ids.raw_avx512f = Some(true);
        deep
    }

    fn tech() -> TechReport {
        let ids = ["avx2", "avx512f", "fma", "aes", "sha_ni", "bmi2", "popcnt", "pdpe1gb", "rdrand", "vaes", "rdseed"];
        TechReport {
            available: true,
            categories: vec![TechCategory {
                name: "Test".into(),
                items: ids.iter().map(|id| TechItem {
                    id: (*id).into(), title: (*id).into(), present: true
                }).collect(),
                score: String::new(),
            }],
            ..Default::default()
        }
    }

    #[test]
    fn correct_visible_cpu_has_no_contradictions() {
        let result = analyze(&cpu(false), &probe(), &tech());
        assert_eq!(result.consistency, "ok");
        assert!(result.contradictions.is_empty());
        assert_eq!(result.effective_cpu_cores, Some(4.0));
    }

    #[test]
    fn quota_and_cpuset_limit_effective_capacity() {
        let mut deep = probe();
        deep.cgroup.cpu_quota_cores = Some(2.5);
        deep.cgroup.cpuset_effective_count = Some(3);
        let result = analyze(&cpu(true), &deep, &tech());
        assert_eq!(result.effective_cpu_cores, Some(2.5));
        assert_eq!(result.quota_cores, Some(2.5));
        assert_eq!(result.cpuset_count, Some(3));
        assert!(result.contradictions.is_empty());
    }

    #[test]
    fn virtualized_missing_features_are_not_physical_cpu_proof() {
        let mut deep = probe();
        deep.cpu_ids.has_hypervisor_flag = true;
        deep.cpu_ids.raw_avx512f = Some(false);
        let result = analyze(&cpu(true), &deep, &TechReport { available: true, ..Default::default() });
        assert_eq!(result.consistency, "masked");
        assert!(result.contradictions.is_empty());
        assert!(!result.masked.is_empty());
    }

    #[test]
    fn one_cpuid_difference_is_a_warning() {
        let mut deep = probe();
        deep.cpu_ids.raw_model = Some(0x40);
        let result = analyze(&cpu(false), &deep, &tech());
        assert_eq!(result.consistency, "warn");
        assert_eq!(result.contradictions.len(), 1);
    }
}
