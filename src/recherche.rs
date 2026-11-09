//! Recherche approfondie : le modèle prépare plusieurs requêtes, on cherche sur le web,
//! on lit les pages (en parallèle), un second tour comble ce qui manque, puis le modèle
//! rédige une réponse qui cite ses sources numérotées. Aucune clé nécessaire (DuckDuckGo).

use serde_json::{Value, json};

use crate::brain::{self, Brain, Event, Host};
use crate::{profile, web};

const MAX_SOURCES: usize = 8;
const PAR_SOURCE: usize = 3000;

pub struct Source {
    pub titre: String,
    pub url: String,
    pub texte: String,
}

/// Interface qui ne montre rien : pour les étapes internes (requêtes, bilan), dont le
/// texte ne doit pas s'écrire dans la bulle.
struct Muet<'a>(&'a mut dyn Host);

impl Host for Muet<'_> {
    fn confirm(&mut self, q: &str) -> bool {
        self.0.confirm(q)
    }
    fn event(&mut self, e: Event) {
        if let Event::Engine(_) = e {
            self.0.event(e);
        }
    }
}

pub fn approfondie(b: &mut Brain, history: &mut Vec<Value>, host: &mut dyn Host) -> Result<String, String> {
    let question = history.iter().rev().find(|m| m["role"] == "user").and_then(|m| m["content"].as_str()).unwrap_or("").to_string();
    if question.trim().is_empty() {
        return Err(t!("Quelle est la question ?").into());
    }
    let date = chrono::Local::now().format("%Y-%m-%d").to_string();
    let mut sources: Vec<Source> = vec![];
    let mut faites: Vec<String> = vec![];
    let consigne = format!(
        "Date : {date}.\nQuestion : {question}\n\nDonne 3 à 5 requêtes de recherche web variées et précises pour y répondre \
         (en anglais quand cela trouvera de meilleures sources). Une requête par ligne, rien d'autre."
    );
    let mut requetes = demander_requetes(b, &consigne, host).unwrap_or_default();
    if requetes.is_empty() {
        requetes.push(question.chars().take(200).collect());
    }
    for tour in 0..2 {
        let candidats = chercher(&requetes, &mut faites, &sources, host);
        lire(candidats, &mut sources, host);
        if tour == 1 || sources.len() >= MAX_SOURCES || sources.is_empty() {
            break;
        }
        let lu: String = sources.iter().enumerate().map(|(i, s)| format!("[{}] {} — {}\n", i + 1, s.titre, s.texte.chars().take(300).collect::<String>().replace('\n', " "))).collect();
        let consigne = format!(
            "Question : {question}\n\nSources déjà lues :\n{lu}\nRequêtes déjà faites : {}\n\nS'il manque une information importante pour bien répondre, \
             donne 1 ou 2 nouvelles requêtes, une par ligne. Sinon réponds exactement : RIEN",
            faites.join(" ; ")
        );
        requetes = demander_requetes(b, &consigne, host).unwrap_or_default();
        if requetes.is_empty() {
            break;
        }
    }
    if sources.is_empty() {
        return Err(t!("Recherche impossible : aucune page lisible trouvée (réseau ?).").into());
    }
    // Synthèse, écrite au fil de l'eau dans la bulle.
    let mut system = profile::system_prompt(&b.cfg);
    system.push_str(
        "\n\n## Recherche approfondie\nRéponds à la dernière question en t'appuyant sur les sources numérotées fournies, \
         et cite-les ainsi : [1], [2]. Commence par la réponse courte, puis les détails utiles. Signale les désaccords \
         entre sources et ce qui reste incertain. N'invente rien qui ne soit pas dans les sources ; n'ajoute pas de liste \
         des sources à la fin (elle est ajoutée automatiquement).",
    );
    let mut corps = format!("{question}\n\n---\nSources (lues le {date}) :\n");
    for (i, s) in sources.iter().enumerate() {
        corps.push_str(&format!("\n[{}] {} — {}\n{}\n", i + 1, s.titre, s.url, s.texte));
    }
    let mut messages = vec![json!({"role": "system", "content": system})];
    // Le fil de la discussion (sans la question, remplacée par sa version avec sources).
    let n = history.len().saturating_sub(1);
    messages.extend(history[..n].iter().filter(|m| m["role"] == "user" || (m["role"] == "assistant" && m.get("tool_calls").is_none())).cloned());
    messages.push(json!({"role": "user", "content": corps}));
    let msg = b.complete(&messages, &json!([]), host)?;
    let mut texte = brain::clean(msg.get("content").and_then(Value::as_str).unwrap_or(""));
    if texte.trim().is_empty() {
        return Err(t!("(réponse vide)").into());
    }
    texte.push_str(&format!("\n\n**{}**\n", t!("Sources")));
    for (i, s) in sources.iter().enumerate() {
        let titre = if s.titre.trim().is_empty() { s.url.clone() } else { s.titre.replace(['[', ']'], "") };
        texte.push_str(&format!("{}. [{}]({})\n", i + 1, titre, s.url));
    }
    history.push(json!({"role": "assistant", "content": texte}));
    Ok(texte)
}

/// Demande des requêtes au modèle (sans rien afficher) et les nettoie.
fn demander_requetes(b: &mut Brain, consigne: &str, host: &mut dyn Host) -> Result<Vec<String>, String> {
    let messages = vec![
        json!({"role": "system", "content": "Tu prépares des recherches web. Tu réponds uniquement par des requêtes, une par ligne, sans numéros, puces ni guillemets."}),
        json!({"role": "user", "content": consigne}),
    ];
    let msg = b.complete(&messages, &json!([]), &mut Muet(host))?;
    Ok(requetes(&brain::clean(msg.get("content").and_then(Value::as_str).unwrap_or(""))))
}

/// Lignes de requêtes d'une réponse du modèle (« RIEN » : aucune).
pub fn requetes(texte: &str) -> Vec<String> {
    let t = texte.trim();
    if t.eq_ignore_ascii_case("rien") || t.eq_ignore_ascii_case("none") || t.to_uppercase().starts_with("RIEN") {
        return vec![];
    }
    t.lines()
        .map(|l| {
            let l = l.trim().trim_start_matches(['-', '*', '•']).trim();
            let l = l.trim_start_matches(|c: char| c.is_ascii_digit()).trim_start_matches(['.', ')']).trim();
            l.trim_matches(['"', '«', '»', '“', '”', '`']).trim().to_string()
        })
        .filter(|l| l.len() >= 3 && l.len() <= 200 && !l.ends_with(':'))
        .take(5)
        .collect()
}

/// Lance les recherches ; rend les pages nouvelles à lire (titre, adresse, extrait).
fn chercher(requetes: &[String], faites: &mut Vec<String>, deja: &[Source], host: &mut dyn Host) -> Vec<(String, String, String)> {
    let place = MAX_SOURCES.saturating_sub(deja.len());
    let mut out: Vec<(String, String, String)> = vec![];
    for q in requetes {
        if faites.contains(q) {
            continue;
        }
        faites.push(q.clone());
        host.event(Event::Tool("recherche_web".into(), t!("je cherche sur le web « {} »", q)));
        let Ok(rs) = web::search(q, 6) else { continue };
        let mut pris = 0;
        for r in rs {
            if pris >= 3 || out.len() >= place {
                break;
            }
            let vu = |u: &str| deja.iter().any(|s| same_page(&s.url, u)) || out.iter().any(|(_, x, _)| same_page(x, u));
            // Deux pages au plus par site : des points de vue variés.
            let d = site(&r.url);
            let meme_site = deja.iter().filter(|s| site(&s.url) == d).count() + out.iter().filter(|(_, x, _)| site(x) == d).count();
            if !vu(&r.url) && meme_site < 2 {
                out.push((r.titre, r.url, r.extrait));
                pris += 1;
            }
        }
    }
    out
}

fn same_page(a: &str, b: &str) -> bool {
    let n = |u: &str| u.trim_end_matches('/').split('#').next().unwrap_or("").replace("://www.", "://").to_string();
    n(a) == n(b)
}

/// Lit les pages en parallèle ; une page illisible garde son extrait de recherche.
fn lire(candidats: Vec<(String, String, String)>, sources: &mut Vec<Source>, host: &mut dyn Host) {
    for (_, url, _) in &candidats {
        host.event(Event::Tool("lire_page_web".into(), t!("je lis {}", domaine(url))));
    }
    let fils: Vec<_> = candidats
        .into_iter()
        .map(|(titre, url, extrait)| {
            std::thread::spawn(move || {
                let texte = web::page_text(&url, PAR_SOURCE * 3).map(|t| condenser(&t)).unwrap_or_default();
                let texte = if texte.chars().count() < 200 { extrait } else { texte };
                Source { titre, url, texte }
            })
        })
        .collect();
    for f in fils {
        if let Ok(s) = f.join() {
            if s.texte.trim().chars().count() >= 40 {
                sources.push(s);
            }
        }
    }
}

/// Texte de page sans lignes vides ni bouts de menu, tronqué.
fn condenser(t: &str) -> String {
    let mut out = String::new();
    for l in t.lines().map(str::trim).filter(|l| l.split_whitespace().count() >= 4) {
        if out.chars().count() + l.chars().count() > PAR_SOURCE {
            break;
        }
        out.push_str(l);
        out.push('\n');
    }
    out
}

/// Site sans sous-domaine (« cdn.kernel.org » → « kernel.org »).
fn site(url: &str) -> String {
    let d = domaine(url);
    let p: Vec<&str> = d.split('.').collect();
    if p.len() > 2 { p[p.len() - 2..].join(".") } else { d }
}

pub fn domaine(url: &str) -> String {
    url.split("://").nth(1).unwrap_or(url).split('/').next().unwrap_or(url).trim_start_matches("www.").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_queries() {
        assert_eq!(requetes("1. prix du riz Madagascar 2026\n- \"rice price Madagascar\"\n\n• riz importé"), vec!["prix du riz Madagascar 2026", "rice price Madagascar", "riz importé"]);
        assert!(requetes("RIEN").is_empty());
        assert!(requetes("Rien.").is_empty());
        assert_eq!(requetes("Voici les requêtes :\nrust layer shell").len(), 1);
    }

    #[test]
    fn dedups_and_condenses() {
        assert!(same_page("https://www.ex.mg/a/", "https://ex.mg/a#x"));
        assert!(!same_page("https://ex.mg/a", "https://ex.mg/b"));
        assert_eq!(domaine("https://www.wikipedia.org/wiki/X"), "wikipedia.org");
        assert_eq!(site("https://cdn.kernel.org/pub"), "kernel.org");
        assert_eq!(condenser("Accueil\nMenu\nCeci est une vraie phrase utile.\n"), "Ceci est une vraie phrase utile.\n");
    }
}
