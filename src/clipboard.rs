//! Historique du presse-papiers via `wl-paste --watch` (wl-clipboard).
//!
//! Les copies marquées secrètes par les gestionnaires de mots de passe
//! (`x-kde-passwordManagerHint`) ne sont jamais enregistrées.

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

use base64::Engine;
use serde::{Deserialize, Serialize};

use crate::paths;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Entree {
    pub texte: String,
    /// Horodatage RFC 3339.
    pub quand: String,
}

pub fn load() -> Vec<Entree> {
    std::fs::read_to_string(paths::clipboard_file())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save(list: &[Entree]) -> std::io::Result<()> {
    let s = serde_json::to_string(list).map_err(std::io::Error::other)?;
    paths::write_private(&paths::clipboard_file(), s.as_bytes())
}

/// Ajoute une entrée en tête (sans doublon), borne la taille.
pub fn push(list: &mut Vec<Entree>, texte: String, max: usize) {
    if texte.trim().is_empty() {
        return;
    }
    list.retain(|e| e.texte != texte);
    list.insert(0, Entree { texte, quand: chrono::Local::now().to_rfc3339() });
    list.truncate(max.max(1));
}

/// Place du texte dans le presse-papiers.
pub fn copy(texte: &str) -> std::io::Result<()> {
    use std::io::Write;
    let mut child = Command::new("wl-copy").stdin(Stdio::piped()).spawn()?;
    child.stdin.take().expect("stdin").write_all(texte.as_bytes())?;
    child.wait()?;
    Ok(())
}

const WATCH_SCRIPT: &str = "if wl-paste -l 2>/dev/null | grep -q passwordManagerHint; then cat >/dev/null; echo SECRET; \
                            else base64 -w0; echo; fi";

/// Surveille le presse-papiers (bloquant : à lancer dans un fil). Appelle `on_new`
/// pour chaque nouvelle copie texte.
pub fn watch(max: usize, mut on_new: impl FnMut(&str)) {
    if !crate::config::which("wl-paste") {
        eprintln!("{}", t!("lkatta : wl-paste absent, historique du presse-papiers désactivé"));
        return;
    }
    // Meurt avec le maki : sinon une relance laisse un surveillant orphelin.
    let child = crate::paths::mourir_avec_parent(&mut Command::new("wl-paste"))
        .args(["--type", "text", "--watch", "sh", "-c", WATCH_SCRIPT])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn();
    let Ok(mut child) = child else { return };
    let out = child.stdout.take().expect("stdout");
    for line in BufReader::new(out).lines() {
        let Ok(line) = line else { break };
        if line == "SECRET" || line.is_empty() {
            continue;
        }
        let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(line.trim()) else { continue };
        let texte = String::from_utf8_lossy(&bytes).into_owned();
        if texte.len() > 20_000 {
            continue; // trop gros pour un historique léger
        }
        // Recharger : le panneau a pu vider l'historique.
        let mut list = load();
        push(&mut list, texte.clone(), max);
        let _ = save(&list);
        on_new(&texte);
    }
    let _ = child.wait();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_dedup_and_truncate() {
        let mut l = vec![];
        push(&mut l, "a".into(), 2);
        push(&mut l, "b".into(), 2);
        push(&mut l, "a".into(), 2);
        assert_eq!(l.iter().map(|e| e.texte.as_str()).collect::<Vec<_>>(), ["a", "b"]);
        push(&mut l, "c".into(), 2);
        assert_eq!(l.len(), 2);
        push(&mut l, "  ".into(), 2);
        assert_eq!(l[0].texte, "c");
    }
}
