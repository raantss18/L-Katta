//! Index local des fichiers (SQLite FTS5) : noms et contenu des fichiers texte du dossier
//! personnel, pour retrouver « le document où je parlais de… » en un instant.
//!
//! Sobre par construction (cible : machine 8 Go) : un fil de priorité minimale (processeur et
//! disque), une base sur disque, aucune liste gardée en mémoire ; seul ce qui a changé depuis
//! le dernier passage est relu. Les emplacements interdits par les règles strictes, les dossiers
//! cachés et les dossiers de fabrication ne sont jamais lus.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, OptionalExtension, params};

use crate::paths;

/// Contenu gardé au plus par document dans la table des fichiers, et par fichier de code (le
/// début suffit à le retrouver). Les documents entiers vont, découpés, dans les passages.
const MAX_CONTENU: usize = 12 * 1024;
/// Texte d'un document découpé en passages, au plus (au-delà, la fin n'est pas lue).
const MAX_DOCUMENT: usize = 600 * 1024;
/// Pages d'un PDF lues au plus.
const PAGES_PDF: u32 = 300;
/// Version du schéma : à chaque changement, tous les fichiers sont relus une fois.
const SCHEMA: &str = "2";
const MAX_CODE: usize = 4 * 1024;
/// Fichier texte au-delà duquel on n'indexe que le nom.
const MAX_TEXTE: u64 = 2 * 1024 * 1024;
const MAX_PDF: u64 = 30 * 1024 * 1024;
/// Profondeur (sous le dossier personnel) jusqu'où entrent les fichiers qui ne sont pas des documents.
const PROFONDEUR_NOMS: usize = 4;
/// Garde-fou : au-delà, on s'arrête (dossier personnel démesuré).
const MAX_FICHIERS: usize = 200_000;
/// Passages d'un document que l'utilisateur ne touche plus (ni ouvert récemment, ni modifié
/// depuis un an) : le début seulement (sommaire, introduction).
const PASSAGES_ANCIEN: usize = 120;
/// Ancienneté au-delà de laquelle un document non rouvert est « ancien ».
const UN_AN: i64 = 365 * 24 * 3600;
/// Texte au-delà duquel un document est un livre (environ 60 pages).
const LIVRE: usize = 150 * 1024;

/// Taille de base au-delà de laquelle seuls les noms entrent : selon la machine (25 Mo par Go
/// de mémoire vive, entre 100 et 600 Mo) et jamais plus de 2 % de la place libre du disque.
fn budget_base() -> u64 {
    const MO: u64 = 1024 * 1024;
    let ram_go = std::fs::read_to_string("/proc/meminfo")
        .ok()
        .and_then(|s| s.lines().find(|l| l.starts_with("MemTotal:")).and_then(|l| l.split_whitespace().nth(1)?.parse::<u64>().ok()))
        .map(|ko| ko / (1024 * 1024))
        .unwrap_or(8);
    let selon_ram = (ram_go * 25 * MO).clamp(100 * MO, 600 * MO);
    let dossier = chemin_base().parent().map(Path::to_path_buf).unwrap_or_else(paths::data_dir);
    let libre = libre_sur(&dossier).unwrap_or(u64::MAX);
    selon_ram.min(libre / 50).max(20 * MO)
}

/// Place libre (octets) sur le disque de `d`.
fn libre_sur(d: &Path) -> Option<u64> {
    use std::os::unix::ffi::OsStrExt;
    let c = std::ffi::CString::new(d.as_os_str().as_bytes()).ok()?;
    let mut st: libc::statvfs = unsafe { std::mem::zeroed() };
    (unsafe { libc::statvfs(c.as_ptr(), &mut st) } == 0).then(|| st.f_bavail as u64 * st.f_frsize as u64)
}

/// Fichiers ouverts récemment (liste commune des bureaux : ~/.local/share/recently-used.xbel,
/// tenue par KDE, GNOME, Xfce…) : ce dont l'utilisateur se sert vraiment.
fn recents() -> std::collections::HashSet<String> {
    let f = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).unwrap_or_else(|| paths::home_dir().join(".local/share")).join("recently-used.xbel");
    let Ok(s) = std::fs::read_to_string(f) else { return Default::default() };
    s.split("href=\"file://").skip(1).filter_map(|r| r.split('"').next()).map(decoder_uri).collect()
}

/// `%C3%A9` → `é` (adresses file:// de la liste des récents).
fn decoder_uri(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).replace("&amp;", "&").replace("&apos;", "'")
}

/// Documents : nom et contenu, où qu'ils soient (projets compris).
const EXT_DOC: &[&str] = &["txt", "md", "markdown", "rst", "org", "tex", "bib", "html", "htm", "srt", "ics", "vcf", "pdf", "adoc", "rtf"];

/// Code et scripts : nom et début du contenu, hors projets seulement (dans un projet, la
/// recherche du côté Code s'en charge, et les sources tierces gonfleraient l'index).
const EXT_CODE: &[&str] = &[
    "rs", "py", "js", "ts", "tsx", "jsx", "c", "h", "cpp", "hpp", "java", "kt", "go", "rb", "php", "sh", "bash", "zsh", "fish", "lua", "dart",
    "sql", "r", "jl", "m", "css", "scss", "vue", "svelte", "swift", "nix", "el", "hs", "ml", "scala", "toml", "yaml", "yml", "ini", "conf", "cfg",
];

/// Un dossier qui contient l'un de ces fichiers est un projet (dépôt, sources, SDK…).
const MARQUES_PROJET: &[&str] = &[
    ".git", "Cargo.toml", "package.json", "CMakeLists.txt", "Makefile", "setup.py", "pyproject.toml", "go.mod", "pom.xml", "build.gradle",
    "build.gradle.kts", "meson.build", "ProjectSettings", "project.godot", "configure", "composer.json", "Gemfile", "pubspec.yaml", "Package.swift", "mix.exs", ".hg", ".svn",
];

#[derive(Debug, Clone, Copy, PartialEq)]
enum Genre {
    /// Nom et contenu.
    Document,
    /// Nom et début du contenu.
    Code,
    /// Nom seulement.
    Nom,
}

fn extension(p: &Path) -> String {
    p.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase()
}

/// Ce qu'on garde d'un fichier (`None` : rien, il n'entre pas dans l'index).
fn genre(p: &Path, dans_projet: bool, profondeur: usize) -> Option<Genre> {
    let e = extension(p);
    // Pages web : en masse dans les documentations techniques ; gardées près de la surface.
    let page = matches!(e.as_str(), "html" | "htm");
    if EXT_DOC.contains(&e.as_str()) && !(page && (dans_projet || profondeur > PROFONDEUR_NOMS)) {
        Some(Genre::Document)
    } else if dans_projet || profondeur > PROFONDEUR_NOMS {
        // Les fichiers d'une personne sont rangés près de la surface ; les arbres profonds
        // (sources, images de système, SDK) sont des logiciels : seuls leurs documents entrent.
        None
    } else if EXT_CODE.contains(&e.as_str()) {
        Some(Genre::Code)
    } else {
        Some(Genre::Nom)
    }
}

/// Extensions qui font d'un dossier un projet (solution Visual Studio, projet Unreal, Qt…).
const EXT_PROJET: &[&str] = &["uproject", "sln", "csproj", "vcxproj", "pro", "xcodeproj", "cabal", "gemspec"];

/// Un dossier est un projet s'il porte une marque (dépôt, fichier de construction), ou s'il
/// contient beaucoup de code : sources d'un moteur, d'un SDK, dépendances recopiées…
fn est_projet(entrees: &[(String, bool)]) -> bool {
    let mut code = 0;
    for (nom, _) in entrees {
        if MARQUES_PROJET.contains(&nom.as_str()) {
            return true;
        }
        let e = nom.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
        if EXT_PROJET.contains(&e.as_str()) {
            return true;
        }
        if EXT_CODE.contains(&e.as_str()) {
            code += 1;
            if code >= 12 {
                return true;
            }
        }
    }
    false
}

/// Parcours en profondeur sans suivre les liens : (chemin, métadonnées, genre). Les dossiers
/// cachés et de fabrication sont sautés ; sous un projet, seuls les documents sortent.
fn parcourir(racine: &Path, mut f: impl FnMut(&Path, &std::fs::Metadata, Genre) -> bool) {
    let mut pile = vec![(racine.to_path_buf(), false, 0usize)];
    while let Some((d, parent_projet, prof)) = pile.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        // (nom, est un dossier) ; les liens symboliques ne sont pas suivis.
        let entrees: Vec<(String, bool)> = rd
            .flatten()
            .filter_map(|e| {
                let ft = e.file_type().ok()?;
                (ft.is_dir() || ft.is_file()).then(|| (e.file_name().to_string_lossy().into_owned(), ft.is_dir()))
            })
            .collect();
        let projet = parent_projet || est_projet(&entrees);
        for (nom, dossier) in entrees {
            if nom.starts_with('.') {
                continue;
            }
            let p = d.join(&nom);
            if dossier {
                if !sauter(&nom) {
                    pile.push((p, projet, prof + 1));
                }
            } else if let Some(g) = genre(&p, projet, prof) {
                let Ok(md) = std::fs::symlink_metadata(&p) else { continue };
                if !f(&p, &md, g) {
                    return;
                }
            }
        }
    }
}

/// Dossiers jamais parcourus (fabrication, dépendances, caches, corbeilles).
const SAUTER: &[&str] = &[
    "node_modules", "target", "build", "dist", "out", "__pycache__", "venv", "env", "site-packages", "vendor", "Pods", "DerivedData",
    "snap", "Steam", "steamapps", "lost+found", "Trash", "Corbeille", "go", "android-sdk", "Android", "flutter", "miniconda3", "anaconda3",
    "Intermediate", "Binaries", "DerivedDataCache", "PackageCache", "_build", "bower_components",
];

/// Ouvre la base (pour src/rag.rs).
pub(crate) fn connexion() -> rusqlite::Result<Connection> {
    ouvrir()
}

pub fn chemin_base() -> PathBuf {
    #[cfg(test)]
    if let Some(b) = BASE_ESSAI.with(|b| b.borrow().clone()) {
        return b;
    }
    paths::data_dir().join("index.db")
}

#[cfg(test)]
thread_local! {
    static BASE_ESSAI: std::cell::RefCell<Option<PathBuf>> = const { std::cell::RefCell::new(None) };
}

/// `/home/x/Documents/a.md` → `~/Documents/a.md`.
fn tilde(p: &str) -> String {
    let h = paths::home_dir().to_string_lossy().into_owned();
    match p.strip_prefix(&h) {
        Some(r) if !h.is_empty() => format!("~{r}"),
        _ => p.to_string(),
    }
}

fn ouvrir() -> rusqlite::Result<Connection> {
    if let Some(d) = chemin_base().parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let c = Connection::open(chemin_base())?;
    c.busy_timeout(Duration::from_secs(5))?;
    c.execute_batch(
        "PRAGMA journal_mode=WAL;
         PRAGMA synchronous=NORMAL;
         PRAGMA cache_size=-4000;
         CREATE TABLE IF NOT EXISTS fichiers(id INTEGER PRIMARY KEY, chemin TEXT UNIQUE NOT NULL, mtime INTEGER, taille INTEGER, vu INTEGER);
         CREATE VIRTUAL TABLE IF NOT EXISTS docs USING fts5(nom, contenu, tokenize='unicode61 remove_diacritics 2');
         CREATE TABLE IF NOT EXISTS etat(cle TEXT PRIMARY KEY, valeur TEXT);
         CREATE TABLE IF NOT EXISTS passage_info(id INTEGER PRIMARY KEY, fichier INTEGER NOT NULL, page INTEGER, rang INTEGER);
         CREATE INDEX IF NOT EXISTS passage_fichier ON passage_info(fichier);
         CREATE VIRTUAL TABLE IF NOT EXISTS passages USING fts5(texte, tokenize='unicode61 remove_diacritics 2', detail=none);
         CREATE TABLE IF NOT EXISTS vecteurs(passage INTEGER PRIMARY KEY, modele TEXT, v BLOB);",
    )?;
    // L'index décrit les fichiers de l'utilisateur : lisible par lui seul.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(chemin_base(), std::fs::Permissions::from_mode(0o600));
    }
    Ok(c)
}

fn maintenant() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// Ce fil passe en priorité minimale : processeur (nice 19) et disque (classe « idle »).
fn se_faire_discret() {
    unsafe {
        let tid = libc::syscall(libc::SYS_gettid) as libc::id_t;
        libc::setpriority(libc::PRIO_PROCESS, tid, 19);
        // ioprio_set(IOPRIO_WHO_PROCESS, tid, IOPRIO_CLASS_IDLE << 13)
        libc::syscall(libc::SYS_ioprio_set, 1, tid as libc::c_long, 3i64 << 13);
    }
}

fn sauter(nom: &str) -> bool {
    nom.starts_with('.') || SAUTER.contains(&nom)
}

/// Texte d'un fichier pour l'index (vide : seul le nom compte). Un document est lu en entier
/// (jusqu'à `MAX_DOCUMENT`) ; les pages d'un PDF sont séparées par un saut de page (\x0c).
fn contenu(p: &Path, taille: u64, g: Genre) -> String {
    let ext = extension(p);
    let max = match g {
        Genre::Nom => return String::new(),
        Genre::Code => MAX_CODE,
        Genre::Document => MAX_DOCUMENT,
    };
    if ext == "pdf" {
        return if taille <= MAX_PDF { pdf(p) } else { String::new() };
    }
    if taille > MAX_TEXTE {
        return String::new();
    }
    let Ok(b) = read_debut(p, max) else { return String::new() };
    let b = &b[..];
    // Binaire déguisé : ignoré.
    if b.iter().take(4096).filter(|&&c| c == 0).count() > 0 {
        return String::new();
    }
    let s = String::from_utf8_lossy(b);
    if matches!(ext.as_str(), "html" | "htm" | "xml") { crate::web::strip_tags(&s) } else { s.into_owned() }
}

/// Les `n` premiers octets (sans lire le reste du fichier).
fn read_debut(p: &Path, n: usize) -> std::io::Result<Vec<u8>> {
    use std::io::Read;
    let mut v = Vec::with_capacity(n.min(64 * 1024));
    std::fs::File::open(p)?.take(n as u64).read_to_end(&mut v)?;
    Ok(v)
}

/// Texte des premières pages d'un PDF (pdftotext, s'il est installé), pages séparées par \x0c.
fn pdf(p: &Path) -> String {
    let o = crate::hote::commande("pdftotext")
        .args(["-l", &PAGES_PDF.to_string(), "-q", "-enc", "UTF-8"])
        .arg(p)
        .arg("-")
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output();
    match o {
        Ok(o) if o.status.success() => {
            let s = String::from_utf8_lossy(&o.stdout);
            s.chars().take(MAX_DOCUMENT).collect()
        }
        _ => String::new(),
    }
}

/// Compte rendu d'un passage.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Bilan {
    pub vus: usize,
    pub relus: usize,
    pub retires: usize,
}

/// Parcourt `racine` et met l'index à jour (seuls les fichiers modifiés sont relus).
pub fn mettre_a_jour(racine: &Path) -> Result<Bilan, String> {
    let c = ouvrir().map_err(|e| e.to_string())?;
    let regles = crate::regles::load();
    // Numéro de passage strictement croissant (deux passages dans la même seconde restent distincts).
    let precedent: i64 = c
        .query_row("SELECT valeur FROM etat WHERE cle='passage'", [], |r| r.get::<_, String>(0))
        .optional()
        .map_err(|e| e.to_string())?
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let generation = maintenant().max(precedent + 1);
    // Nouveau schéma (passages) : tout est relu une fois, en tâche de fond.
    let schema: Option<String> = c.query_row("SELECT valeur FROM etat WHERE cle='schema'", [], |r| r.get(0)).optional().map_err(|e| e.to_string())?;
    if schema.as_deref() != Some(SCHEMA) {
        c.execute_batch("UPDATE fichiers SET mtime=-1").map_err(|e| e.to_string())?;
        c.execute("INSERT OR REPLACE INTO etat(cle, valeur) VALUES('schema', ?1)", [SCHEMA]).map_err(|e| e.to_string())?;
    }
    let mut bilan = Bilan::default();
    c.execute_batch("BEGIN").map_err(|e| e.to_string())?;
    let mut lot = 0;
    let mut erreur: Option<String> = None;
    let budget = budget_base();
    let recents = recents();
    let mut trop_gros = std::fs::metadata(chemin_base()).is_ok_and(|m| m.len() > budget);
    parcourir(racine, |p, md, g| {
        if bilan.vus >= MAX_FICHIERS {
            return false;
        }
        if regles.path_forbidden(p) {
            return true;
        }
        let r = (|| -> rusqlite::Result<()> {
            let mtime = md.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_secs() as i64).unwrap_or(0);
            let taille = md.len();
            let chemin = p.to_string_lossy().into_owned();
            bilan.vus += 1;
            let connu: Option<(i64, i64)> = c.query_row("SELECT id, mtime FROM fichiers WHERE chemin=?1", [&chemin], |r| Ok((r.get(0)?, r.get(1)?))).optional()?;
            match connu {
                Some((id, m)) if m == mtime => {
                    c.execute("UPDATE fichiers SET vu=?1 WHERE id=?2", params![generation, id])?;
                }
                connu => {
                    let texte = if trop_gros { String::new() } else { contenu(p, taille, g) };
                    let fichier = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                    let nom = format!("{fichier} {}", chemin.replace(['/', '_', '-', '.'], " "));
                    let id = match connu {
                        Some((id, _)) => {
                            c.execute("UPDATE fichiers SET mtime=?1, taille=?2, vu=?3 WHERE id=?4", params![mtime, taille as i64, generation, id])?;
                            c.execute("DELETE FROM docs WHERE rowid=?1", [id])?;
                            retirer_passages(&c, "fichier=?1", params![id])?;
                            id
                        }
                        None => {
                            c.execute("INSERT INTO fichiers(chemin, mtime, taille, vu) VALUES(?1, ?2, ?3, ?4)", params![chemin, mtime, taille as i64, generation])?;
                            c.last_insert_rowid()
                        }
                    };
                    let debut: String = texte.chars().take(if g == Genre::Code { MAX_CODE } else { MAX_CONTENU }).collect();
                    c.execute("INSERT INTO docs(rowid, nom, contenu) VALUES(?1, ?2, ?3)", params![id, nom, debut.replace('\x0c', "\n")])?;
                    if g == Genre::Document {
                        // En entier : un document rouvert récemment, ou un document de taille
                        // ordinaire modifié dans l'année. Un livre jamais rouvert (copié là un
                        // jour) et les vieux documents : leur début seulement.
                        let vivant = recents.contains(&chemin) || (maintenant() - mtime < UN_AN && texte.len() < LIVRE);
                        let max = if vivant { crate::rag::MAX_PASSAGES } else { PASSAGES_ANCIEN };
                        for (rang, (page, morceau)) in crate::rag::decouper(&texte, max).into_iter().enumerate() {
                            c.execute("INSERT INTO passage_info(fichier, page, rang) VALUES(?1, ?2, ?3)", params![id, page, rang as i64])?;
                            c.execute("INSERT INTO passages(rowid, texte) VALUES(?1, ?2)", params![c.last_insert_rowid(), morceau])?;
                        }
                    }
                    bilan.relus += 1;
                    lot += 1;
                }
            }
            Ok(())
        })();
        if let Err(e) = r {
            erreur = Some(e.to_string());
            return false;
        }
        // Petits lots : la base reste lisible pendant le passage, la mémoire reste basse.
        if lot >= 300 {
            lot = 0;
            trop_gros = trop_gros || std::fs::metadata(chemin_base()).is_ok_and(|m| m.len() > budget);
            if let Err(e) = c.execute_batch("COMMIT; BEGIN") {
                erreur = Some(e.to_string());
                return false;
            }
        }
        true
    });
    if let Some(e) = erreur {
        let _ = c.execute_batch("ROLLBACK");
        return Err(e);
    }
    // Disparus depuis le dernier passage (sous cette racine seulement).
    let prefixe = format!("{}%", racine.to_string_lossy());
    retirer_passages(&c, "fichier IN (SELECT id FROM fichiers WHERE vu < ?1 AND chemin LIKE ?2)", params![generation, prefixe]).map_err(|e| e.to_string())?;
    c.execute("DELETE FROM docs WHERE rowid IN (SELECT id FROM fichiers WHERE vu < ?1 AND chemin LIKE ?2)", params![generation, prefixe])
        .map_err(|e| e.to_string())?;
    // (FTS5 ne compte pas ses suppressions : on compte celles de la table des fichiers.)
    bilan.retires = c.execute("DELETE FROM fichiers WHERE vu < ?1 AND chemin LIKE ?2", params![generation, prefixe]).map_err(|e| e.to_string())?;
    c.execute("INSERT OR REPLACE INTO etat(cle, valeur) VALUES('passage', ?1)", [generation.to_string()]).map_err(|e| e.to_string())?;
    c.execute_batch("COMMIT").map_err(|e| e.to_string())?;
    if bilan.retires > 200 || bilan.relus > 5000 {
        let _ = c.execute_batch("INSERT INTO docs(docs) VALUES('optimize'); INSERT INTO passages(passages) VALUES('optimize');");
    }
    Ok(bilan)
}

/// Retire les passages (texte, place, vecteurs) des fichiers désignés par `condition`
/// (une condition sur `passage_info`).
fn retirer_passages(c: &Connection, condition: &str, p: impl rusqlite::Params + Copy) -> rusqlite::Result<()> {
    c.execute(&format!("DELETE FROM passages WHERE rowid IN (SELECT id FROM passage_info WHERE {condition})"), p)?;
    c.execute(&format!("DELETE FROM vecteurs WHERE passage IN (SELECT id FROM passage_info WHERE {condition})"), p)?;
    c.execute(&format!("DELETE FROM passage_info WHERE {condition}"), p)?;
    Ok(())
}

/// Fil d'indexation : premier passage quelques minutes après le démarrage, puis toutes les 6 h.
pub fn surveiller() {
    std::thread::Builder::new()
        .name("index".into())
        .spawn(|| {
            se_faire_discret();
            std::thread::sleep(Duration::from_secs(180));
            loop {
                // Coupé dans les réglages : l'index est effacé, rien n'est plus lu.
                if !crate::config::Config::load().index {
                    let _ = std::fs::remove_file(chemin_base());
                    for ext in ["-wal", "-shm"] {
                        let _ = std::fs::remove_file(format!("{}{ext}", chemin_base().display()));
                    }
                } else if let Err(e) = mettre_a_jour(&paths::home_dir()) {
                    eprintln!("lkatta index : {e}");
                }
                // Toutes les 6 h ; regarde toutes les 10 min si le réglage a changé.
                for _ in 0..36 {
                    std::thread::sleep(Duration::from_secs(600));
                    // Vecteurs des passages (si un modèle d'embedding tourne) : 90 s au plus par tour.
                    crate::rag::vectoriser(usize::MAX, false, Duration::from_secs(90));
                    if crate::config::Config::load().index != std::path::Path::new(&chemin_base()).exists() {
                        break;
                    }
                }
            }
        })
        .ok();
}

/// Requête FTS5 sûre : chaque mot entre guillemets, préfixe accepté (« factu » trouve « facture »).
fn requete_fts(q: &str) -> String {
    mots(q).iter().map(|w| format!("\"{w}\"*")).collect::<Vec<_>>().join(" ")
}

/// Mots qui ne disent rien du sujet (français, anglais) : retirés d'une question en langage courant.
const MOTS_VIDES: &[&str] = &[
    "le", "la", "les", "un", "une", "des", "de", "du", "au", "aux", "et", "ou", "en", "dans", "sur", "sous", "par", "pour", "avec", "sans", "que", "qui",
    "quoi", "quel", "quelle", "quels", "quelles", "dont", "est", "sont", "été", "être", "avoir", "ai", "as", "a", "ont", "mon", "ma", "mes", "ton", "ta", "tes",
    "son", "sa", "ses", "notre", "nos", "votre", "vos", "leur", "leurs", "ce", "cet", "cette", "ces", "je", "tu", "il", "elle", "on", "nous", "vous", "ils",
    "elles", "me", "te", "se", "moi", "toi", "lui", "y", "ne", "pas", "plus", "dit", "dis", "dire", "fait", "faire", "comment", "pourquoi", "quand", "où",
    "peux", "peut", "veux", "trouve", "trouver", "cherche", "chercher", "fichier", "fichiers", "document", "documents", "parle", "parlais", "parlait",
    "selon", "après", "d", "l", "j", "qu", "c", "s", "n", "m", "t", "the", "an", "of", "to", "in", "on", "for", "and", "or", "is", "are", "was",
    "what", "which", "who", "how", "why", "when", "where", "my", "your", "about", "does", "do", "did", "say", "says", "file", "files", "find", "me",
];

/// Mots utiles d'une question : minuscules, sans les mots vides (s'il en reste), 12 au plus.
pub(crate) fn mots(q: &str) -> Vec<String> {
    let tous: Vec<String> = q.split(|c: char| !c.is_alphanumeric()).filter(|w| w.chars().count() >= 2).map(str::to_lowercase).collect();
    let mut utiles: Vec<String> = tous.iter().filter(|w| !MOTS_VIDES.contains(&w.as_str())).cloned().collect();
    if utiles.is_empty() {
        utiles = tous;
    }
    let mut vus = std::collections::HashSet::new();
    utiles.retain(|w| vus.insert(w.clone()));
    utiles.truncate(12);
    utiles
}

/// Requête souple : n'importe lequel des mots utiles ; BM25 classe d'abord les textes qui en
/// ont le plus (une question en langage courant trouve quand même).
pub(crate) fn requete_souple(q: &str) -> String {
    mots(q).iter().map(|w| format!("\"{w}\"*")).collect::<Vec<_>>().join(" OR ")
}

#[derive(Debug, Clone, PartialEq)]
pub struct Trouve {
    pub chemin: String,
    pub extrait: String,
    pub mtime: i64,
}

/// Fichiers dont le nom, le chemin ou le contenu contiennent tous les mots de `q`.
pub fn chercher(q: &str, n: usize) -> Result<Vec<Trouve>, String> {
    if !chemin_base().exists() {
        return Err(t!("l'index n'est pas encore construit (premier passage quelques minutes après le démarrage)").into());
    }
    let fts = requete_fts(q);
    if fts.is_empty() {
        return Ok(vec![]);
    }
    let n = n.clamp(1, 50);
    let c = ouvrir().map_err(|e| e.to_string())?;
    let regles = crate::regles::load();
    let lire = |requete: &str| -> Result<Vec<Trouve>, String> {
        let mut st = c
            .prepare_cached(
                "SELECT f.chemin, snippet(docs, 1, '«', '»', '…', 14), f.mtime FROM docs JOIN fichiers f ON f.id = docs.rowid
                 WHERE docs MATCH ?1 ORDER BY bm25(docs, 4.0, 1.0) LIMIT ?2",
            )
            .map_err(|e| e.to_string())?;
        let rows = st
            .query_map(params![requete, n as i64], |r| Ok(Trouve { chemin: r.get(0)?, extrait: r.get::<_, String>(1)?, mtime: r.get(2)? }))
            .map_err(|e| e.to_string())?;
        Ok(rows.flatten().filter(|t| Path::new(&t.chemin).exists() && !regles.path_forbidden(Path::new(&t.chemin))).collect())
    };
    // D'abord les fichiers qui ont tous les mots ; s'il en manque, ceux qui en ont une partie,
    // puis les documents dont un passage (au-delà du début gardé ici) contient les mots.
    let mut l = lire(&fts)?;
    let souple = requete_souple(q);
    for requete in [&fts, &souple] {
        if l.len() >= n {
            break;
        }
        let mut st = c
            .prepare_cached(
                "SELECT f.chemin, substr(passages.texte, 1, 160) || '…', f.mtime FROM passages JOIN passage_info pi ON pi.id = passages.rowid
                 JOIN fichiers f ON f.id = pi.fichier WHERE passages MATCH ?1 ORDER BY bm25(passages) LIMIT ?2",
            )
            .map_err(|e| e.to_string())?;
        let rows = st
            .query_map(params![requete, (n * 3) as i64], |r| Ok(Trouve { chemin: r.get(0)?, extrait: r.get::<_, String>(1)?, mtime: r.get(2)? }))
            .map_err(|e| e.to_string())?;
        for t in rows.flatten() {
            if l.len() >= n {
                break;
            }
            if !l.iter().any(|x| x.chemin == t.chemin) && Path::new(&t.chemin).exists() && !regles.path_forbidden(Path::new(&t.chemin)) {
                l.push(t);
            }
        }
    }
    if l.len() < n {
        if souple != fts {
            for t in lire(&souple)? {
                if l.len() >= n {
                    break;
                }
                if !l.iter().any(|x| x.chemin == t.chemin) {
                    l.push(t);
                }
            }
        }
    }
    Ok(l)
}

pub fn texte(l: &[Trouve]) -> String {
    if l.is_empty() {
        return t!("Rien dans l'index pour cette recherche.").into();
    }
    l.iter()
        .map(|t| {
            let date = chrono::DateTime::from_timestamp(t.mtime, 0).map(|d| d.with_timezone(&chrono::Local).format("%d/%m/%Y").to_string()).unwrap_or_default();
            let ex = t.extrait.split_whitespace().collect::<Vec<_>>().join(" ");
            if ex.is_empty() { format!("- {} ({date})", tilde(&t.chemin)) } else { format!("- {} ({date})\n  {ex}", tilde(&t.chemin)) }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Taille de l'index et date du dernier passage (pour l'inventaire).
pub fn etat() -> Option<(i64, String)> {
    if !chemin_base().exists() {
        return None;
    }
    let c = ouvrir().ok()?;
    let n: i64 = c.query_row("SELECT count(*) FROM fichiers", [], |r| r.get(0)).ok()?;
    let p: Option<String> = c.query_row("SELECT valeur FROM etat WHERE cle='passage'", [], |r| r.get(0)).optional().ok()?;
    let quand = p
        .and_then(|v| v.parse::<i64>().ok())
        .and_then(|t| chrono::DateTime::from_timestamp(t, 0))
        .map(|d| d.with_timezone(&chrono::Local).format("%d/%m %H:%M").to_string())
        .unwrap_or_else(|| "—".into());
    Some((n, quand))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fts_query_is_safe() {
        assert_eq!(requete_fts("facture d'EDF 2026"), "\"facture\"* \"edf\"* \"2026\"*");
        assert_eq!(requete_fts("\" OR 1=1 --"), "\"or\"*");
        assert_eq!(requete_souple("Que dit mon contrat sur le préavis ?"), "\"contrat\"* OR \"préavis\"*");
        assert_eq!(requete_souple("de la"), "\"de\"* OR \"la\"*");
        assert!(sauter("node_modules") && sauter(".git") && !sauter("Documents"));
    }

    #[test]
    fn indexes_and_finds() {
        let d = std::env::temp_dir().join(format!("lkatta-index-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        BASE_ESSAI.with(|b| *b.borrow_mut() = Some(d.join("index.db")));
        let d = d.join("home");
        std::fs::create_dir_all(d.join("cours")).unwrap();
        std::fs::create_dir_all(d.join("node_modules")).unwrap();
        std::fs::write(d.join("cours/analyse.md"), "Théorème de Rolle et accroissements finis").unwrap();
        std::fs::write(d.join("node_modules/x.js"), "Rolle").unwrap();
        // Un projet : seuls ses documents entrent ; un script hors projet entre avec son code.
        std::fs::create_dir_all(d.join("projet/src")).unwrap();
        std::fs::write(d.join("projet/Cargo.toml"), "[package]").unwrap();
        std::fs::write(d.join("projet/src/main.rs"), "fn rolle() {}").unwrap();
        std::fs::write(d.join("projet/LISEZMOI.md"), "Projet zébulon").unwrap();
        std::fs::write(d.join("sauvegarde.sh"), "rsync zebulon").unwrap();
        let b = mettre_a_jour(&d).unwrap();
        assert_eq!(b.relus, 3);
        let z: Vec<String> = chercher("zebulon", 5).unwrap().into_iter().map(|t| t.chemin).collect();
        assert!(z.iter().any(|c| c.ends_with("LISEZMOI.md")) && z.iter().any(|c| c.ends_with("sauvegarde.sh")), "{z:?}");
        let r = chercher("theoreme rolle", 5).unwrap();
        assert!(r[0].chemin.ends_with("analyse.md"));
        assert_eq!(mettre_a_jour(&d).unwrap().relus, 0);
        std::fs::remove_file(d.join("cours/analyse.md")).unwrap();
        assert_eq!(mettre_a_jour(&d).unwrap().retires, 1);
        assert!(chercher("rolle", 5).unwrap().is_empty());
        let _ = std::fs::remove_dir_all(d.parent().unwrap());
    }

    #[test]
    fn answers_natural_questions_from_deep_passages() {
        let d = std::env::temp_dir().join(format!("lkatta-rag-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("home/Documents")).unwrap();
        BASE_ESSAI.with(|b| *b.borrow_mut() = Some(d.join("index.db")));
        let h = d.join("home");
        // La réponse est loin dans le document (au-delà des 12 Ko d'avant).
        let remplissage = "Article sans rapport avec la question, qui parle de bureaux et de horaires. ".repeat(400);
        std::fs::write(h.join("Documents/contrat.md"), format!("{remplissage}\n\nLe préavis de démission est de deux mois.\n\n{remplissage}")).unwrap();
        std::fs::write(h.join("Documents/recette.md"), "Le gâteau au chocolat demande deux heures de repos.").unwrap();
        mettre_a_jour(&h).unwrap();
        // Avant : « que dit mon contrat sur le préavis » exigeait tous les mots, donc rien.
        let l = crate::rag::chercher("Que dit mon contrat sur le préavis ?", 3).unwrap();
        assert!(!l.is_empty() && l[0].chemin.ends_with("contrat.md") && l[0].texte.contains("deux mois"), "{l:?}");
        assert!(l[0].sur);
        // Fichiers : repli souple quand tous les mots ne sont pas dans un même fichier.
        assert!(chercher("préavis gâteau", 5).unwrap().len() == 2);
        // Le fichier relu remplace ses passages ; supprimé, ils partent.
        std::fs::write(h.join("Documents/contrat.md"), "Le préavis passe à trois mois.").unwrap();
        let futur = std::fs::File::options().write(true).open(h.join("Documents/contrat.md")).unwrap();
        futur.set_modified(SystemTime::now() + Duration::from_secs(5)).unwrap();
        mettre_a_jour(&h).unwrap();
        let l = crate::rag::chercher("préavis", 5).unwrap();
        assert_eq!(l.len(), 1);
        assert!(l[0].texte.contains("trois mois"));
        std::fs::remove_file(h.join("Documents/contrat.md")).unwrap();
        mettre_a_jour(&h).unwrap();
        assert!(crate::rag::chercher("préavis", 5).unwrap().is_empty());
        let c = ouvrir().unwrap();
        let n: i64 = c.query_row("SELECT count(*) FROM passage_info", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 1);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn adapts_to_machine_and_habits() {
        assert_eq!(decoder_uri("/home/a/Mes%20cours/r%C3%A9sum%C3%A9.pdf"), "/home/a/Mes cours/résumé.pdf");
        assert_eq!(decoder_uri("/x/100%"), "/x/100%");
        let b = budget_base();
        assert!((20 * 1024 * 1024..=600 * 1024 * 1024).contains(&b), "{b}");
    }
}
