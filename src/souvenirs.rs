//! Souvenirs durables (repris de Tontolo `memory.py` et de mi-saina) : tous les quelques
//! tours, un petit modèle note les faits utiles plus tard (préférences, projets, personnes) ;
//! ils sont dédoublonnés, jamais secrets, et seuls ceux qui ont un rapport avec la question
//! sont glissés dans la conversation, dans un budget de caractères. Les préférences dites
//! exprès (`/retiens`, profil.md) restent à part et sont toujours données au modèle.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::paths;
use crate::semantique::{cosinus, recouvrement, vecteur};

const MAX: usize = 120;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Souvenir {
    pub texte: String,
    /// Horodatage RFC 3339.
    pub quand: String,
    #[serde(default)]
    pub usages: u32,
}

fn fichier() -> std::path::PathBuf {
    paths::data_dir().join("souvenirs.json")
}

pub fn load() -> Vec<Souvenir> {
    std::fs::read_to_string(fichier()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

fn save(l: &[Souvenir]) {
    if let Ok(s) = serde_json::to_string_pretty(l) {
        let _ = paths::write_private(&fichier(), s.as_bytes());
    }
}

/// Un texte qui ne doit jamais être gardé ni envoyé en mémoire (clé, mot de passe, carte…).
pub fn secret(t: &str) -> bool {
    use std::sync::OnceLock;
    static RE: OnceLock<regex_lite::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        regex_lite::Regex::new(
            r"(?i)(\b(sk-|sk_|gsk_|hf_|ghp_|gho_|github_pat_|xai-|nvapi-|AIza)[A-Za-z0-9_\-]{8,}|\b(?:\d[ -]?){13,19}\b|\b[A-Z]{2}\d{2}(?:[ ]?[A-Z0-9]{4}){3,}|mot de passe|password|passwd|\bmdp\b|code pin|\bpin\b|teny miafina)",
        )
        .expect("regex")
    });
    re.is_match(t)
}

/// Masque les clés et numéros sensibles d'un texte (journal des échanges).
pub fn masquer(t: &str) -> String {
    use std::sync::OnceLock;
    static RE: OnceLock<regex_lite::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        regex_lite::Regex::new(r"\b(sk-|sk_|gsk_|hf_|ghp_|gho_|github_pat_|xai-|nvapi-|AIza)[A-Za-z0-9_\-]{8,}|\b(?:\d[ -]?){13,19}\b").expect("regex")
    });
    re.replace_all(t, "[secret masqué]").into_owned()
}

/// Un fait proposé par le modèle est-il digne d'être gardé ?
fn acceptable(t: &str) -> bool {
    let n = t.chars().count();
    let l = t.to_lowercase();
    // Des souhaits du moment ou des affirmations sur la machine (souvent inventées) : non.
    let mauvais = ["veut que tu", "voudrait que tu", "s'il te plaît", "please", "/home/", "est installé", "is installed", "santé", "maladie", "health"];
    (8..=220).contains(&n) && !secret(t) && !mauvais.iter().any(|m| l.contains(m))
}

/// Ajoute un souvenir ; un quasi-doublon est remplacé par le plus récent.
pub fn ajouter(texte: &str) -> bool {
    let texte = texte.trim().trim_start_matches(['-', '•', '*']).trim();
    if !acceptable(texte) {
        return false;
    }
    let mut l = load();
    l.retain(|s| recouvrement(&s.texte, texte) < 0.75 && recouvrement(texte, &s.texte) < 0.75);
    l.insert(0, Souvenir { texte: texte.to_string(), quand: chrono::Local::now().to_rfc3339(), usages: 0 });
    if l.len() > MAX {
        // On garde les plus utilisés, puis les plus récents.
        let mut idx: Vec<usize> = (0..l.len()).collect();
        idx.sort_by_key(|&i| (std::cmp::Reverse(l[i].usages), i));
        let garder: std::collections::HashSet<usize> = idx.into_iter().take(MAX).collect();
        l = l.into_iter().enumerate().filter(|(i, _)| garder.contains(i)).map(|(_, s)| s).collect();
    }
    save(&l);
    true
}

/// Oublie les souvenirs qui contiennent ce texte ; rend leur nombre.
pub fn oublier(motif: &str) -> usize {
    let m = crate::semantique::simplifier(motif.trim());
    if m.is_empty() {
        return 0;
    }
    let mut l = load();
    let n = l.len();
    l.retain(|s| !crate::semantique::simplifier(&s.texte).contains(&m));
    save(&l);
    n - l.len()
}

/// Les souvenirs qui ont un rapport avec la question, dans un budget de caractères.
pub fn pertinents(question: &str, budget: usize) -> Vec<String> {
    let mut l = load();
    if l.is_empty() || question.trim().is_empty() {
        return vec![];
    }
    let vq = vecteur(question);
    let maintenant = chrono::Local::now();
    let mut notes: Vec<(f32, usize)> = l
        .iter()
        .enumerate()
        .filter_map(|(i, s)| {
            let rec = recouvrement(&s.texte, question).max(recouvrement(question, &s.texte));
            let cos = cosinus(&vq, &vecteur(&s.texte));
            if rec == 0.0 && cos < 0.35 {
                return None;
            }
            let jours = chrono::DateTime::parse_from_rfc3339(&s.quand).map(|d| (maintenant.signed_duration_since(d).num_hours() as f32 / 24.0).max(0.0)).unwrap_or(365.0);
            let recence = (-jours / 30.0).exp();
            Some((2.0 * rec.max(cos) + 0.3 * recence + 0.05 * s.usages.min(10) as f32, i))
        })
        .collect();
    notes.sort_by(|a, b| b.0.total_cmp(&a.0));
    let mut out = vec![];
    let mut taille = 0;
    for (_, i) in notes {
        let t = &l[i].texte;
        if taille + t.len() > budget {
            break;
        }
        taille += t.len();
        out.push(t.clone());
        l[i].usages += 1;
    }
    if !out.is_empty() {
        save(&l);
    }
    out
}

/// Liste lisible (`/souvenirs`).
pub fn texte() -> String {
    let l = load();
    if l.is_empty() {
        return t!("Je n'ai encore noté aucun souvenir. J'en note au fil des discussions (préférences, projets, personnes) ; /oublie <mot> pour en effacer.").into();
    }
    let mut s = t!("{} souvenir(s), des plus récents aux plus anciens (/oublie <mot> pour en effacer) :", l.len());
    for x in l.iter().take(30) {
        s.push_str(&format!("\n• {}", x.texte));
    }
    s
}

/// Fait noter les souvenirs durables de la fin d'une discussion par un petit modèle.
/// Rend le nombre de souvenirs ajoutés.
pub fn extraire(historique: &[Value], prenom: &str) -> Result<usize, String> {
    let fin: Vec<String> = historique
        .iter()
        .filter(|m| (m["role"] == "user" || m["role"] == "assistant") && m["content"].is_string())
        .rev()
        .take(8)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .map(|m| {
            let qui = if m["role"] == "user" { prenom } else { "L Katta" };
            format!("{qui} : {}", m["content"].as_str().unwrap_or("").chars().take(800).collect::<String>())
        })
        .collect();
    if fin.is_empty() {
        return Ok(0);
    }
    let deja: Vec<String> = load().into_iter().take(40).map(|s| format!("- {}", s.texte)).collect();
    let consigne = format!(
        "Tu tiens le carnet de souvenirs de L Katta sur {prenom}. D'après la discussion, note seulement des faits DURABLES et utiles plus tard : \
         préférences, habitudes, projets en cours, personnes de son entourage, contraintes. Pas de sujet du moment, pas de secret (mot de passe, clé, \
         numéro), rien sur la santé, aucune affirmation sur la machine ou les fichiers. Une phrase courte par fait, à la troisième personne \
         (« {prenom} préfère… »). Ne répète pas ce qui est déjà noté. Réponds uniquement en JSON : {{\"souvenirs\": [\"…\"]}} (liste vide si rien).\n\nDéjà noté :\n{}",
        if deja.is_empty() { "(rien)".to_string() } else { deja.join("\n") }
    );
    let messages = vec![json!({"role": "system", "content": consigne}), json!({"role": "user", "content": fin.join("\n\n")})];
    let opts = crate::moteur::Options { max_tokens: 900, ..Default::default() };
    let (msg, _) = crate::moteur::complete(&messages, &json!([]), "auto", &opts, &mut |_| {})?;
    let t = crate::brain::clean(msg.get("content").and_then(Value::as_str).unwrap_or(""));
    if std::env::var_os("LKATTA_DEBUG").is_some() {
        eprintln!("souvenirs proposés : {t}");
    }
    Ok(lire_json(&t).iter().filter(|s| ajouter(s)).count())
}

/// Liste `souvenirs` d'une réponse JSON (même entourée de texte ou de ```).
pub fn lire_json(t: &str) -> Vec<String> {
    let (Some(a), Some(b)) = (t.find('{'), t.rfind('}')) else { return vec![] };
    if b <= a {
        return vec![];
    }
    serde_json::from_str::<Value>(&t[a..=b])
        .ok()
        .and_then(|v| v["souvenirs"].as_array().cloned())
        .map(|l| l.iter().filter_map(|x| x.as_str().map(String::from)).collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guards_secrets_and_junk() {
        assert!(secret("ma clé est sk-or-v1-abcdef1234567890"));
        assert!(secret("carte 4970 1012 3456 7890"));
        assert!(secret("mon mot de passe est soleil"));
        assert!(!secret("Antsa préfère le thé au café"));
        assert_eq!(masquer("clé gsk_ABCDEFGH12345678 ok"), "clé [secret masqué] ok");
        assert!(acceptable("Antsa prépare un cours de mathématiques pour la L1"));
        assert!(!acceptable("Antsa veut que tu nettoies le cache"));
        assert!(!acceptable("ok"));
        assert_eq!(lire_json("```json\n{\"souvenirs\": [\"A aime le thé\"]}\n```"), vec!["A aime le thé"]);
        assert!(lire_json("rien").is_empty());
    }

    #[test]
    fn stores_dedups_and_recalls() {
        let dir = std::env::temp_dir().join(format!("lkatta-souv-{}", std::process::id()));
        // Fichier propre au test (le vrai carnet n'est pas touché).
        let _ = std::fs::create_dir_all(&dir);
        let f = dir.join("souvenirs.json");
        let l = vec![
            Souvenir { texte: "Antsa prépare un cours de probabilités pour ses étudiants de L1".into(), quand: chrono::Local::now().to_rfc3339(), usages: 0 },
            Souvenir { texte: "Antsa aime le romazava".into(), quand: chrono::Local::now().to_rfc3339(), usages: 0 },
        ];
        std::fs::write(&f, serde_json::to_string(&l).unwrap()).unwrap();
        let lus: Vec<Souvenir> = serde_json::from_str(&std::fs::read_to_string(&f).unwrap()).unwrap();
        assert_eq!(lus.len(), 2);
        // Pertinence : calcul direct (sans toucher au vrai fichier).
        let q = "Aide-moi à finir mon cours de probabilités";
        let vq = vecteur(q);
        let a = cosinus(&vq, &vecteur(&lus[0].texte)).max(recouvrement(q, &lus[0].texte));
        let b = cosinus(&vq, &vecteur(&lus[1].texte)).max(recouvrement(q, &lus[1].texte));
        assert!(a > 0.35 && b < 0.35, "{a} {b}");
        let _ = std::fs::remove_dir_all(dir);
    }
}

#[cfg(test)]
mod essai_reel {
    /// Essai réel (réseau) : `cargo test extrait_des_souvenirs -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn extrait_des_souvenirs() {
        let dir = std::env::temp_dir().join(format!("lkatta-souv-reel-{}", std::process::id()));
        // SAFETY : essai lancé seul (--ignored) ; aucun autre fil ne lit l'environnement.
        unsafe { std::env::set_var("XDG_DATA_HOME", &dir) };
        let h = vec![
            serde_json::json!({"role": "user", "content": "Je prépare un cours de probabilités pour mes étudiants de L1 à Antananarivo, pour lundi."}),
            serde_json::json!({"role": "assistant", "content": "Très bien ! Veux-tu un plan en trois parties ?"}),
            serde_json::json!({"role": "user", "content": "Oui. Et rappelle-toi que je préfère les exemples avec des zébus plutôt qu'avec des dés."}),
            serde_json::json!({"role": "assistant", "content": "Noté, des zébus partout."}),
        ];
        let n = super::extraire(&h, "Antsa").unwrap();
        println!("{n} souvenir(s) :\n{}", super::texte());
        assert!(n >= 1);
        let p = super::pertinents("fais-moi un exercice pour mon cours de probabilités", 900);
        println!("pertinents : {p:?}");
        assert!(!p.is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }
}
