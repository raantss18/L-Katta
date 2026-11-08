//! Démon : maki à l'écran + fils de fond (socket, rappels, presse-papiers, veille).

use std::time::Duration;


use crate::config::Config;
use crate::ipc::{self, Msg};
use crate::{clipboard, mascot, paths, reminders, tools, web};

pub fn run() -> Result<(), String> {
    if ipc::daemon_running() {
        return Err(t!("L Katta tourne déjà (lkatta quitter pour l'arrêter).").into());
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
            let evenements = crate::agenda::today_lines(&Config::load().agenda);
            let m = crate::conseils::accueil(&prenom, chrono::Local::now(), &alertes, &evenements);
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

    // Mises à jour : au démarrage puis chaque jour.
    if cfg.maj != "non" {
        let tx = tx.clone();
        let auto = cfg.maj == "auto";
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(90));
            let mut proposed = String::new();
            loop {
                if let Ok(Some(rel)) = crate::maj::check() {
                    if auto {
                        if crate::maj::install(&rel).is_ok() {
                            let _ = tx.send(Msg::Say(t!("Je me suis mis à jour ({}). Je redémarre !", rel.version)));
                            std::thread::sleep(Duration::from_secs(4));
                            crate::maj::restart();
                            let _ = tx.send(Msg::Quit);
                            return;
                        }
                    } else if proposed != rel.version {
                        proposed = rel.version.clone();
                        let m = crate::conseils::Message::new(
                            crate::conseils::Kind::News,
                            t!("Mise à jour {} disponible", rel.version),
                            t!("Tu as la version {}. {}", crate::maj::current(), rel.notes),
                        )
                        .with_link(format!("maj:{}", rel.page));
                        let _ = tx.send(Msg::Bubble(m, true));
                    }
                }
                std::thread::sleep(Duration::from_secs(24 * 3600));
            }
        });
    }

    // Catalogue du moteur intégré : rafraîchi au démarrage puis toutes les 12 h.
    {
        std::thread::spawn(move || loop {
            crate::moteur::refresh_if_old();
            std::thread::sleep(Duration::from_secs(3600));
        });
    }

    // Nouveaux mails : signalés dans une bulle (seulement ceux arrivés depuis le lancement).
    {
        let tx = tx.clone();
        std::thread::spawn(move || {
            let mut seen: std::collections::HashMap<String, u64> = Default::default();
            loop {
                let cfg = Config::load();
                if cfg.alertes.mails {
                    for c in &cfg.mail {
                        let Ok(list) = crate::mail::list(c, true, 8) else { continue };
                        let max = list.iter().map(|e| e.uid).max().unwrap_or(0);
                        let first = !seen.contains_key(&c.adresse);
                        let last = seen.get(&c.adresse).copied().unwrap_or(max);
                        let fresh: Vec<&crate::mail::Entete> = list.iter().filter(|e| e.uid > last).collect();
                        seen.insert(c.adresse.clone(), max.max(last));
                        if first || fresh.is_empty() {
                            continue;
                        }
                        let who = |e: &crate::mail::Entete| e.de.split('<').next().unwrap_or(&e.de).trim().trim_matches('"').to_string();
                        let titre = if fresh.len() == 1 { t!("Nouveau mail").to_string() } else { t!("{} nouveaux mails", fresh.len()) };
                        let texte = fresh.iter().take(3).map(|e| format!("{}{} {}", who(e), if crate::lang::en() { ":" } else { " :" }, e.sujet)).collect::<Vec<_>>().join("\n");
                        let m = crate::conseils::Message::new(crate::conseils::Kind::Mail, titre, texte).with_link(format!("mails:{}", c.adresse));
                        let _ = tx.send(Msg::Bubble(m, true));
                    }
                }
                std::thread::sleep(Duration::from_secs(600));
            }
        });
    }

    // Agenda : prévenir avant chaque rendez-vous.
    {
        let tx = tx.clone();
        std::thread::spawn(move || {
            let mut warned: std::collections::HashSet<String> = Default::default();
            let mut cache: (std::time::Instant, Vec<crate::agenda::Evenement>) = (std::time::Instant::now() - Duration::from_secs(3600), vec![]);
            loop {
                let cfg = Config::load();
                if cfg.alertes.agenda && !cfg.agenda.is_empty() {
                    if cache.0.elapsed() > Duration::from_secs(1800) {
                        if let Ok(evs) = crate::agenda::upcoming(&cfg.agenda, 2) {
                            cache = (std::time::Instant::now(), evs);
                        }
                    }
                    let now = chrono::Local::now();
                    let avance = chrono::Duration::minutes(cfg.alertes.avance_min as i64);
                    for e in cache.1.iter().filter(|e| !e.journee && e.debut > now && e.debut - now <= avance) {
                        let key = format!("{}|{}", e.debut, e.titre);
                        if warned.insert(key) {
                            let mins = (e.debut - now).num_minutes().max(1);
                            let lieu = if e.lieu.is_empty() { String::new() } else { format!(" — {}", e.lieu) };
                            let m = crate::conseils::Message::new(
                                crate::conseils::Kind::Reminder,
                                t!("Dans {} min", mins),
                                format!("{} · {}{lieu}", crate::fr::heure(&e.debut), e.titre),
                            );
                            let _ = tx.send(Msg::Bubble(m, true));
                        }
                    }
                }
                std::thread::sleep(Duration::from_secs(300));
            }
        });
    }

    // Fenêtre active (script KWin), « Ne pas déranger », micro : pour rester discret.
    {
        let t1 = tx.clone();
        crate::bureau::demarrer(move |m| {
            let _ = t1.send(m);
        });
        let t3 = std::sync::Mutex::new(tx.clone());
        crate::bureau::raccourci(&cfg.maki.raccourci, move |m| {
            if let Ok(t) = t3.lock() {
                let _ = t.send(m);
            }
        });
        let t2 = std::sync::Mutex::new(tx.clone());
        crate::bureau::veiller(move |m| {
            if let Ok(t) = t2.lock() {
                let _ = t.send(m);
            }
        });
    }

    // Écran où l'on travaille : le maki peut l'y rejoindre.
    {
        let tx = tx.clone();
        std::thread::spawn(move || {
            let mut last = String::new();
            loop {
                if let Some(n) = crate::kde::active_output() {
                    if n != last {
                        last = n.clone();
                        let _ = tx.send(Msg::ActiveOutput(n));
                    }
                }
                std::thread::sleep(Duration::from_millis(700));
            }
        });
    }

    // Rappels (vérifiés toutes les 15 s).
    {
        let tx = tx.clone();
        std::thread::spawn(move || {
            loop {
                for r in reminders::take_due(chrono::Local::now()) {
                    let _ = tools::notify(t!("Rappel de L Katta"), &r.texte);
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
    crate::bureau::retirer_script();
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
                    let m = Message::new(Kind::News, t!("Nouvelle Arch Linux"), t!("« {} » — à lire avant la prochaine mise à jour.", n.titre));
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
