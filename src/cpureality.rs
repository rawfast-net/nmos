
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
    pub consistency: String, // ok | warn | fail
    pub expected_tech: Vec<TechExpect>,
    pub mismatches: Vec<String>,
    pub matches: Vec<String>,
    pub notes: Vec<String>,
    pub confidence: String,
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
    /// обязательные флаги
    must: &'static [&'static str],
    /// желательные
    should: &'static [&'static str],
}

pub fn analyze(cpu: &CpuInfo, deep: &DeepProbe, tech: &TechReport) -> CpuRealityReport {
    let mut notes = Vec::new();
    let mut mismatches = Vec::new();
    let mut matches = Vec::new();

    let decoded = decode_uarch(deep.cpu_ids.family, deep.cpu_ids.model, &cpu.vendor);
    let decoded_from = format!(
        "vendor={} family={:?} model={:?} stepping={:?}",
        cpu.vendor, deep.cpu_ids.family, deep.cpu_ids.model, deep.cpu_ids.stepping
    );

    let cat = catalog::match_model(&cpu.model);
    let catalog_match = cat.map(|c| c.name.to_string());
    let catalog_gen = cat.map(|c| c.gen.to_string());

    // Профиль ожидаемых технологий: сначала по декодированному uarch, иначе по каталогу gen
    let profile = decoded
        .as_ref()
        .and_then(|u| profile_by_name(u))
        .or_else(|| cat.and_then(|c| profile_by_gen(c.gen)));

    let mut expected_tech = Vec::new();
    if let Some(p) = profile {
        notes.push(format!("Профиль технологий: {}", p.name));
        for id in p.must {
            let present = tech::has_flag(tech, id) || flag_fallback(tech, deep, id);
            expected_tech.push(TechExpect {
                id: (*id).into(),
                title: title_of(id),
                required: true,
                present,
            });
            if present {
                matches.push(format!("MUST {} OK", title_of(id)));
            } else {
                mismatches.push(format!(
                    "НЕТ обязательной технологии {} — не похоже на {}",
                    title_of(id),
                    p.name
                ));
            }
        }
        for id in p.should {
            // В KVM/госте constant_tsc часто не экспонируется — не шумим
            if *id == "constant_tsc" && deep.cpu_ids.has_hypervisor_flag {
                continue;
            }
            let present = tech::has_flag(tech, id) || flag_fallback(tech, deep, id);
            expected_tech.push(TechExpect {
                id: (*id).into(),
                title: title_of(id),
                required: false,
                present,
            });
            if !present {
                notes.push(format!("Нет желательной: {}", title_of(id)));
            } else {
                matches.push(format!("SHOULD {} OK", title_of(id)));
            }
        }
    } else {
        notes.push("Не удалось подобрать профиль uarch — сверка по базовым x86-64-v*".into());
        for id in ["lm", "sse2", "sse4_2", "popcnt", "nx"] {
            let present = tech::has_flag(tech, id) || flag_fallback(tech, deep, id);
            expected_tech.push(TechExpect {
                id: id.into(),
                title: title_of(id),
                required: true,
                present,
            });
            if !present {
                mismatches.push(format!("Нет базового {}", title_of(id)));
            }
        }
    }

    // Имя в паспорте vs family/model decode
    if let (Some(uarch), Some(c)) = (decoded.as_ref(), cat) {
        let aligned = uarch_aligned(c.gen, uarch);

        if aligned {
            matches.push(format!("Паспорт «{}» согласован с decode «{}»", c.name, uarch));
        } else {
            mismatches.push(format!(
                "Паспорт gen «{}», а CPUID decode «{}» — возможная подмена model name",
                c.gen, uarch
            ));
        }
    }

    // Hypervisor + «bare metal» имя
    if deep.cpu_ids.has_hypervisor_flag {
        notes.push("CPUID.hypervisor=1 — это гость гипервизора".into());
    }

    // BogoMIPS sanity vs passport MHz
    if let (Some(b), Some(mhz)) = (cpu.bogomips, cpu.freq_mhz_max.or(cpu.freq_mhz_reported)) {
        let ratio = (b / 2.0) / mhz as f64;
        if !(0.7..=1.4).contains(&ratio) {
            mismatches.push(format!(
                "BogoMIPS/2={:.0} vs passport {} МГц (ratio {:.2})",
                b / 2.0,
                mhz,
                ratio
            ));
        } else {
            matches.push(format!("BogoMIPS согласован с ~{} МГц", mhz));
        }
    }

    // ISA level vs catalog
    if let Some(c) = cat {
        let isa = tech.isa_level.to_lowercase();
        let gen = c.gen.to_lowercase();
        if (gen.contains("zen3") || gen.contains("zen4") || gen.contains("zen5") || gen.contains("ice lake") || gen.contains("sapphire"))
            && !(isa.contains("v3") || isa.contains("v4") || isa.contains("avx2") || isa.contains("amx"))
        {
            mismatches.push(format!(
                "Для {} нужен минимум x86-64-v3/AVX2, факт ISA: {}",
                c.name, tech.isa_level
            ));
        }
    }

    let fail_n = mismatches.len();
    let consistency = if fail_n == 0 {
        "ok"
    } else if fail_n <= 2 {
        "warn"
    } else {
        "fail"
    }
    .to_string();

    let confidence = if decoded.is_some() && tech.available && cat.is_some() {
        "высокая"
    } else if tech.available {
        "средняя"
    } else {
        "низкая"
    }
    .to_string();

    CpuRealityReport {
        decoded_uarch: decoded,
        decoded_from,
        catalog_match,
        catalog_gen,
        consistency,
        expected_tech,
        mismatches,
        matches,
        notes,
        confidence,
    }
}

fn flag_fallback(tech: &TechReport, deep: &DeepProbe, id: &str) -> bool {
    if tech::has_flag(tech, id) {
        return true;
    }
    match id {
        "aes" => deep.cpu_ids.has_aes,
        "avx" => deep.cpu_ids.has_avx,
        "avx2" => deep.cpu_ids.has_avx2,
        "avx512f" => deep.cpu_ids.has_avx512,
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
    if n.contains("zen") {
        return Some(&ZEN2);
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
    // AMX — Intel-only; constant_tsc часто маскируется в KVM — не should
    should: &["pdpe1gb", "rdrand", "vaes", "rdseed"],
};

/// «Zen5» ↔ «Zen 5», «Family 1Ah» и т.п.
fn uarch_aligned(gen: &str, uarch: &str) -> bool {
    let gen_l = gen.to_lowercase();
    let u_l = uarch.to_lowercase();
    let gen_n = normalize_arch(&gen_l);
    let u_n = normalize_arch(&u_l);

    if !gen_n.is_empty() && (u_n.contains(&gen_n) || gen_n.contains(&u_n)) {
        return true;
    }
    // Zen N без пробела
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

/// Декод CPUID family/model → микроархитектура (основные серверные).
fn decode_uarch(family: Option<u32>, model: Option<u32>, vendor: &str) -> Option<String> {
    let family = family?;
    let model = model?;
    let v = vendor.to_lowercase();

    if v.contains("amd") || v.contains("authenticamd") {
        // AMD: DisplayFamily_DisplayModel
        // Zen2 EPYC: Family 23 (17h), models 0x30-0x3F etc
        // Zen3: Family 25 (19h)
        // Zen4: Family 25 (19h) models 0x10-0x1F (Genoa) / etc
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
                // Zen5 Granite Ridge / Turin / Strix
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
        // Intel: Family 6 mostly for modern
        if family == 6 {
            return Some(match model {
                0x3E => "Intel Ivy Bridge-EP".into(),
                0x3F => "Intel Haswell-EP".into(),
                0x4F | 0x56 => "Intel Broadwell-EP/EX".into(),
                0x55 => "Intel Skylake/Cascade Lake-SP".into(),
                0x6A | 0x6C => "Intel Ice Lake-SP".into(),
                0x8F => "Intel Sapphire Rapids".into(),
                0xCF => "Intel Emerald Rapids".into(),
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
