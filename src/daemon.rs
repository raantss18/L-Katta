//! Démon : maki à l'écran + fils de fond (socket, rappels, presse-papiers, veille).

use std::time::Duration;

use chrono::Timelike;

use crate::config::Config;
use crate::ipc::{self, Msg};
use crate::{clipboard, fr, mascot, paths, reminders, tools, web};

pub fn run() -> Result<(), String> {
    if ipc::daemon_running() {
        return Err("L Katta tourne déjà (lkatta quitter pour l'arrêter).".into());
    }
    let cfg = Config::load();
    let _ = crate::profile::ensure(false);

    let (tx, rx) = smithay_client_toolkit::reexports::calloop::channel::channel::<Msg>();

    // Socket de commande.
    {
        let tx = tx.clone();
        ipc::listen(move |m| {
            let _ = tx.send(m);
        })
        .map_err(|e| format!("socket : {e}"))?;
    }

    // Salutation.
    {
        let tx = tx.clone();
        let prenom = cfg.prenom.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(1500));
            let _ = tx.send(Msg::Say(greeting(&prenom)));
        });
    }

    // Bureaux virtuels KDE : le maki retrouve sa place sur chacun.
    {
        let tx = tx.clone();
        std::thread::spawn(move || crate::kde::watch(|id| {
            let _ = tx.send(Msg::Desktop(id));
        }));
    }

    // Rappels (vérifiés toutes les 15 s).
    {
        let tx = tx.clone();
        std::thread::spawn(move || {
            loop {
                for r in reminders::take_due(chrono::Local::now()) {
                    let _ = tools::notify("Rappel de L Katta", &r.texte);
                    let _ = tx.send(Msg::Reminder(format!("⏰ {}", r.texte)));
                }
                std::thread::sleep(Duration::from_secs(15));
            }
        });
    }

    // Presse-papiers.
    if cfg.presse_papiers.actif {
        let max = cfg.presse_papiers.taille;
        std::thread::spawn(move || clipboard::watch(max, |_| {}));
    }

    // Veille discrète : santé de la machine et nouvelles importantes.
    {
        let tx = tx.clone();
        let cfg = cfg.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(90));
            loop {
                for hint in watch_once(&cfg) {
                    let _ = tx.send(Msg::Hint(hint));
                }
                std::thread::sleep(Duration::from_secs(30 * 60));
            }
        });
    }

    let _ = std::fs::write(paths::runtime_dir().join("demon.pid"), std::process::id().to_string());
    let r = mascot::run(cfg, rx);
    let _ = std::fs::remove_file(paths::socket_file());
    r
}

pub fn greeting(prenom: &str) -> String {
    let now = chrono::Local::now();
    let mut s = format!("{} {prenom} ! Il est {}.", fr::salutation(now.hour()), fr::heure(&now));
    let today = reminders::today_pending(now);
    match today.len() {
        0 => {}
        1 => s.push_str(&format!(" Un rappel aujourd'hui : {}.", today[0].texte)),
        n => s.push_str(&format!(" {n} rappels aujourd'hui.")),
    }
    s
}

#[derive(serde::Serialize, serde::Deserialize, Default)]
struct WatchState {
    #[serde(default)]
    arch_news: String,
    /// Constats déjà signalés.
    #[serde(default)]
    vus: Vec<String>,
}

/// Un passage de veille ; renvoie les remarques nouvelles (une seule fois par constat).
fn watch_once(_cfg: &Config) -> Vec<String> {
    let path = paths::data_dir().join("veille.json");
    let mut st: WatchState =
        std::fs::read_to_string(&path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
    let mut hints = vec![];

    // Bilan de santé (porté de mi-saina) : on ne signale que ce qui compte, une fois.
    let constats = crate::sante::bilan();
    for c in constats.iter().filter(|c| c.gravite != "info") {
        if !st.vus.contains(&c.id) {
            hints.push(format!("{} — {} {}", c.titre, c.detail, c.conseil));
        }
    }
    st.vus = constats.iter().map(|c| c.id.clone()).collect();

    // Nouvelles Arch Linux (souvent une intervention manuelle avant mise à jour).
    if std::path::Path::new("/etc/arch-release").exists() {
        if let Ok(items) = web::feed("https://archlinux.org/feeds/news/", 1) {
            if let Some(n) = items.first() {
                if !st.arch_news.is_empty() && n.titre != st.arch_news {
                    hints.push(format!("Nouvelle Arch Linux : « {} ». À lire avant la prochaine mise à jour.", n.titre));
                }
                st.arch_news = n.titre.clone();
            }
        }
    }
    if let Ok(s) = serde_json::to_string(&st) {
        let _ = paths::write_private(&path, s.as_bytes());
    }
    hints
}
