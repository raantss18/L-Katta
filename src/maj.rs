//! Mise à jour interne depuis les releases GitHub, vérifiée par signature Ed25519.
//!
//! Chaque version publie `SHA256SUMS` (les empreintes des fichiers) et `SHA256SUMS.sig` (la
//! signature de ce fichier par la clé de l'auteur, gardée hors du dépôt). La clé publique est
//! dans ce binaire : même avec le compte GitHub de l'auteur, on ne peut pas faire installer
//! un fichier que la clé n'a pas signé. Une version sans signature valide est refusée.

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

use serde_json::Value;

use crate::paths;

const REPO: &str = "raantss18/L-Katta";

/// Clé publique Ed25519 des versions (la clé privée : `release-keys/ed25519.pem`, hors Git).
const CLE_PUBLIQUE: [u8; 32] = [
    0x52, 0x41, 0xc7, 0xcb, 0xe0, 0x1d, 0x0f, 0x06, 0xdf, 0x7c, 0x89, 0xcf, 0xd7, 0x5d, 0x7c, 0xec, 0x09, 0x6c, 0xc8, 0x44, 0x8c, 0x48, 0x41, 0xdd, 0x11, 0x1f,
    0x31, 0x92, 0x50, 0x7d, 0x29, 0x29,
];

/// `sommes` (contenu de SHA256SUMS) est-il signé par `cle` ? `sig_b64` : 64 octets en base64.
fn signature_valide(cle: &[u8; 32], sommes: &[u8], sig_b64: &str) -> bool {
    use base64::Engine;
    let Ok(sig) = base64::engine::general_purpose::STANDARD.decode(sig_b64.split_whitespace().collect::<String>()) else { return false };
    let (Ok(pk), Ok(sig)) = (ed25519_compact::PublicKey::from_slice(cle), ed25519_compact::Signature::from_slice(&sig)) else { return false };
    pk.verify(sommes, &sig).is_ok()
}

/// Empreinte attendue d'un fichier : la ligne de SHA256SUMS qui porte exactement son nom.
fn empreinte_attendue(sommes: &str, nom: &str) -> Option<String> {
    sommes.lines().find_map(|l| {
        let (h, n) = l.split_once(char::is_whitespace)?;
        let n = n.trim().trim_start_matches('*');
        (n == nom && h.len() == 64 && h.chars().all(|c| c.is_ascii_hexdigit())).then(|| h.to_lowercase())
    })
}

/// Vérifie un fichier téléchargé : signature des empreintes, puis empreinte de ce fichier.
fn verifier(dir: &std::path::Path, nom: &str) -> Result<(), String> {
    let sommes = std::fs::read(dir.join("SHA256SUMS")).map_err(|e| e.to_string())?;
    let sig = std::fs::read_to_string(dir.join("SHA256SUMS.sig")).map_err(|e| e.to_string())?;
    if !signature_valide(&CLE_PUBLIQUE, &sommes, &sig) {
        return Err(t!("signature de la version incorrecte : mise à jour refusée").into());
    }
    let attendue = empreinte_attendue(&String::from_utf8_lossy(&sommes), nom).ok_or_else(|| t!("empreinte SHA-256 incorrecte : mise à jour refusée").to_string())?;
    let o = Command::new("sha256sum").arg(dir.join(nom)).output().map_err(|e| e.to_string())?;
    let obtenue = String::from_utf8_lossy(&o.stdout).split_whitespace().next().unwrap_or("").to_lowercase();
    if !o.status.success() || obtenue != attendue {
        return Err(t!("empreinte SHA-256 incorrecte : mise à jour refusée").into());
    }
    Ok(())
}

/// `lkatta verifier-signature SHA256SUMS SHA256SUMS.sig` : contrôle avant de publier.
pub fn verifier_fichiers(sommes: &str, sig: &str) -> Result<(), String> {
    let s = std::fs::read(sommes).map_err(|e| format!("{sommes} : {e}"))?;
    let g = std::fs::read_to_string(sig).map_err(|e| format!("{sig} : {e}"))?;
    if signature_valide(&CLE_PUBLIQUE, &s, &g) { Ok(()) } else { Err(t!("signature de la version incorrecte : mise à jour refusée").into()) }
}

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
    /// Signature de SHA256SUMS.
    sig: String,
    pub page: String,
}

/// Connexion lente (quelques dizaines de Ko/s) : le téléchargement peut durer plusieurs
/// minutes. On abandonne seulement si le serveur ne répond pas, ou au bout de 30 minutes.
fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(20)))
        .timeout_recv_response(Some(Duration::from_secs(60)))
        .timeout_global(Some(Duration::from_secs(30 * 60)))
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
    let sig = url(&|n| n == "SHA256SUMS.sig");
    if (archive.is_empty() && appimage.is_empty()) || sums.is_empty() {
        return Err(t!("la release {} n'a pas d'archive Linux vérifiable", tag));
    }
    if sig.is_empty() {
        return Err(t!("la version {} n'est pas signée : mise à jour refusée", tag));
    }
    Ok(Some(Release {
        version: tag.trim_start_matches('v').to_string(),
        notes: v["name"].as_str().unwrap_or("").to_string(),
        archive,
        appimage,
        sums,
        sig,
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
        None => crate::paths::self_exe(),
    };
    let bin = installed_bin();
    if is_appimage(&bin) && !rel.appimage.is_empty() {
        return install_appimage(rel, &bin);
    }
    if exe.canonicalize().ok() != bin.canonicalize().ok() {
        return Err(t!(
            "cette copie n'est pas l'installation ({}) : mets à jour les sources (git pull) ou lance « lkatta installer »",
            bin.display()
        ));
    }
    let dir = paths::data_dir().join("maj");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let name = rel.archive.rsplit('/').next().filter(|n| !n.is_empty() && !n.starts_with('.')).unwrap_or("lkatta.tar.gz").to_string();
    let tar = dir.join(&name);
    download(&rel.archive, &tar)?;
    download(&rel.sums, &dir.join("SHA256SUMS"))?;
    download(&rel.sig, &dir.join("SHA256SUMS.sig"))?;
    // Signature et empreinte vérifiées avant toute installation.
    verifier(&dir, &name)?;
    let st = Command::new("tar").arg("-xzf").arg(&tar).arg("-C").arg(&dir).status().map_err(|e| e.to_string())?;
    if !st.success() {
        return Err(t!("archive illisible").into());
    }
    let new_bin = walkdir::WalkDir::new(&dir)
        .into_iter()
        .flatten()
        .find(|e| e.file_name() == "lkatta" && e.file_type().is_file())
        .map(|e| e.into_path())
        .ok_or(t!("binaire absent de l'archive"))?;
    let out = Command::new(&new_bin).arg("version").output().map_err(|e| t!("nouveau binaire inutilisable : {}", e))?;
    if !String::from_utf8_lossy(&out.stdout).contains(&rel.version) {
        return Err(t!("le nouveau binaire ne donne pas la version attendue").into());
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
    let name = rel.appimage.rsplit('/').next().filter(|n| !n.is_empty() && !n.starts_with('.')).unwrap_or("LKatta.AppImage").to_string();
    download(&rel.appimage, &dir.join(&name))?;
    download(&rel.sums, &dir.join("SHA256SUMS"))?;
    download(&rel.sig, &dir.join("SHA256SUMS.sig"))?;
    verifier(&dir, &name)?;
    use std::os::unix::fs::PermissionsExt;
    let new = dir.join(&name);
    std::fs::set_permissions(&new, std::fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())?;
    let out = Command::new(&new).arg("version").env("APPIMAGE_EXTRACT_AND_RUN", "1").env("APPIMAGELAUNCHER_DISABLE", "1").output().map_err(|e| t!("nouvelle AppImage inutilisable : {}", e))?;
    if !String::from_utf8_lossy(&out.stdout).contains(&rel.version) {
        return Err(t!("la nouvelle AppImage ne donne pas la version attendue").into());
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
        .args(["-c", &format!("sleep 1.5; APPIMAGELAUNCHER_DISABLE=1 exec setsid {} >/dev/null 2>&1", bin.display())])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signature_and_checksums() {
        use base64::Engine;
        let kp = ed25519_compact::KeyPair::from_seed(ed25519_compact::Seed::new([7u8; 32]));
        let sommes = b"aaaa  autre\n0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef  LKatta-0.2.1-x86_64.AppImage\n";
        let sig = base64::engine::general_purpose::STANDARD.encode(kp.sk.sign(sommes, None).as_ref());
        let pk: [u8; 32] = kp.pk.as_ref().try_into().unwrap();
        assert!(signature_valide(&pk, sommes, &format!("{sig}\n")));
        // Un octet changé, une autre clé, une signature abîmée : refus.
        assert!(!signature_valide(&pk, b"aaaa  autre\n", &sig));
        assert!(!signature_valide(&CLE_PUBLIQUE, sommes, &sig));
        assert!(!signature_valide(&pk, sommes, "pas-du-base64"));
        assert!(!signature_valide(&pk, sommes, ""));
        let s = String::from_utf8_lossy(sommes);
        assert_eq!(empreinte_attendue(&s, "LKatta-0.2.1-x86_64.AppImage").as_deref(), Some("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"));
        // Le nom doit être exact (un fichier absent de la liste n'est jamais accepté).
        assert_eq!(empreinte_attendue(&s, "LKatta-0.2.1"), None);
        assert_eq!(empreinte_attendue(&s, "autre"), None);
    }

    #[test]
    fn versions() {
        assert!(newer("v0.0.2", "0.0.1"));
        assert!(newer("0.1.0", "0.0.9"));
        assert!(newer("1.0.0", "0.9.9"));
        assert!(!newer("v0.0.1", "0.0.1"));
        assert!(!newer("0.0.1", "0.0.2"));
    }
}
