
use serde::Serialize;
use std::collections::BTreeSet;

#[derive(Debug, Clone, Serialize, Default)]
pub struct TechReport {
    pub available: bool,
    pub total_flags: usize,
    pub isa_level: String,
    pub generation_hint: String,
    pub categories: Vec<TechCategory>,
    pub present_highlights: Vec<String>,
    pub missing_important: Vec<String>,
    pub raw_interesting: Vec<String>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TechCategory {
    pub name: String,
    pub items: Vec<TechItem>,
    pub score: String, // full | partial | none
}

#[derive(Debug, Clone, Serialize)]
pub struct TechItem {
    pub id: String,
    pub title: String,
    pub present: bool,
}

struct FlagDef {
    id: &'static str,
    /// альтернативные имена флага в /proc/cpuinfo
    aliases: &'static [&'static str],
    title: &'static str,
}

struct CatDef {
    name: &'static str,
    flags: &'static [FlagDef],
}

fn catalog() -> &'static [CatDef] {
    &[
        CatDef {
            name: "SIMD / векторные",
            flags: &[
                FlagDef { id: "sse", aliases: &["sse"], title: "SSE" },
                FlagDef { id: "sse2", aliases: &["sse2"], title: "SSE2" },
                FlagDef { id: "sse3", aliases: &["sse3", "pni"], title: "SSE3" },
                FlagDef { id: "ssse3", aliases: &["ssse3"], title: "SSSE3" },
                FlagDef { id: "sse4_1", aliases: &["sse4_1"], title: "SSE4.1" },
                FlagDef { id: "sse4_2", aliases: &["sse4_2"], title: "SSE4.2" },
                FlagDef { id: "avx", aliases: &["avx"], title: "AVX" },
                FlagDef { id: "avx2", aliases: &["avx2"], title: "AVX2" },
                FlagDef { id: "fma", aliases: &["fma", "fma3"], title: "FMA3" },
                FlagDef { id: "f16c", aliases: &["f16c"], title: "F16C" },
                FlagDef { id: "avx512f", aliases: &["avx512f"], title: "AVX-512F" },
                FlagDef { id: "avx512dq", aliases: &["avx512dq"], title: "AVX-512DQ" },
                FlagDef { id: "avx512bw", aliases: &["avx512bw"], title: "AVX-512BW" },
                FlagDef { id: "avx512vl", aliases: &["avx512vl"], title: "AVX-512VL" },
                FlagDef { id: "avx512vnni", aliases: &["avx512_vnni", "avx512vnni"], title: "AVX-512 VNNI" },
                FlagDef { id: "avx512bf16", aliases: &["avx512_bf16", "avx512bf16"], title: "AVX-512 BF16" },
                FlagDef { id: "amx_tile", aliases: &["amx_tile", "amx-tile"], title: "AMX Tile" },
                FlagDef { id: "amx_bf16", aliases: &["amx_bf16", "amx-bf16"], title: "AMX BF16" },
                FlagDef { id: "amx_int8", aliases: &["amx_int8", "amx-int8"], title: "AMX INT8" },
            ],
        },
        CatDef {
            name: "Криптография / RNG",
            flags: &[
                FlagDef { id: "aes", aliases: &["aes", "aes-ni"], title: "AES-NI" },
                FlagDef { id: "pclmulqdq", aliases: &["pclmulqdq", "pclmul"], title: "PCLMULQDQ" },
                FlagDef { id: "sha_ni", aliases: &["sha_ni", "sha"], title: "SHA-NI" },
                FlagDef { id: "vaes", aliases: &["vaes"], title: "VAES" },
                FlagDef { id: "vpclmulqdq", aliases: &["vpclmulqdq"], title: "VPCLMULQDQ" },
                FlagDef { id: "rdrand", aliases: &["rdrand"], title: "RDRAND" },
                FlagDef { id: "rdseed", aliases: &["rdseed"], title: "RDSEED" },
                FlagDef { id: "gfni", aliases: &["gfni"], title: "GFNI" },
            ],
        },
        CatDef {
            name: "Виртуализация",
            flags: &[
                FlagDef { id: "vmx", aliases: &["vmx"], title: "Intel VT-x (VMX)" },
                FlagDef { id: "svm", aliases: &["svm"], title: "AMD-V (SVM)" },
                FlagDef { id: "ept", aliases: &["ept"], title: "Intel EPT" },
                FlagDef { id: "npt", aliases: &["npt", "npton"], title: "AMD NPT" },
                FlagDef { id: "vpid", aliases: &["vpid"], title: "VPID" },
                FlagDef { id: "tpr_shadow", aliases: &["tpr_shadow"], title: "TPR shadow" },
                FlagDef { id: "vnmi", aliases: &["vnmi"], title: "VNMI" },
                FlagDef { id: "flexpriority", aliases: &["flexpriority"], title: "FlexPriority" },
                FlagDef { id: "ept_ad", aliases: &["ept_ad"], title: "EPT A/D" },
                FlagDef { id: "hypervisor", aliases: &["hypervisor"], title: "Hypervisor present" },
            ],
        },
        CatDef {
            name: "Память / адресность",
            flags: &[
                FlagDef { id: "lm", aliases: &["lm"], title: "Long Mode (64-bit)" },
                FlagDef { id: "pae", aliases: &["pae"], title: "PAE" },
                FlagDef { id: "pse", aliases: &["pse"], title: "PSE" },
                FlagDef { id: "pdpe1gb", aliases: &["pdpe1gb"], title: "1GB pages" },
                FlagDef { id: "nx", aliases: &["nx", "xd"], title: "NX / XD bit" },
                FlagDef { id: "smep", aliases: &["smep"], title: "SMEP" },
                FlagDef { id: "smap", aliases: &["smap"], title: "SMAP" },
                FlagDef { id: "pku", aliases: &["pku"], title: "PKU / MPK" },
                FlagDef { id: "lahf_lm", aliases: &["lahf_lm"], title: "LAHF/SAHF" },
            ],
        },
        CatDef {
            name: "Безопасность CPU",
            flags: &[
                FlagDef { id: "ibt", aliases: &["ibt"], title: "Intel CET IBT" },
                FlagDef { id: "shstk", aliases: &["shstk"], title: "Shadow Stack" },
                FlagDef { id: "ssbd", aliases: &["ssbd"], title: "SSBD" },
                FlagDef { id: "md_clear", aliases: &["md_clear"], title: "MD_CLEAR" },
                FlagDef { id: "stibp", aliases: &["stibp"], title: "STIBP" },
                FlagDef { id: "ibpb", aliases: &["ibpb"], title: "IBPB" },
                FlagDef { id: "ibrs", aliases: &["ibrs"], title: "IBRS" },
                FlagDef { id: "flush_l1d", aliases: &["flush_l1d"], title: "L1D flush" },
            ],
        },
        CatDef {
            name: "Арифметика / бит. ops",
            flags: &[
                FlagDef { id: "popcnt", aliases: &["popcnt"], title: "POPCNT" },
                FlagDef { id: "lzcnt", aliases: &["lzcnt", "abm"], title: "LZCNT / ABM" },
                FlagDef { id: "bmi1", aliases: &["bmi1"], title: "BMI1" },
                FlagDef { id: "bmi2", aliases: &["bmi2"], title: "BMI2" },
                FlagDef { id: "adx", aliases: &["adx"], title: "ADX" },
                FlagDef { id: "movbe", aliases: &["movbe"], title: "MOVBE" },
                FlagDef { id: "cx16", aliases: &["cx16"], title: "CMPXCHG16B" },
                FlagDef { id: "xsave", aliases: &["xsave"], title: "XSAVE" },
                FlagDef { id: "osxsave", aliases: &["osxsave"], title: "OSXSAVE" },
                FlagDef { id: "xsaveopt", aliases: &["xsaveopt"], title: "XSAVEOPT" },
            ],
        },
        CatDef {
            name: "Таймеры / питание / SMT",
            flags: &[
                FlagDef { id: "ht", aliases: &["ht"], title: "Hyper-Threading / SMT" },
                FlagDef { id: "constant_tsc", aliases: &["constant_tsc"], title: "Constant TSC" },
                FlagDef { id: "nonstop_tsc", aliases: &["nonstop_tsc"], title: "Nonstop TSC" },
                FlagDef { id: "tsc_deadline_timer", aliases: &["tsc_deadline_timer"], title: "TSC deadline" },
                FlagDef { id: "aperfmperf", aliases: &["aperfmperf"], title: "APERF/MPERF" },
                FlagDef { id: "hwp", aliases: &["hwp"], title: "Intel HWP" },
                FlagDef { id: "hwp_act_window", aliases: &["hwp_act_window"], title: "HWP window" },
                FlagDef { id: "hwp_epp", aliases: &["hwp_epp"], title: "HWP EPP" },
                FlagDef { id: "acpi", aliases: &["acpi"], title: "ACPI" },
                FlagDef { id: "tm", aliases: &["tm", "tm2"], title: "Thermal Monitor" },
            ],
        },
        CatDef {
            name: "ARM (если есть)",
            flags: &[
                FlagDef { id: "asimd", aliases: &["asimd", "neon"], title: "ASIMD / NEON" },
                FlagDef { id: "aes_arm", aliases: &["aes"], title: "AES (ARM)" },
                FlagDef { id: "sha1", aliases: &["sha1"], title: "SHA1" },
                FlagDef { id: "sha2", aliases: &["sha2"], title: "SHA2" },
                FlagDef { id: "crc32", aliases: &["crc32"], title: "CRC32" },
                FlagDef { id: "atomics", aliases: &["atomics", "lse"], title: "LSE atomics" },
                FlagDef { id: "sve", aliases: &["sve"], title: "SVE" },
                FlagDef { id: "sve2", aliases: &["sve2"], title: "SVE2" },
            ],
        },
    ]
}

pub fn collect() -> TechReport {
    let flags = read_all_flags();
    if flags.is_empty() {
        return TechReport {
            available: false,
            notes: vec!["Не удалось прочитать CPU flags (нужен Linux /proc/cpuinfo)".into()],
            ..Default::default()
        };
    }

    let has = |aliases: &[&str]| -> bool {
        aliases.iter().any(|a| {
            let a = a.to_lowercase();
            flags.contains(&a)
                || flags.iter().any(|f| f == &a || f.replace('-', "_") == a.replace('-', "_"))
        })
    };

    let mut categories = Vec::new();
    let mut present_highlights = Vec::new();
    let mut missing_important = Vec::new();
    let is_x86 = has(&["lm"]) || has(&["sse2"]) || has(&["avx"]) || has(&["avx2"]);
    let is_arm = has(&["asimd"]) || has(&["neon"]);

    for cat in catalog() {
        // ARM-категорию только на реальном ARM (не из-за x86 флага aes)
        if cat.name.starts_with("ARM") {
            if !is_arm || is_x86 {
                continue;
            }
        }

        let mut items = Vec::new();
        let mut present_n = 0usize;
        for f in cat.flags {
            let present = has(f.aliases);
            if present {
                present_n += 1;
                // ключевые подсветки
                if matches!(
                    f.id,
                    "avx" | "avx2" | "avx512f" | "aes" | "sha_ni" | "vmx" | "svm"
                        | "hypervisor" | "amx_tile" | "pdpe1gb" | "sve" | "asimd"
                ) {
                    present_highlights.push(f.title.to_string());
                }
            }
            items.push(TechItem {
                id: f.id.to_string(),
                title: f.title.to_string(),
                present,
            });
        }

        let score = if present_n == 0 {
            "none"
        } else if present_n == cat.flags.len() {
            "full"
        } else {
            "partial"
        };

        categories.push(TechCategory {
            name: cat.name.to_string(),
            items,
            score: score.into(),
        });
    }

    // Важные «дыры» для x86 серверов (constant_tsc в госте часто нет — не important)
    let important = [
        ("sse4_2", &["sse4_2"][..], "SSE4.2"),
        ("avx", &["avx"][..], "AVX"),
        ("avx2", &["avx2"][..], "AVX2"),
        ("aes", &["aes"][..], "AES-NI"),
        ("popcnt", &["popcnt"][..], "POPCNT"),
        ("nx", &["nx", "xd"][..], "NX bit"),
    ];
    for (_, aliases, title) in important {
        if !has(aliases) && (has(&["sse2"]) || has(&["lm"])) {
            missing_important.push(title.to_string());
        }
    }
    if !has(&["constant_tsc"]) && (has(&["sse2"]) || has(&["lm"])) && !has(&["hypervisor"]) {
        missing_important.push("constant_tsc".into());
    }

    let isa_level = detect_isa_level(&flags);
    let generation_hint = detect_gen_hint(&flags);
    let mut notes = Vec::new();

    if has(&["hypervisor"]) {
        notes.push("Флаг hypervisor=yes — это гостевая ВМ, не bare-metal".into());
        if !has(&["constant_tsc"]) {
            notes.push(
                "Нет constant_tsc в госте — нормально для KVM; не доказательство подмены CPU".into(),
            );
        }
    }
    if has(&["avx512f"]) && !has(&["avx2"]) {
        notes.push("Странно: AVX-512 без AVX2 — возможна порча/маска флагов".into());
    }
    if has(&["avx2"]) && !has(&["avx"]) {
        notes.push("AVX2 без AVX — флаги выглядят повреждёнными".into());
    }
    if !has(&["aes"]) && has(&["avx2"]) {
        notes.push("AVX2 есть, AES-NI нет — на серверных CPU подозрительно (маска?)".into());
        missing_important.push("AES-NI (ожидался)".into());
    }
    if has(&["vmx"]) || has(&["svm"]) {
        notes.push("Гостю отдали VT-x/AMD-V — возможен nested virtualization".into());
    }

    // Интересные сырые флаги (отсортированные), которые часто смотрят на VDS
    let watch = [
        "avx", "avx2", "avx512f", "avx512dq", "avx512bw", "avx512vl", "avx512_vnni",
        "aes", "sha_ni", "sha", "pclmulqdq", "rdrand", "rdseed", "bmi1", "bmi2", "adx",
        "fma", "f16c", "popcnt", "pdpe1gb", "ht", "hypervisor", "vmx", "svm",
        "constant_tsc", "nonstop_tsc", "ssbd", "ibt", "amx_tile", "md_clear",
    ];
    let mut raw_interesting: Vec<String> = watch
        .iter()
        .filter(|w| flags.iter().any(|f| f == **w || f.replace('-', "_") == w.replace('-', "_")))
        .map(|s| (*s).to_string())
        .collect();
    raw_interesting.sort();
    raw_interesting.dedup();

    present_highlights.sort();
    present_highlights.dedup();

    TechReport {
        available: true,
        total_flags: flags.len(),
        isa_level,
        generation_hint,
        categories,
        present_highlights,
        missing_important,
        raw_interesting,
        notes,
    }
}

fn detect_isa_level(flags: &BTreeSet<String>) -> String {
    let h = |s: &str| flags.contains(s);
    if h("amx_tile") || h("amx-tile") {
        "x86-64-v4 + AMX (Sapphire Rapids / Zen5 server class)".into()
    } else if h("avx512f") {
        "x86-64-v4 (AVX-512)".into()
    } else if h("avx2") && h("bmi2") && h("f16c") && h("fma") {
        "x86-64-v3 (AVX2 / BMI2 / FMA)".into()
    } else if h("sse4_2") && h("popcnt") && h("ssse3") {
        "x86-64-v2 (SSE4.2 / POPCNT)".into()
    } else if h("sse2") || h("lm") {
        "x86-64-v1 (базовый 64-bit)".into()
    } else if h("asimd") || h("neon") {
        "ARM NEON / ASIMD".into()
    } else {
        "неизвестно".into()
    }
}

fn detect_gen_hint(flags: &BTreeSet<String>) -> String {
    let h = |s: &str| flags.contains(s) || flags.contains(&s.replace('-', "_"));
    if h("amx_tile") {
        "Intel Sapphire Rapids+ / похожий класс".into()
    } else if h("avx_vnni") && h("avx512_bf16") {
        // Zen5 часто: AVX-VNNI + AVX512-BF16 без AMX
        "Zen5 / Zen4 desktop или Ice Lake+ (AVX-VNNI + BF16)".into()
    } else if h("avx512_bf16") || h("avx512bf16") {
        "Ice Lake / Cooper Lake / Zen4+ (AVX512-BF16)".into()
    } else if h("avx512f") && (h("avx512vnni") || h("avx512_vnni")) {
        "Cascade Lake / Ice Lake / Zen4 server".into()
    } else if h("avx512f") {
        "Skylake-SP / серверный AVX-512 класс".into()
    } else if h("avx2") && (h("sha_ni") || h("sha")) {
        "примерно Zen+ / Goldmont+ / Cannon Lake и новее".into()
    } else if h("avx2") {
        "Haswell / Excavator / Zen и новее (минимум AVX2)".into()
    } else if h("avx") {
        "Sandy Bridge / Bulldozer — AVX без AVX2 (старый)".into()
    } else if h("sse4_2") {
        "Nehalem / Barcelona-era и новее".into()
    } else {
        "старый или сильно урезанный (маски флагов?)".into()
    }
}

fn read_all_flags() -> BTreeSet<String> {
    #[allow(unused_mut)]
    let mut set = BTreeSet::new();

    #[cfg(target_os = "linux")]
    {
        if let Ok(content) = std::fs::read_to_string("/proc/cpuinfo") {
            for line in content.lines() {
                if line.starts_with("flags") || line.starts_with("Features") {
                    if let Some(rest) = line.split(':').nth(1) {
                        for f in rest.split_whitespace() {
                            set.insert(f.to_lowercase());
                        }
                    }
                    break; // достаточно первого CPU
                }
            }
        }
        // lscpu -J / Flags line
        if set.is_empty() {
            if let Ok(out) = std::process::Command::new("lscpu").output() {
                let s = String::from_utf8_lossy(&out.stdout);
                for line in s.lines() {
                    if line.to_lowercase().starts_with("flags:") {
                        if let Some(rest) = line.split(':').nth(1) {
                            for f in rest.split_whitespace() {
                                set.insert(f.to_lowercase());
                            }
                        }
                    }
                }
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        // Через PowerShell CIM — ограниченно, но хоть что-то
        if let Ok(out) = std::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                "(Get-CimInstance Win32_Processor).Caption",
            ])
            .output()
        {
            let _ = out;
        }
    }

    set
}

/// Быстрые булевы проверки для truth/oversell.
pub fn has_flag(report: &TechReport, id: &str) -> bool {
    report
        .categories
        .iter()
        .flat_map(|c| c.items.iter())
        .any(|i| i.id == id && i.present)
}
