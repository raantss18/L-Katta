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

/// Remplace un `~` initial par le dossier personnel.
pub fn expand(p: &str) -> PathBuf {
    if p == "~" {
        home()
    } else if let Some(rest) = p.strip_prefix("~/") {
        home().join(rest)
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
    let tmp = path.with_extension("tmp");
    {
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&tmp)?;
        f.write_all(contents)?;
    }
    std::fs::rename(tmp, path)
}
