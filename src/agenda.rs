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
    // Bornes : un agenda distant ne doit pas pouvoir faire déborder les dates.
    let interval: i64 = get("INTERVAL").parse().unwrap_or(1).clamp(1, 1000);
    let until = parse_dt(&get("UNTIL")).map(|(d, _)| d);
    let count: usize = get("COUNT").parse().unwrap_or(usize::MAX);
    let step = |d: DateTime<Local>| -> Option<DateTime<Local>> {
        Some(match get("FREQ").as_str() {
            "DAILY" => d.checked_add_signed(Duration::days(interval))?,
            "WEEKLY" => d.checked_add_signed(Duration::weeks(interval))?,
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

// --- agenda et carnet locaux (ce que L Katta crée lui-même) ---------------------------------

/// Agenda tenu par L Katta : lu avec les autres, et c'est là qu'il ajoute les événements.
pub fn agenda_local() -> std::path::PathBuf {
    #[cfg(test)]
    if let Some(d) = ESSAI.with(|d| d.borrow().clone()) {
        return d.join("agenda.ics");
    }
    paths::data_dir().join("agenda.ics")
}

pub fn carnet_local() -> std::path::PathBuf {
    #[cfg(test)]
    if let Some(d) = ESSAI.with(|d| d.borrow().clone()) {
        return d.join("contacts.vcf");
    }
    paths::data_dir().join("contacts.vcf")
}

#[cfg(test)]
thread_local! {
    static ESSAI: std::cell::RefCell<Option<std::path::PathBuf>> = const { std::cell::RefCell::new(None) };
}

/// Sources configurées, plus le fichier local s'il existe.
fn avec_local(sources: &[String], local: std::path::PathBuf) -> Vec<String> {
    let mut v = sources.to_vec();
    if local.exists() {
        v.push(local.to_string_lossy().into_owned());
    }
    v
}

/// Y a-t-il un agenda à lire (configuré ou local) ?
pub fn a_un_agenda(sources: &[String]) -> bool {
    !sources.is_empty() || agenda_local().exists()
}

/// Texte iCalendar/vCard échappé (RFC 5545 §3.3.11).
fn echapper(s: &str) -> String {
    s.replace('\\', "\\\\").replace(';', "\\;").replace(',', "\\,").replace("\r\n", "\\n").replace(['\n', '\r'], "\\n")
}

/// Lignes de 75 octets au plus (les suivantes commencent par une espace).
fn plier(l: &str) -> String {
    let mut out = String::new();
    let mut n = 0;
    for ch in l.chars() {
        if n + ch.len_utf8() > 74 {
            out.push_str("\r\n ");
            n = 1;
        }
        out.push(ch);
        n += ch.len_utf8();
    }
    out.push_str("\r\n");
    out
}

/// Un événement à créer.
#[derive(Debug, Clone, PartialEq)]
pub struct Nouveau {
    pub titre: String,
    pub debut: DateTime<Local>,
    pub duree_min: i64,
    pub journee: bool,
    pub lieu: String,
    pub note: String,
}

fn vevent(e: &Nouveau, uid: &str) -> String {
    let stamp = Utc::now().format("%Y%m%dT%H%M%SZ");
    let mut v = format!("BEGIN:VEVENT\r\nUID:{uid}\r\nDTSTAMP:{stamp}\r\n");
    if e.journee {
        let d = e.debut.date_naive();
        v.push_str(&format!("DTSTART;VALUE=DATE:{}\r\nDTEND;VALUE=DATE:{}\r\n", d.format("%Y%m%d"), (d + Duration::days(1)).format("%Y%m%d")));
    } else {
        let f = |d: DateTime<Local>| d.with_timezone(&Utc).format("%Y%m%dT%H%M%SZ").to_string();
        v.push_str(&format!("DTSTART:{}\r\nDTEND:{}\r\n", f(e.debut), f(e.debut + Duration::minutes(e.duree_min.clamp(5, 24 * 60)))));
    }
    v.push_str(&plier(&format!("SUMMARY:{}", echapper(&e.titre))));
    if !e.lieu.is_empty() {
        v.push_str(&plier(&format!("LOCATION:{}", echapper(&e.lieu))));
    }
    if !e.note.is_empty() {
        v.push_str(&plier(&format!("DESCRIPTION:{}", echapper(&e.note))));
    }
    v.push_str("END:VEVENT\r\n");
    v
}

const ENTETE_ICS: &str = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//L Katta//FR\r\nCALSCALE:GREGORIAN\r\n";

/// Ajoute l'événement à l'agenda local ; renvoie aussi une invitation .ics (un fichier par
/// événement, à joindre à un mail ou à ouvrir dans l'agenda de son choix).
pub fn ajouter(e: &Nouveau) -> Result<std::path::PathBuf, String> {
    if e.titre.trim().is_empty() {
        return Err(t!("il faut un titre").into());
    }
    let uid = format!("{}-{}@lkatta", Utc::now().timestamp_millis(), fastrand::u32(..));
    let ev = vevent(e, &uid);
    let f = agenda_local();
    if let Some(d) = f.parent() {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    let ancien = std::fs::read_to_string(&f).unwrap_or_default();
    let corps = ancien.trim_end().strip_suffix("END:VCALENDAR").map(str::to_string).unwrap_or_else(|| ENTETE_ICS.to_string());
    let corps = if corps.is_empty() || !corps.contains("BEGIN:VCALENDAR") { ENTETE_ICS.to_string() } else { corps };
    let corps = if corps.ends_with('\n') { corps } else { format!("{corps}\r\n") };
    paths::write_private(&f, format!("{corps}{ev}END:VCALENDAR\r\n").as_bytes()).map_err(|e| e.to_string())?;
    let inv_dir = f.parent().unwrap_or(std::path::Path::new(".")).join("invitations");
    std::fs::create_dir_all(&inv_dir).map_err(|e| e.to_string())?;
    let nom: String = e.titre.chars().map(|c| if c.is_alphanumeric() || c == '-' { c } else { '-' }).take(40).collect();
    let inv = inv_dir.join(format!("{}-{}.ics", e.debut.format("%Y-%m-%d"), nom.trim_matches('-')));
    std::fs::write(&inv, format!("{ENTETE_ICS}METHOD:PUBLISH\r\n{ev}END:VCALENDAR\r\n")).map_err(|e| e.to_string())?;
    Ok(inv)
}

/// Retire de l'agenda local les événements dont le titre contient `titre`. Renvoie les titres
/// retirés (les agendas en ligne ne sont jamais modifiés).
pub fn retirer(titre: &str) -> Result<Vec<String>, String> {
    let q = titre.trim().to_lowercase();
    if q.is_empty() {
        return Err(t!("il faut un titre").into());
    }
    let f = agenda_local();
    let txt = std::fs::read_to_string(&f).map_err(|_| t!("l'agenda local est vide").to_string())?;
    let mut out = String::new();
    let mut bloc: Option<String> = None;
    let mut retires = vec![];
    for l in txt.split_inclusive('\n') {
        let nu = l.trim_end();
        if nu == "BEGIN:VEVENT" {
            bloc = Some(l.to_string());
        } else if let Some(b) = bloc.as_mut() {
            b.push_str(l);
            if nu == "END:VEVENT" {
                let b = bloc.take().unwrap_or_default();
                let titre_ev = unfold(&b).iter().find_map(|x| x.strip_prefix("SUMMARY:").map(unescape)).unwrap_or_default();
                if titre_ev.to_lowercase().contains(&q) {
                    retires.push(titre_ev);
                } else {
                    out.push_str(&b);
                }
            }
        } else {
            out.push_str(l);
        }
    }
    if !retires.is_empty() {
        paths::write_private(&f, out.as_bytes()).map_err(|e| e.to_string())?;
    }
    Ok(retires)
}

/// Ajoute un contact au carnet local (vCard 3.0).
pub fn ajouter_contact(nom: &str, email: &str, tel: &str) -> Result<(), String> {
    let nom = nom.trim();
    if nom.is_empty() || (email.trim().is_empty() && tel.trim().is_empty()) {
        return Err(t!("il faut un nom et une adresse ou un numéro").into());
    }
    if !email.trim().is_empty() && !email.contains('@') {
        return Err(t!("adresse invalide").into());
    }
    let mut v = format!("BEGIN:VCARD\r\nVERSION:3.0\r\n{}", plier(&format!("FN:{}", echapper(nom))));
    if !email.trim().is_empty() {
        v.push_str(&plier(&format!("EMAIL:{}", echapper(email.trim()))));
    }
    if !tel.trim().is_empty() {
        v.push_str(&plier(&format!("TEL:{}", echapper(tel.trim()))));
    }
    v.push_str("END:VCARD\r\n");
    let f = carnet_local();
    if let Some(d) = f.parent() {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    let ancien = std::fs::read_to_string(&f).unwrap_or_default();
    paths::write_private(&f, format!("{ancien}{v}").as_bytes()).map_err(|e| e.to_string())
}

/// Événements des `jours` prochains jours, toutes sources confondues (agenda local compris).
pub fn upcoming(sources: &[String], jours: i64) -> Result<Vec<Evenement>, String> {
    let sources = avec_local(sources, agenda_local());
    let sources = &sources[..];
    if sources.is_empty() {
        return Err(t!("aucun agenda configuré (/regle agenda <adresse iCal secrète ou fichier .ics>)").into());
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
        return t!("Rien à l'agenda sur cette période.").into();
    }
    evs.iter()
        .map(|e| {
            let jour = crate::fr::date_longue(&e.debut);
            // Sans l'heure finale (« lundi 9 novembre 2026 », « Monday, November 9, 2026 »).
            let jour = jour.rsplit_once(',').map(|(j, _)| j).unwrap_or(&jour);
            let h = if e.journee { t!("journée").to_string() } else { crate::fr::heure(&e.debut) };
            let lieu = if e.lieu.is_empty() { String::new() } else { format!(" ({})", e.lieu) };
            t!("- {}, {} : {}{}", jour, h, e.titre, lieu)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

// --- contacts (vCard) -------------------------------------------------------------------

/// Cherche un contact par nom, adresse ou numéro dans les fichiers vCard.
pub fn contacts(sources: &[String], q: &str) -> Result<String, String> {
    let sources = avec_local(sources, carnet_local());
    let sources = &sources[..];
    if sources.is_empty() {
        return Err(t!("aucun carnet configuré (/regle contacts <fichier .vcf>, ex. export de Google Contacts)").into());
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
    Ok(if hits.is_empty() { t!("Aucun contact pour « {} ».", q) } else { hits.join("\n") })
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
    fn creates_and_removes_local_events() {
        let d = std::env::temp_dir().join(format!("lkatta-agenda-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        ESSAI.with(|e| *e.borrow_mut() = Some(d.clone()));
        let debut = Local::now() + Duration::days(1);
        let e = Nouveau { titre: "Soutenance, salle 12; M1".into(), debut, duree_min: 90, journee: false, lieu: "Ankatso".into(), note: String::new() };
        let inv = ajouter(&e).unwrap();
        ajouter(&Nouveau { titre: "Férié".into(), journee: true, ..e.clone() }).unwrap();
        assert!(std::fs::read_to_string(&inv).unwrap().contains("SUMMARY:Soutenance\\, salle 12\\; M1"));
        let evs = upcoming(&[], 3).unwrap();
        assert!(evs.iter().any(|x| x.titre == "Soutenance, salle 12; M1" && x.lieu == "Ankatso" && (x.debut - debut).num_seconds().abs() < 2));
        assert!(evs.iter().any(|x| x.titre == "Férié" && x.journee));
        assert_eq!(retirer("soutenance").unwrap(), vec!["Soutenance, salle 12; M1".to_string()]);
        assert_eq!(upcoming(&[], 3).unwrap().len(), 1);
        ajouter_contact("Voahangy R.", "voa@ex.mg", "").unwrap();
        assert!(contacts(&[], "voa").unwrap().contains("Voahangy R. · voa@ex.mg"));
        assert!(ajouter_contact("X", "", "").is_err());
        let _ = std::fs::remove_dir_all(&d);
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
