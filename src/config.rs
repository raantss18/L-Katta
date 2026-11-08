//! Configuration (`~/.config/lkatta/config.toml`), créée au premier lancement.

use serde::{Deserialize, Serialize};

use crate::paths;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Prénom utilisé pour saluer.
    pub prenom: String,
    pub maki: MakiCfg,
    /// Moteurs essayés dans l'ordre : le premier qui répond gagne.
    pub moteur: Vec<Moteur>,
    pub actualites: Actualites,
    pub presse_papiers: PressePapiers,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct MakiCfg {
    pub actif: bool,
    /// Échelle du personnage (1.0 = environ 90 px de haut).
    pub taille: f32,
    pub acrobaties: bool,
    /// Minutes minimum entre deux numéros (arbre, téléportation, escalade, salto).
    pub numeros_min: u32,
    /// « bords » : il reste près des bords gauche et droit (zone de travail libre) ;
    /// « libre » : tout l'écran.
    pub zone: String,
    /// Se promène d'un écran à l'autre.
    pub ecrans_multiples: bool,
    /// Minutes minimum entre deux paroles spontanées (les rappels passent toujours).
    pub parole_min: u32,
    /// Le maki dort entre ces deux heures (bornes [début, fin[).
    pub sommeil: [u32; 2],
    /// Durée d'affichage d'une bulle, en secondes.
    pub bulle_s: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Moteur {
    pub nom: String,
    /// Base d'une API compatible OpenAI, ex. `http://127.0.0.1:8765/v1`.
    pub url: String,
    pub modele: String,
    /// Modèles de repli envoyés dans `models` (OpenRouter).
    pub replis: Vec<String>,
    /// Clé : variable d'environnement…
    pub cle_env: String,
    /// …ou fichier `KEY=valeur` (ex. `~/.config/openrouter/env`).
    pub cle_fichier: String,
    /// Force un prix nul (OpenRouter : `provider.max_price = 0`).
    pub gratuit_seulement: bool,
    /// Commande pour démarrer le moteur s'il ne répond pas (ex. `tontolo serve`).
    pub demarrer: String,
    pub actif: bool,
}

impl Default for Moteur {
    fn default() -> Self {
        Self {
            nom: String::new(),
            url: String::new(),
            modele: "auto".into(),
            replis: vec![],
            cle_env: String::new(),
            cle_fichier: String::new(),
            gratuit_seulement: true,
            demarrer: String::new(),
            actif: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Actualites {
    pub flux: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PressePapiers {
    pub actif: bool,
    /// Nombre d'entrées gardées.
    pub taille: usize,
}

impl Default for MakiCfg {
    fn default() -> Self {
        Self { actif: true, taille: 1.0, acrobaties: true, numeros_min: 6, zone: "bords".into(), ecrans_multiples: true, parole_min: 25, sommeil: [23, 7], bulle_s: 9 }
    }
}

impl Default for Actualites {
    fn default() -> Self {
        Self {
            flux: vec![
                "https://archlinux.org/feeds/news/".into(),
                "https://kde.org/index.xml".into(),
                "https://linuxfr.org/news.atom".into(),
            ],
        }
    }
}

impl Default for PressePapiers {
    fn default() -> Self {
        Self { actif: true, taille: 40 }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            prenom: default_prenom(),
            maki: MakiCfg::default(),
            moteur: default_moteurs(),
            actualites: Actualites::default(),
            presse_papiers: PressePapiers::default(),
        }
    }
}

fn default_prenom() -> String {
    // Champ GECOS de /etc/passwd, sinon $USER.
    let user = std::env::var("USER").unwrap_or_default();
    if let Ok(passwd) = std::fs::read_to_string("/etc/passwd") {
        for line in passwd.lines() {
            let f: Vec<&str> = line.split(':').collect();
            if f.len() > 4 && f[0] == user {
                let gecos = f[4].split(',').next().unwrap_or("").trim();
                if let Some(first) = gecos.split_whitespace().next() {
                    return first.to_string();
                }
            }
        }
    }
    user
}

pub fn which(cmd: &str) -> bool {
    std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).any(|d| d.join(cmd).is_file()))
        .unwrap_or(false)
}

fn default_moteurs() -> Vec<Moteur> {
    let tontolo_start = if which("tontolo") {
        "tontolo serve".to_string()
    } else {
        let core = paths::home_dir().join("GitHub/TONTOLO/core");
        if core.join("pyproject.toml").is_file() && which("uv") {
            format!("uv run --project {} tontolo serve", core.display())
        } else {
            String::new()
        }
    };
    vec![
        Moteur {
            nom: "Tontolo".into(),
            url: "http://127.0.0.1:8765/v1".into(),
            modele: "auto".into(),
            demarrer: tontolo_start,
            ..Default::default()
        },
        Moteur {
            nom: "OpenRouter (gratuit)".into(),
            url: "https://openrouter.ai/api/v1".into(),
            modele: "nvidia/nemotron-3.5-lightning:free".into(),
            replis: vec!["thinkingmachines/inkling:free".into(), "openrouter/free".into()],
            cle_env: "OPENROUTER_API_KEY".into(),
            cle_fichier: "~/.config/openrouter/env".into(),
            gratuit_seulement: true,
            ..Default::default()
        },
        Moteur {
            nom: "Ollama (local)".into(),
            url: "http://127.0.0.1:11434/v1".into(),
            modele: "qwen3.5:4b".into(),
            ..Default::default()
        },
    ]
}

impl Config {
    /// Charge la configuration ; la crée avec les valeurs par défaut si absente.
    pub fn load() -> Config {
        let path = paths::config_file();
        match std::fs::read_to_string(&path) {
            Ok(s) => match toml::from_str::<Config>(&s) {
                Ok(c) => {
                    // Réglages ajoutés par une nouvelle version : on les écrit dans le fichier
                    // pour qu'ils soient visibles (valeurs déjà choisies conservées).
                    if toml::to_string_pretty(&c).is_ok_and(|full| full.lines().filter(|l| l.contains(" = ")).count() > s.lines().filter(|l| l.contains(" = ")).count()) {
                        let _ = c.save();
                    }
                    c
                }
                Err(e) => {
                    eprintln!("lkatta : config.toml illisible ({e}), valeurs par défaut utilisées");
                    Config::default()
                }
            },
            Err(_) => {
                let c = Config::default();
                let _ = c.save();
                c
            }
        }
    }

    pub fn save(&self) -> std::io::Result<()> {
        let s = toml::to_string_pretty(self).map_err(std::io::Error::other)?;
        let header = "# Configuration de L Katta. Modifiable à la main ; relancer le maki après :\n\
                      #   lkatta quitter && lkatta &\n\
                      # Moteurs : essayés dans l'ordre, le premier qui répond gagne (actif = false pour en couper un).\n\
                      # maki.zone : \"bords\" (laisse le milieu libre) ou \"libre\" ; maki.taille : 1.0 = ~90 px.\n\n";
        paths::write_private(&paths::config_file(), format!("{header}{s}").as_bytes())
    }
}

impl Moteur {
    /// Clé API éventuelle (variable d'environnement puis fichier).
    pub fn cle(&self) -> Option<String> {
        if !self.cle_env.is_empty() {
            if let Ok(v) = std::env::var(&self.cle_env) {
                if !v.trim().is_empty() {
                    return Some(v.trim().to_string());
                }
            }
        }
        if self.cle_fichier.is_empty() {
            return None;
        }
        let txt = std::fs::read_to_string(paths::expand(&self.cle_fichier)).ok()?;
        for line in txt.lines() {
            let line = line.trim().trim_start_matches("export ").trim();
            if let Some((k, v)) = line.split_once('=') {
                let k = k.trim();
                if (self.cle_env.is_empty() && k.ends_with("KEY")) || k == self.cle_env {
                    let v = v.trim().trim_matches('"').trim_matches('\'');
                    if !v.is_empty() {
                        return Some(v.to_string());
                    }
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_toml() {
        let c = Config::default();
        let s = toml::to_string_pretty(&c).unwrap();
        let back: Config = toml::from_str(&s).unwrap();
        assert_eq!(back.moteur.len(), c.moteur.len());
        assert_eq!(back.maki.sommeil, [23, 7]);
    }

    #[test]
    fn partial_toml_uses_defaults() {
        let c: Config = toml::from_str("prenom = \"Rabe\"\n[maki]\ntaille = 1.5\n").unwrap();
        assert_eq!(c.prenom, "Rabe");
        assert_eq!(c.maki.taille, 1.5);
        assert!(c.maki.actif);
    }
}
