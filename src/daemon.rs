//! Démon : maki à l'écran + fils de fond (socket, rappels, presse-papiers, veille).

use std::time::Duration;


use crate::config::Config;
use crate::ipc::{self, Msg};
use crate::{clipboard, mascot, paths, reminders, tools, web};

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
            // Il arrive (suspendu en haut, puis il tombe) avant de parler.
            std::thread::sleep(Duration::from_millis(3200));
            let alertes: Vec<String> = crate::sante::bilan_rapide().into_iter().map(|c| c.titre.to_lowercase()).collect();
            let m = crate::conseils::accueil(&prenom, chrono::Local::now(), &alertes);
            let _ = tx.send(Msg::Bubble(m, true));
        });
    }

    // Une astuce de temps en temps (réglage maki.conseils_h, 0 = jamais).
    if cfg.maki.conseils_h > 0 {
        let tx = tx.clone();
        let every = Duration::from_secs(cfg.maki.conseils_h as u64 * 3600);
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(25 * 60).min(every));
            loop {
                let _ = tx.send(Msg::Bubble(crate::conseils::astuce(), false));
                std::thread::sleep(every);
            }
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
                    let _ = tx.send(Msg::Reminder(r.texte.clone()));
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
                for m in watch_once(&cfg) {
                    let _ = tx.send(Msg::Bubble(m, false));
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

#[derive(serde::Serialize, serde::Deserialize, Default)]
struct WatchState {
    #[serde(default)]
    arch_news: String,
    /// Constats déjà signalés.
    #[serde(default)]
    vus: Vec<String>,
}

/// Un passage de veille ; renvoie les remarques nouvelles (une seule fois par constat).
fn watch_once(_cfg: &Config) -> Vec<crate::conseils::Message> {
    use crate::conseils::{Kind, Message};
    let path = paths::data_dir().join("veille.json");
    let mut st: WatchState =
        std::fs::read_to_string(&path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
    let mut hints = vec![];

    // Bilan de santé (porté de mi-saina) : on ne signale que ce qui compte, une fois.
    let constats = crate::sante::bilan();
    for c in constats.iter().filter(|c| c.gravite != "info") {
        if !st.vus.contains(&c.id) {
            hints.push(Message::new(Kind::Alert, c.titre.clone(), format!("{} {}", c.detail, c.conseil)));
        }
    }
    st.vus = constats.iter().map(|c| c.id.clone()).collect();

    // Nouvelles Arch Linux (souvent une intervention manuelle avant mise à jour).
    if std::path::Path::new("/etc/arch-release").exists() {
        if let Ok(items) = web::feed("https://archlinux.org/feeds/news/", 1) {
            if let Some(n) = items.first() {
                if !st.arch_news.is_empty() && n.titre != st.arch_news {
                    let m = Message::new(Kind::News, "Nouvelle Arch Linux", format!("« {} » — à lire avant la prochaine mise à jour.", n.titre));
                    hints.push(if n.lien.is_empty() { m } else { m.with_link(n.lien.clone()) });
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
