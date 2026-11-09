//! Notifications reçues par le bureau, écoutées localement (`dbus-monitor`, appels
//! `org.freedesktop.Notifications.Notify`) et gardées 24 h dans un fichier privé, pour les
//! résumer sur demande (`/notifs`, outil `notifications_recentes`). Rien ne sort de la
//! machine tant qu'on ne demande pas de résumé.

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};

use crate::paths;

const MAX: usize = 60;
const DUREE_H: i64 = 24;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Notif {
    pub app: String,
    pub titre: String,
    pub texte: String,
    /// Horodatage RFC 3339.
    pub quand: String,
}

fn fichier() -> std::path::PathBuf {
    paths::data_dir().join("notifications.json")
}

/// Notifications des dernières 24 h, la plus récente d'abord.
pub fn load() -> Vec<Notif> {
    let l: Vec<Notif> = std::fs::read_to_string(fichier()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
    let limite = chrono::Local::now() - chrono::Duration::hours(DUREE_H);
    l.into_iter().filter(|n| chrono::DateTime::parse_from_rfc3339(&n.quand).is_ok_and(|d| d > limite)).collect()
}

fn ajouter(n: Notif) {
    let mut l = load();
    // Une même notification répétée (mise à jour d'un téléchargement…) remplace l'ancienne.
    l.retain(|x| !(x.app == n.app && x.titre == n.titre));
    l.insert(0, n);
    l.truncate(MAX);
    if let Ok(s) = serde_json::to_string(&l) {
        let _ = paths::write_private(&fichier(), s.as_bytes());
    }
}

/// Écoute les notifications (bloquant : à lancer dans un fil du démon).
pub fn ecouter() {
    if !crate::config::which("dbus-monitor") {
        return;
    }
    let Ok(mut child) = paths::mourir_avec_parent(&mut Command::new("dbus-monitor"))
        .args(["--session", "type='method_call',interface='org.freedesktop.Notifications',member='Notify'"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    else {
        return;
    };
    let Some(out) = child.stdout.take() else { return };
    let mut p = Analyseur::default();
    for line in BufReader::new(out).lines() {
        let Ok(line) = line else { break };
        if let Some(n) = p.ligne(&line) {
            ajouter(n);
        }
    }
}

/// Lit la sortie de `dbus-monitor` ligne à ligne : après « member=Notify », les chaînes de
/// premier niveau sont, dans l'ordre, l'application, l'icône, le titre et le texte.
#[derive(Default)]
pub struct Analyseur {
    dans: bool,
    chaines: Vec<String>,
    /// Chaîne sur plusieurs lignes en cours.
    ouverte: Option<String>,
}

impl Analyseur {
    pub fn ligne(&mut self, l: &str) -> Option<Notif> {
        if l.starts_with("method call") {
            self.dans = l.contains("member=Notify");
            self.chaines.clear();
            self.ouverte = None;
            return None;
        }
        if !self.dans {
            return None;
        }
        if let Some(cur) = &mut self.ouverte {
            cur.push('\n');
            if let Some(fin) = l.strip_suffix('"') {
                cur.push_str(fin);
                let s = self.ouverte.take().unwrap_or_default();
                return self.pousser(s);
            }
            cur.push_str(l);
            return None;
        }
        // Chaînes de premier niveau seulement (3 espaces) : pas celles des actions ni des indices.
        let Some(v) = l.strip_prefix("   string \"") else { return None };
        match v.strip_suffix('"') {
            Some(s) => self.pousser(s.to_string()),
            None => {
                self.ouverte = Some(v.to_string());
                None
            }
        }
    }

    fn pousser(&mut self, s: String) -> Option<Notif> {
        self.chaines.push(s);
        if self.chaines.len() < 4 {
            return None;
        }
        self.dans = false;
        let c = std::mem::take(&mut self.chaines);
        let app = c[0].trim().to_string();
        // Les nôtres n'ont pas à être résumées.
        if app.eq_ignore_ascii_case("l katta") || app.eq_ignore_ascii_case("lkatta") || (c[2].trim().is_empty() && c[3].trim().is_empty()) {
            return None;
        }
        Some(Notif { app, titre: c[2].trim().to_string(), texte: nettoyer(&c[3]), quand: chrono::Local::now().to_rfc3339() })
    }
}

/// Texte de notification sans balises (certaines applis envoient du HTML simple).
fn nettoyer(s: &str) -> String {
    crate::web::strip_tags(s).trim().chars().take(600).collect()
}

/// Liste lisible (pour le cerveau ou la bulle), des plus récentes aux plus anciennes.
pub fn texte(l: &[Notif], n: usize) -> String {
    if l.is_empty() {
        return t!("Aucune notification ces dernières 24 heures.").into();
    }
    l.iter()
        .take(n)
        .map(|x| {
            let h = chrono::DateTime::parse_from_rfc3339(&x.quand).map(|d| d.format("%H:%M").to_string()).unwrap_or_default();
            let corps = if x.texte.is_empty() { String::new() } else { format!(" — {}", x.texte.replace('\n', " ")) };
            format!("• {h} {} : {}{corps}", x.app, x.titre)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Compte par application : « Signal ×3, Thunderbird ×2 ».
pub fn compte(l: &[Notif]) -> String {
    let mut c: Vec<(String, usize)> = vec![];
    for n in l {
        match c.iter_mut().find(|(a, _)| *a == n.app) {
            Some((_, k)) => *k += 1,
            None => c.push((n.app.clone(), 1)),
        }
    }
    c.sort_by(|a, b| b.1.cmp(&a.1));
    c.iter().map(|(a, k)| if *k > 1 { format!("{a} ×{k}") } else { a.clone() }).collect::<Vec<_>>().join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Essai réel sur le bus de session : `cargo test ecoute_reelle -- --ignored`.
    #[test]
    #[ignore]
    fn ecoute_reelle() {
        std::thread::spawn(ecouter);
        std::thread::sleep(std::time::Duration::from_millis(800));
        let _ = Command::new("notify-send").args(["-a", "Essai L Katta", "Titre d'essai", "Corps d'essai"]).status();
        std::thread::sleep(std::time::Duration::from_millis(1200));
        assert!(load().iter().any(|n| n.app == "Essai L Katta" && n.titre == "Titre d'essai" && n.texte == "Corps d'essai"), "{:?}", load());
    }

    #[test]
    fn parses_dbus_monitor_output() {
        let sortie = r#"signal time=1.0 sender=org.freedesktop.DBus -> destination=:1.9 serial=2 path=/org/freedesktop/DBus; interface=org.freedesktop.DBus; member=NameAcquired
   string ":1.9"
method call time=1731171234.1 sender=:1.45 -> destination=:1.20 serial=12 path=/org/freedesktop/Notifications; interface=org.freedesktop.Notifications; member=Notify
   string "Thunderbird"
   uint32 0
   string "thunderbird"
   string "Réunion demain"
   string "Bonjour,
la réunion est <b>déplacée</b> à 10 h."
   array [
      string "default"
      string "Ouvrir"
   ]
   array [
      dict entry(
         string "urgency"
         variant             byte 1
      )
   ]
   int32 -1
method call time=2.0 sender=:1.50 -> destination=:1.20 serial=3 path=/org/freedesktop/Notifications; interface=org.freedesktop.Notifications; member=Notify
   string "L Katta"
   uint32 0
   string ""
   string "Rappel"
   string "Boire de l'eau"
"#;
        let mut a = Analyseur::default();
        let n: Vec<Notif> = sortie.lines().filter_map(|l| a.ligne(l)).collect();
        assert_eq!(n.len(), 1, "{n:?}");
        assert_eq!((n[0].app.as_str(), n[0].titre.as_str()), ("Thunderbird", "Réunion demain"));
        assert_eq!(n[0].texte, "Bonjour,\nla réunion est déplacée à 10 h.");
        let l = vec![n[0].clone(), Notif { app: "Signal".into(), titre: "Hery".into(), texte: String::new(), quand: n[0].quand.clone() }, Notif { app: "Signal".into(), titre: "Mamy".into(), texte: String::new(), quand: n[0].quand.clone() }];
        assert_eq!(compte(&l), "Signal ×2, Thunderbird");
        assert!(texte(&l, 5).contains("Thunderbird : Réunion demain — Bonjour, la réunion"));
    }
}
