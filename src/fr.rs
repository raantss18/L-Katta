//! Petites aides de langue française (dates, salutations).

use chrono::{DateTime, Datelike, Local, Timelike};

const JOURS: [&str; 7] = ["lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi", "dimanche"];
const MOIS: [&str; 12] = [
    "janvier", "février", "mars", "avril", "mai", "juin", "juillet", "août", "septembre", "octobre",
    "novembre", "décembre",
];

const DAYS: [&str; 7] = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];
const MONTHS: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];

/// « dimanche 8 novembre 2026, 14:05 » (en anglais : « Sunday, November 8, 2026, 14:05 »).
pub fn date_longue(d: &DateTime<Local>) -> String {
    if crate::lang::en() {
        return format!("{}, {} {}, {}, {:02}:{:02}", DAYS[d.weekday().num_days_from_monday() as usize], MONTHS[d.month0() as usize], d.day(), d.year(), d.hour(), d.minute());
    }
    format!(
        "{} {} {} {}, {:02}:{:02}",
        JOURS[d.weekday().num_days_from_monday() as usize],
        d.day(),
        MOIS[d.month0() as usize],
        d.year(),
        d.hour(),
        d.minute()
    )
}

/// « 14h05 » (en anglais : « 14:05 »).
pub fn heure(d: &DateTime<Local>) -> String {
    if crate::lang::en() {
        return format!("{}:{:02}", d.hour(), d.minute());
    }
    if d.minute() == 0 { format!("{}h", d.hour()) } else { format!("{}h{:02}", d.hour(), d.minute()) }
}

/// Salutation selon l'heure.
pub fn salutation(hour: u32) -> &'static str {
    if crate::lang::en() {
        return match hour {
            5..=11 => "Good morning",
            12..=17 => "Good afternoon",
            18..=22 => "Good evening",
            _ => "Hey, still up",
        };
    }
    match hour {
        5..=11 => "Bonjour",
        12..=17 => "Bon après-midi",
        18..=22 => "Bonsoir",
        _ => "Coucou, encore debout",
    }
}

/// « de Hery », « d'Antsa ».
pub fn de(nom: &str) -> String {
    let first = nom.chars().next().map(|c| c.to_lowercase().next().unwrap_or(c));
    if first.is_some_and(|c| "aeiouyéèêàâîôû".contains(c)) { format!("d'{nom}") } else { format!("de {nom}") }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn formats() {
        let d = Local.with_ymd_and_hms(2026, 11, 8, 9, 0, 0).unwrap();
        assert_eq!(date_longue(&d), "dimanche 8 novembre 2026, 09:00");
        assert_eq!(heure(&d), "9h");
        assert_eq!(salutation(9), "Bonjour");
        assert_eq!(salutation(2), "Coucou, encore debout");
        assert_eq!(de("Antsa"), "d'Antsa");
        assert_eq!(de("Hery"), "de Hery");
        assert_eq!(de("Rabe"), "de Rabe");
    }
}
