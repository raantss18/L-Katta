//! Rappels : stockés dans `rappels.json`, vérifiés par le démon.

use chrono::{DateTime, Duration, Local, NaiveDateTime, NaiveTime, TimeZone};
use serde::{Deserialize, Serialize};

use crate::paths;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Rappel {
    pub id: u64,
    /// Horodatage RFC 3339 (heure locale).
    pub quand: String,
    pub texte: String,
    #[serde(default)]
    pub fait: bool,
}

impl Rappel {
    pub fn date(&self) -> Option<DateTime<Local>> {
        DateTime::parse_from_rfc3339(&self.quand).ok().map(|d| d.with_timezone(&Local))
    }
}

pub fn load() -> Vec<Rappel> {
    std::fs::read_to_string(paths::reminders_file())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save(list: &[Rappel]) -> std::io::Result<()> {
    let s = serde_json::to_string_pretty(list).map_err(std::io::Error::other)?;
    paths::write_private(&paths::reminders_file(), s.as_bytes())
}

pub fn add(quand: DateTime<Local>, texte: &str) -> std::io::Result<Rappel> {
    let mut list = load();
    let id = list.iter().map(|r| r.id).max().unwrap_or(0) + 1;
    let r = Rappel { id, quand: quand.to_rfc3339(), texte: texte.trim().to_string(), fait: false };
    list.push(r.clone());
    list.sort_by_key(|r| r.date());
    save(&list)?;
    Ok(r)
}

pub fn remove(id: u64) -> std::io::Result<bool> {
    let mut list = load();
    let n = list.len();
    list.retain(|r| r.id != id);
    save(&list)?;
    Ok(list.len() != n)
}

/// Rappels échus non faits ; les marque faits. Purge les rappels faits de plus de 7 jours.
pub fn take_due(now: DateTime<Local>) -> Vec<Rappel> {
    let mut list = load();
    let mut due = vec![];
    for r in list.iter_mut() {
        if !r.fait && r.date().is_some_and(|d| d <= now) {
            r.fait = true;
            due.push(r.clone());
        }
    }
    let before = list.len();
    list.retain(|r| !(r.fait && r.date().is_some_and(|d| now - d > Duration::days(7))));
    if !due.is_empty() || list.len() != before {
        let _ = save(&list);
    }
    due
}

/// Rappels à venir aujourd'hui.
pub fn today_pending(now: DateTime<Local>) -> Vec<Rappel> {
    load()
        .into_iter()
        .filter(|r| !r.fait && r.date().is_some_and(|d| d.date_naive() == now.date_naive()))
        .collect()
}

/// Comprend « 10m », « +2h », « dans 20 min », « 18:30 », « 18h30 », « demain 9h »,
/// « 2026-11-09 14:00 ».
pub fn parse_when(s: &str, now: DateTime<Local>) -> Option<DateTime<Local>> {
    let s = s.trim().to_lowercase();
    let s = s.trim_start_matches("dans ").trim_start_matches('+').trim();
    // Durée relative : nombre + unité.
    let digits: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
    if !digits.is_empty() {
        let rest = s[digits.len()..].trim();
        let n: i64 = digits.parse().ok()?;
        let unit = match rest {
            "s" | "sec" | "secondes" | "seconde" => Some(Duration::seconds(n)),
            "m" | "min" | "mn" | "minute" | "minutes" => Some(Duration::minutes(n)),
            "h" | "heure" | "heures" if !rest.is_empty() => Some(Duration::hours(n)),
            "j" | "jour" | "jours" | "d" => Some(Duration::days(n)),
            _ => None,
        };
        if let Some(d) = unit {
            return Some(now + d);
        }
    }
    // Date absolue complète.
    for fmt in ["%Y-%m-%d %H:%M", "%Y-%m-%dT%H:%M", "%Y-%m-%dT%H:%M:%S", "%d/%m/%Y %H:%M"] {
        if let Ok(n) = NaiveDateTime::parse_from_str(&s, fmt) {
            return Local.from_local_datetime(&n).single();
        }
    }
    if let Ok(d) = DateTime::parse_from_rfc3339(&s) {
        return Some(d.with_timezone(&Local));
    }
    // Heure seule, éventuellement précédée de « demain ».
    let (day, time_part) = match s.strip_prefix("demain") {
        Some(rest) => (now.date_naive() + Duration::days(1), rest.trim().trim_start_matches('à').trim()),
        None => (now.date_naive(), s.trim_start_matches('à').trim()),
    };
    let t = parse_time(time_part)?;
    let mut dt = Local.from_local_datetime(&day.and_time(t)).single()?;
    if dt <= now && !s.starts_with("demain") {
        dt += Duration::days(1);
    }
    Some(dt)
}

fn parse_time(s: &str) -> Option<NaiveTime> {
    let s = s.replace('h', ":");
    let s = s.trim_end_matches(':');
    let (h, m) = match s.split_once(':') {
        Some((h, m)) => (h.trim().parse().ok()?, if m.is_empty() { 0 } else { m.trim().parse().ok()? }),
        None => (s.trim().parse().ok()?, 0),
    };
    NaiveTime::from_hms_opt(h, m, 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> DateTime<Local> {
        Local.with_ymd_and_hms(2026, 11, 8, 14, 0, 0).unwrap()
    }

    #[test]
    fn relative() {
        assert_eq!(parse_when("10m", now()), Some(now() + Duration::minutes(10)));
        assert_eq!(parse_when("dans 2 h", now()), Some(now() + Duration::hours(2)));
        assert_eq!(parse_when("+3j", now()), Some(now() + Duration::days(3)));
    }

    #[test]
    fn absolute() {
        let t = parse_when("18h30", now()).unwrap();
        assert_eq!(t, Local.with_ymd_and_hms(2026, 11, 8, 18, 30, 0).unwrap());
        // Heure passée : demain.
        let t = parse_when("9:00", now()).unwrap();
        assert_eq!(t, Local.with_ymd_and_hms(2026, 11, 9, 9, 0, 0).unwrap());
        let t = parse_when("demain 8h", now()).unwrap();
        assert_eq!(t, Local.with_ymd_and_hms(2026, 11, 9, 8, 0, 0).unwrap());
        let t = parse_when("2026-12-01 10:15", now()).unwrap();
        assert_eq!(t, Local.with_ymd_and_hms(2026, 12, 1, 10, 15, 0).unwrap());
        assert_eq!(parse_when("n'importe quoi", now()), None);
    }
}
