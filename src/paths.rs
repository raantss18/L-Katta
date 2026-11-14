//! Emplacements des fichiers de L Katta (XDG).

use std::path::PathBuf;

fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/tmp"))
}

fn xdg(var: &str, fallback: &str) -> PathBuf {
    std::env::var_os(var)
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| home().join(fallback))
}

/// `~/.config/lkatta`
pub fn config_dir() -> PathBuf {
    xdg("XDG_CONFIG_HOME", ".config").join("lkatta")
}

/// `~/.local/share/lkatta`
pub fn data_dir() -> PathBuf {
    xdg("XDG_DATA_HOME", ".local/share").join("lkatta")
}

/// Dossier d'exécution (socket, verrous). Repli sur le dossier de données.
pub fn runtime_dir() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(|p| PathBuf::from(p).join("lkatta"))
        .unwrap_or_else(|| data_dir().join("run"))
}

/// Dossiers de L Katta lisibles par l'utilisateur seul (discussions, index, captures,
/// presse-papiers : rien pour les autres comptes de la machine).
pub fn fermer_dossiers() {
    use std::os::unix::fs::PermissionsExt;
    for d in [config_dir(), data_dir(), data_dir().join("captures"), data_dir().join("fils")] {
        let _ = std::fs::create_dir_all(&d);
        let _ = std::fs::set_permissions(&d, std::fs::Permissions::from_mode(0o700));
    }
    for f in ["moteur.log", "relance.log", "garde-memoire.log"] {
        let _ = std::fs::set_permissions(data_dir().join(f), std::fs::Permissions::from_mode(0o600));
    }
}

pub fn config_file() -> PathBuf {
    config_dir().join("config.toml")
}
pub fn prompt_file() -> PathBuf {
    config_dir().join("prompt.md")
}
pub fn machine_file() -> PathBuf {
    config_dir().join("machine.md")
}
pub fn reminders_file() -> PathBuf {
    data_dir().join("rappels.json")
}
pub fn clipboard_file() -> PathBuf {
    data_dir().join("presse-papiers.json")
}
pub fn socket_file() -> PathBuf {
    runtime_dir().join("lkatta.sock")
}

pub fn home_dir() -> PathBuf {
    home()
}

/// Chemin du programme à relancer : l'AppImage si on tourne depuis elle ; sinon le binaire
/// courant (sans le suffixe « (deleted) » que Linux ajoute quand le fichier a été remplacé
/// pendant qu'il tournait, par une mise à jour ou une recompilation), ou à défaut celui
/// installé dans ~/.local/bin.
/// Le processus lancé meurt avec L Katta (moniteurs `gdbus monitor` : sinon ils restent
/// orphelins à chaque relance).
pub fn mourir_avec_parent(cmd: &mut std::process::Command) -> &mut std::process::Command {
    use std::os::unix::process::CommandExt;
    // SAFETY : prctl est sûr entre fork et exec (pas d'allocation).
    unsafe {
        cmd.pre_exec(|| {
            libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM);
            Ok(())
        })
    }
}

pub fn self_exe() -> PathBuf {
    if let Some(a) = std::env::var_os("APPIMAGE") {
        return PathBuf::from(a);
    }
    let installed = home().join(".local/bin/lkatta");
    match std::env::current_exe() {
        Ok(p) => {
            let s = p.to_string_lossy();
            let clean = PathBuf::from(s.strip_suffix(" (deleted)").unwrap_or(&s));
            if clean.exists() { clean } else { installed }
        }
        Err(_) => installed,
    }
}

thread_local! {
    /// Dossier de travail du fil qui tourne (côté Code : le projet).
    static TRAVAIL: std::cell::RefCell<Option<PathBuf>> = const { std::cell::RefCell::new(None) };
}

/// Fixe le dossier de travail de ce fil d'exécution (`None` : dossier personnel).
pub fn set_travail(d: Option<PathBuf>) {
    TRAVAIL.with(|t| *t.borrow_mut() = d.filter(|p| p.is_dir()));
}

/// Dossier d'où partent les commandes et les chemins relatifs.
pub fn travail() -> PathBuf {
    TRAVAIL.with(|t| t.borrow().clone()).unwrap_or_else(home)
}

/// Remplace un `~` initial par le dossier personnel ; un chemin relatif part du dossier de
/// travail (le projet, côté Code).
pub fn expand(p: &str) -> PathBuf {
    if p == "~" {
        home()
    } else if let Some(rest) = p.strip_prefix("~/") {
        // « ~//x » reste sous le dossier personnel (join remplacerait tout par « /x »).
        home().join(rest.trim_start_matches('/'))
    } else if !p.is_empty() && !p.starts_with('/') {
        travail().join(p)
    } else {
        PathBuf::from(p)
    }
}

/// Écrit un fichier de façon atomique, droits 600 (données personnelles).
pub fn write_private(path: &std::path::Path, contents: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    // Nom temporaire propre à ce fichier et à cet appel (« regles.toml » et « regles.md » ne
    // partagent plus « regles.tmp ») ; créé neuf pour que les droits 600 s'appliquent toujours.
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let nom = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let tmp = path.with_file_name(format!(".{nom}.{}-{}.tmp", std::process::id(), N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
    let _ = std::fs::remove_file(&tmp);
    {
        let mut f = std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&tmp)?;
        if let Err(e) = f.write_all(contents) {
            let _ = std::fs::remove_file(&tmp);
            return Err(e);
        }
    }
    std::fs::rename(tmp, path)
}
