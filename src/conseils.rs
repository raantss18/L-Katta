//! Accueil et conseils : ce que le maki dit de lui-même, choisi selon le moment et la
//! machine, sans répéter deux fois la même astuce avant d'avoir fait le tour.

use chrono::{DateTime, Datelike, Local, Timelike};
use serde::{Deserialize, Serialize};

use crate::{fr, paths, reminders};

/// Genre d'une bulle : il décide de son allure et de la réaction du maki.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    Hello,
    Tip,
    Reminder,
    Alert,
    News,
    Answer,
    Info,
    /// Nouveaux mails.
    Mail,
}

impl Kind {
    pub fn parse(s: &str) -> Option<Kind> {
        Some(match s {
            "bonjour" => Kind::Hello,
            "astuce" => Kind::Tip,
            "rappel" => Kind::Reminder,
            "alerte" => Kind::Alert,
            "nouvelle" => Kind::News,
            "reponse" => Kind::Answer,
            "info" => Kind::Info,
            "mail" => Kind::Mail,
            _ => return None,
        })
    }
}

/// Un message complet pour une bulle.
#[derive(Debug, Clone, PartialEq)]
pub struct Message {
    pub kind: Kind,
    pub title: String,
    pub text: String,
    /// Lien à ouvrir (nouvelles) ou question à poser (alertes).
    pub link: Option<String>,
}

impl Message {
    pub fn new(kind: Kind, title: impl Into<String>, text: impl Into<String>) -> Self {
        Self { kind, title: title.into(), text: text.into(), link: None }
    }
    pub fn with_link(mut self, l: impl Into<String>) -> Self {
        self.link = Some(l.into());
        self
    }
}

/// Astuces : L Katta lui-même, KDE Plasma, et l'entretien de la machine.
const ASTUCES: &[&str] = &[
    "Clique-moi pour discuter. Je peux chercher un fichier, ouvrir une appli ou faire le point sur ta machine.",
    "Attrape-moi et lance-moi ! Près d'un bord je m'y accroche, en haut je m'y suspends.",
    "Tape /reglages dans ma bulle : tu verras tous mes réglages, et /regle pour en changer un.",
    "Tu me trouves trop bavard ? Clic droit sur moi : je deviens discret.",
    "Je peux te rappeler quelque chose : « rappelle-moi d'appeler Hery à 18h30 ».",
    "Dans ma bulle, clique sur une de mes réponses : elle part dans le presse-papiers.",
    "/modeles 1 te montre les modèles d'IA que je peux utiliser ; /regle moteur 1 modele … pour en choisir un.",
    "Demande-moi « fais le bilan de santé » : disques, mémoire, services, mises à jour, journal.",
    "Je retiens tes préférences : « retiens que je préfère paru ». /retiens marche aussi.",
    "Dis-moi « ouvre le gestionnaire de fichiers » : je retrouve l'appli même avec un nom approximatif.",
    "Je connais tes bureaux virtuels : « va sur le bureau 3 ».",
    "Clic molette sur moi : un numéro surprise (arbre, escalade, téléportation…).",
    "Meta+V ouvre l'historique du presse-papiers de Plasma ; moi, je garde aussi le mien (/presse).",
    "Meta+W affiche la vue d'ensemble de Plasma : toutes tes fenêtres et tes bureaux d'un coup d'œil.",
    "Alt+Espace ouvre KRunner : calculs, conversions, lancement d'applis, recherche de fichiers.",
    "Meta+Shift+S capture une zone de l'écran avec Spectacle.",
    "Ctrl+F1 à Ctrl+F4 changent de bureau virtuel dans Plasma.",
    "Meta+L verrouille la session quand tu t'éloignes.",
    "Meta+. ouvre le sélecteur d'emoji de Plasma.",
    "Meta+E ouvre Dolphin directement.",
    "Dans Dolphin, F4 ouvre un terminal intégré dans le dossier affiché.",
    "Dans Dolphin, Ctrl+I filtre les fichiers du dossier pendant que tu tapes.",
    "Le cache de pacman grossit avec le temps : paccache -rk2 garde seulement les deux dernières versions (droits admin).",
    "journalctl --disk-usage montre la place prise par les journaux système.",
    "du -h -d1 ~ | sort -hr | head montre tes plus gros dossiers. Ou demande-le-moi.",
    "Avant une mise à jour d'Arch, un œil sur archlinux.org/news évite les mauvaises surprises. Je surveille pour toi.",
    "Ctrl+R dans le terminal retrouve une ancienne commande en tapant un morceau.",
    "Une pause de cinq minutes par heure repose les yeux : regarde au loin quelques secondes.",
];

#[derive(Serialize, Deserialize, Default)]
struct State {
    /// Astuces déjà montrées (indices), remises à zéro quand on a fait le tour.
    vues: Vec<usize>,
}

fn state_file() -> std::path::PathBuf {
    paths::data_dir().join("conseils.json")
}

/// Prochaine astuce jamais vue (ordre mélangé, sans répétition).
pub fn astuce() -> Message {
    let mut st: State = std::fs::read_to_string(state_file()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
    if st.vues.len() >= ASTUCES.len() {
        st.vues.clear();
    }
    let libres: Vec<usize> = (0..ASTUCES.len()).filter(|i| !st.vues.contains(i)).collect();
    // Les premières fois, les astuces sur L Katta lui-même (dans l'ordre).
    let i = if st.vues.len() < 3 { libres[0] } else { libres[fastrand::usize(0..libres.len())] };
    st.vues.push(i);
    if let Ok(s) = serde_json::to_string(&st) {
        let _ = paths::write_private(&state_file(), s.as_bytes());
    }
    Message::new(Kind::Tip, "Astuce", ASTUCES[i])
}

fn moment(d: &DateTime<Local>) -> &'static str {
    match d.hour() {
        5..=8 => "Tôt ce matin, déjà au travail !",
        9..=11 => "Belle matinée pour avancer.",
        12..=13 => "C'est l'heure de manger, non ?",
        14..=17 => "L'après-midi est à toi.",
        18..=21 => "Fin de journée en vue.",
        _ => "Il est tard… pense à te reposer.",
    }
}

/// Message d'accueil : date, puis la chose la plus utile du moment.
pub fn accueil(prenom: &str, now: DateTime<Local>, alertes: &[String], evenements: &[String]) -> Message {
    let jour = fr::date_longue(&now);
    let jour = jour.split(',').next().unwrap_or("").to_string();
    let titre = format!("{} {prenom} !", fr::salutation(now.hour()));
    let mut lignes = vec![format!("{} — {}", capitalize(&jour), fr::heure(&now))];
    if !evenements.is_empty() {
        lignes.push(format!("Agenda : {}.", evenements.join(" · ")));
    }
    let rappels = reminders::today_pending(now);
    if let Some(r) = rappels.first() {
        let quand = r.date().map(|d| fr::heure(&d)).unwrap_or_default();
        if rappels.len() == 1 {
            lignes.push(format!("Aujourd'hui : {} à {quand}.", r.texte));
        } else {
            lignes.push(format!("{} rappels aujourd'hui, le prochain : {} à {quand}.", rappels.len(), r.texte));
        }
    } else if let Some(a) = alertes.first() {
        lignes.push(format!("À surveiller : {a}"));
    } else if !evenements.is_empty() {
    } else if now.weekday().num_days_from_monday() >= 5 {
        lignes.push("C'est le week-end : rien d'urgent de mon côté.".into());
    } else {
        lignes.push(moment(&now).into());
    }
    Message::new(Kind::Hello, titre, lignes.join("\n"))
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn welcome_mentions_day_and_hour() {
        let d = Local.with_ymd_and_hms(2026, 11, 9, 9, 5, 0).unwrap();
        let m = accueil("Antsa", d, &[], &[]);
        assert_eq!(m.kind, Kind::Hello);
        assert_eq!(m.title, "Bonjour Antsa !");
        assert!(m.text.starts_with("Lundi 9 novembre 2026 — 9h05"), "{}", m.text);
        let w = accueil("Antsa", Local.with_ymd_and_hms(2026, 11, 8, 15, 0, 0).unwrap(), &["Disque presque plein".into()], &[]);
        assert!(w.text.contains("À surveiller"));
        let a = accueil("Antsa", d, &[], &["10h Réunion".into()]);
        assert!(a.text.contains("Agenda : 10h Réunion."));
        assert!(w.text.contains("À surveiller"));
    }

    #[test]
    fn kinds_roundtrip() {
        assert_eq!(Kind::parse("astuce"), Some(Kind::Tip));
        assert_eq!(Kind::parse("alerte"), Some(Kind::Alert));
        assert_eq!(Kind::parse("zzz"), None);
    }
}
