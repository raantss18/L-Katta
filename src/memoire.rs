//! Mémoire durable : préférences retenues (`profil.md`, injecté dans le prompt système)
//! et journal des échanges, consultable. Porté de mi-saina (userctx.py, mémoire FTS).

use std::io::Write;

use serde_json::json;

use crate::paths;

fn profil_file() -> std::path::PathBuf {
    paths::config_dir().join("profil.md")
}
fn journal_file() -> std::path::PathBuf {
    paths::data_dir().join("historique.jsonl")
}

pub fn profil() -> String {
    std::fs::read_to_string(profil_file()).unwrap_or_default()
}

/// Ajoute une préférence (sans doublon).
pub fn retenir(fait: &str) -> Result<String, String> {
    let fait = fait.trim().trim_start_matches("- ");
    if fait.is_empty() {
        return Err("rien à retenir".into());
    }
    let cur = profil();
    if cur.lines().any(|l| l.contains(fait)) {
        return Ok(format!("Je le savais déjà : {fait}"));
    }
    let mut s = if cur.is_empty() { "# Préférences retenues par L Katta\n".to_string() } else { cur };
    if !s.ends_with('\n') {
        s.push('\n');
    }
    s.push_str(&format!("- {fait}  _(retenu le {})_\n", chrono::Local::now().format("%Y-%m-%d")));
    paths::write_private(&profil_file(), s.as_bytes()).map_err(|e| e.to_string())?;
    Ok(format!("Retenu : {fait}"))
}

/// Oublie les préférences contenant ce texte.
pub fn oublier(motif: &str) -> Result<String, String> {
    let cur = profil();
    let m = motif.to_lowercase();
    let kept: Vec<&str> = cur.lines().filter(|l| !(l.starts_with("- ") && l.to_lowercase().contains(&m))).collect();
    let n = cur.lines().count() - kept.len();
    paths::write_private(&profil_file(), (kept.join("\n") + "\n").as_bytes()).map_err(|e| e.to_string())?;
    Ok(format!("{n} préférence(s) oubliée(s)."))
}

/// Journalise un échange (question, réponse).
pub fn journaliser(question: &str, reponse: &str) {
    let line = json!({"quand": chrono::Local::now().to_rfc3339(), "q": question, "r": reponse}).to_string();
    let _ = std::fs::create_dir_all(paths::data_dir());
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(journal_file()) {
        use std::os::unix::fs::PermissionsExt;
        let _ = f.set_permissions(std::fs::Permissions::from_mode(0o600));
        let _ = writeln!(f, "{line}");
    }
}

/// Recherche plein texte (tous les mots) dans le journal des échanges, plus récents d'abord.
pub fn chercher(requete: &str) -> String {
    let mots: Vec<String> = requete.to_lowercase().split_whitespace().filter(|w| w.len() > 2).map(String::from).collect();
    let txt = std::fs::read_to_string(journal_file()).unwrap_or_default();
    let mut hits = vec![];
    for line in txt.lines().rev() {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else { continue };
        let q = v["q"].as_str().unwrap_or("");
        let r = v["r"].as_str().unwrap_or("");
        let all = format!("{q} {r}").to_lowercase();
        if !mots.is_empty() && mots.iter().all(|m| all.contains(m.as_str())) {
            let date = v["quand"].as_str().unwrap_or("").get(..10).unwrap_or("");
            let r: String = r.chars().take(240).collect();
            hits.push(format!("- {date} « {q} » → {}", r.replace('\n', " ")));
            if hits.len() >= 8 {
                break;
            }
        }
    }
    if hits.is_empty() { format!("Rien dans nos échanges passés sur « {requete} ».") } else { hits.join("\n") }
}

/// Derniers échanges (contexte de la bulle au démarrage).
pub fn derniers(n: usize) -> Vec<(String, String)> {
    let txt = std::fs::read_to_string(journal_file()).unwrap_or_default();
    let mut v: Vec<(String, String)> = txt
        .lines()
        .rev()
        .take(n)
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .map(|v| (v["q"].as_str().unwrap_or("").to_string(), v["r"].as_str().unwrap_or("").to_string()))
        .collect();
    v.reverse();
    v
}
