//! Agenda (iCalendar : adresse iCal secrète ou fichier .ics) et contacts (fichiers vCard).

use chrono::{DateTime, Datelike, Duration, Local, NaiveDate, NaiveDateTime, TimeZone, Utc};

use crate::paths;

#[derive(Debug, Clone, PartialEq)]
pub struct Evenement {
    pub debut: DateTime<Local>,
    pub journee: bool,
    pub titre: String,
    pub lieu: String,
}

fn fetch(src: &str) -> Result<String, String> {
    if src.starts_with("http://") || src.starts_with("https://") || src.starts_with("webcal://") {
        let url = src.replacen("webcal://", "https://", 1);
        let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(std::time::Duration::from_secs(20))).build().into();
        let mut r = agent.get(&url).call().map_err(|e| e.to_string())?;
        r.body_mut().with_config().limit(20 * 1024 * 1024).read_to_string().map_err(|e| e.to_string())
    } else {
        std::fs::read_to_string(paths::expand(src)).map_err(|e| format!("{src} : {e}"))
    }
}

/// Lignes « dépliées » (RFC 5545 : une ligne qui commence par un espace continue la précédente).
fn unfold(txt: &str) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    for l in txt.replace("\r\n", "\n").lines() {
        if (l.starts_with(' ') || l.starts_with('\t')) && !out.is_empty() {
            out.last_mut().unwrap().push_str(&l[1..]);
        } else {
            out.push(l.to_string());
        }
    }
    out
}

fn unescape(s: &str) -> String {
    s.replace("\\n", " ").replace("\\N", " ").replace("\\,", ",").replace("\\;", ";").replace("\\\\", "\\")
}

/// `20261109` → journée entière ; `20261109T090000Z` → UTC ; sinon heure locale.
fn parse_dt(v: &str) -> Option<(DateTime<Local>, bool)> {
    let v = v.trim();
    if v.len() == 8 {
        let d = NaiveDate::parse_from_str(v, "%Y%m%d").ok()?;
        return Some((Local.from_local_datetime(&d.and_hms_opt(0, 0, 0)?).single()?, true));
    }
    if let Some(z) = v.strip_suffix('Z') {
        let n = NaiveDateTime::parse_from_str(z, "%Y%m%dT%H%M%S").ok()?;
        return Some((Utc.from_utc_datetime(&n).with_timezone(&Local), false));
    }
    let n = NaiveDateTime::parse_from_str(v, "%Y%m%dT%H%M%S").ok()?;
    Some((Local.from_local_datetime(&n).single()?, false))
}

/// Événements entre `from` et `to` (récurrences DAILY/WEEKLY/MONTHLY/YEARLY simples).
pub fn parse(ics: &str, from: DateTime<Local>, to: DateTime<Local>) -> Vec<Evenement> {
    let mut out = vec![];
    let mut cur: Option<(Option<(DateTime<Local>, bool)>, String, String, String)> = None;
    for l in unfold(ics) {
        if l == "BEGIN:VEVENT" {
            cur = Some((None, String::new(), String::new(), String::new()));
            continue;
        }
        if l == "END:VEVENT" {
            if let Some((Some((start, day)), titre, lieu, rrule)) = cur.take() {
                for d in occurrences(start, &rrule, from, to) {
                    out.push(Evenement { debut: d, journee: day, titre: unescape(&titre), lieu: unescape(&lieu) });
                }
            }
            continue;
        }
        let Some((ev_start, titre, lieu, rrule)) = cur.as_mut() else { continue };
        let Some((k, v)) = l.split_once(':') else { continue };
        let key = k.split(';').next().unwrap_or("");
        match key {
            "DTSTART" => *ev_start = parse_dt(v),
            "SUMMARY" => *titre = v.to_string(),
            "LOCATION" => *lieu = v.to_string(),
            "RRULE" => *rrule = v.to_string(),
            _ => {}
        }
    }
    out.sort_by_key(|e| e.debut);
    out
}

fn occurrences(start: DateTime<Local>, rrule: &str, from: DateTime<Local>, to: DateTime<Local>) -> Vec<DateTime<Local>> {
    let in_range = |d: DateTime<Local>| d >= from && d < to;
    if rrule.is_empty() {
        return if in_range(start) || (start < from && start + Duration::hours(1) > from) { vec![start] } else { vec![] };
    }
    let get = |k: &str| rrule.split(';').find_map(|p| p.strip_prefix(&format!("{k}="))).unwrap_or("").to_string();
    let interval: i64 = get("INTERVAL").parse().unwrap_or(1).max(1);
    let until = parse_dt(&get("UNTIL")).map(|(d, _)| d);
    let count: usize = get("COUNT").parse().unwrap_or(usize::MAX);
    let step = |d: DateTime<Local>| -> Option<DateTime<Local>> {
        Some(match get("FREQ").as_str() {
            "DAILY" => d + Duration::days(interval),
            "WEEKLY" => d + Duration::weeks(interval),
            "MONTHLY" => {
                let m0 = d.month0() as i64 + interval;
                let (y, m) = (d.year() + (m0 / 12) as i32, (m0 % 12) as u32 + 1);
                Local.from_local_datetime(&NaiveDate::from_ymd_opt(y, m, d.day())?.and_time(d.time())).single()?
            }
            "YEARLY" => Local.from_local_datetime(&NaiveDate::from_ymd_opt(d.year() + interval as i32, d.month(), d.day())?.and_time(d.time())).single()?,
            _ => return None,
        })
    };
    let mut v = vec![];
    let mut d = start;
    let mut n = 0;
    while d < to && n < count && n < 5000 {
        if until.is_some_and(|u| d > u) {
            break;
        }
        if in_range(d) {
            v.push(d);
        }
        n += 1;
        match step(d) {
            Some(nd) => d = nd,
            None => break,
        }
    }
    v
}

/// Événements des `jours` prochains jours, toutes sources confondues.
pub fn upcoming(sources: &[String], jours: i64) -> Result<Vec<Evenement>, String> {
    if sources.is_empty() {
        return Err("aucun agenda configuré (/regle agenda <adresse iCal secrète ou fichier .ics>)".into());
    }
    let now = Local::now();
    let from = Local.from_local_datetime(&now.date_naive().and_hms_opt(0, 0, 0).unwrap()).single().unwrap_or(now);
    let to = from + Duration::days(jours.clamp(1, 60));
    let mut all = vec![];
    let mut errs = vec![];
    for s in sources {
        match fetch(s) {
            Ok(t) => all.extend(parse(&t, from, to)),
            Err(e) => errs.push(e),
        }
    }
    if all.is_empty() && !errs.is_empty() {
        return Err(errs.join(" ; "));
    }
    all.sort_by_key(|e| e.debut);
    Ok(all)
}

/// Événements d'aujourd'hui en lignes courtes (« 10h Réunion »), pour l'accueil.
pub fn today_lines(sources: &[String]) -> Vec<String> {
    let today = Local::now().date_naive();
    upcoming(sources, 1)
        .unwrap_or_default()
        .into_iter()
        .filter(|e| e.debut.date_naive() == today)
        .take(4)
        .map(|e| if e.journee { e.titre } else { format!("{} {}", crate::fr::heure(&e.debut), e.titre) })
        .collect()
}

pub fn text(evs: &[Evenement]) -> String {
    if evs.is_empty() {
        return "Rien à l'agenda sur cette période.".into();
    }
    evs.iter()
        .map(|e| {
            let jour = crate::fr::date_longue(&e.debut);
            let jour = jour.split(',').next().unwrap_or("");
            let h = if e.journee { "journée".to_string() } else { crate::fr::heure(&e.debut) };
            let lieu = if e.lieu.is_empty() { String::new() } else { format!(" ({})", e.lieu) };
            format!("- {jour}, {h} : {}{lieu}", e.titre)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

// --- contacts (vCard) -------------------------------------------------------------------

/// Cherche un contact par nom, adresse ou numéro dans les fichiers vCard.
pub fn contacts(sources: &[String], q: &str) -> Result<String, String> {
    if sources.is_empty() {
        return Err("aucun carnet configuré (/regle contacts <fichier .vcf>, ex. export de Google Contacts)".into());
    }
    let q = q.to_lowercase();
    let mut hits = vec![];
    for s in sources {
        let txt = fetch(s)?;
        let mut card: Vec<String> = vec![];
        for l in unfold(&txt) {
            if l.starts_with("BEGIN:VCARD") {
                card.clear();
            } else if l.starts_with("END:VCARD") {
                let field = |k: &str| {
                    card.iter()
                        .filter(|c| c.split([':', ';']).next() == Some(k))
                        .filter_map(|c| c.split_once(':').map(|(_, v)| unescape(v)))
                        .collect::<Vec<_>>()
                };
                let nom = field("FN").first().cloned().unwrap_or_default();
                let (mails, tels) = (field("EMAIL"), field("TEL"));
                let all = format!("{nom} {} {}", mails.join(" "), tels.join(" ")).to_lowercase();
                if !q.is_empty() && all.contains(&q) {
                    hits.push(format!("- {nom} · {} · {}", mails.join(", "), tels.join(", ")));
                }
            } else {
                card.push(l);
            }
        }
    }
    hits.truncate(15);
    Ok(if hits.is_empty() { format!("Aucun contact pour « {q} ».") } else { hits.join("\n") })
}

#[cfg(test)]
mod tests {
    use super::*;

    const ICS: &str = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nDTSTART:20261109T060000Z\r\nSUMMARY:Réunion du\r\n  labo\r\nLOCATION:Ankatso\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nDTSTART;VALUE=DATE:20261110\r\nSUMMARY:Férié\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nDTSTART;TZID=Indian/Antananarivo:20261002T140000\r\nRRULE:FREQ=WEEKLY;INTERVAL=1\r\nSUMMARY:Cours M1\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";

    #[test]
    fn parses_events_and_weekly_rule() {
        let from = Local.with_ymd_and_hms(2026, 11, 9, 0, 0, 0).unwrap();
        let to = from + Duration::days(7);
        let e = parse(ICS, from, to);
        let titles: Vec<&str> = e.iter().map(|e| e.titre.as_str()).collect();
        assert!(titles.contains(&"Réunion du labo"));
        assert!(titles.contains(&"Férié"));
        // Le cours du vendredi 2 octobre revient chaque semaine : vendredi 13 novembre.
        let cours = e.iter().find(|e| e.titre == "Cours M1").unwrap();
        assert_eq!(cours.debut.date_naive(), NaiveDate::from_ymd_opt(2026, 11, 13).unwrap());
        assert!(e.iter().find(|e| e.titre == "Férié").unwrap().journee);
    }

    #[test]
    fn finds_contacts() {
        let dir = std::env::temp_dir().join(format!("lkatta-vcf-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("c.vcf");
        std::fs::write(&f, "BEGIN:VCARD\nFN:Hery Rakoto\nEMAIL;TYPE=work:hery@ex.mg\nTEL:+261 34 00 000 00\nEND:VCARD\n").unwrap();
        let r = contacts(&[f.to_string_lossy().into_owned()], "hery").unwrap();
        assert!(r.contains("Hery Rakoto · hery@ex.mg"));
        let _ = std::fs::remove_dir_all(dir);
    }
}
