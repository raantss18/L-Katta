//! Sauvegardes planifiées des dossiers choisis vers un disque, une clé ou un partage monté (NAS).
//!
//! Chaque sauvegarde est une copie datée (`lkatta-sauvegardes/AAAA-MM-JJ_HHMM/`) faite par
//! rsync : les fichiers inchangés depuis la copie précédente y sont des liens physiques, donc une
//! nouvelle copie ne prend que la place de ce qui a changé, et chacune se lit comme un dossier
//! normal. Lancée à basse priorité (nice, ionice) pour ne pas gêner. Les `garder` dernières copies
//! restent ; seules les copies de L Katta sont effacées, jamais rien d'autre sur la destination.
//!
//! Rien ne part tant que `sauvegarde.destination` est vide.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

use crate::config::{Config, Sauvegarde};
use crate::conseils::{Kind, Message};
use crate::paths;

/// Dossiers jamais copiés : ils se recréent tout seuls.
const EXCLUS: &[&str] = &["node_modules/", "target/", ".cache/", "__pycache__/", ".venv/", ".gradle/", ".dart_tool/", ".next/"];

#[derive(Debug, Default, Serialize, Deserialize)]
struct Etat {
    #[serde(default)]
    derniere: Option<String>,
    /// Jour (AAAA-MM-JJ) du dernier avertissement « destination absente ».
    #[serde(default)]
    averti: String,
}

fn fichier() -> PathBuf {
    paths::data_dir().join("sauvegarde.json")
}

fn charger() -> Etat {
    std::fs::read_to_string(fichier()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

fn enregistrer(e: &Etat) {
    if let Ok(s) = serde_json::to_string(e) {
        let _ = paths::write_private(&fichier(), s.as_bytes());
    }
}

fn derniere(e: &Etat) -> Option<DateTime<Local>> {
    e.derniere.as_deref().and_then(|d| DateTime::parse_from_rfc3339(d).ok()).map(|d| d.with_timezone(&Local))
}

/// Nom d'une copie : « 2026-11-16_0930 ».
fn est_copie(nom: &str) -> bool {
    let b = nom.as_bytes();
    b.len() == 15 && b[4] == b'-' && b[7] == b'-' && b[10] == b'_' && b.iter().enumerate().all(|(i, c)| [4, 7, 10].contains(&i) || c.is_ascii_digit())
}

/// Copies existantes, de la plus ancienne à la plus récente.
fn copies(racine: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(racine)
        .map(|r| r.flatten().map(|e| e.path()).filter(|p| p.is_dir() && !p.is_symlink() && p.file_name().is_some_and(|n| est_copie(&n.to_string_lossy()))).collect())
        .unwrap_or_default();
    v.sort();
    v
}

/// Les dossiers à sauvegarder, chacun avec le nom qu'il aura dans la copie.
fn sources(r: &Sauvegarde) -> Vec<(PathBuf, String)> {
    let mut v: Vec<(PathBuf, String)> = vec![];
    for d in &r.dossiers {
        let p = paths::expand(d);
        if !p.is_dir() {
            continue;
        }
        let base = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "racine".into());
        let mut nom = base.clone();
        let mut i = 2;
        while v.iter().any(|(_, n)| *n == nom) {
            nom = format!("{base}-{i}");
            i += 1;
        }
        v.push((p, nom));
    }
    v
}

/// Lance une sauvegarde maintenant. Rend un résumé.
pub fn lancer(r: &Sauvegarde) -> Result<String, String> {
    if r.destination.trim().is_empty() {
        return Err(t!("aucune destination réglée : lkatta regle sauvegarde.destination /chemin/du/disque").into());
    }
    let dest = paths::expand(&r.destination);
    if !dest.is_dir() {
        return Err(t!("destination absente : {} (disque branché, partage monté ?)", dest.display()).into());
    }
    let srcs = sources(r);
    if srcs.is_empty() {
        return Err(t!("aucun des dossiers à sauvegarder n'existe ({})", r.dossiers.join(", ")).into());
    }
    let dest_c = std::fs::canonicalize(&dest).map_err(|e| e.to_string())?;
    for (s, _) in &srcs {
        let s = std::fs::canonicalize(s).map_err(|e| e.to_string())?;
        if dest_c.starts_with(&s) || s.starts_with(&dest_c) {
            return Err(t!("la destination est dans un dossier sauvegardé (ou l'inverse) : {}", s.display()).into());
        }
    }
    let racine = dest.join("lkatta-sauvegardes");
    std::fs::create_dir_all(&racine).map_err(|e| t!("destination en lecture seule ? {}", e))?;
    // Copies interrompues d'une fois précédente : ce sont les nôtres, on les retire.
    if let Ok(rd) = std::fs::read_dir(&racine) {
        for e in rd.flatten() {
            let n = e.file_name().to_string_lossy().into_owned();
            if n.ends_with(".partiel") && est_copie(n.trim_end_matches(".partiel")) {
                let _ = std::fs::remove_dir_all(e.path());
            }
        }
    }
    let t0 = std::time::Instant::now();
    let nom = Local::now().format("%Y-%m-%d_%H%M").to_string();
    let precedente = copies(&racine).into_iter().rev().find(|p| p.file_name().is_some_and(|n| n.to_string_lossy() != nom));
    let partiel = racine.join(format!("{nom}.partiel"));
    std::fs::create_dir_all(&partiel).map_err(|e| e.to_string())?;
    let rsync = crate::config::which("rsync");
    for (src, n) in &srcs {
        let cible = partiel.join(n);
        let mut c = if crate::config::which("nice") { crate::hote::commande("nice") } else { crate::hote::commande("env") };
        if crate::config::which("nice") {
            c.args(["-n", "19"]);
            if crate::config::which("ionice") {
                c.args(["ionice", "-c3"]);
            }
        }
        if rsync {
            c.args(["rsync", "-a", "--no-specials", "--no-devices"]);
            for x in EXCLUS {
                c.arg(format!("--exclude={x}"));
            }
            if let Some(p) = &precedente {
                c.arg(format!("--link-dest={}", p.join(n).display()));
            }
            c.arg(format!("{}/", src.display())).arg(format!("{}/", cible.display()));
        } else {
            // Sans rsync : copie complète (plus lente, plus de place).
            c.args(["cp", "-a", "--reflink=auto"]).arg(src).arg(&cible);
        }
        let o = c.stdin(std::process::Stdio::null()).output().map_err(|e| e.to_string())?;
        // rsync 24 : des fichiers ont disparu pendant la copie (normal sur un dossier vivant).
        if !o.status.success() && o.status.code() != Some(24) {
            let err = String::from_utf8_lossy(&o.stderr).lines().take(3).collect::<Vec<_>>().join(" ");
            return Err(t!("copie de {} interrompue : {}", src.display(), err).into());
        }
    }
    let finale = racine.join(&nom);
    if finale.exists() {
        // Deux sauvegardes dans la même minute : la nouvelle remplace l'autre.
        let _ = std::fs::remove_dir_all(&finale);
    }
    std::fs::rename(&partiel, &finale).map_err(|e| e.to_string())?;
    let retirees = elaguer(&racine, r.garder.max(1));
    let mut msg = t!(
        "Sauvegarde faite : {} dossier(s) dans {} ({} s).",
        srcs.len(),
        finale.display(),
        t0.elapsed().as_secs()
    );
    if !rsync {
        msg.push_str(&format!(" {}", t!("Installe rsync : les copies suivantes ne prendront que la place de ce qui change.")));
    }
    if retirees > 0 {
        msg.push_str(&format!(" {}", t!("{} ancienne(s) copie(s) retirée(s).", retirees)));
    }
    Ok(msg)
}

/// Lance la sauvegarde et note sa date si elle a réussi (commande, outil, planification).
pub fn lancer_et_noter(r: &Sauvegarde) -> Result<String, String> {
    let m = lancer(r)?;
    let mut e = charger();
    e.derniere = Some(Local::now().to_rfc3339());
    enregistrer(&e);
    Ok(m)
}

/// Ne garde que les `garder` copies les plus récentes ; rend le nombre de copies retirées.
fn elaguer(racine: &Path, garder: u32) -> usize {
    let l = copies(racine);
    let trop = l.len().saturating_sub(garder as usize);
    l.into_iter().take(trop).filter(|p| p.parent() == Some(racine) && std::fs::remove_dir_all(p).is_ok()).count()
}

/// État en texte.
pub fn etat(r: &Sauvegarde) -> String {
    if r.destination.trim().is_empty() {
        return t!("Sauvegardes non réglées. Exemple : lkatta regle sauvegarde.destination /run/media/moi/Disque (dossiers : sauvegarde.dossiers, par défaut ~/Documents).").into();
    }
    let dest = paths::expand(&r.destination);
    let n = copies(&dest.join("lkatta-sauvegardes")).len();
    let quand = derniere(&charger()).map(|d| crate::fr::date_longue(&d)).unwrap_or_else(|| t!("jamais").into());
    let dispo = if dest.is_dir() { t!("présente") } else { t!("absente pour l'instant") };
    t!(
        "Sauvegarde de {} vers {} ({}), tous les {} jours, {} copies gardées.\nDernière : {} ; {} copie(s) sur la destination.",
        r.dossiers.join(", "),
        dest.display(),
        dispo,
        r.jours,
        r.garder,
        quand,
        n
    )
}

/// En arrière-plan : toutes les heures, lance la sauvegarde quand elle est due et que la
/// destination est là ; prévient (une fois par jour au plus) si elle manque depuis trop longtemps.
pub fn surveiller(envoyer: impl Fn(Message) + Send + 'static) {
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(15 * 60));
        loop {
            let cfg = Config::load();
            if let Some(m) = passage(&cfg.sauvegarde, Local::now()) {
                envoyer(m);
            }
            std::thread::sleep(std::time::Duration::from_secs(3600));
        }
    });
}

fn passage(r: &Sauvegarde, now: DateTime<Local>) -> Option<Message> {
    if r.destination.trim().is_empty() || r.jours == 0 {
        return None;
    }
    let mut e = charger();
    let jours = derniere(&e).map(|d| (now - d).num_days()).unwrap_or(i64::MAX);
    if jours < r.jours as i64 {
        return None;
    }
    if paths::expand(&r.destination).is_dir() {
        return Some(match lancer_et_noter(r) {
            Ok(m) => Message::new(Kind::Info, t!("Sauvegarde"), m),
            Err(err) => Message::new(Kind::Alert, t!("Sauvegarde impossible"), err),
        });
    }
    let aujourdhui = now.format("%Y-%m-%d").to_string();
    if jours >= r.jours as i64 + 3 && e.averti != aujourdhui {
        e.averti = aujourdhui;
        enregistrer(&e);
        let depuis = if jours == i64::MAX { t!("Aucune sauvegarde encore faite.").to_string() } else { t!("Dernière sauvegarde il y a {} jours.", jours) };
        return Some(Message::new(Kind::Alert, t!("Sauvegarde en attente"), format!("{depuis} {}", t!("Branche le disque (ou monte le partage) : {}", r.destination))));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dossier(n: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("lkatta-sauv-{}-{n}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos() % 1_000_000));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn names_of_copies() {
        assert!(est_copie("2026-11-16_0930"));
        assert!(!est_copie("2026-11-16_0930.partiel"));
        assert!(!est_copie("Documents"));
        assert!(!est_copie("2026-1x-16_0930"));
    }

    #[test]
    fn backs_up_shares_unchanged_files_and_prunes() {
        let src = dossier("src");
        let dest = dossier("dest");
        std::fs::write(src.join("a.txt"), "un").unwrap();
        std::fs::create_dir_all(src.join("projet/node_modules")).unwrap();
        std::fs::write(src.join("projet/node_modules/gros.js"), "x").unwrap();
        let r = Sauvegarde { destination: dest.to_string_lossy().into(), dossiers: vec![src.to_string_lossy().into()], jours: 7, garder: 2 };
        let racine = dest.join("lkatta-sauvegardes");
        // Trois copies « anciennes » faites à la main, puis une vraie : il n'en reste que deux.
        for n in ["2026-01-01_0000", "2026-01-02_0000", "2026-01-03_0000"] {
            std::fs::create_dir_all(racine.join(n)).unwrap();
        }
        std::fs::write(dest.join("autre.txt"), "pas à nous").unwrap();
        let m = lancer(&r).unwrap();
        assert!(m.contains("1"), "{m}");
        let l = copies(&racine);
        assert_eq!(l.len(), 2, "{l:?}");
        let nom = src.file_name().unwrap().to_string_lossy().into_owned();
        let derniere = l.last().unwrap().join(&nom);
        assert_eq!(std::fs::read_to_string(derniere.join("a.txt")).unwrap(), "un");
        assert!(!derniere.join("projet/node_modules").exists(), "dossiers recréables exclus");
        assert!(dest.join("autre.txt").exists(), "rien d'autre n'est touché");
        // Une destination dans la source est refusée.
        let r2 = Sauvegarde { destination: src.join("sous").to_string_lossy().into(), ..r.clone() };
        std::fs::create_dir_all(src.join("sous")).unwrap();
        assert!(lancer(&r2).is_err());
        let _ = std::fs::remove_dir_all(&src);
        let _ = std::fs::remove_dir_all(&dest);
    }

    #[test]
    fn missing_destination_is_reported() {
        let r = Sauvegarde { destination: "/nulle/part/lkatta-essai".into(), dossiers: vec!["~/Documents".into()], jours: 7, garder: 5 };
        assert!(lancer(&r).unwrap_err().contains("absente") || crate::lang::en());
        assert!(passage(&Sauvegarde { destination: String::new(), ..r }, Local::now()).is_none());
    }
}
