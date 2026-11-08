//! Petites aides de langue française (dates, salutations).

use chrono::{DateTime, Datelike, Local, Timelike};

const JOURS: [&str; 7] = ["lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi", "dimanche"];
const MOIS: [&str; 12] = [
    "janvier", "février", "mars", "avril", "mai", "juin", "juillet", "août", "septembre", "octobre",
    "novembre", "décembre",
];

/// « dimanche 8 novembre 2026, 14:05 »
pub fn date_longue(d: &DateTime<Local>) -> String {
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

/// « 14h05 »
pub fn heure(d: &DateTime<Local>) -> String {
    if d.minute() == 0 { format!("{}h", d.hour()) } else { format!("{}h{:02}", d.hour(), d.minute()) }
}

/// Salutation selon l'heure.
pub fn salutation(hour: u32) -> &'static str {
    match hour {
        5..=11 => "Bonjour",
        12..=17 => "Bon après-midi",
        18..=22 => "Bonsoir",
        _ => "Coucou, encore debout",
    }
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
    }
}
