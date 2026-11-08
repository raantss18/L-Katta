//! Mise à jour interne depuis les releases GitHub, vérifiée par SHA-256.

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

use serde_json::Value;

use crate::paths;

const REPO: &str = "raantss18/L-Katta";

pub fn current() -> String {
    // LKATTA_FAUX_VERSION : uniquement pour essayer la mise à jour dans un dossier de test.
    std::env::var("LKATTA_FAUX_VERSION").unwrap_or_else(|_| env!("CARGO_PKG_VERSION").to_string())
}

/// « 0.0.10 » > « 0.0.9 » ; préfixe « v » ignoré.
pub fn newer(remote: &str, local: &str) -> bool {
    let parse = |v: &str| v.trim_start_matches('v').split('.').map(|x| x.parse::<u32>().unwrap_or(0)).collect::<Vec<_>>();
    parse(remote) > parse(local)
}

#[derive(Debug, Clone)]
pub struct Release {
    pub version: String,
    pub notes: String,
    archive: String,
    appimage: String,
    sums: String,
    pub page: String,
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(120)))
        .user_agent(concat!("lkatta/", env!("CARGO_PKG_VERSION")))
        .build()
        .into()
}

/// Dernière release publiée, si elle est plus récente que la version en cours.
pub fn check() -> Result<Option<Release>, String> {
    let mut r = agent()
        .get(format!("https://api.github.com/repos/{REPO}/releases/latest"))
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| e.to_string())?;
    let v: Value = r.body_mut().read_json().map_err(|e| e.to_string())?;
    let tag = v["tag_name"].as_str().unwrap_or("").to_string();
    if tag.is_empty() || !newer(&tag, &current()) {
        return Ok(None);
    }
    let assets = v["assets"].as_array().cloned().unwrap_or_default();
    let url = |pred: &dyn Fn(&str) -> bool| {
        assets
            .iter()
            .find(|a| pred(a["name"].as_str().unwrap_or("")))
            .and_then(|a| a["browser_download_url"].as_str())
            .unwrap_or("")
            .to_string()
    };
    let archive = url(&|n| n.ends_with("x86_64-linux.tar.gz"));
    let sums = url(&|n| n == "SHA256SUMS");
    let appimage = url(&|n| n.ends_with("x86_64.AppImage"));
    if (archive.is_empty() && appimage.is_empty()) || sums.is_empty() {
        return Err(format!("la release {tag} n'a pas d'archive Linux vérifiable"));
    }
    Ok(Some(Release {
        version: tag.trim_start_matches('v').to_string(),
        notes: v["name"].as_str().unwrap_or("").to_string(),
        archive,
        appimage,
        sums,
        page: v["html_url"].as_str().unwrap_or("").to_string(),
    }))
}

/// Binaire installé (~/.local/bin/lkatta) : seul celui-ci est remplacé.
pub fn installed_bin() -> PathBuf {
    paths::home_dir().join(".local/bin/lkatta")
}

fn download(url: &str, to: &std::path::Path) -> Result<(), String> {
    let mut r = agent().get(url).call().map_err(|e| e.to_string())?;
    let bytes = r.body_mut().with_config().limit(200 * 1024 * 1024).read_to_vec().map_err(|e| e.to_string())?;
    std::fs::write(to, bytes).map_err(|e| e.to_string())
}

/// Le fichier installé est-il une AppImage (signature « AI\x02 » à l'octet 8) ?
fn is_appimage(p: &std::path::Path) -> bool {
    use std::io::Read;
    let mut b = [0u8; 11];
    std::fs::File::open(p).and_then(|mut f| f.read_exact(&mut b)).is_ok() && &b[8..11] == b"AI\x02"
}

/// Télécharge, vérifie, installe. Renvoie la nouvelle version.
pub fn install(rel: &Release) -> Result<String, String> {
    let exe = match std::env::var_os("APPIMAGE") {
        Some(a) => PathBuf::from(a),
        None => std::env::current_exe().map_err(|e| e.to_string())?,
    };
    let bin = installed_bin();
    if is_appimage(&bin) && !rel.appimage.is_empty() {
        return install_appimage(rel, &bin);
    }
    if exe.canonicalize().ok() != bin.canonicalize().ok() {
        return Err(format!(
            "cette copie n'est pas l'installation ({}) : mets à jour les sources (git pull) ou lance « lkatta installer »",
            bin.display()
        ));
    }
    let dir = paths::data_dir().join("maj");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let name = rel.archive.rsplit('/').next().unwrap_or("lkatta.tar.gz").to_string();
    let tar = dir.join(&name);
    download(&rel.archive, &tar)?;
    download(&rel.sums, &dir.join("SHA256SUMS"))?;
    // Vérification de l'empreinte avant toute installation.
    let ok = Command::new("sha256sum")
        .args(["--check", "--ignore-missing", "--status", "SHA256SUMS"])
        .current_dir(&dir)
        .status()
        .map_err(|e| e.to_string())?;
    if !ok.success() {
        return Err("empreinte SHA-256 incorrecte : mise à jour refusée".into());
    }
    let st = Command::new("tar").arg("-xzf").arg(&tar).arg("-C").arg(&dir).status().map_err(|e| e.to_string())?;
    if !st.success() {
        return Err("archive illisible".into());
    }
    let new_bin = walkdir::WalkDir::new(&dir)
        .into_iter()
        .flatten()
        .find(|e| e.file_name() == "lkatta" && e.file_type().is_file())
        .map(|e| e.into_path())
        .ok_or("binaire absent de l'archive")?;
    let out = Command::new(&new_bin).arg("version").output().map_err(|e| format!("nouveau binaire inutilisable : {e}"))?;
    if !String::from_utf8_lossy(&out.stdout).contains(&rel.version) {
        return Err("le nouveau binaire ne donne pas la version attendue".into());
    }
    let tmp = bin.with_extension("new");
    std::fs::copy(&new_bin, &tmp).map_err(|e| e.to_string())?;
    // Garde l'ancien pour revenir en arrière.
    let _ = std::fs::copy(&bin, bin.with_extension("ancien"));
    std::fs::rename(&tmp, &bin).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_dir_all(&dir);
    Ok(rel.version.clone())
}

fn install_appimage(rel: &Release, bin: &std::path::Path) -> Result<String, String> {
    let dir = paths::data_dir().join("maj");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let name = rel.appimage.rsplit('/').next().unwrap_or("LKatta.AppImage").to_string();
    download(&rel.appimage, &dir.join(&name))?;
    download(&rel.sums, &dir.join("SHA256SUMS"))?;
    let ok = Command::new("sha256sum").args(["--check", "--ignore-missing", "--status", "SHA256SUMS"]).current_dir(&dir).status().map_err(|e| e.to_string())?;
    if !ok.success() {
        return Err("empreinte SHA-256 incorrecte : mise à jour refusée".into());
    }
    use std::os::unix::fs::PermissionsExt;
    let new = dir.join(&name);
    std::fs::set_permissions(&new, std::fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())?;
    let out = Command::new(&new).arg("version").env("APPIMAGE_EXTRACT_AND_RUN", "1").output().map_err(|e| format!("nouvelle AppImage inutilisable : {e}"))?;
    if !String::from_utf8_lossy(&out.stdout).contains(&rel.version) {
        return Err("la nouvelle AppImage ne donne pas la version attendue".into());
    }
    let tmp = bin.with_extension("new");
    std::fs::copy(&new, &tmp).map_err(|e| e.to_string())?;
    let _ = std::fs::copy(bin, bin.with_extension("ancien"));
    std::fs::rename(&tmp, bin).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_dir_all(&dir);
    Ok(rel.version.clone())
}

/// Relance le maki avec le nouveau binaire.
pub fn restart() {
    let bin = installed_bin();
    let _ = Command::new("sh")
        .args(["-c", &format!("sleep 1.5; exec setsid {} >/dev/null 2>&1", bin.display())])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}

#[cfg(test)]
mod tests {
    use super::newer;

    #[test]
    fn versions() {
        assert!(newer("v0.0.2", "0.0.1"));
        assert!(newer("0.1.0", "0.0.9"));
        assert!(newer("1.0.0", "0.9.9"));
        assert!(!newer("v0.0.1", "0.0.1"));
        assert!(!newer("0.0.1", "0.0.2"));
    }
}
