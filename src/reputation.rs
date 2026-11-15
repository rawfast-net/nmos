use crate::dns::Resolver;
use rayon::prelude::*;
use serde::Serialize;
use std::net::Ipv4Addr;

#[derive(Debug, Clone, Serialize)]
pub struct ReputationReport {
    pub ip: String,
    pub checks: Vec<BlocklistCheck>,
    pub listed: usize,
    pub policy: usize,
    pub other_flags: usize,
    pub clear: usize,
    pub inconclusive: usize,
    pub unsupported: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct BlocklistCheck {
    pub name: &'static str,
    pub zone: &'static str,
    pub source: &'static str,
    pub status: &'static str,
    pub answers: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Copy)]
enum Kind {
    Mail,
    Wide,
    Backscatter,
    Bogon,
    Spfbl,
}

#[derive(Clone, Copy)]
struct Zone {
    name: &'static str,
    domain: &'static str,
    kind: Kind,
}

const ZONES: &[Zone] = &[
    Zone {
        name: "BARRACUDACENTRAL",
        domain: "b.barracudacentral.org",
        kind: Kind::Mail,
    },
    Zone {
        name: "FABELSOURCES",
        domain: "spamsources.fabel.dk",
        kind: Kind::Mail,
    },
    Zone {
        name: "KEMPTBL",
        domain: "dnsbl.kempt.net",
        kind: Kind::Mail,
    },
    Zone {
        name: "PSBL",
        domain: "psbl.surriel.com",
        kind: Kind::Mail,
    },
    Zone {
        name: "SCHULTE",
        domain: "rbl.schulte.org",
        kind: Kind::Mail,
    },
    Zone {
        name: "SPAMCOP",
        domain: "bl.spamcop.net",
        kind: Kind::Mail,
    },
    Zone {
        name: "UCEPROTECTL1",
        domain: "dnsbl-1.uceprotect.net",
        kind: Kind::Mail,
    },
    Zone {
        name: "UCEPROTECTL2",
        domain: "dnsbl-2.uceprotect.net",
        kind: Kind::Wide,
    },
    Zone {
        name: "UCEPROTECTL3",
        domain: "dnsbl-3.uceprotect.net",
        kind: Kind::Wide,
    },
    Zone {
        name: "DRONEBL",
        domain: "dnsbl.dronebl.org",
        kind: Kind::Mail,
    },
    Zone {
        name: "SINGULARIS",
        domain: "singular.ttk.pte.hu",
        kind: Kind::Mail,
    },
    Zone {
        name: "BACKSCATTERER",
        domain: "ips.backscatterer.org",
        kind: Kind::Backscatter,
    },
    Zone {
        name: "INTERSERVER",
        domain: "rbl.interserver.net",
        kind: Kind::Mail,
    },
    Zone {
        name: "BLOCKLISTDE",
        domain: "bl.blocklist.de",
        kind: Kind::Mail,
    },
    Zone {
        name: "TEAMCYMRUV4",
        domain: "v4.fullbogons.cymru.com",
        kind: Kind::Bogon,
    },
    Zone {
        name: "TEAMCYMRU",
        domain: "bogons.cymru.com",
        kind: Kind::Bogon,
    },
    Zone {
        name: "GBUDBNET",
        domain: "truncate.gbudb.net",
        kind: Kind::Mail,
    },
    Zone {
        name: "PEDANTICORG",
        domain: "spam.pedantic.org",
        kind: Kind::Mail,
    },
    Zone {
        name: "ANONMAILSDE",
        domain: "spam.dnsbl.anonmails.de",
        kind: Kind::Mail,
    },
    Zone {
        name: "JIPPG",
        domain: "mail-abuse.blacklist.jippg.org",
        kind: Kind::Mail,
    },
    Zone {
        name: "MAILSPIKE",
        domain: "bl.mailspike.net",
        kind: Kind::Mail,
    },
    Zone {
        name: "0SPAM",
        domain: "rbl.0spam.org",
        kind: Kind::Mail,
    },
    Zone {
        name: "SPFBL",
        domain: "dnsbl.spfbl.net",
        kind: Kind::Spfbl,
    },
];

pub fn check(ip: &str) -> Option<ReputationReport> {
    let addr: Ipv4Addr = ip.parse().ok()?;
    if !public_ipv4(addr) {
        return None;
    }
    let resolver = Resolver::system();
    let octets = addr.octets();
    let reverse = format!("{}.{}.{}.{}", octets[3], octets[2], octets[1], octets[0]);
    let zen = check_zen(resolver.as_ref(), &reverse);
    let mut checks = Vec::new();
    checks.push(zen.clone());
    for (name, codes, policy_only) in [
        ("CBL", &[4u8][..], false),
        ("SBL", &[2, 3, 9][..], false),
        ("SBL-PBL", &[2, 3, 9, 10, 11][..], true),
        ("SBL-XBL", &[2, 3, 4, 5, 6, 7, 8, 9][..], false),
        ("XBL", &[4, 5, 6, 7][..], false),
        ("PBL", &[10, 11][..], true),
    ] {
        checks.push(derived(name, codes, policy_only, &zen));
    }
    checks.push(BlocklistCheck {
        name: "SERVICESNET",
        zone: "—",
        source: "справочно",
        status: "unsupported",
        answers: Vec::new(),
        detail: "Действующая DNS-зона не подтверждена".into(),
    });
    checks.push(BlocklistCheck {
        name: "USENIX",
        zone: "—",
        source: "справочно",
        status: "unsupported",
        answers: Vec::new(),
        detail: "USENIX применяет чужие списки; собственной публичной DNSBL нет".into(),
    });
    let direct: Vec<_> = if let Some(resolver) = resolver.as_ref() {
        rayon::ThreadPoolBuilder::new()
            .num_threads(6)
            .build()
            .ok()
            .map(|pool| {
                pool.install(|| {
                    ZONES
                        .par_iter()
                        .map(|zone| check_zone(resolver, *zone, &reverse))
                        .collect()
                })
            })
            .unwrap_or_else(|| {
                ZONES
                    .iter()
                    .map(|zone| check_zone(resolver, *zone, &reverse))
                    .collect()
            })
    } else {
        ZONES
            .iter()
            .map(|zone| unavailable(zone.name, zone.domain, "DNS-сервер не найден"))
            .collect()
    };
    checks.extend(direct);
    let independent: Vec<_> = checks.iter().filter(|c| c.source == "DNSBL").collect();
    let listed = independent.iter().filter(|c| c.status == "listed").count();
    let policy = independent.iter().filter(|c| c.status == "policy").count();
    let other_flags = independent.iter().filter(|c| c.status == "other").count();
    let clear = independent
        .iter()
        .filter(|c| c.status == "not_listed")
        .count();
    let inconclusive = independent.len() - listed - policy - other_flags - clear;
    let unsupported = checks.iter().filter(|c| c.status == "unsupported").count();
    Some(ReputationReport {
        ip: ip.into(),
        checks,
        listed,
        policy,
        other_flags,
        clear,
        inconclusive,
        unsupported,
    })
}

fn public_ipv4(addr: Ipv4Addr) -> bool {
    let o = addr.octets();
    !(addr.is_private()
        || addr.is_loopback()
        || addr.is_link_local()
        || addr.is_broadcast()
        || addr.is_documentation()
        || addr.is_unspecified()
        || o[0] >= 224
        || o[0] == 0
        || (o[0] == 100 && (64..=127).contains(&o[1]))
        || (o[0] == 198 && (o[1] == 18 || o[1] == 19)))
}

fn check_zen(resolver: Option<&Resolver>, reverse: &str) -> BlocklistCheck {
    let zone = Zone {
        name: "Spamhaus ZEN",
        domain: "zen.spamhaus.org",
        kind: Kind::Mail,
    };
    let Some(resolver) = resolver else {
        return unavailable(zone.name, zone.domain, "DNS-сервер не найден");
    };
    let control = resolver.query("2.0.0.127.zen.spamhaus.org", 1);
    let negative = resolver.query("1.0.0.127.zen.spamhaus.org", 1);
    match (control, negative) {
        (Ok(positive), Ok(negative))
            if positive.rcode == 0
                && !positive.records.is_empty()
                && !has_error_code(&positive.records)
                && negative.rcode == 3 =>
        {
            let mut check = check_zone(resolver, zone, reverse);
            if check.status == "listed" && pbl_only(&check.answers) {
                check.status = "policy";
                check.detail =
                    "Только PBL: прямую отправку почты ограничивает политика сети".into();
            }
            check
        }
        _ => unavailable(
            zone.name,
            zone.domain,
            "Контрольные запросы Spamhaus не прошли; возможна блокировка публичного резолвера",
        ),
    }
}

fn check_zone(resolver: &Resolver, zone: Zone, reverse: &str) -> BlocklistCheck {
    if zone.domain == "b.barracudacentral.org" {
        match resolver.query("2.0.0.127.b.barracudacentral.org", 1) {
            Ok(control) if control.records.contains(&Ipv4Addr::new(127, 0, 0, 2)) => {}
            _ => return unavailable(
                zone.name,
                zone.domain,
                "Контрольный запрос Barracuda не прошёл; возможно, DNS-резолвер не зарегистрирован",
            ),
        }
    }
    let query = format!("{reverse}.{}", zone.domain);
    let answer = match resolver.query(&query, 1) {
        Ok(answer) => answer,
        Err(error) => return unavailable(zone.name, zone.domain, &error),
    };
    if answer.rcode == 3 {
        return match resolver.query(zone.domain, 2) {
            Ok(probe) if probe.rcode == 0 && probe.count > 0 => BlocklistCheck {
                name: zone.name,
                zone: zone.domain,
                source: "DNSBL",
                status: "not_listed",
                answers: Vec::new(),
                detail: "NXDOMAIN; DNS-зона отвечает".into(),
            },
            _ => unavailable(
                zone.name,
                zone.domain,
                "Зона не подтвердила работу; NXDOMAIN не считается чистым результатом",
            ),
        };
    }
    if answer.rcode != 0 {
        return unavailable(
            zone.name,
            zone.domain,
            &format!("DNS-код ошибки {}", answer.rcode),
        );
    }
    let codes: Vec<_> = answer.records.iter().map(ToString::to_string).collect();
    if answer.records.is_empty() {
        return BlocklistCheck {
            name: zone.name,
            zone: zone.domain,
            source: "DNSBL",
            status: "inconclusive",
            answers: codes,
            detail: "Ответ без A-записи".into(),
        };
    }
    if has_error_code(&answer.records) {
        return unavailable(zone.name, zone.domain, "DNSBL вернула служебный код ошибки");
    }
    if answer.records.iter().any(|ip| ip.octets()[0] != 127) {
        return BlocklistCheck {
            name: zone.name,
            zone: zone.domain,
            source: "DNSBL",
            status: "inconclusive",
            answers: codes,
            detail: "Неожиданный адрес в DNS-ответе".into(),
        };
    }
    match resolver.query(&format!("1.0.0.127.{}", zone.domain), 1) {
        Ok(control) if control.rcode == 3 => {}
        _ => {
            return unavailable(
                zone.name,
                zone.domain,
                "Контрольный незаблокированный IP не дал NXDOMAIN",
            )
        }
    }
    let (status, detail) = match zone.kind {
        Kind::Mail => ("listed", "IP присутствует в списке"),
        Kind::Wide => (
            "listed",
            "Список охватывает сеть или ASN; это не прямое доказательство спама данного IP",
        ),
        Kind::Backscatter => (
            "other",
            "Обнаружен backscatter; это не список рассылки спама",
        ),
        Kind::Bogon => ("other", "Адрес отмечен как bogon; это не спам-база"),
        Kind::Spfbl if codes.iter().all(|code| code == "127.0.0.4") => (
            "policy",
            "Адрес не подходит для почтового сервера по политике SPFBL",
        ),
        Kind::Spfbl => ("listed", "Репутационная отметка SPFBL; смотрите код ответа"),
    };
    BlocklistCheck {
        name: zone.name,
        zone: zone.domain,
        source: "DNSBL",
        status,
        answers: codes,
        detail: detail.into(),
    }
}

fn derived(
    name: &'static str,
    codes: &[u8],
    policy_only: bool,
    zen: &BlocklistCheck,
) -> BlocklistCheck {
    let matches: Vec<_> = zen
        .answers
        .iter()
        .filter(|answer| {
            answer.parse::<Ipv4Addr>().ok().is_some_and(|ip| {
                let o = ip.octets();
                o[0..3] == [127, 0, 0] && codes.contains(&o[3])
            })
        })
        .cloned()
        .collect();
    let status = if matches.is_empty() {
        if matches!(zen.status, "listed" | "policy" | "not_listed") {
            "not_listed"
        } else {
            "unavailable"
        }
    } else if policy_only && pbl_only(&matches) {
        "policy"
    } else {
        "listed"
    };
    BlocklistCheck {
        name,
        zone: zen.zone,
        source: "из ZEN",
        status,
        answers: matches,
        detail: if status == "unavailable" {
            "Исходный ответ ZEN недоступен".into()
        } else if status == "policy" {
            "PBL: ограничение прямой почтовой отправки".into()
        } else {
            "Выведено из кодов Spamhaus ZEN без повторного запроса".into()
        },
    }
}

fn pbl_only(codes: &[String]) -> bool {
    !codes.is_empty()
        && codes
            .iter()
            .all(|code| matches!(code.as_str(), "127.0.0.10" | "127.0.0.11"))
}

fn has_error_code(records: &[Ipv4Addr]) -> bool {
    records
        .iter()
        .any(|ip| ip.octets()[0..3] == [127, 255, 255])
}

fn unavailable(name: &'static str, zone: &'static str, detail: &str) -> BlocklistCheck {
    BlocklistCheck {
        name,
        zone,
        source: "DNSBL",
        status: "unavailable",
        answers: Vec::new(),
        detail: detail.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_spamhaus_codes_without_duplicate_queries() {
        let zen = BlocklistCheck {
            name: "Spamhaus ZEN",
            zone: "zen.spamhaus.org",
            source: "DNSBL",
            status: "listed",
            answers: vec!["127.0.0.4".into(), "127.0.0.10".into()],
            detail: String::new(),
        };
        assert_eq!(derived("CBL", &[4], false, &zen).status, "listed");
        assert_eq!(derived("PBL", &[10, 11], true, &zen).status, "policy");
        assert_eq!(derived("SBL", &[2, 3, 9], false, &zen).status, "not_listed");
    }

    #[test]
    fn rejects_non_public_ip() {
        for ip in ["127.0.0.1", "10.0.0.1", "192.168.1.1", "invalid", "::1"] {
            assert!(check(ip).is_none());
        }
    }
}
