//! Modèle local de secours (Ollama) : choisi d'après la machine (mémoire, carte graphique),
//! téléchargé sur demande, essayé en dernier quand les modèles en ligne ne répondent pas
//! (ou sans réseau). Rien n'est installé sans un clic de l'utilisateur.

use std::time::Duration;

const OLLAMA: &str = "http://127.0.0.1:11434";

/// Ce que la machine offre à un modèle.
#[derive(Debug, Clone, PartialEq)]
pub struct Machine {
    pub ram_go: f32,
    /// Carte graphique dédiée : (nom, mémoire en Go).
    pub gpu: Option<(String, f32)>,
}

/// Un modèle local possible (famille Qwen3 : sait appeler des outils).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Candidat {
    pub tag: &'static str,
    /// Taille du téléchargement (Go).
    pub poids_go: f32,
    /// Mémoire qu'il lui faut pour tourner à l'aise (Go).
    pub besoin_go: f32,
}

pub const CANDIDATS: [Candidat; 5] = [
    Candidat { tag: "qwen3:14b", poids_go: 9.3, besoin_go: 12.0 },
    Candidat { tag: "qwen3:8b", poids_go: 5.2, besoin_go: 7.0 },
    Candidat { tag: "qwen3:4b", poids_go: 2.5, besoin_go: 3.5 },
    Candidat { tag: "qwen3:1.7b", poids_go: 1.4, besoin_go: 2.0 },
    Candidat { tag: "qwen3:0.6b", poids_go: 0.5, besoin_go: 1.0 },
];

pub fn machine() -> Machine {
    let ram_go = std::fs::read_to_string("/proc/meminfo")
        .ok()
        .and_then(|s| s.lines().find_map(|l| l.strip_prefix("MemTotal:")).and_then(|v| v.trim().trim_end_matches("kB").trim().parse::<f32>().ok()))
        .map(|kb| kb / 1024.0 / 1024.0)
        .unwrap_or(4.0);
    Machine { ram_go, gpu: nvidia().or_else(amd) }
}

fn nvidia() -> Option<(String, f32)> {
    let o = std::process::Command::new("nvidia-smi").args(["--query-gpu=name,memory.total", "--format=csv,noheader,nounits"]).stderr(std::process::Stdio::null()).output().ok()?;
    let l = String::from_utf8_lossy(&o.stdout);
    let (nom, mo) = l.lines().next()?.rsplit_once(',')?;
    Some((nom.trim().to_string(), mo.trim().parse::<f32>().ok()? / 1024.0))
}

fn amd() -> Option<(String, f32)> {
    let mut best: Option<f32> = None;
    for e in std::fs::read_dir("/sys/class/drm").ok()?.flatten() {
        let p = e.path().join("device/mem_info_vram_total");
        if let Some(go) = std::fs::read_to_string(p).ok().and_then(|s| s.trim().parse::<f64>().ok()).map(|b| (b / 1024f64.powi(3)) as f32) {
            best = Some(best.map_or(go, |b| b.max(go)));
        }
    }
    // Un processeur graphique intégré n'annonce que quelques centaines de Mo : ignoré.
    best.filter(|g| *g >= 3.0).map(|g| ("AMD".to_string(), g))
}

/// Le meilleur modèle pour cette machine. Sur carte graphique, il tient dans sa mémoire ;
/// sans elle, il tourne sur le processeur : on reste petit (lent sinon).
pub fn conseil(m: &Machine) -> Candidat {
    let budget = match &m.gpu {
        Some((_, v)) if *v >= 3.5 => v + 0.5,
        _ if m.ram_go >= 30.0 => 7.0,
        _ => (m.ram_go * 0.5).min(3.6),
    };
    CANDIDATS.iter().copied().find(|c| c.besoin_go <= budget).unwrap_or(CANDIDATS[CANDIDATS.len() - 1])
}

/// Description lisible de la machine.
pub fn resume(m: &Machine) -> String {
    match &m.gpu {
        Some((nom, v)) => t!("{} Go de mémoire, carte graphique {} ({} Go)", format!("{:.0}", m.ram_go), nom, format!("{:.0}", v)),
        None => t!("{} Go de mémoire, sans carte graphique dédiée", format!("{:.0}", m.ram_go)),
    }
}

/// Ollama est-il installé (dans le PATH) ?
pub fn ollama_installe() -> bool {
    std::env::var_os("PATH").is_some_and(|p| std::env::split_paths(&p).any(|d| d.join("ollama").is_file()))
}

fn agent(secs: u64) -> ureq::Agent {
    ureq::Agent::config_builder().timeout_connect(Some(Duration::from_secs(2))).timeout_global(Some(Duration::from_secs(secs))).build().into()
}

/// Modèles installés dans Ollama (`None` : Ollama ne tourne pas).
pub fn installes() -> Option<Vec<String>> {
    let v: serde_json::Value = agent(3).get(format!("{OLLAMA}/api/tags")).call().ok()?.body_mut().read_json().ok()?;
    Some(v["models"].as_array().into_iter().flatten().filter_map(|m| m["name"].as_str().map(String::from)).collect())
}

/// Démarre le service Ollama s'il est installé mais arrêté.
fn demarrer() -> Result<(), String> {
    if installes().is_some() {
        return Ok(());
    }
    if !ollama_installe() {
        return Err(t!("Ollama n'est pas installé.").into());
    }
    std::process::Command::new("ollama")
        .arg("serve")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    for _ in 0..20 {
        std::thread::sleep(Duration::from_millis(300));
        if installes().is_some() {
            return Ok(());
        }
    }
    Err(t!("Ollama ne démarre pas.").into())
}

/// Télécharge un modèle (long : à lancer dans un fil), puis rafraîchit le catalogue.
pub fn telecharger(tag: &str) -> Result<String, String> {
    demarrer()?;
    let mut r = agent(3 * 3600)
        .post(format!("{OLLAMA}/api/pull"))
        .send_json(serde_json::json!({"model": tag, "stream": false}))
        .map_err(|e| e.to_string())?;
    let v: serde_json::Value = r.body_mut().read_json().unwrap_or_default();
    if let Some(e) = v["error"].as_str() {
        return Err(e.to_string());
    }
    let _ = crate::moteur::refresh();
    Ok(t!("{} est installé : je m'en servirai quand les modèles en ligne ne répondent pas.", tag))
}

/// Ce que le panneau de réglages montre (lu en arrière-plan, jamais pendant le dessin).
#[derive(Debug, Clone)]
pub struct Etat {
    pub machine: Machine,
    pub conseil: Candidat,
    pub ollama: bool,
    /// Modèles installés (`None` : Ollama arrêté).
    pub installes: Option<Vec<String>>,
}

impl Etat {
    /// Le modèle conseillé est-il déjà là ?
    pub fn pret(&self) -> bool {
        self.installes.as_ref().is_some_and(|l| l.iter().any(|x| x == self.conseil.tag))
    }
    /// Un modèle local quelconque est-il installé ?
    pub fn un_modele(&self) -> Option<String> {
        self.installes.as_ref().and_then(|l| l.first().cloned())
    }
}

/// Dernier état connu ; relu en arrière-plan s'il a plus de 10 s.
pub fn etat() -> Option<Etat> {
    use std::sync::{Mutex, OnceLock};
    use std::time::Instant;
    static C: OnceLock<Mutex<(Option<Instant>, Option<Etat>)>> = OnceLock::new();
    let c = C.get_or_init(|| Mutex::new((None, None)));
    let mut g = c.lock().ok()?;
    if g.0.is_none_or(|t| t.elapsed() > Duration::from_secs(10)) {
        g.0 = Some(Instant::now());
        std::thread::spawn(|| {
            let machine = machine();
            let e = Etat { conseil: conseil(&machine), machine, ollama: ollama_installe(), installes: installes() };
            if let Some(c) = C.get() {
                if let Ok(mut g) = c.lock() {
                    g.1 = Some(e);
                }
            }
        });
    }
    g.1.clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advice_fits_the_machine() {
        let m = |ram: f32, gpu: Option<f32>| Machine { ram_go: ram, gpu: gpu.map(|g| ("GPU".to_string(), g)) };
        assert_eq!(conseil(&m(8.0, Some(2.0))).tag, "qwen3:4b"); // MX250 : processeur
        assert_eq!(conseil(&m(4.0, None)).tag, "qwen3:1.7b");
        assert_eq!(conseil(&m(2.0, None)).tag, "qwen3:0.6b");
        assert_eq!(conseil(&m(16.0, Some(8.0))).tag, "qwen3:8b");
        assert_eq!(conseil(&m(32.0, Some(16.0))).tag, "qwen3:14b");
        assert_eq!(conseil(&m(64.0, None)).tag, "qwen3:8b");
        let r = resume(&m(16.0, None));
        assert!(r.contains("16"));
    }
}
