//! Clés d'API des fournisseurs gratuits « avec clé » et mots de passe des comptes :
//! toujours rangés dans le trousseau du bureau (Secret Service : KWallet, GNOME…),
//! jamais dans un fichier de L Katta. Registre repris de Tontolo (adresses fixes : une
//! clé n'est envoyée qu'à l'hôte prévu).

use std::collections::HashMap;
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};

/// Un fournisseur gratuit qui demande une clé.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fournisseur {
    pub id: &'static str,
    pub nom: &'static str,
    pub base_url: &'static str,
    /// Liste des modèles si elle n'est pas à `{base_url}/models`.
    pub models_url: Option<&'static str>,
    /// Où créer la clé.
    pub key_url: &'static str,
    pub description: &'static str,
    /// Seuls les modèles dont l'identifiant finit ainsi sont gratuits (sinon : tous).
    pub free_suffix: Option<&'static str>,
}

pub const FOURNISSEURS: &[Fournisseur] = &[
    Fournisseur { id: "openrouter", nom: "OpenRouter", base_url: "https://openrouter.ai/api/v1", models_url: None, key_url: "https://openrouter.ai/settings/keys", description: "Des dizaines de modèles « :free », un seul compte.", free_suffix: Some(":free") },
    Fournisseur { id: "groq", nom: "Groq", base_url: "https://api.groq.com/openai/v1", models_url: None, key_url: "https://console.groq.com/keys", description: "Très rapide. Offre gratuite permanente (débit limité).", free_suffix: None },
    Fournisseur { id: "gemini", nom: "Google Gemini", base_url: "https://generativelanguage.googleapis.com/v1beta/openai", models_url: None, key_url: "https://aistudio.google.com/apikey", description: "Gemini Flash et Pro, clé Google AI Studio gratuite.", free_suffix: None },
    Fournisseur { id: "mistral", nom: "Mistral AI", base_url: "https://api.mistral.ai/v1", models_url: None, key_url: "https://console.mistral.ai/api-keys", description: "Offre « Experiment » gratuite, tous les modèles Mistral.", free_suffix: None },
    Fournisseur { id: "cerebras", nom: "Cerebras", base_url: "https://api.cerebras.ai/v1", models_url: None, key_url: "https://cloud.cerebras.ai", description: "Parmi les plus rapides. Gratuit, débit limité.", free_suffix: None },
    Fournisseur { id: "github", nom: "GitHub Models", base_url: "https://models.github.ai/inference", models_url: Some("https://models.github.ai/catalog/models"), key_url: "https://github.com/settings/personal-access-tokens/new", description: "GPT, Llama, DeepSeek… jeton GitHub (droit « models: read »).", free_suffix: None },
    Fournisseur { id: "sambanova", nom: "SambaNova Cloud", base_url: "https://api.sambanova.ai/v1", models_url: None, key_url: "https://cloud.sambanova.ai/apis", description: "Grands modèles ouverts, rapides. Gratuit, débit limité.", free_suffix: None },
    Fournisseur { id: "nvidia", nom: "NVIDIA NIM", base_url: "https://integrate.api.nvidia.com/v1", models_url: None, key_url: "https://build.nvidia.com/settings/api-keys", description: "Des dizaines de modèles ouverts, gratuits pour essayer.", free_suffix: None },
    Fournisseur { id: "together", nom: "Together AI", base_url: "https://api.together.xyz/v1", models_url: None, key_url: "https://api.together.ai/settings/api-keys", description: "Seulement les modèles « Free » (les autres sont payants).", free_suffix: Some("-Free") },
    Fournisseur { id: "huggingface", nom: "Hugging Face", base_url: "https://router.huggingface.co/v1", models_url: None, key_url: "https://huggingface.co/settings/tokens", description: "Routeur d'inférence, crédits gratuits chaque mois.", free_suffix: None },
];

pub fn fournisseur(id: &str) -> Option<&'static Fournisseur> {
    FOURNISSEURS.iter().find(|f| f.id == id)
}

fn cache() -> &'static Mutex<HashMap<String, Option<String>>> {
    static C: OnceLock<Mutex<HashMap<String, Option<String>>>> = OnceLock::new();
    C.get_or_init(Default::default)
}

fn lookup(attrs: &[&str]) -> Option<String> {
    let o = Command::new("secret-tool").arg("lookup").args(attrs).stderr(Stdio::null()).output().ok()?;
    let v = String::from_utf8_lossy(&o.stdout).trim_end_matches('\n').to_string();
    (!v.is_empty()).then_some(v)
}

/// Range un secret dans le trousseau (lu sur l'entrée standard, jamais en argument).
pub fn store(label: &str, attrs: &[&str], secret: &str) -> Result<(), String> {
    let mut child = Command::new("secret-tool")
        .arg("store")
        .arg(format!("--label={label}"))
        .args(attrs)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| "secret-tool absent (paquet libsecret)".to_string())?;
    child.stdin.take().ok_or("stdin")?.write_all(secret.as_bytes()).map_err(|e| e.to_string())?;
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    cache().lock().map(|mut c| c.clear()).ok();
    if out.status.success() { Ok(()) } else { Err(format!("trousseau : {}", String::from_utf8_lossy(&out.stderr).trim())) }
}

pub fn clear(attrs: &[&str]) {
    let _ = Command::new("secret-tool").arg("clear").args(attrs).status();
    cache().lock().map(|mut c| c.clear()).ok();
}

/// Une clé plausible (pas un texte à remplacer) ?
pub fn plausible(id: &str, k: &str) -> bool {
    let k = k.trim();
    if k.len() < 20 || k.contains(char::is_whitespace) {
        return false;
    }
    match id {
        "openrouter" => k.starts_with("sk-or-") && k.len() >= 40,
        "groq" => k.starts_with("gsk_"),
        "github" => k.starts_with("github_pat_") || k.starts_with("ghp_"),
        "huggingface" => k.starts_with("hf_"),
        "nvidia" => k.starts_with("nvapi-"),
        _ => true,
    }
}

/// Clé d'un fournisseur : trousseau, puis variable `ID_API_KEY` ; OpenRouter accepte aussi
/// la clé déjà configurée pour `orfree` (~/.config/openrouter/env).
pub fn cle(id: &str) -> Option<String> {
    if let Some(v) = cache().lock().ok().and_then(|c| c.get(id).cloned()) {
        return v;
    }
    let mut v = lookup(&["service", "lkatta", "fournisseur", id]);
    if v.is_none() {
        v = std::env::var(format!("{}_API_KEY", id.to_uppercase())).ok().filter(|k| !k.trim().is_empty());
    }
    if v.is_none() && id == "openrouter" {
        v = crate::config::openrouter_key(&crate::config::Config::load());
    }
    let v = v.filter(|k| plausible(id, k));
    if let Ok(mut c) = cache().lock() {
        c.insert(id.to_string(), v.clone());
    }
    v
}

pub fn set_cle(id: &str, key: &str) -> Result<(), String> {
    let f = fournisseur(id).ok_or("fournisseur inconnu")?;
    let key = key.trim();
    if !plausible(id, key) {
        return Err(format!("cette clé ne ressemble pas à une clé {} : vérifie le copier-coller", f.nom));
    }
    store(&format!("L Katta : clé {}", f.nom), &["service", "lkatta", "fournisseur", id], key)
}

pub fn remove_cle(id: &str) {
    clear(&["service", "lkatta", "fournisseur", id]);
}

/// « sk-or…a1b2 » : pour montrer qu'une clé est enregistrée sans la révéler.
pub fn masked(k: &str) -> String {
    let n = k.chars().count();
    if n <= 8 {
        return "••••".into();
    }
    format!("{}…{}", k.chars().take(4).collect::<String>(), k.chars().skip(n - 4).collect::<String>())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_is_https_and_unique() {
        let mut ids = std::collections::HashSet::new();
        for f in FOURNISSEURS {
            assert!(f.base_url.starts_with("https://") && f.key_url.starts_with("https://"));
            assert!(ids.insert(f.id));
        }
        assert_eq!(masked("sk-or-v1-abcdef123456"), "sk-o…3456");
        assert_eq!(masked("abc"), "••••");
        assert!(!plausible("openrouter", "sk-or-v1-COLLE-MOI"));
        assert!(plausible("openrouter", &format!("sk-or-v1-{}", "a".repeat(64))));
        assert!(!plausible("groq", "abcdefghijklmnopqrstuvwxyz"));
    }
}
