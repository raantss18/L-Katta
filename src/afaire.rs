//! Liste de choses à faire : `afaire.json` dans le dossier de données. Une chose peut avoir une
//! échéance (rappelée le jour venu dans l'accueil) et un projet (le dossier du côté Code).

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

use crate::paths;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Chose {
    pub id: u64,
    pub texte: String,
    #[serde(default)]
    pub fait: bool,
    /// RFC 3339.
    pub cree: String,
    /// Échéance (RFC 3339), facultative.
    #[serde(default)]
    pub echeance: Option<String>,
    #[serde(default)]
    pub projet: Option<String>,
    /// Quand elle a été cochée (RFC 3339) : les choses faites partent au bout de 30 jours.
    #[serde(default)]
    pub faite_le: Option<String>,
}

impl Chose {
    pub fn echeance(&self) -> Option<DateTime<Local>> {
        self.echeance.as_deref().and_then(|e| DateTime::parse_from_rfc3339(e).ok()).map(|d| d.with_timezone(&Local))
    }
}

fn fichier() -> std::path::PathBuf {
    #[cfg(test)]
    if let Some(f) = ESSAI.with(|f| f.borrow().clone()) {
        return f;
    }
    paths::data_dir().join("afaire.json")
}

#[cfg(test)]
thread_local! {
    static ESSAI: std::cell::RefCell<Option<std::path::PathBuf>> = const { std::cell::RefCell::new(None) };
}

pub fn charger() -> Vec<Chose> {
    std::fs::read_to_string(fichier()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

fn enregistrer(l: &[Chose]) -> Result<(), String> {
    let _ = std::fs::create_dir_all(paths::data_dir());
    let s = serde_json::to_string_pretty(l).map_err(|e| e.to_string())?;
    paths::write_private(&fichier(), s.as_bytes()).map_err(|e| e.to_string())
}

pub fn ajouter(texte: &str, echeance: Option<DateTime<Local>>, projet: Option<String>) -> Result<Chose, String> {
    let texte = texte.trim();
    if texte.is_empty() {
        return Err(t!("rien à noter").into());
    }
    let mut l = charger();
    let id = l.iter().map(|c| c.id).max().unwrap_or(0) + 1;
    let c = Chose {
        id,
        texte: texte.to_string(),
        fait: false,
        cree: Local::now().to_rfc3339(),
        echeance: echeance.map(|d| d.to_rfc3339()),
        projet,
        faite_le: None,
    };
    l.push(c.clone());
    enregistrer(&l)?;
    Ok(c)
}

/// Coche (ou décoche) une chose ; purge celles faites depuis plus de 30 jours.
pub fn cocher(id: u64, fait: bool) -> Result<Option<Chose>, String> {
    let mut l = charger();
    let now = Local::now();
    let mut r = None;
    for c in l.iter_mut().filter(|c| c.id == id) {
        c.fait = fait;
        c.faite_le = fait.then(|| now.to_rfc3339());
        r = Some(c.clone());
    }
    l.retain(|c| {
        !c.fait || c.faite_le.as_deref().and_then(|d| DateTime::parse_from_rfc3339(d).ok()).is_none_or(|d| (now - d.with_timezone(&Local)).num_days() < 30)
    });
    enregistrer(&l)?;
    Ok(r)
}

pub fn supprimer(id: u64) -> Result<bool, String> {
    let mut l = charger();
    let n = l.len();
    l.retain(|c| c.id != id);
    enregistrer(&l)?;
    Ok(l.len() != n)
}

/// À faire, triées : en retard, puis par échéance, puis sans échéance (les plus anciennes d'abord).
pub fn en_cours() -> Vec<Chose> {
    let mut l: Vec<Chose> = charger().into_iter().filter(|c| !c.fait).collect();
    l.sort_by_key(|c| (c.echeance().is_none(), c.echeance(), c.id));
    l
}

/// Toute la liste triée (à faire d'abord), ou seulement ce qui reste à faire.
pub fn charger_trie(toutes: bool) -> Vec<Chose> {
    let mut l = en_cours();
    if toutes {
        l.extend(charger().into_iter().filter(|c| c.fait));
    }
    l
}

/// Choses dues aujourd'hui ou en retard (pour l'accueil du matin).
pub fn du_jour(now: DateTime<Local>) -> Vec<Chose> {
    en_cours().into_iter().filter(|c| c.echeance().is_some_and(|d| d.date_naive() <= now.date_naive())).collect()
}

pub fn texte(l: &[Chose], toutes: bool) -> String {
    let l: Vec<&Chose> = if toutes { l.iter().collect() } else { l.iter().filter(|c| !c.fait).collect() };
    if l.is_empty() {
        return t!("Rien à faire de noté.").into();
    }
    let now = Local::now();
    l.iter()
        .map(|c| {
            let coche = if c.fait { "☑" } else { "☐" };
            let ech = c
                .echeance()
                .map(|d| {
                    let retard = !c.fait && d < now;
                    let j = crate::fr::date_longue(&d);
                    if retard { t!(" — en retard ({})", j) } else { t!(" — pour {}", j) }
                })
                .unwrap_or_default();
            let projet = c.projet.as_deref().map(|p| format!(" [{p}]")).unwrap_or_default();
            format!("n°{} {coche} {}{projet}{ech}", c.id, c.texte)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_checks_and_sorts() {
        let f = std::env::temp_dir().join(format!("lkatta-afaire-{}.json", std::process::id()));
        let _ = std::fs::remove_file(&f);
        ESSAI.with(|e| *e.borrow_mut() = Some(f.clone()));
        let now = Local::now();
        let a = ajouter("Relire le chapitre 3", None, None).unwrap();
        let b = ajouter("Payer la JIRAMA", Some(now - chrono::Duration::days(1)), None).unwrap();
        ajouter("Rendre le TP", Some(now + chrono::Duration::days(2)), Some("L-Katta".into())).unwrap();
        let l = en_cours();
        assert_eq!(l[0].id, b.id, "en retard d'abord");
        assert_eq!(l.last().unwrap().id, a.id, "sans échéance à la fin");
        assert_eq!(du_jour(now).len(), 1);
        assert!(texte(&l, false).contains("en retard"));
        cocher(b.id, true).unwrap();
        assert_eq!(en_cours().len(), 2);
        assert!(supprimer(a.id).unwrap());
        assert!(ajouter("  ", None, None).is_err());
        let _ = std::fs::remove_file(&f);
    }
}
