//! Briefing du jour : le matin, une bulle qui résume la journée (météo, agenda, rappels, choses à
//! faire, mails non lus, machine) ; le soir, un petit bilan (ce qui est fait, ce qui reste,
//! demain). Une fois par jour chacun, à l'heure réglée (`[briefing]` dans config.toml).
//!
//! Le maki tourne souvent des jours entiers : l'accueil du démarrage ne suffit pas. Si tu es
//! absent ou occupé (visio, plein écran), la bulle attend ton retour.
//!
//! Météo : seulement si une ville est réglée et qu'Internet est permis. Open-Meteo, sans compte ;
//! seul le nom de la ville (puis ses coordonnées) part sur le réseau.

use chrono::{DateTime, Datelike, Duration, Local, Timelike};
use serde::{Deserialize, Serialize};

use crate::conseils::{Kind, Message};
use crate::config::Config;
use crate::{fr, paths};

#[derive(Debug, Default, Serialize, Deserialize)]
struct Etat {
    /// Jour (AAAA-MM-JJ) du dernier briefing du matin, du dernier bilan du soir.
    #[serde(default)]
    matin: String,
    #[serde(default)]
    soir: String,
    /// Coordonnées de la ville réglée (évite de les redemander chaque jour).
    #[serde(default)]
    ville: Option<(String, f64, f64)>,
}

fn fichier() -> std::path::PathBuf {
    paths::data_dir().join("briefing.json")
}

fn charger() -> Etat {
    std::fs::read_to_string(fichier()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

fn enregistrer(e: &Etat) {
    if let Ok(s) = serde_json::to_string(e) {
        let _ = paths::write_private(&fichier(), s.as_bytes());
    }
}

fn jour(d: &DateTime<Local>) -> String {
    d.format("%Y-%m-%d").to_string()
}

/// L'accueil du démarrage vient de donner les nouvelles du matin : pas de second briefing.
pub fn accueil_fait(now: DateTime<Local>) {
    if (5..13).contains(&now.hour()) {
        let mut e = charger();
        e.matin = jour(&now);
        enregistrer(&e);
    }
}

/// Que faut-il envoyer maintenant ? (matin, soir), d'après l'heure et ce qui est déjà fait.
fn a_faire(cfg: &Config, now: DateTime<Local>, e: &Etat) -> (bool, bool) {
    let (h, j) = (now.hour(), jour(&now));
    let b = &cfg.briefing;
    let matin = b.matin_h > 0 && h >= b.matin_h && h < 13.max(b.matin_h + 3) && e.matin != j;
    let soir = b.soir_h > 0 && h >= b.soir_h && h <= 23 && e.soir != j;
    (matin, soir)
}

/// Tourne en arrière-plan : vérifie toutes les cinq minutes s'il est l'heure d'un briefing.
pub fn surveiller(envoyer: impl Fn(Message) + Send + 'static) {
    std::thread::spawn(move || {
        // Pas tout de suite après l'accueil du démarrage.
        std::thread::sleep(std::time::Duration::from_secs(10 * 60));
        loop {
            let cfg = Config::load();
            let now = Local::now();
            let mut e = charger();
            let (matin, soir) = a_faire(&cfg, now, &e);
            if matin {
                e.matin = jour(&now);
                enregistrer(&e);
                envoyer(du_matin(&cfg, now));
            } else if soir {
                e.soir = jour(&now);
                enregistrer(&e);
                envoyer(du_soir(&cfg, now));
            }
            std::thread::sleep(std::time::Duration::from_secs(5 * 60));
        }
    });
}

/// Briefing du matin.
pub fn du_matin(cfg: &Config, now: DateTime<Local>) -> Message {
    let mut l = vec![];
    if let Some(m) = meteo(&cfg.briefing.ville) {
        l.push(m);
    }
    let agenda = crate::agenda::today_lines(&cfg.agenda);
    l.push(if agenda.is_empty() { t!("Agenda : rien de prévu aujourd'hui.").to_string() } else { t!("Agenda : {}.", agenda.join(" · ")) });
    let rappels = crate::reminders::today_pending(now);
    if let Some(r) = rappels.first() {
        let quand = r.date().map(|d| fr::heure(&d)).unwrap_or_default();
        l.push(if rappels.len() == 1 { t!("Rappel : {} à {}.", r.texte, quand) } else { t!("{} rappels, le premier : {} à {}.", rappels.len(), r.texte, quand) });
    }
    let choses = crate::afaire::du_jour(now);
    if !choses.is_empty() {
        let noms: Vec<String> = choses.iter().take(3).map(|c| c.texte.clone()).collect();
        let plus = if choses.len() > 3 { t!(" (et {} autres)", choses.len() - 3) } else { String::new() };
        l.push(t!("À faire : {}{}.", noms.join(" · "), plus));
    }
    if let Some(n) = mails_non_lus(cfg) {
        if n > 0 {
            l.push(if n == 1 { t!("1 mail non lu.").to_string() } else { t!("{} mails non lus.", n) });
        }
    }
    if let Some(a) = crate::sante::bilan_rapide().first() {
        l.push(t!("Machine : {}.", a.titre.to_lowercase()));
    }
    if l.len() <= 1 && now.weekday().num_days_from_monday() >= 5 {
        l.push(t!("C'est le week-end : rien d'urgent de mon côté.").into());
    }
    Message::new(Kind::Hello, t!("Ta journée, {}", cfg.prenom), l.join("\n"))
}

/// Bilan du soir.
pub fn du_soir(cfg: &Config, now: DateTime<Local>) -> Message {
    let mut l = vec![];
    let aujourdhui = now.date_naive();
    let faites: Vec<String> = crate::afaire::charger()
        .into_iter()
        .filter(|c| c.fait && c.faite_le.as_deref().and_then(|d| DateTime::parse_from_rfc3339(d).ok()).is_some_and(|d| d.with_timezone(&Local).date_naive() == aujourdhui))
        .map(|c| c.texte)
        .collect();
    if faites.is_empty() {
        l.push(t!("Rien de coché dans la liste aujourd'hui.").to_string());
    } else {
        l.push(t!("Fait aujourd'hui : {}.", faites.iter().take(4).cloned().collect::<Vec<_>>().join(" · ")));
    }
    let reste = crate::afaire::du_jour(now).len();
    if reste > 0 {
        l.push(if reste == 1 { t!("Il reste 1 chose prévue pour aujourd'hui.").to_string() } else { t!("Il reste {} choses prévues pour aujourd'hui.", reste) });
    }
    // Demain : agenda et rappels.
    let demain = aujourdhui + Duration::days(1);
    let evs: Vec<String> = crate::agenda::upcoming(&cfg.agenda, 2)
        .unwrap_or_default()
        .into_iter()
        .filter(|e| e.debut.date_naive() == demain)
        .take(3)
        .map(|e| if e.journee { e.titre } else { format!("{} {}", fr::heure(&e.debut), e.titre) })
        .collect();
    let rappels = crate::reminders::load().into_iter().filter(|r| !r.fait && r.date().is_some_and(|d| d.date_naive() == demain)).count();
    match (evs.is_empty(), rappels) {
        (true, 0) => l.push(t!("Demain : rien à l'agenda.").into()),
        (false, 0) => l.push(t!("Demain : {}.", evs.join(" · "))),
        (true, n) => l.push(t!("Demain : {} rappel(s).", n)),
        (false, n) => l.push(t!("Demain : {} ; {} rappel(s).", evs.join(" · "), n)),
    }
    l.push(if now.hour() >= 21 { t!("Il se fait tard : pense à te reposer.") } else { t!("Bonne soirée !") }.into());
    Message::new(Kind::Hello, t!("Bilan du jour"), l.join("\n"))
}

/// Mails non lus, tous comptes confondus (`None` : aucun compte, ou aucun n'a répondu).
fn mails_non_lus(cfg: &Config) -> Option<usize> {
    if cfg.mail.is_empty() || !crate::regles::load().internet || !crate::regles::load().lire_mails {
        return None;
    }
    let n: Vec<usize> = cfg.mail.iter().filter_map(|c| crate::mail::list(c, true, 99).ok()).map(|v| v.len()).collect();
    (!n.is_empty()).then(|| n.iter().sum())
}

/// Météo du jour pour la ville réglée (Open-Meteo, sans compte), en une ligne.
pub fn meteo(ville: &str) -> Option<String> {
    let ville = ville.trim();
    if ville.is_empty() || !crate::regles::load().internet {
        return None;
    }
    let mut e = charger();
    let (lat, lon) = match &e.ville {
        Some((v, la, lo)) if v == ville => (*la, *lo),
        _ => {
            let langue = if crate::lang::en() { "en" } else { "fr" };
            let url = format!("https://geocoding-api.open-meteo.com/v1/search?count=1&language={langue}&name={}", crate::web::encode(ville));
            let v: serde_json::Value = serde_json::from_str(&crate::web::get_text(&url).ok()?).ok()?;
            let r = v.get("results")?.get(0)?;
            let (la, lo) = (r.get("latitude")?.as_f64()?, r.get("longitude")?.as_f64()?);
            e.ville = Some((ville.to_string(), la, lo));
            enregistrer(&e);
            (la, lo)
        }
    };
    let url = format!(
        "https://api.open-meteo.com/v1/forecast?latitude={lat:.3}&longitude={lon:.3}&daily=weather_code,temperature_2m_max,temperature_2m_min,precipitation_probability_max&timezone=auto&forecast_days=1"
    );
    let v: serde_json::Value = serde_json::from_str(&crate::web::get_text(&url).ok()?).ok()?;
    let d = v.get("daily")?;
    let premier = |k: &str| d.get(k).and_then(|x| x.get(0)).and_then(|x| x.as_f64());
    let (code, max, min) = (premier("weather_code")?, premier("temperature_2m_max")?, premier("temperature_2m_min")?);
    let pluie = premier("precipitation_probability_max").map(|p| t!(", pluie {} %", p.round())).unwrap_or_default();
    Some(t!("Météo à {} : {}, {} à {} °C{}.", ville, ciel(code as u32), min.round(), max.round(), pluie))
}

/// Code météo de l'OMS (WMO) → quelques mots.
fn ciel(code: u32) -> &'static str {
    match code {
        0 => t!("ciel dégagé"),
        1 | 2 => t!("quelques nuages"),
        3 => t!("couvert"),
        45 | 48 => t!("brouillard"),
        51..=57 => t!("bruine"),
        61..=67 => t!("pluie"),
        71..=77 => t!("neige"),
        80..=82 => t!("averses"),
        85 | 86 => t!("averses de neige"),
        95..=99 => t!("orages"),
        _ => t!("temps variable"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn cfg(matin: u32, soir: u32) -> Config {
        let mut c = Config::default();
        c.briefing.matin_h = matin;
        c.briefing.soir_h = soir;
        c
    }

    #[test]
    fn once_a_day_at_the_set_hours() {
        let e = Etat::default();
        let a = |h: u32| Local.with_ymd_and_hms(2026, 11, 16, h, 5, 0).unwrap();
        assert_eq!(a_faire(&cfg(8, 18), a(7), &e), (false, false));
        assert_eq!(a_faire(&cfg(8, 18), a(8), &e), (true, false));
        assert_eq!(a_faire(&cfg(8, 18), a(14), &e), (false, false));
        assert_eq!(a_faire(&cfg(8, 18), a(18), &e), (false, true));
        // Déjà fait aujourd'hui : rien ; coupé (0) : rien.
        let fait = Etat { matin: "2026-11-16".into(), soir: "2026-11-16".into(), ville: None };
        assert_eq!(a_faire(&cfg(8, 18), a(9), &fait), (false, false));
        assert_eq!(a_faire(&cfg(8, 18), a(19), &fait), (false, false));
        assert_eq!(a_faire(&cfg(0, 0), a(9), &e), (false, false));
        // Matin réglé tard (11 h) : la fenêtre va jusqu'à 14 h.
        assert_eq!(a_faire(&cfg(11, 18), a(13), &e), (true, false));
    }

    #[test]
    fn weather_codes_have_words() {
        assert_eq!(ciel(0), "ciel dégagé");
        assert_eq!(ciel(63), "pluie");
        assert_eq!(ciel(96), "orages");
        assert_eq!(ciel(1234), "temps variable");
    }

    #[test]
    fn evening_message_is_built_without_network() {
        let c = cfg(8, 18);
        let m = du_soir(&c, Local.with_ymd_and_hms(2026, 11, 16, 22, 0, 0).unwrap());
        assert_eq!(m.kind, Kind::Hello);
        assert!(m.text.contains("Demain"), "{}", m.text);
        assert!(meteo("").is_none());
    }
}
