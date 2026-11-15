
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct CatalogCpu {
    pub name: &'static str,
    
    pub aliases: &'static [&'static str],
    pub cores_typical: usize,
    pub gen: &'static str,
    pub notes: &'static str,
}

pub fn catalog() -> &'static [CatalogCpu] {
    &[
        CatalogCpu {
            name: "QEMU Virtual CPU / Common KVM",
            aliases: &["qemu virtual", "common kvm", "virtual cpu", "kvm processor"],
            cores_typical: 1,
            gen: "virtual-generic",
            notes: "Имя ничего не гарантирует — смотрите бенчмарк",
        },
        CatalogCpu {
            name: "Intel Atom / низкий shared",
            aliases: &["atom", "c2750", "c3758", "n2800"],
            cores_typical: 2,
            gen: "atom",
            notes: "Бюджетные VPS",
        },

        CatalogCpu {
            name: "Intel Xeon E5-2620 v2",
            aliases: &["e5-2620 v2", "e5-2620v2"],
            cores_typical: 6,
            gen: "Ivy Bridge-EP",
            notes: "Старый, часто oversold",
        },
        CatalogCpu {
            name: "Intel Xeon E5-2650 v2",
            aliases: &["e5-2650 v2", "e5-2650v2"],
            cores_typical: 8,
            gen: "Ivy Bridge-EP",
            notes: "Популярный shared VDS",
        },
        CatalogCpu {
            name: "Intel Xeon E5-2680 v2",
            aliases: &["e5-2680 v2", "e5-2680v2"],
            cores_typical: 10,
            gen: "Ivy Bridge-EP",
            notes: "Серверный процессор Ivy Bridge",
        },
        CatalogCpu {
            name: "Intel Xeon E5-2630 v3",
            aliases: &["e5-2630 v3", "e5-2630v3"],
            cores_typical: 8,
            gen: "Haswell-EP",
            notes: "",
        },
        CatalogCpu {
            name: "Intel Xeon E5-2650 v3",
            aliases: &["e5-2650 v3", "e5-2650v3"],
            cores_typical: 10,
            gen: "Haswell-EP",
            notes: "",
        },
        CatalogCpu {
            name: "Intel Xeon E5-2680 v3",
            aliases: &["e5-2680 v3", "e5-2680v3"],
            cores_typical: 12,
            gen: "Haswell-EP",
            notes: "Очень частый на VDS",
        },
        CatalogCpu {
            name: "Intel Xeon E5-2690 v3",
            aliases: &["e5-2690 v3", "e5-2690v3"],
            cores_typical: 12,
            gen: "Haswell-EP",
            notes: "",
        },
        CatalogCpu {
            name: "Intel Xeon E5-2620 v4",
            aliases: &["e5-2620 v4", "e5-2620v4"],
            cores_typical: 8,
            gen: "Broadwell-EP",
            notes: "",
        },
        CatalogCpu {
            name: "Intel Xeon E5-2650 v4",
            aliases: &["e5-2650 v4", "e5-2650v4"],
            cores_typical: 12,
            gen: "Broadwell-EP",
            notes: "Топ среди E5 на VDS",
        },
        CatalogCpu {
            name: "Intel Xeon E5-2680 v4",
            aliases: &["e5-2680 v4", "e5-2680v4"],
            cores_typical: 14,
            gen: "Broadwell-EP",
            notes: "Самый популярный E5 v4",
        },
        CatalogCpu {
            name: "Intel Xeon E5-2690 v4",
            aliases: &["e5-2690 v4", "e5-2690v4"],
            cores_typical: 14,
            gen: "Broadwell-EP",
            notes: "",
        },

        CatalogCpu {
            name: "Intel Xeon Silver 4214",
            aliases: &["silver 4214", "xeon silver 4214"],
            cores_typical: 12,
            gen: "Cascade Lake",
            notes: "",
        },
        CatalogCpu {
            name: "Intel Xeon Gold 5218",
            aliases: &["gold 5218", "5218"],
            cores_typical: 16,
            gen: "Cascade Lake",
            notes: "Частый у облаков",
        },
        CatalogCpu {
            name: "Intel Xeon Gold 6230",
            aliases: &["gold 6230", "6230"],
            cores_typical: 20,
            gen: "Cascade Lake",
            notes: "",
        },
        CatalogCpu {
            name: "Intel Xeon Gold 6248",
            aliases: &["gold 6248", "6248"],
            cores_typical: 20,
            gen: "Cascade Lake",
            notes: "",
        },
        CatalogCpu {
            name: "Intel Xeon Gold 6338",
            aliases: &["gold 6338", "6338"],
            cores_typical: 32,
            gen: "Ice Lake",
            notes: "Популярный dedicated/VDS",
        },
        CatalogCpu {
            name: "Intel Xeon Gold 6348",
            aliases: &["gold 6348", "6348"],
            cores_typical: 28,
            gen: "Ice Lake",
            notes: "",
        },
        CatalogCpu {
            name: "Intel Xeon Gold 6430",
            aliases: &["gold 6430", "6430"],
            cores_typical: 32,
            gen: "Sapphire Rapids",
            notes: "",
        },
        CatalogCpu {
            name: "Intel Xeon Platinum 8259CL",
            aliases: &["8259cl", "platinum 8259"],
            cores_typical: 24,
            gen: "Cascade Lake (AWS)",
            notes: "AWS-класс",
        },
        CatalogCpu {
            name: "Intel Xeon Platinum 8375C",
            aliases: &["8375c", "platinum 8375"],
            cores_typical: 32,
            gen: "Ice Lake (AWS)",
            notes: "",
        },
        CatalogCpu {
            name: "Intel Xeon Platinum 8380",
            aliases: &["platinum 8380", "8380"],
            cores_typical: 40,
            gen: "Ice Lake",
            notes: "Топовый Ice Lake",
        },
        CatalogCpu {
            name: "Intel Xeon Platinum 8480+",
            aliases: &["platinum 8480", "8480"],
            cores_typical: 56,
            gen: "Sapphire Rapids",
            notes: "Топ Intel SPR",
        },

        CatalogCpu {
            name: "AMD EPYC 7302",
            aliases: &["epyc 7302", "7302"],
            cores_typical: 16,
            gen: "Zen2 Rome",
            notes: "Популярный VDS",
        },
        CatalogCpu {
            name: "AMD EPYC 7402",
            aliases: &["epyc 7402", "7402"],
            cores_typical: 24,
            gen: "Zen2 Rome",
            notes: "",
        },
        CatalogCpu {
            name: "AMD EPYC 7502",
            aliases: &["epyc 7502", "7502"],
            cores_typical: 32,
            gen: "Zen2 Rome",
            notes: "",
        },
        CatalogCpu {
            name: "AMD EPYC 7542",
            aliases: &["epyc 7542", "7542"],
            cores_typical: 32,
            gen: "Zen2 Rome",
            notes: "",
        },
        CatalogCpu {
            name: "AMD EPYC 7742",
            aliases: &["epyc 7742", "7742"],
            cores_typical: 64,
            gen: "Zen2 Rome",
            notes: "Часто режут на vCPU",
        },
        CatalogCpu {
            name: "AMD EPYC 7B12 / 7B13 (GCP)",
            aliases: &["7b12", "7b13", "epyc 7b"],
            cores_typical: 64,
            gen: "Zen2 Rome (GCP)",
            notes: "Google Cloud",
        },

        CatalogCpu {
            name: "AMD EPYC 7313",
            aliases: &["epyc 7313", "7313"],
            cores_typical: 16,
            gen: "Zen3 Milan",
            notes: "Отличный VDS-класс",
        },
        CatalogCpu {
            name: "AMD EPYC 7443 / 7443P",
            aliases: &["epyc 7443", "7443p", "7443"],
            cores_typical: 24,
            gen: "Zen3 Milan",
            notes: "Очень популярный dedicated/VDS",
        },
        CatalogCpu {
            name: "AMD EPYC 7543",
            aliases: &["epyc 7543", "7543"],
            cores_typical: 32,
            gen: "Zen3 Milan",
            notes: "",
        },
        CatalogCpu {
            name: "AMD EPYC 7713",
            aliases: &["epyc 7713", "7713"],
            cores_typical: 64,
            gen: "Zen3 Milan",
            notes: "",
        },
        CatalogCpu {
            name: "AMD EPYC 7763",
            aliases: &["epyc 7763", "7763"],
            cores_typical: 64,
            gen: "Zen3 Milan",
            notes: "Топ Milan, часто в тарифах",
        },

        CatalogCpu {
            name: "AMD EPYC 9124",
            aliases: &["epyc 9124", "9124"],
            cores_typical: 16,
            gen: "Zen4 Genoa",
            notes: "",
        },
        CatalogCpu {
            name: "AMD EPYC 9354",
            aliases: &["epyc 9354", "9354"],
            cores_typical: 32,
            gen: "Zen4 Genoa",
            notes: "Популярный Genoa VDS",
        },
        CatalogCpu {
            name: "AMD EPYC 9454",
            aliases: &["epyc 9454", "9454"],
            cores_typical: 48,
            gen: "Zen4 Genoa",
            notes: "",
        },
        CatalogCpu {
            name: "AMD EPYC 9554",
            aliases: &["epyc 9554", "9554"],
            cores_typical: 64,
            gen: "Zen4 Genoa",
            notes: "",
        },
        CatalogCpu {
            name: "AMD EPYC 9654",
            aliases: &["epyc 9654", "9654"],
            cores_typical: 96,
            gen: "Zen4 Genoa",
            notes: "Топовый Genoa",
        },
        CatalogCpu {
            name: "AMD EPYC 9754 (Bergamo)",
            aliases: &["epyc 9754", "9754", "bergamo"],
            cores_typical: 128,
            gen: "Zen4c Bergamo",
            notes: "Плотные ядра",
        },
        CatalogCpu {
            name: "AMD Ryzen 5 3600 / 3600X",
            aliases: &["ryzen 5 3600", "3600x"],
            cores_typical: 6,
            gen: "Zen2",
            notes: "",
        },
        CatalogCpu {
            name: "AMD Ryzen 7 3700X / 3800X",
            aliases: &["ryzen 7 3700", "3700x", "3800x", "ryzen 7 3800"],
            cores_typical: 8,
            gen: "Zen2",
            notes: "",
        },
        CatalogCpu {
            name: "AMD Ryzen 9 3900X / 3950X",
            aliases: &["ryzen 9 3900", "3900x", "3950x", "ryzen 9 3950"],
            cores_typical: 12,
            gen: "Zen2",
            notes: "",
        },

        CatalogCpu {
            name: "AMD Ryzen 5 5500 / 5600G",
            aliases: &["ryzen 5 5500", "5600g", "ryzen 5 5600g"],
            cores_typical: 6,
            gen: "Zen3",
            notes: "",
        },
        CatalogCpu {
            name: "AMD Ryzen 5 5600 / 5600X",
            aliases: &["ryzen 5 5600x", "ryzen 5 5600", "5600x"],
            cores_typical: 6,
            gen: "Zen3",
            notes: "Частый Hetzner/dedicated",
        },
        CatalogCpu {
            name: "AMD Ryzen 5 5600X3D",
            aliases: &["5600x3d", "ryzen 5 5600x3d"],
            cores_typical: 6,
            gen: "Zen3",
            notes: "Большой L3",
        },
        CatalogCpu {
            name: "AMD Ryzen 7 5700G / 5700X",
            aliases: &["5700g", "ryzen 7 5700x", "ryzen 7 5700"],
            cores_typical: 8,
            gen: "Zen3",
            notes: "",
        },
        CatalogCpu {
            name: "AMD Ryzen 7 5700X3D / 5800X3D",
            aliases: &["5700x3d", "5800x3d", "ryzen 7 5800x3d"],
            cores_typical: 8,
            gen: "Zen3",
            notes: "",
        },
        CatalogCpu {
            name: "AMD Ryzen 7 5800X / 5800XT",
            aliases: &["5800x", "5800xt", "ryzen 7 5800"],
            cores_typical: 8,
            gen: "Zen3",
            notes: "Частый dedicated",
        },
        CatalogCpu {
            name: "AMD Ryzen 9 5900X / 5900XT",
            aliases: &["ryzen 9 5900", "5900x", "5900xt"],
            cores_typical: 12,
            gen: "Zen3",
            notes: "",
        },
        CatalogCpu {
            name: "AMD Ryzen 9 5950X",
            aliases: &["ryzen 9 5950", "5950x"],
            cores_typical: 16,
            gen: "Zen3",
            notes: "Топ Zen3 desktop",
        },

        CatalogCpu {
            name: "AMD Ryzen 5 7500F / 7600 / 7600X",
            aliases: &["7500f", "ryzen 5 7600", "7600x", "ryzen 5 7500", "ryzen 5 7600x"],
            cores_typical: 6,
            gen: "Zen4",
            notes: "",
        },
        CatalogCpu {
            name: "AMD Ryzen 7 7700 / 7700X",
            aliases: &["ryzen 7 7700", "7700x"],
            cores_typical: 8,
            gen: "Zen4",
            notes: "В госте KVM ST обычно ниже desktop",
        },
        CatalogCpu {
            name: "AMD Ryzen 7 7800X3D",
            aliases: &["7800x3d", "ryzen 7 7800x3d"],
            cores_typical: 8,
            gen: "Zen4",
            notes: "Игровой X3D",
        },
        CatalogCpu {
            name: "AMD Ryzen 9 7900 / 7900X",
            aliases: &["ryzen 9 7900", "7900x"],
            cores_typical: 12,
            gen: "Zen4",
            notes: "",
        },
        CatalogCpu {
            name: "AMD Ryzen 9 7900X3D",
            aliases: &["7900x3d"],
            cores_typical: 12,
            gen: "Zen4",
            notes: "",
        },
        CatalogCpu {
            name: "AMD Ryzen 9 7950X / 7950X3D",
            aliases: &["ryzen 9 7950x", "7950x", "ryzen 9 7950", "7950x3d", "ryzen 9 7950x3d"],
            cores_typical: 16,
            gen: "Zen4",
            notes: "Топ desktop для dedicated",
        },

        CatalogCpu {
            name: "AMD Ryzen 5 9600 / 9600X",
            aliases: &["ryzen 5 9600", "9600x"],
            cores_typical: 6,
            gen: "Zen5",
            notes: "",
        },
        CatalogCpu {
            name: "AMD Ryzen 7 9700X",
            aliases: &["ryzen 7 9700", "9700x"],
            cores_typical: 8,
            gen: "Zen5",
            notes: "",
        },
        CatalogCpu {
            name: "AMD Ryzen 9 9900X / 9900X3D",
            aliases: &["ryzen 9 9900", "9900x", "9900x3d"],
            cores_typical: 12,
            gen: "Zen5",
            notes: "",
        },
        CatalogCpu {
            name: "AMD Ryzen 9 9950X / 9950X3D",
            aliases: &["ryzen 9 9950", "9950x", "9950x3d"],
            cores_typical: 16,
            gen: "Zen5",
            notes: "При малом числе vCPU сравнение производительности ограничено",
        },

        CatalogCpu {
            name: "AMD Ryzen Threadripper 3960X / 3970X",
            aliases: &["3960x", "3970x", "threadripper 3960", "threadripper 3970"],
            cores_typical: 24,
            gen: "Zen2",
            notes: "",
        },
        CatalogCpu {
            name: "AMD Ryzen Threadripper 5960X / 5975WX",
            aliases: &["5960x", "5975wx", "threadripper 5960", "threadripper 5975"],
            cores_typical: 24,
            gen: "Zen3",
            notes: "",
        },
        CatalogCpu {
            name: "AMD Ryzen Threadripper 7960X / 7970X",
            aliases: &["7960x", "7970x", "threadripper 7960", "threadripper 7970"],
            cores_typical: 24,
            gen: "Zen4",
            notes: "",
        },

        CatalogCpu {
            name: "Intel Core i5-10400 / 11400 / 11400F",
            aliases: &["i5-10400", "i5-11400", "10400f", "11400f"],
            cores_typical: 6,
            gen: "Comet/Rocket Lake",
            notes: "",
        },
        CatalogCpu {
            name: "Intel Core i7-10700 / 11700",
            aliases: &["i7-10700", "i7-11700", "10700k", "11700k"],
            cores_typical: 8,
            gen: "Comet/Rocket Lake",
            notes: "",
        },
        CatalogCpu {
            name: "Intel Core i9-10900 / 11900",
            aliases: &["i9-10900", "i9-11900", "10900k", "11900k"],
            cores_typical: 10,
            gen: "Comet/Rocket Lake",
            notes: "",
        },

        CatalogCpu {
            name: "Intel Core i3-12100 / 12100F",
            aliases: &["i3-12100", "12100f"],
            cores_typical: 4,
            gen: "Alder Lake",
            notes: "",
        },
        CatalogCpu {
            name: "Intel Core i5-12400 / 12400F",
            aliases: &["i5-12400", "12400f"],
            cores_typical: 10,
            gen: "Alder Lake",
            notes: "",
        },
        CatalogCpu {
            name: "Intel Core i5-12600 / 12600K",
            aliases: &["i5-12600", "12600k", "12600kf"],
            cores_typical: 10,
            gen: "Alder Lake",
            notes: "",
        },
        CatalogCpu {
            name: "Intel Core i7-12700 / 12700K",
            aliases: &["i7-12700", "12700k", "12700kf", "12700f"],
            cores_typical: 12,
            gen: "Alder Lake",
            notes: "",
        },
        CatalogCpu {
            name: "Intel Core i9-12900 / 12900K",
            aliases: &["i9-12900", "12900k", "12900kf"],
            cores_typical: 16,
            gen: "Alder Lake",
            notes: "",
        },

        CatalogCpu {
            name: "Intel Core i5-13400 / 13500 / 13600K",
            aliases: &["i5-13400", "i5-13500", "i5-13600", "13400f", "13600k"],
            cores_typical: 10,
            gen: "Raptor Lake",
            notes: "",
        },
        CatalogCpu {
            name: "Intel Core i7-13700 / 13700K",
            aliases: &["i7-13700", "13700k", "13700kf", "13700f"],
            cores_typical: 16,
            gen: "Raptor Lake",
            notes: "",
        },
        CatalogCpu {
            name: "Intel Core i9-13900 / 13900K",
            aliases: &["i9-13900", "13900k", "13900kf"],
            cores_typical: 24,
            gen: "Raptor Lake",
            notes: "Топ dedicated",
        },

        CatalogCpu {
            name: "Intel Core i5-14400 / 14500 / 14600K",
            aliases: &["i5-14400", "i5-14500", "i5-14600", "14400f", "14600k"],
            cores_typical: 14,
            gen: "Raptor Lake Refresh",
            notes: "",
        },
        CatalogCpu {
            name: "Intel Core i7-14700 / 14700K",
            aliases: &["i7-14700", "14700k", "14700kf", "14700f"],
            cores_typical: 20,
            gen: "Raptor Lake Refresh",
            notes: "",
        },
        CatalogCpu {
            name: "Intel Core i9-14900 / 14900K",
            aliases: &["i9-14900", "14900k", "14900kf"],
            cores_typical: 24,
            gen: "Raptor Lake Refresh",
            notes: "",
        },

        CatalogCpu {
            name: "Intel Core Ultra 5 245K / 245KF",
            aliases: &["ultra 5 245", "245kf"],
            cores_typical: 14,
            gen: "Arrow Lake",
            notes: "",
        },
        CatalogCpu {
            name: "Intel Core Ultra 7 265K / 265KF",
            aliases: &["ultra 7 265", "265kf"],
            cores_typical: 20,
            gen: "Arrow Lake",
            notes: "",
        },
        CatalogCpu {
            name: "Intel Core Ultra 9 285K",
            aliases: &["ultra 9 285", "285k"],
            cores_typical: 24,
            gen: "Arrow Lake",
            notes: "",
        },

        CatalogCpu {
            name: "Intel Xeon E-2236 / E-2276G / E-2288G",
            aliases: &["e-2236", "e-2276", "e-2288", "e-2176", "e-2136"],
            cores_typical: 6,
            gen: "Coffee Lake",
            notes: "Частый у Hetzner/OVH dedicated",
        },
        CatalogCpu {
            name: "Intel Xeon E-2388G / E-2378",
            aliases: &["e-2388", "e-2378", "e-2336", "e-2356"],
            cores_typical: 8,
            gen: "Rocket Lake",
            notes: "",
        },
        CatalogCpu {
            name: "Intel Xeon Silver 4210 / 4214R",
            aliases: &["silver 4210", "4210r", "4214r"],
            cores_typical: 10,
            gen: "Cascade Lake",
            notes: "",
        },
        CatalogCpu {
            name: "Intel Xeon Gold 5220 / 6240 / 6242",
            aliases: &["gold 5220", "gold 6240", "gold 6242"],
            cores_typical: 18,
            gen: "Cascade Lake",
            notes: "",
        },
        CatalogCpu {
            name: "Intel Xeon Gold 5318Y / 5320",
            aliases: &["gold 5318", "gold 5320", "5318y"],
            cores_typical: 24,
            gen: "Ice Lake",
            notes: "",
        },
        CatalogCpu {
            name: "Intel Xeon Gold 5418Y / 5420+",
            aliases: &["gold 5418", "gold 5420", "5418y", "5420+"],
            cores_typical: 24,
            gen: "Sapphire Rapids",
            notes: "",
        },
        CatalogCpu {
            name: "Intel Xeon Gold 6448Y / 6454S",
            aliases: &["6448y", "6454s", "gold 6448", "gold 6454"],
            cores_typical: 32,
            gen: "Sapphire Rapids",
            notes: "",
        },
        CatalogCpu {
            name: "Intel Xeon Silver 4310 / 4314 / 4316",
            aliases: &["silver 4310", "silver 4314", "silver 4316"],
            cores_typical: 12,
            gen: "Ice Lake",
            notes: "",
        },

        CatalogCpu {
            name: "AMD EPYC 7252 / 7272",
            aliases: &["epyc 7252", "epyc 7272"],
            cores_typical: 8,
            gen: "Zen2 Rome",
            notes: "",
        },
        CatalogCpu {
            name: "AMD EPYC 7452 / 7532",
            aliases: &["epyc 7452", "epyc 7532"],
            cores_typical: 32,
            gen: "Zen2 Rome",
            notes: "",
        },
        CatalogCpu {
            name: "AMD EPYC 7343 / 7413",
            aliases: &["epyc 7343", "epyc 7413"],
            cores_typical: 16,
            gen: "Zen3 Milan",
            notes: "",
        },
        CatalogCpu {
            name: "AMD EPYC 7513 / 7643",
            aliases: &["epyc 7513", "epyc 7643"],
            cores_typical: 32,
            gen: "Zen3 Milan",
            notes: "",
        },
        CatalogCpu {
            name: "AMD EPYC 9254 / 9334",
            aliases: &["epyc 9254", "epyc 9334"],
            cores_typical: 24,
            gen: "Zen4 Genoa",
            notes: "",
        },
        CatalogCpu {
            name: "AMD EPYC 9474F / 9534",
            aliases: &["epyc 9474", "epyc 9534", "9474f"],
            cores_typical: 48,
            gen: "Zen4 Genoa",
            notes: "F = высокая частота",
        },
        CatalogCpu {
            name: "AMD EPYC 9174F / 9184X",
            aliases: &["epyc 9174", "epyc 9184", "9174f", "9184x"],
            cores_typical: 16,
            gen: "Zen4 Genoa",
            notes: "",
        },
        CatalogCpu {
            name: "AMD EPYC 9965 / 9755 (Turin)",
            aliases: &["epyc 9965", "epyc 9755", "turin"],
            cores_typical: 128,
            gen: "Zen5",
            notes: "Turin — смотрите family 1Ah",
        },

        CatalogCpu {
            name: "AMD EPYC 9015",
            aliases: &["epyc 9015", "9015"],
            cores_typical: 8,
            gen: "Zen5",
            notes: "Нет эталона nmos",
        },
        CatalogCpu {
            name: "AMD EPYC 9115",
            aliases: &["epyc 9115", "9115"],
            cores_typical: 16,
            gen: "Zen5",
            notes: "Нет эталона nmos",
        },
        CatalogCpu {
            name: "AMD EPYC 9135",
            aliases: &["epyc 9135", "9135"],
            cores_typical: 16,
            gen: "Zen5",
            notes: "Нет эталона nmos",
        },
        CatalogCpu {
            name: "AMD EPYC 9175F",
            aliases: &["epyc 9175f", "9175f"],
            cores_typical: 16,
            gen: "Zen5",
            notes: "Нет эталона nmos",
        },
        CatalogCpu {
            name: "AMD EPYC 9255",
            aliases: &["epyc 9255", "9255"],
            cores_typical: 24,
            gen: "Zen5",
            notes: "Нет эталона nmos",
        },
        CatalogCpu {
            name: "AMD EPYC 9275F",
            aliases: &["epyc 9275f", "9275f"],
            cores_typical: 24,
            gen: "Zen5",
            notes: "Нет эталона nmos",
        },
        CatalogCpu {
            name: "AMD EPYC 9335",
            aliases: &["epyc 9335", "9335"],
            cores_typical: 32,
            gen: "Zen5",
            notes: "Нет эталона nmos",
        },
        CatalogCpu {
            name: "AMD EPYC 9355",
            aliases: &["epyc 9355", "9355"],
            cores_typical: 32,
            gen: "Zen5",
            notes: "Нет эталона nmos",
        },
        CatalogCpu {
            name: "AMD EPYC 9355P",
            aliases: &["epyc 9355p", "9355p"],
            cores_typical: 32,
            gen: "Zen5",
            notes: "Нет эталона nmos",
        },
        CatalogCpu {
            name: "AMD EPYC 9365",
            aliases: &["epyc 9365", "9365"],
            cores_typical: 36,
            gen: "Zen5",
            notes: "Нет эталона nmos",
        },
        CatalogCpu {
            name: "AMD EPYC 9375F",
            aliases: &["epyc 9375f", "9375f"],
            cores_typical: 32,
            gen: "Zen5",
            notes: "Нет эталона nmos",
        },
        CatalogCpu {
            name: "AMD EPYC 9455",
            aliases: &["epyc 9455", "9455"],
            cores_typical: 48,
            gen: "Zen5",
            notes: "Нет эталона nmos",
        },
        CatalogCpu {
            name: "AMD EPYC 9455P",
            aliases: &["epyc 9455p", "9455p"],
            cores_typical: 48,
            gen: "Zen5",
            notes: "Нет эталона nmos",
        },
        CatalogCpu {
            name: "AMD EPYC 9475F",
            aliases: &["epyc 9475f", "9475f"],
            cores_typical: 48,
            gen: "Zen5",
            notes: "Нет эталона nmos",
        },
        CatalogCpu {
            name: "AMD EPYC 9535",
            aliases: &["epyc 9535", "9535"],
            cores_typical: 64,
            gen: "Zen5",
            notes: "Нет эталона nmos",
        },
        CatalogCpu {
            name: "AMD EPYC 9555",
            aliases: &["epyc 9555", "9555"],
            cores_typical: 64,
            gen: "Zen5",
            notes: "Нет эталона nmos",
        },
        CatalogCpu {
            name: "AMD EPYC 9555P",
            aliases: &["epyc 9555p", "9555p"],
            cores_typical: 64,
            gen: "Zen5",
            notes: "Нет эталона nmos",
        },
        CatalogCpu {
            name: "AMD EPYC 9565",
            aliases: &["epyc 9565", "9565"],
            cores_typical: 72,
            gen: "Zen5",
            notes: "Нет эталона nmos",
        },
        CatalogCpu {
            name: "AMD EPYC 9575F",
            aliases: &["epyc 9575f", "9575f"],
            cores_typical: 64,
            gen: "Zen5",
            notes: "Нет эталона nmos",
        },
        CatalogCpu {
            name: "AMD EPYC 9645",
            aliases: &["epyc 9645", "9645"],
            cores_typical: 96,
            gen: "Zen5",
            notes: "Нет эталона nmos",
        },
        CatalogCpu {
            name: "AMD EPYC 9655",
            aliases: &["epyc 9655", "9655"],
            cores_typical: 96,
            gen: "Zen5",
            notes: "Нет эталона nmos",
        },
        CatalogCpu {
            name: "AMD EPYC 9655P",
            aliases: &["epyc 9655p", "9655p"],
            cores_typical: 96,
            gen: "Zen5",
            notes: "Нет эталона nmos",
        },
        CatalogCpu {
            name: "AMD EPYC 9745",
            aliases: &["epyc 9745", "9745"],
            cores_typical: 128,
            gen: "Zen5",
            notes: "Нет эталона nmos",
        },
        CatalogCpu {
            name: "AMD EPYC 9825",
            aliases: &["epyc 9825", "9825"],
            cores_typical: 144,
            gen: "Zen5",
            notes: "Нет эталона nmos",
        },
        CatalogCpu {
            name: "AMD EPYC 9845",
            aliases: &["epyc 9845", "9845"],
            cores_typical: 160,
            gen: "Zen5",
            notes: "Нет эталона nmos",
        },
        CatalogCpu {
            name: "Ampere Altra",
            aliases: &["ampere altra", "altra"],
            cores_typical: 80,
            gen: "Neoverse N1",
            notes: "ARM VDS",
        },
        CatalogCpu {
            name: "Ampere Altra Max",
            aliases: &["altra max"],
            cores_typical: 128,
            gen: "Neoverse N1",
            notes: "",
        },
        CatalogCpu {
            name: "AWS Graviton2",
            aliases: &["graviton2"],
            cores_typical: 64,
            gen: "Neoverse N1",
            notes: "",
        },
        CatalogCpu {
            name: "AWS Graviton3",
            aliases: &["graviton3", "neoverse-v1"],
            cores_typical: 64,
            gen: "Neoverse V1",
            notes: "",
        },
        CatalogCpu {
            name: "AWS Graviton4",
            aliases: &["graviton4", "neoverse-v2"],
            cores_typical: 96,
            gen: "Neoverse V2",
            notes: "",
        },
        CatalogCpu {
            name: "Intel Xeon 6980P",
            aliases: &["xeon 6980p", "6980p"],
            cores_typical: 128,
            gen: "Granite Rapids",
            notes: "Intel Xeon 6 P-core; Intel ARK",
        },
        CatalogCpu {
            name: "Intel Xeon 6960P",
            aliases: &["xeon 6960p", "6960p"],
            cores_typical: 72,
            gen: "Granite Rapids",
            notes: "Intel Xeon 6 P-core; Intel ARK",
        },
        CatalogCpu {
            name: "AMD EPYC 9996",
            aliases: &["epyc 9996", "9996"],
            cores_typical: 256,
            gen: "Zen6/Zen6c",
            notes: "AMD EPYC 9006; AMD specifications",
        },
        CatalogCpu {
            name: "AMD EPYC 9966",
            aliases: &["epyc 9966", "9966"],
            cores_typical: 192,
            gen: "Zen6/Zen6c",
            notes: "AMD EPYC 9006; AMD specifications",
        },
        CatalogCpu {
            name: "AMD EPYC 9846",
            aliases: &["epyc 9846", "9846"],
            cores_typical: 168,
            gen: "Zen6/Zen6c",
            notes: "AMD EPYC 9006; AMD specifications",
        },
        CatalogCpu {
            name: "AMD EPYC 9756",
            aliases: &["epyc 9756", "9756"],
            cores_typical: 128,
            gen: "Zen6/Zen6c",
            notes: "AMD EPYC 9006; AMD specifications",
        },
    ]
}


pub fn match_model(model: &str) -> Option<&'static CatalogCpu> {
    let m = model.to_lowercase();
    let mut best: Option<(&CatalogCpu, i32)> = None;
    for cpu in catalog() {
        for alias in cpu.aliases {
            let a = *alias;
            if a.len() < 4 {
                continue;
            }
            
            if a.chars().all(|c| c.is_ascii_digit()) && a.len() < 5 && a != m {
                continue;
            }
            let Some(pos) = m.find(a) else {
                continue;
            };
            
            let before_ok = pos == 0
                || !m.as_bytes()[pos - 1].is_ascii_alphanumeric();
            let after = pos + a.len();
            let after_ok = after >= m.len()
                || !m.as_bytes()[after].is_ascii_alphanumeric();
            if !after_ok { continue; }
            
            let mostly_sku = a
                .chars()
                .filter(|c| c.is_ascii_alphanumeric())
                .all(|c| c.is_ascii_digit() || matches!(c, 'x' | 'f' | 'k' | 'r' | 'g' | 'p' | 'y'));
            if mostly_sku && a.len() <= 8 && !(before_ok && after_ok) {
                
                let after_ch = m.as_bytes().get(after).copied();
                if after_ch.is_some_and(|c| c.is_ascii_digit()) {
                    continue;
                }
                if !before_ok {
                    continue;
                }
            }

            let model_x3d = m.contains("x3d");
            let alias_x3d = a.contains("x3d");
            if model_x3d != alias_x3d {
                continue;
            }

            let mut score = (a.len() as i32) * 10
                + if a.contains(' ') || a.contains('-') {
                    8
                } else {
                    0
                };
            if before_ok && after_ok {
                score += 15;
            }
            if a.contains("ryzen")
                || a.contains("epyc")
                || a.contains("xeon")
                || a.contains("threadripper")
                || a.contains("i5-")
                || a.contains("i7-")
                || a.contains("i9-")
                || a.contains("ultra")
                || a.contains("silver")
                || a.contains("gold")
                || a.contains("platinum")
            {
                score += 25;
            }
            if alias_x3d {
                score += 30;
            }
            
            
            if best.map(|(_, s)| score > s).unwrap_or(true) {
                best = Some((cpu, score));
            }
        }
    }
    best.map(|(c, _)| c)
}


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaimModelMatch {
    Confirmed,
    Contradicted,
    Unverifiable,
}

pub fn compare_claim_model(claimed: &str, actual: &str) -> ClaimModelMatch {
    let claimed = claimed.trim();
    let actual = actual.trim();
    if claimed.is_empty() || actual.is_empty() {
        return ClaimModelMatch::Unverifiable;
    }
    let claim_catalog = match_model(claimed);
    let actual_catalog = match_model(actual);
    let (Some(claim_catalog), Some(actual_catalog)) = (claim_catalog, actual_catalog) else {
        return ClaimModelMatch::Unverifiable;
    };
    if claim_catalog.gen == "virtual-generic" || actual_catalog.gen == "virtual-generic" {
        return ClaimModelMatch::Unverifiable;
    }
    if claim_catalog.name != actual_catalog.name {
        return ClaimModelMatch::Contradicted;
    }
    match (model_sku(claimed), model_sku(actual)) {
        (Some(claim), Some(fact)) if claim == fact => ClaimModelMatch::Confirmed,
        (Some(_), Some(_)) => ClaimModelMatch::Contradicted,
        _ => ClaimModelMatch::Unverifiable,
    }
}

fn model_sku(value: &str) -> Option<String> {
    value.to_lowercase()
        .split_whitespace()
        .map(|part| part.trim_matches(|ch: char| !ch.is_ascii_alphanumeric() && ch != '-'))
        .find(|part| part.len() >= 4 && part.chars().any(|ch| ch.is_ascii_digit()))
        .map(str::to_string)
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifies_added_models_and_suffixes() {
        for (input, expected) in [
            ("AMD EPYC 9655 96-Core Processor", "AMD EPYC 9655"),
            ("AMD EPYC 9655P 96-Core Processor", "AMD EPYC 9655P"),
            ("AMD EPYC 9475F 48-Core Processor", "AMD EPYC 9475F"),
            ("9655", "AMD EPYC 9655"),
            ("AMD Ryzen 9 7950X 16-Core Processor", "AMD Ryzen 9 7950X / 7950X3D"),
        ] {
            assert_eq!(match_model(input).map(|cpu| cpu.name), Some(expected), "{input}");
        }
        assert!(match_model("AMD EPYC 96550").is_none());
    }

    #[test]
    fn compares_exact_sku_without_treating_generic_names_as_proof() {
        assert_eq!(compare_claim_model("AMD EPYC 9655", "AMD EPYC 9655 96-Core Processor"), ClaimModelMatch::Confirmed);
        assert_eq!(compare_claim_model("AMD EPYC 9655", "AMD EPYC 9655P"), ClaimModelMatch::Contradicted);
        assert_eq!(compare_claim_model("Intel Xeon Gold 5220", "Intel Xeon Gold 6240"), ClaimModelMatch::Contradicted);
        assert_eq!(compare_claim_model("AMD EPYC 9655", "QEMU Virtual CPU"), ClaimModelMatch::Unverifiable);
        assert_eq!(compare_claim_model("AMD EPYC", "AMD EPYC 9655"), ClaimModelMatch::Unverifiable);
    }

}
