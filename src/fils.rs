//! Fils de discussion : chaque conversation est un fil, d'un côté de la bulle.
//!
//! - **Discussion** : le divers (questions, idées, web, mails, agenda). Fils rangés par date.
//! - **Code** : manipuler fichiers et système, dans le dossier d'un projet. Fils rangés par
//!   projet (dossier), puis par date.
//!
//! Un fichier par fil (`fils/<id>.json`, droits 600) et un index léger (`fils/index.json`) :
//! la bulle ne garde en mémoire que le fil ouvert ; l'historique lit l'index. Chaque fil a sa
//! propre fiche (résumé de son début), donc deux sujets ne se mélangent jamais.
//!
//! Titre : provisoire dès la première question (ses premiers mots), puis un petit modèle en
//! propose un meilleur en arrière-plan. Recherche : par le sens (vecteurs de `semantique`) sur
//! le titre, les premières questions et la fiche, sans réseau.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::chat::Entry;
use crate::paths;

/// Côté de la bulle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Cote {
    #[default]
    Discussion,
    Code,
}

/// Un fil complet (celui qui est ouvert).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Fil {
    pub id: String,
    pub cote: Cote,
    #[serde(default)]
    pub titre: String,
    /// Côté Code : dossier du projet (chemin avec `~`).
    #[serde(default)]
    pub dossier: Option<String>,
    pub cree: String,
    #[serde(default)]
    pub maj: String,
    /// Messages envoyés au modèle (la fin de la discussion ; le début est dans la fiche).
    #[serde(default)]
    pub history: Vec<Value>,
    /// Ce que la bulle affiche.
    #[serde(default)]
    pub entries: Vec<Entry>,
    #[serde(default)]
    pub fiche: String,
}

/// Ce que l'historique montre d'un fil, sans l'ouvrir.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Resume {
    pub id: String,
    pub cote: Cote,
    pub titre: String,
    #[serde(default)]
    pub dossier: Option<String>,
    pub maj: String,
    #[serde(default)]
    pub questions: usize,
    /// Premières questions et début de fiche (pour la recherche par le sens).
    #[serde(default)]
    pub texte: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Index {
    #[serde(default)]
    fils: Vec<Resume>,
    #[serde(default)]
    actif_discussion: Option<String>,
    #[serde(default)]
    actif_code: Option<String>,
    /// Dernier côté utilisé.
    #[serde(default)]
    cote: Cote,
    /// Dernier dossier de projet du côté Code.
    #[serde(default)]
    dossier: Option<String>,
}

fn dir() -> std::path::PathBuf {
    paths::data_dir().join("fils")
}
fn index_file() -> std::path::PathBuf {
    dir().join("index.json")
}
fn fil_file(id: &str) -> std::path::PathBuf {
    // L'identifiant vient de nous, mais on ne fait jamais confiance à un nom de fichier.
    let sur: String = id.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').collect();
    dir().join(format!("{sur}.json"))
}

fn lire_index() -> Index {
    std::fs::read_to_string(index_file()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}
fn ecrire_index(i: &Index) {
    if let Ok(t) = serde_json::to_string(i) {
        let _ = paths::write_private(&index_file(), t.as_bytes());
    }
}

fn maintenant() -> String {
    chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, false)
}

/// Un fil vide (enregistré seulement à sa première question).
pub fn nouveau(cote: Cote, dossier: Option<String>) -> Fil {
    let n = chrono::Local::now();
    Fil {
        id: format!("{}-{:04x}", n.format("%Y%m%d-%H%M%S"), fastrand::u16(..)),
        cote,
        dossier: if cote == Cote::Code { dossier.or_else(dossier_code) } else { None },
        cree: maintenant(),
        maj: maintenant(),
        ..Default::default()
    }
}

pub fn charger(id: &str) -> Option<Fil> {
    let t = std::fs::read_to_string(fil_file(id)).ok()?;
    serde_json::from_str(&t).ok()
}

/// Nombre de questions posées dans un fil.
pub fn questions(f: &Fil) -> usize {
    questions_dans(&f.entries)
}

pub fn questions_dans(entries: &[Entry]) -> usize {
    entries.iter().filter(|e| matches!(e, Entry::User(_))).count()
}

/// Enregistre le fil (s'il a au moins une question) et met l'index à jour.
pub fn enregistrer(f: &mut Fil) {
    let n = questions(f);
    if n == 0 && f.history.is_empty() {
        return;
    }
    // Rien n'a changé depuis le dernier enregistrement : le fil garde sa date (rouvrir un
    // fil ne le fait pas remonter dans l'historique).
    if let Some(avant) = charger(&f.id) {
        if avant.entries == f.entries && avant.history == f.history && avant.fiche == f.fiche && avant.titre == f.titre && avant.dossier == f.dossier {
            f.maj = avant.maj;
            return;
        }
    }
    f.maj = maintenant();
    if f.titre.trim().is_empty() {
        f.titre = titre_provisoire(premiere_question(f).as_deref().unwrap_or(""));
    }
    if let Ok(t) = serde_json::to_string(f) {
        let _ = paths::write_private(&fil_file(&f.id), t.as_bytes());
    }
    let r = resume(f);
    let mut i = lire_index();
    i.fils.retain(|x| x.id != f.id);
    i.fils.push(r);
    match f.cote {
        Cote::Discussion => i.actif_discussion = Some(f.id.clone()),
        Cote::Code => {
            i.actif_code = Some(f.id.clone());
            if f.dossier.is_some() {
                i.dossier = f.dossier.clone();
            }
        }
    }
    ecrire_index(&i);
}

fn premiere_question(f: &Fil) -> Option<String> {
    f.entries.iter().find_map(|e| if let Entry::User(q) = e { Some(q.clone()) } else { None })
}

fn resume(f: &Fil) -> Resume {
    let qs: Vec<String> = f.entries.iter().filter_map(|e| if let Entry::User(q) = e { Some(q.chars().take(200).collect()) } else { None }).take(4).collect();
    let fiche: String = f.fiche.chars().take(400).collect();
    Resume {
        id: f.id.clone(),
        cote: f.cote,
        titre: f.titre.clone(),
        dossier: f.dossier.clone(),
        maj: f.maj.clone(),
        questions: questions(f),
        texte: format!("{}\n{}", qs.join("\n"), fiche),
    }
}

/// Fil ouvert en dernier de ce côté.
pub fn actif(cote: Cote) -> Option<String> {
    let i = lire_index();
    match cote {
        Cote::Discussion => i.actif_discussion,
        Cote::Code => i.actif_code,
    }
}

/// Retient le côté utilisé (rouvert au prochain démarrage).
pub fn retenir_cote(cote: Cote) {
    let mut i = lire_index();
    if i.cote != cote {
        i.cote = cote;
        ecrire_index(&i);
    }
}

pub fn dernier_cote() -> Cote {
    lire_index().cote
}

/// Dernier dossier de projet du côté Code.
pub fn dossier_code() -> Option<String> {
    lire_index().dossier
}

/// Tous les fils d'un côté, du plus récent au plus ancien.
pub fn liste(cote: Cote) -> Vec<Resume> {
    let mut v: Vec<Resume> = lire_index().fils.into_iter().filter(|r| r.cote == cote).collect();
    v.sort_by(|a, b| b.maj.cmp(&a.maj));
    v
}

/// Met un fil à la corbeille (`fils/corbeille/`), récupérable à la main.
pub fn supprimer(id: &str) {
    let src = fil_file(id);
    let dst = dir().join("corbeille").join(src.file_name().unwrap_or_default());
    let _ = std::fs::create_dir_all(dst.parent().unwrap());
    let _ = std::fs::rename(&src, &dst);
    let mut i = lire_index();
    i.fils.retain(|x| x.id != id);
    if i.actif_discussion.as_deref() == Some(id) {
        i.actif_discussion = None;
    }
    if i.actif_code.as_deref() == Some(id) {
        i.actif_code = None;
    }
    ecrire_index(&i);
}

/// Change le titre d'un fil (titre trouvé par le petit modèle).
pub fn renommer(id: &str, titre: &str) {
    let Some(mut f) = charger(id) else { return };
    f.titre = titre.to_string();
    let maj = f.maj.clone();
    enregistrer(&mut f);
    // Un titre n'est pas une activité : la date du fil ne bouge pas.
    let mut i = lire_index();
    if let Some(r) = i.fils.iter_mut().find(|r| r.id == id) {
        r.maj = maj;
    }
    ecrire_index(&i);
}

/// Les premiers mots de la question, en titre (en attendant mieux).
pub fn titre_provisoire(q: &str) -> String {
    let q = q.trim().trim_start_matches('/');
    let mots: Vec<&str> = q.split_whitespace().take(7).collect();
    let mut t = mots.join(" ");
    if t.chars().count() > 48 {
        t = format!("{}…", t.chars().take(47).collect::<String>());
    } else if q.split_whitespace().count() > 7 {
        t.push('…');
    }
    let mut c = t.chars();
    match c.next() {
        Some(p) => p.to_uppercase().collect::<String>() + c.as_str(),
        None => crate::lang::trd("Sans titre").to_string(),
    }
}

/// Un vrai titre (2 à 6 mots) par un petit modèle, d'après le début du fil.
pub fn titrer(history: &[Value]) -> Result<String, String> {
    let texte: String = history
        .iter()
        .filter(|m| m["role"] == "user" || m["role"] == "assistant")
        .filter_map(|m| m["content"].as_str())
        .take(4)
        .map(|c| c.chars().take(600).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n---\n");
    if texte.trim().is_empty() {
        return Err("vide".into());
    }
    let messages = vec![
        json!({"role": "system", "content": "Donne un titre court (2 à 6 mots) à cette discussion, dans sa langue, sans guillemets ni point final, sans emoji. Réponds seulement par le titre."}),
        json!({"role": "user", "content": texte}),
    ];
    let opts = crate::moteur::Options { max_tokens: 60, ..Default::default() };
    let (msg, _) = crate::moteur::complete(&messages, &json!([]), "auto", &opts, &mut |_| {})?;
    let t = crate::brain::clean(msg.get("content").and_then(Value::as_str).unwrap_or(""));
    let t = t.lines().find(|l| !l.trim().is_empty()).unwrap_or("").trim().trim_matches(|c| c == '"' || c == '«' || c == '»' || c == '*' || c == '#' || c == '.').trim().to_string();
    if t.is_empty() || t.chars().count() > 70 { Err("titre inutilisable".into()) } else { Ok(t) }
}

/// Fils qui correspondent à une recherche (par le sens et par les mots), les meilleurs d'abord.
pub fn chercher(cote: Cote, requete: &str) -> Vec<Resume> {
    let q = requete.trim();
    if q.is_empty() {
        return liste(cote);
    }
    let vq = crate::semantique::vecteur(q);
    let qs = crate::semantique::simplifier(q);
    let mut v: Vec<(f32, Resume)> = liste(cote)
        .into_iter()
        .map(|r| {
            let tout = format!("{} {} {}", r.titre, r.dossier.as_deref().unwrap_or(""), r.texte);
            let sens = crate::semantique::cosinus(&vq, &crate::semantique::vecteur(&tout));
            let mots = crate::semantique::recouvrement(q, &tout);
            let titre = if crate::semantique::simplifier(&r.titre).contains(&qs) { 0.5 } else { 0.0 };
            (sens + mots * 0.6 + titre, r)
        })
        .filter(|(s, _)| *s >= 0.18)
        .collect();
    v.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    v.into_iter().map(|(_, r)| r).take(40).collect()
}

/// Repères de temps de l'historique (côté Discussion).
pub fn periode(maj: &str) -> &'static str {
    let Ok(d) = chrono::DateTime::parse_from_rfc3339(maj) else { return "Plus ancien" };
    let auj = chrono::Local::now().date_naive();
    let j = d.with_timezone(&chrono::Local).date_naive();
    let ecart = (auj - j).num_days();
    match ecart {
        ..=0 => "Aujourd'hui",
        1 => "Hier",
        2..=6 => "Cette semaine",
        7..=30 => "Ce mois-ci",
        _ => "Plus ancien",
    }
}

/// Nom court d'un projet (dernier élément du dossier).
pub fn nom_projet(dossier: &str) -> String {
    let d = dossier.trim_end_matches('/');
    d.rsplit('/').next().filter(|s| !s.is_empty()).unwrap_or(d).to_string()
}

/// Date courte d'un fil (heure aujourd'hui, sinon jour et mois).
pub fn date_courte(maj: &str) -> String {
    let Ok(d) = chrono::DateTime::parse_from_rfc3339(maj) else { return String::new() };
    let d = d.with_timezone(&chrono::Local);
    if d.date_naive() == chrono::Local::now().date_naive() { d.format("%H:%M").to_string() } else { d.format("%d/%m").to_string() }
}

/// Dossier de projet valide : existe, est un dossier, sous le dossier personnel ou ailleurs
/// mais jamais la racine ni un dossier système.
pub fn dossier_valide(chemin: &str) -> Result<String, String> {
    let c = chemin.trim().trim_end_matches('/');
    if c.is_empty() {
        return Err(crate::lang::trd("Donne le chemin d'un dossier (ex. ~/GitHub/mon-projet).").into());
    }
    let p = paths::expand(c);
    let p = if p.is_relative() { paths::home_dir().join(p) } else { p };
    if !p.is_dir() {
        return Err(t!("Ce dossier n'existe pas : {}", c));
    }
    let interdits = ["/", "/etc", "/usr", "/bin", "/sbin", "/boot", "/dev", "/proc", "/sys", "/var", "/lib", "/lib64", "/root"];
    let abs = p.canonicalize().unwrap_or(p.clone());
    if interdits.iter().any(|i| abs == std::path::Path::new(i)) {
        return Err(t!("Pas un dossier système comme projet : {}", abs.display()));
    }
    let home = paths::home_dir();
    Ok(match abs.strip_prefix(&home) {
        Ok(r) if r.as_os_str().is_empty() => "~".into(),
        Ok(r) => format!("~/{}", r.display()),
        Err(_) => abs.display().to_string(),
    })
}

/// Première ouverture : la discussion d'avant (journal récent et fiche) devient le premier fil.
pub fn migrer() {
    if index_file().exists() {
        return;
    }
    let mut f = nouveau(Cote::Discussion, None);
    for e in crate::memoire::derniers(3) {
        f.entries.push(Entry::User(e.q.clone()));
        f.entries.extend(e.etapes.iter().map(|s| Entry::Info(s.clone())));
        f.entries.push(Entry::Maki(e.r.clone()));
        f.history.push(json!({"role": "user", "content": e.q}));
        f.history.push(json!({"role": "assistant", "content": e.r}));
    }
    let ancienne = paths::data_dir().join("fiche-discussion.txt");
    f.fiche = std::fs::read_to_string(&ancienne).unwrap_or_default();
    if questions(&f) > 0 {
        f.titre = crate::lang::trd("Discussion d'avant").to_string();
        enregistrer(&mut f);
    } else {
        ecrire_index(&Index::default());
    }
    let _ = std::fs::remove_file(ancienne);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_and_periods() {
        assert_eq!(titre_provisoire("pourquoi mon disque est plein ?"), "Pourquoi mon disque est plein ?");
        assert_eq!(titre_provisoire("range les photos de 2025 par mois dans des dossiers séparés"), "Range les photos de 2025 par mois…");
        assert_eq!(nom_projet("~/GitHub/L-Katta/"), "L-Katta");
        assert_eq!(periode(&maintenant()), "Aujourd'hui");
        assert_eq!(periode("2020-01-01T10:00:00+03:00"), "Plus ancien");
    }

    #[test]
    fn rejects_system_folders() {
        assert!(dossier_valide("/").is_err());
        assert!(dossier_valide("/etc").is_err());
        assert!(dossier_valide("/nexiste/pas").is_err());
        assert!(dossier_valide("/tmp").is_ok());
        assert!(fil_file("../../etc/passwd").to_string_lossy().ends_with("etcpasswd.json"));
    }
}
