//! Pilotage de Konsole par D-Bus : lancer une commande dans un onglet visible, lire le
//! texte affiché par le terminal courant, y écrire. L'utilisateur voit tout ; les
//! commandes administrateur y demandent son mot de passe à lui, jamais au maki.

use std::process::{Command, Stdio};

use crate::config::which;

fn qdbus(args: &[&str]) -> Option<String> {
    let bin = if which("qdbus6") { "qdbus6" } else { "qdbus" };
    let o = Command::new(bin).args(args).stderr(Stdio::null()).output().ok()?;
    o.status.success().then(|| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

/// Services D-Bus des Konsole ouvertes, la plus récente d'abord.
fn konsoles() -> Vec<String> {
    let mut v: Vec<(u32, String)> = qdbus(&[])
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with("org.kde.konsole"))
        .map(|l| (l.rsplit('-').next().and_then(|p| p.parse().ok()).unwrap_or(0), l.to_string()))
        .collect();
    v.sort_by(|a, b| b.0.cmp(&a.0));
    v.into_iter().map(|(_, s)| s).collect()
}

fn windows(svc: &str) -> Vec<String> {
    qdbus(&[svc]).unwrap_or_default().lines().map(str::trim).filter(|l| l.starts_with("/Windows/")).map(String::from).collect()
}

/// (service, session) du terminal actif de la Konsole la plus récente.
fn current() -> Option<(String, String)> {
    for svc in konsoles() {
        for w in windows(&svc) {
            if let Some(id) = qdbus(&[&svc, &w, "org.kde.konsole.Window.currentSession"]) {
                if !id.is_empty() {
                    return Some((svc, format!("/Sessions/{id}")));
                }
            }
        }
    }
    None
}

pub fn available() -> bool {
    which("konsole")
}

/// Texte affiché par le terminal actif (les `lignes` dernières).
pub fn read(lignes: usize) -> Result<String, String> {
    let (svc, sess) = current().ok_or("aucune Konsole ouverte")?;
    let title = qdbus(&[&svc, &sess, "org.kde.konsole.Session.title", "1"]).unwrap_or_default();
    let txt = qdbus(&[&svc, &sess, "org.kde.konsole.Session.getAllDisplayedText", "true"]).ok_or("lecture refusée par Konsole")?;
    let all: Vec<&str> = txt.lines().collect();
    let start = all.len().saturating_sub(lignes.clamp(5, 300));
    Ok(format!("Terminal « {title} » :\n{}", all[start..].join("\n")))
}

/// Écrit dans le terminal actif ; `entree` valide la ligne (la commande s'exécute).
pub fn write(texte: &str, entree: bool) -> Result<String, String> {
    let (svc, sess) = current().ok_or("aucune Konsole ouverte")?;
    if entree {
        qdbus(&[&svc, &sess, "org.kde.konsole.Session.runCommand", texte]).ok_or("Konsole a refusé")?;
    } else {
        qdbus(&[&svc, &sess, "org.kde.konsole.Session.sendText", texte]).ok_or("Konsole a refusé")?;
    }
    Ok(format!("Écrit dans le terminal{} : {texte}", if entree { " et validé" } else { "" }))
}

/// Lance une commande dans un nouvel onglet Konsole visible (ou une nouvelle fenêtre).
pub fn run_visible(cmd: &str) -> Result<String, String> {
    if !available() {
        return Err("Konsole n'est pas installé".into());
    }
    for svc in konsoles() {
        if let Some(w) = windows(&svc).into_iter().next() {
            if let Some(id) = qdbus(&[&svc, &w, "org.kde.konsole.Window.newSession"]) {
                let sess = format!("/Sessions/{id}");
                let _ = qdbus(&[&svc, &sess, "org.kde.konsole.Session.setTitle", "1", "L Katta"]);
                qdbus(&[&svc, &sess, "org.kde.konsole.Session.runCommand", cmd]).ok_or("Konsole a refusé la commande")?;
                return Ok(format!("Lancé dans un nouvel onglet Konsole (« L Katta ») : {cmd}\nLe résultat s'affiche là-bas ; demande-moi de lire le terminal ensuite."));
            }
        }
    }
    let script = format!("{cmd}; echo; echo '[L Katta] Terminé. Ce terminal reste ouvert.'; exec bash");
    Command::new("setsid")
        .args(["konsole", "-p", "tabtitle=L Katta", "-e", "bash", "-c", &script])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(format!("Lancé dans une nouvelle fenêtre Konsole : {cmd}\nLe résultat s'affiche là-bas ; demande-moi de lire le terminal ensuite."))
}
