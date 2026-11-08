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
        return Err(t!("rien à retenir").into());
    }
    let cur = profil();
    if cur.lines().any(|l| l.contains(fait)) {
        return Ok(t!("Je le savais déjà : {}", fait));
    }
    let mut s = if cur.is_empty() { "# Préférences retenues par L Katta\n".to_string() } else { cur };
    if !s.ends_with('\n') {
        s.push('\n');
    }
    s.push_str(&format!("- {fait}  _(retenu le {})_\n", chrono::Local::now().format("%Y-%m-%d")));
    paths::write_private(&profil_file(), s.as_bytes()).map_err(|e| e.to_string())?;
    Ok(t!("Retenu : {}", fait))
}

/// Oublie les préférences contenant ce texte.
pub fn oublier(motif: &str) -> Result<String, String> {
    let cur = profil();
    let m = motif.to_lowercase();
    let kept: Vec<&str> = cur.lines().filter(|l| !(l.starts_with("- ") && l.to_lowercase().contains(&m))).collect();
    let n = cur.lines().count() - kept.len();
    paths::write_private(&profil_file(), (kept.join("\n") + "\n").as_bytes()).map_err(|e| e.to_string())?;
    Ok(t!("{} préférence(s) oubliée(s).", n))
}

/// Journalise un échange (question, réponse).
pub fn journaliser(question: &str, reponse: &str) {
    journaliser_etapes(question, reponse, &[]);
}

/// Journalise un échange avec les actions faites en route (commandes, outils).
pub fn journaliser_etapes(question: &str, reponse: &str, etapes: &[String]) {
    let mut v = json!({"quand": chrono::Local::now().to_rfc3339(), "q": question, "r": reponse});
    if !etapes.is_empty() {
        v["etapes"] = json!(etapes);
    }
    let line = v.to_string();
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
    if hits.is_empty() { t!("Rien dans nos échanges passés sur « {} ».", requete) } else { hits.join("\n") }
}

/// Les `n` derniers échanges, en liste courte (commande /historique sans mots).
pub fn recents(n: usize) -> String {
    let v = derniers(n);
    if v.is_empty() {
        return t!("Pas encore d'échanges enregistrés.").into();
    }
    let court = |t: &str, k: usize| {
        let t = t.replace('\n', " ");
        if t.chars().count() > k { format!("{}…", t.chars().take(k).collect::<String>()) } else { t }
    };
    let mut out: Vec<String> = v.iter().rev().map(|e| format!("- « {} » → {}", court(&e.q, 60), court(&e.r, 110))).collect();
    out.push(t!("(/historique <mots> pour chercher plus loin)").into());
    out.join("\n")
}

/// Un échange du journal.
#[derive(Debug, Clone, PartialEq)]
pub struct Echange {
    pub q: String,
    pub r: String,
    pub etapes: Vec<String>,
}

/// Derniers échanges (contexte de la bulle au démarrage).
pub fn derniers(n: usize) -> Vec<Echange> {
    let txt = std::fs::read_to_string(journal_file()).unwrap_or_default();
    let mut v: Vec<Echange> = txt
        .lines()
        .rev()
        .take(n)
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .map(|v| Echange {
            q: v["q"].as_str().unwrap_or("").to_string(),
            r: v["r"].as_str().unwrap_or("").to_string(),
            etapes: v["etapes"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default(),
        })
        .collect();
    v.reverse();
    v
}

// --- échange en cours : survit à un arrêt du maki --------------------------------------

fn en_cours_file() -> std::path::PathBuf {
    paths::data_dir().join("en-cours.json")
}

/// Une question part au cerveau.
pub fn en_cours_debut(question: &str) {
    let _ = paths::write_private(&en_cours_file(), json!({"q": question, "etapes": []}).to_string().as_bytes());
}

/// Une étape faite (commande lancée, outil, résultat abrégé).
pub fn en_cours_etape(etape: &str) {
    let Ok(t) = std::fs::read_to_string(en_cours_file()) else { return };
    let Ok(mut v) = serde_json::from_str::<serde_json::Value>(&t) else { return };
    if let Some(a) = v["etapes"].as_array_mut() {
        a.push(json!(etape));
    }
    let _ = paths::write_private(&en_cours_file(), v.to_string().as_bytes());
}

/// L'échange est fini (réponse ou erreur journalisée).
pub fn en_cours_fin() {
    let _ = std::fs::remove_file(en_cours_file());
}

/// Un échange coupé par un arrêt : il est journalisé comme interrompu et rendu.
pub fn en_cours_repris() -> Option<Echange> {
    let t = std::fs::read_to_string(en_cours_file()).ok()?;
    en_cours_fin();
    let v: serde_json::Value = serde_json::from_str(&t).ok()?;
    let q = v["q"].as_str()?.to_string();
    let etapes: Vec<String> = v["etapes"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
    let r = INTERROMPU.to_string();
    journaliser_etapes(&q, &r, &etapes);
    Some(Echange { q, r, etapes })
}

pub const INTERROMPU: &str = "(réponse interrompue : L Katta a été arrêté pendant qu'il travaillait. Repose la question si besoin.)";
