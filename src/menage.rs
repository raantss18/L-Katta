//! Ménage guidé du dossier personnel (règles reprises de home-cleaner) : L Katta recense ce qui
//! prend de la place sans servir, le montre, et ne touche à rien sans accord.
//!
//! - Ce qui se recrée tout seul (caches connus, dossiers de compilation des projets inactifs) est
//!   supprimé définitivement : le mettre à la corbeille ne libérerait rien.
//! - Ce qui ne se recrée pas (gros fichiers anciens et téléchargements inachevés du dossier
//!   Téléchargements) part à la corbeille : on peut le récupérer.
//! - Vider la corbeille est irréversible : dit comme tel dans la question.
//! - Jamais dans un dossier synchronisé par Syncthing (`.stfolder`), ni dans un dossier interdit
//!   par les règles, ni à travers un lien symbolique. Chaque action est notée dans `menage.log`.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::brain::Host;
use crate::paths;

/// Caches que leurs programmes recréent d'eux-mêmes (sous ~/.cache).
const CACHES_SURS: &[&str] = &[
    "thumbnails", "pip", "yarn", "pnpm", "go-build", "mesa_shader_cache", "mesa_shader_cache_db", "typescript", "node-gyp",
    "electron", "electron-builder", "paru", "yay", "pikaur", "deno", "bun", "uv", "pypoetry", "composer", "gradle",
];

/// Dossiers de compilation ou de cache d'un projet, avec le fichier qui prouve que c'en est un
/// (vide : le nom suffit).
const DOSSIERS_PROJET: &[(&str, &[&str])] = &[
    ("target", &["Cargo.toml"]),
    ("node_modules", &["package.json"]),
    ("__pycache__", &[]),
    (".pytest_cache", &[]),
    (".mypy_cache", &[]),
    (".ruff_cache", &[]),
    (".next", &["package.json"]),
    (".nuxt", &["package.json"]),
    (".parcel-cache", &["package.json"]),
    (".turbo", &["package.json"]),
    (".gradle", &["build.gradle", "build.gradle.kts", "settings.gradle", "settings.gradle.kts"]),
    (".dart_tool", &["pubspec.yaml"]),
];

/// Arborescences de système (image à graver, chroot…) : on n'y descend pas, rien n'y est à nous.
const SYSTEMES: &[&str] = &["rootfs", "chroot", "squashfs-root", "sysroot", "usr", "Trash"];

/// Fichiers de téléchargement inachevés ou temporaires.
const TEMPORAIRES: &[&str] = &[".part", ".crdownload", ".tmp", ".temp", ".download", ".partial"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sort {
    /// Supprimé définitivement : se recrée tout seul.
    Supprimer,
    /// Mis à la corbeille (récupérable).
    Corbeille,
    /// La corbeille est vidée (irréversible).
    ViderCorbeille,
}

#[derive(Debug, Clone)]
pub struct Element {
    pub chemin: PathBuf,
    pub taille: u64,
    pub detail: String,
}

#[derive(Debug, Clone)]
pub struct Poste {
    pub id: &'static str,
    pub titre: String,
    pub sort: Sort,
    pub elements: Vec<Element>,
}

impl Poste {
    pub fn total(&self) -> u64 {
        self.elements.iter().map(|e| e.taille).sum()
    }
}

/// Réglages de l'analyse (des jours ; 0 dans les essais).
pub struct Seuils {
    pub projet_inactif_j: u64,
    pub gros_fichier: u64,
    pub fichier_ancien_j: u64,
    pub temporaire_j: u64,
    pub cache_min: u64,
    /// Dossier de compilation plus petit : on ne le propose pas (une liste lisible).
    pub projet_min: u64,
}

impl Default for Seuils {
    fn default() -> Self {
        Seuils { projet_inactif_j: 30, gros_fichier: 50 * 1024 * 1024, fichier_ancien_j: 90, temporaire_j: 2, cache_min: 20 * 1024 * 1024, projet_min: 1024 * 1024 }
    }
}

pub fn lisible(o: u64) -> String {
    match o {
        0..1024 => t!("{} o", o),
        1024..1_048_576 => t!("{} Ko", o / 1024),
        1_048_576..1_073_741_824 => t!("{} Mo", (o as f64 / 1_048_576.0).round()),
        _ => t!("{} Go", format!("{:.1}", o as f64 / 1_073_741_824.0)),
    }
}

/// Taille d'un fichier ou d'un dossier (sans suivre les liens), en 20 s au plus.
fn taille(p: &Path) -> u64 {
    let fin = std::time::Instant::now() + Duration::from_secs(20);
    let mut total = 0;
    let mut pile = vec![p.to_path_buf()];
    while let Some(d) = pile.pop() {
        let Ok(m) = std::fs::symlink_metadata(&d) else { continue };
        if m.is_dir() {
            if std::time::Instant::now() > fin {
                break;
            }
            if let Ok(rd) = std::fs::read_dir(&d) {
                pile.extend(rd.flatten().map(|e| e.path()));
            }
        } else if m.is_file() {
            total += m.len();
        }
    }
    total
}

fn age_j(p: &Path) -> u64 {
    std::fs::symlink_metadata(p)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| SystemTime::now().duration_since(t).ok())
        .map(|d| d.as_secs() / 86_400)
        .unwrap_or(0)
}

/// Âge (jours) de la dernière modification d'un projet : ses fichiers du premier niveau, sauf
/// les dossiers de compilation.
fn projet_inactif_j(projet: &Path) -> u64 {
    let Ok(rd) = std::fs::read_dir(projet) else { return 0 };
    rd.flatten()
        .filter(|e| !DOSSIERS_PROJET.iter().any(|(n, _)| e.file_name() == *n))
        .map(|e| age_j(&e.path()))
        .min()
        .unwrap_or(0)
}

/// Dossier synchronisé par Syncthing (un `.stfolder` dans lui ou un parent, jusqu'au dossier
/// personnel) : on n'y touche pas, la suppression se propagerait aux autres appareils.
fn synchronise(p: &Path, home: &Path) -> bool {
    p.ancestors().take_while(|a| a.starts_with(home)).any(|a| a.join(".stfolder").exists())
}

/// Recense ce qui peut partir.
pub fn analyser(home: &Path, telechargements: &Path, corbeille: &Path, s: &Seuils, regles: &crate::regles::Regles) -> Vec<Poste> {
    let permis = |p: &Path| !regles.path_forbidden(p) && !synchronise(p, home);
    let mut postes = vec![];

    // Corbeille.
    let fichiers = corbeille.join("files");
    let n = std::fs::read_dir(&fichiers).map(|r| r.count()).unwrap_or(0);
    if n > 0 {
        let tl = taille(&fichiers);
        postes.push(Poste {
            id: "corbeille",
            titre: t!("Corbeille ({} éléments)", n),
            sort: Sort::ViderCorbeille,
            elements: vec![Element { chemin: corbeille.to_path_buf(), taille: tl, detail: String::new() }],
        });
    }

    // Caches qui se recréent.
    let mut caches: Vec<Element> = CACHES_SURS
        .iter()
        .map(|n| home.join(".cache").join(n))
        .filter(|p| p.is_dir() && !p.is_symlink() && permis(p))
        .map(|p| {
            let tl = taille(&p);
            Element { detail: p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(), chemin: p, taille: tl }
        })
        .filter(|e| e.taille >= s.cache_min)
        .collect();
    caches.sort_by(|a, b| b.taille.cmp(&a.taille));
    if !caches.is_empty() {
        postes.push(Poste { id: "caches", titre: t!("Caches qui se recréent tout seuls").to_string(), sort: Sort::Supprimer, elements: caches });
    }

    // Dossiers de compilation des projets inactifs.
    let mut projets = vec![];
    let mut pile = vec![(home.to_path_buf(), 0usize)];
    while let Some((d, prof)) = pile.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            let Ok(ft) = e.file_type() else { continue };
            if !ft.is_dir() || ft.is_symlink() {
                continue;
            }
            let nom = e.file_name().to_string_lossy().into_owned();
            if let Some((_, preuves)) = DOSSIERS_PROJET.iter().find(|(n, _)| *n == nom) {
                let ok = preuves.is_empty() || preuves.iter().any(|f| d.join(f).is_file());
                let inactif = projet_inactif_j(&d);
                if ok && d != home && inactif >= s.projet_inactif_j && permis(&p) {
                    let tl = taille(&p);
                    if tl >= s.projet_min.max(1) {
                        projets.push(Element { chemin: p, taille: tl, detail: t!("projet inactif depuis {} jours", inactif) });
                    }
                }
                continue;
            }
            // Dossiers cachés, Corbeille, données des applications : on ne les parcourt pas.
            if nom.starts_with('.') || nom == "snap" || SYSTEMES.contains(&nom.as_str()) || prof >= 6 || p == corbeille {
                continue;
            }
            pile.push((p, prof + 1));
        }
        if projets.len() >= 300 {
            break;
        }
    }
    projets.sort_by(|a, b| b.taille.cmp(&a.taille));
    if !projets.is_empty() {
        postes.push(Poste { id: "projets", titre: t!("Compilations de projets inactifs (se refont à la prochaine compilation)").to_string(), sort: Sort::Supprimer, elements: projets });
    }

    // Téléchargements : gros fichiers anciens, téléchargements inachevés.
    let mut gros = vec![];
    let mut temp = vec![];
    if let Ok(rd) = std::fs::read_dir(telechargements) {
        for e in rd.flatten() {
            let p = e.path();
            let Ok(m) = std::fs::symlink_metadata(&p) else { continue };
            if !m.is_file() || !permis(&p) {
                continue;
            }
            let nom = e.file_name().to_string_lossy().to_lowercase();
            let age = age_j(&p);
            if TEMPORAIRES.iter().any(|x| nom.ends_with(x)) && age >= s.temporaire_j {
                temp.push(Element { chemin: p, taille: m.len(), detail: t!("inachevé, {} jours", age) });
            } else if m.len() >= s.gros_fichier && age >= s.fichier_ancien_j {
                gros.push(Element { chemin: p, taille: m.len(), detail: t!("{} jours sans modification", age) });
            }
        }
    }
    gros.sort_by(|a, b| b.taille.cmp(&a.taille));
    if !gros.is_empty() {
        postes.push(Poste { id: "telechargements", titre: t!("Gros fichiers anciens des Téléchargements").to_string(), sort: Sort::Corbeille, elements: gros });
    }
    if !temp.is_empty() {
        postes.push(Poste { id: "temporaires", titre: t!("Téléchargements inachevés").to_string(), sort: Sort::Corbeille, elements: temp });
    }
    postes
}

fn corbeille_racine() -> PathBuf {
    // Dossier de données XDG (celui de l'hôte dans le Flatpak) : ~/.local/share/Trash.
    paths::data_dir().parent().map(|d| d.join("Trash")).unwrap_or_else(|| paths::home_dir().join(".local/share/Trash"))
}

fn dossier_telechargements() -> PathBuf {
    crate::mail::dossier_telechargements()
}

/// Analyse du dossier personnel de l'utilisateur, en texte.
pub fn rapport() -> String {
    let postes = analyser(&paths::home_dir(), &dossier_telechargements(), &corbeille_racine(), &Seuils::default(), &crate::regles::load());
    texte(&postes)
}

pub fn texte(postes: &[Poste]) -> String {
    if postes.is_empty() {
        return t!("Rien à nettoyer : pas de cache encombrant, pas de compilation oubliée, Téléchargements en ordre.").into();
    }
    let total: u64 = postes.iter().map(Poste::total).sum();
    let mut out = vec![t!("{} récupérables :", lisible(total))];
    for p in postes {
        let sort = match p.sort {
            Sort::Supprimer => t!("supprimé, se recrée tout seul"),
            Sort::Corbeille => t!("à la corbeille, récupérable"),
            Sort::ViderCorbeille => t!("vidée définitivement"),
        };
        out.push(format!("- [{}] {} : {} ({})", p.id, p.titre, lisible(p.total()), sort));
        for e in p.elements.iter().take(5) {
            let d = if e.detail.is_empty() { String::new() } else { format!(", {}", e.detail) };
            out.push(format!("    {} ({}{})", montrer_chemin(&e.chemin), lisible(e.taille), d));
        }
        if p.elements.len() > 5 {
            out.push(t!("    … et {} autres", p.elements.len() - 5));
        }
    }
    out.join("\n")
}

fn montrer_chemin(p: &Path) -> String {
    let home = paths::home_dir();
    match p.strip_prefix(&home) {
        Ok(r) => format!("~/{}", r.display()),
        Err(_) => p.display().to_string(),
    }
}

/// Nettoie les postes choisis (identifiants séparés par des virgules ; vide : tous), après la
/// question à l'utilisateur, qui voit tout ce qui partira.
pub fn nettoyer(ids: &str, host: &mut dyn Host) -> Result<String, String> {
    let home = paths::home_dir();
    let corbeille = corbeille_racine();
    let regles = crate::regles::load();
    let voulus: Vec<&str> = ids.split([',', ' ']).map(str::trim).filter(|s| !s.is_empty()).collect();
    let postes: Vec<Poste> = analyser(&home, &dossier_telechargements(), &corbeille, &Seuils::default(), &regles)
        .into_iter()
        .filter(|p| voulus.is_empty() || voulus.contains(&p.id))
        .collect();
    if postes.is_empty() {
        return Ok(t!("Rien à nettoyer dans ce qui est demandé.").into());
    }
    let irreversible = postes.iter().any(|p| p.sort == Sort::ViderCorbeille);
    let q = format!(
        "{}{}\n\n{}",
        t!("Faire ce ménage ?"),
        if irreversible { t!(" ⚠ vider la corbeille est irréversible").to_string() } else { String::new() },
        texte(&postes)
    );
    if !host.confirm(&q) {
        return Ok("Refusé par l'utilisateur.".into());
    }
    let (mut libere, mut echecs) = (0u64, vec![]);
    for p in &postes {
        for e in &p.elements {
            let r = appliquer(p.sort, &e.chemin, &home, &corbeille);
            journal(p.sort, &e.chemin, e.taille, r.as_ref().err());
            match r {
                Ok(()) => libere += e.taille,
                Err(err) => echecs.push(format!("{} : {err}", montrer_chemin(&e.chemin))),
            }
        }
    }
    let mut msg = t!("Ménage fait : {} libérés.", lisible(libere));
    if !echecs.is_empty() {
        msg.push_str(&format!("\n{}\n{}", t!("Pas pu :"), echecs.join("\n")));
    }
    Ok(msg)
}

/// Une action, après vérification : dans le dossier personnel, pas un lien, toujours là.
fn appliquer(sort: Sort, p: &Path, home: &Path, corbeille: &Path) -> Result<(), String> {
    let m = std::fs::symlink_metadata(p).map_err(|e| e.to_string())?;
    if m.file_type().is_symlink() || !p.starts_with(home) || p == home {
        return Err(t!("refusé (lien ou hors du dossier personnel)").into());
    }
    match sort {
        Sort::Supprimer => {
            if m.is_dir() {
                std::fs::remove_dir_all(p).map_err(|e| e.to_string())
            } else {
                std::fs::remove_file(p).map_err(|e| e.to_string())
            }
        }
        Sort::Corbeille => a_la_corbeille(p, corbeille).map(|_| ()),
        Sort::ViderCorbeille => {
            if p != corbeille {
                return Err(t!("ce n'est pas la corbeille").into());
            }
            for sous in ["files", "info", "expunged"] {
                if let Ok(rd) = std::fs::read_dir(p.join(sous)) {
                    for e in rd.flatten() {
                        let q = e.path();
                        let r = if q.is_dir() && !q.is_symlink() { std::fs::remove_dir_all(&q) } else { std::fs::remove_file(&q) };
                        r.map_err(|e| e.to_string())?;
                    }
                }
            }
            Ok(())
        }
    }
}

/// Met un fichier à la corbeille (spécification freedesktop : `files/` et `info/*.trashinfo`),
/// d'où le gestionnaire de fichiers sait le restaurer.
pub fn a_la_corbeille(p: &Path, corbeille: &Path) -> Result<PathBuf, String> {
    let (files, info) = (corbeille.join("files"), corbeille.join("info"));
    std::fs::create_dir_all(&files).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&info).map_err(|e| e.to_string())?;
    let nom = p.file_name().ok_or("nom de fichier")?.to_string_lossy().into_owned();
    let (base, ext) = match nom.rsplit_once('.') {
        Some((b, e)) if !b.is_empty() => (b.to_string(), format!(".{e}")),
        _ => (nom.clone(), String::new()),
    };
    let libre = (1..10_000)
        .map(|i| if i == 1 { nom.clone() } else { format!("{base}.{i}{ext}") })
        .find(|n| !files.join(n).exists() && !info.join(format!("{n}.trashinfo")).exists())
        .ok_or("corbeille pleine")?;
    let abs = std::fs::canonicalize(p).map_err(|e| e.to_string())?;
    let fiche = format!(
        "[Trash Info]\nPath={}\nDeletionDate={}\n",
        encoder_chemin(&abs.to_string_lossy()),
        chrono::Local::now().format("%Y-%m-%dT%H:%M:%S")
    );
    let fiche_p = info.join(format!("{libre}.trashinfo"));
    std::fs::write(&fiche_p, fiche).map_err(|e| e.to_string())?;
    let dest = files.join(&libre);
    if let Err(e) = std::fs::rename(p, &dest) {
        let _ = std::fs::remove_file(&fiche_p);
        return Err(t!("pas de corbeille possible ici ({})", e).into());
    }
    Ok(dest)
}

fn encoder_chemin(s: &str) -> String {
    s.bytes()
        .map(|b| if b.is_ascii_alphanumeric() || b"/-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") })
        .collect()
}

fn journal(sort: Sort, p: &Path, taille: u64, err: Option<&String>) {
    let ligne = serde_json::json!({
        "quand": chrono::Local::now().to_rfc3339(),
        "action": match sort { Sort::Supprimer => "suppression", Sort::Corbeille => "corbeille", Sort::ViderCorbeille => "vider-corbeille" },
        "chemin": p.to_string_lossy(),
        "taille": taille,
        "erreur": err,
    });
    let f = paths::data_dir().join("menage.log");
    let ancien = std::fs::read_to_string(&f).unwrap_or_default();
    let _ = paths::write_private(&f, format!("{ancien}{ligne}\n").as_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arbre() -> PathBuf {
        let h = std::env::temp_dir().join(format!("lkatta-menage-{}-{}", std::process::id(), rand_suffix()));
        let w = |p: &str, n: usize| {
            let f = h.join(p);
            std::fs::create_dir_all(f.parent().unwrap()).unwrap();
            std::fs::write(f, vec![b'x'; n]).unwrap();
        };
        w("proj/Cargo.toml", 10);
        w("proj/target/debug/gros", 5000);
        w("autre/target/rien", 100); // pas de Cargo.toml : pas un projet Rust
        w("web/package.json", 10);
        w("web/node_modules/a/index.js", 3000);
        w("synchro/.stfolder/x", 1);
        w("synchro/app/Cargo.toml", 10);
        w("synchro/app/target/b", 4000);
        w(".cache/pip/http/x", 2000);
        w(".cache/navigateur/x", 9000); // inconnu : on n'y touche pas
        w("Telechargements/film.mkv", 8000);
        w("Telechargements/moitie.iso.part", 700);
        w("Telechargements/petit.txt", 5);
        w("Trash/files/vieux.txt", 300);
        w("iso/rootfs/usr/lib/python3/__pycache__/m.pyc", 900); // système à graver : intouchable
        h
    }

    fn rand_suffix() -> u64 {
        SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_nanos() as u64 % 1_000_000
    }

    fn seuils() -> Seuils {
        Seuils { projet_inactif_j: 0, gros_fichier: 1000, fichier_ancien_j: 0, temporaire_j: 0, cache_min: 1, projet_min: 1 }
    }

    #[test]
    fn finds_what_can_go_and_spares_the_rest() {
        let h = arbre();
        let postes = analyser(&h, &h.join("Telechargements"), &h.join("Trash"), &seuils(), &crate::regles::Regles::default());
        let chemins = |id: &str| -> Vec<PathBuf> { postes.iter().filter(|p| p.id == id).flat_map(|p| p.elements.iter().map(|e| e.chemin.clone())).collect() };
        assert!(chemins("projets").contains(&h.join("proj/target")));
        assert!(chemins("projets").contains(&h.join("web/node_modules")));
        assert!(!chemins("projets").contains(&h.join("autre/target")), "target sans Cargo.toml");
        assert!(!chemins("projets").iter().any(|p| p.starts_with(h.join("synchro"))), "dossier Syncthing");
        assert!(!chemins("projets").iter().any(|p| p.starts_with(h.join("iso"))), "arborescence système");
        assert_eq!(chemins("caches"), vec![h.join(".cache/pip")]);
        assert_eq!(chemins("telechargements"), vec![h.join("Telechargements/film.mkv")]);
        assert_eq!(chemins("temporaires"), vec![h.join("Telechargements/moitie.iso.part")]);
        assert_eq!(chemins("corbeille"), vec![h.join("Trash")]);
        assert!(texte(&postes).contains("[projets]"));
        let _ = std::fs::remove_dir_all(&h);
    }

    #[test]
    fn actions_trash_delete_and_empty() {
        let h = arbre();
        let corbeille = h.join("Trash");
        // Corbeille : déplacé avec sa fiche, récupérable.
        let dest = a_la_corbeille(&h.join("Telechargements/film.mkv"), &corbeille).unwrap();
        assert!(dest.exists() && !h.join("Telechargements/film.mkv").exists());
        let fiche = std::fs::read_to_string(corbeille.join("info/film.mkv.trashinfo")).unwrap();
        assert!(fiche.contains("Path=") && fiche.contains("Telechargements/film.mkv"));
        // Même nom une seconde fois : pas d'écrasement.
        std::fs::write(h.join("Telechargements/film.mkv"), "y").unwrap();
        let d2 = a_la_corbeille(&h.join("Telechargements/film.mkv"), &corbeille).unwrap();
        assert_ne!(d2, dest);
        // Suppression d'un dossier de compilation ; refus d'un lien ou d'un chemin hors du dossier.
        appliquer(Sort::Supprimer, &h.join("proj/target"), &h, &corbeille).unwrap();
        assert!(!h.join("proj/target").exists() && h.join("proj/Cargo.toml").exists());
        std::os::unix::fs::symlink(h.join("web"), h.join("lien")).unwrap();
        assert!(appliquer(Sort::Supprimer, &h.join("lien"), &h, &corbeille).is_err());
        assert!(h.join("web/package.json").exists());
        assert!(appliquer(Sort::Supprimer, &std::env::temp_dir(), &h, &corbeille).is_err());
        // Vider la corbeille : seulement la corbeille.
        assert!(appliquer(Sort::ViderCorbeille, &h.join("web"), &h, &corbeille).is_err());
        appliquer(Sort::ViderCorbeille, &corbeille, &h, &corbeille).unwrap();
        assert_eq!(std::fs::read_dir(corbeille.join("files")).unwrap().count(), 0);
        let _ = std::fs::remove_dir_all(&h);
    }
}
