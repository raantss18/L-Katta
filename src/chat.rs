//! Discussion dans la bulle du maki : pas de fenêtre, le cerveau tourne dans un fil
//! du démon et la bulle affiche l'échange, les actions et les confirmations.

use std::sync::mpsc::{self, Receiver, Sender};

use serde_json::{Value, json};

use crate::brain::{self, Brain, Event, Host};
use crate::config::Config;
use crate::{clipboard, memoire, tools};

#[derive(Debug, Clone, PartialEq)]
pub enum Entry {
    User(String),
    Maki(String),
    Info(String),
    Error(String),
}

enum Work {
    Ev(Event),
    Confirm(String, Sender<bool>),
    Done(Result<String, String>, Vec<Value>),
}

/// Ce que la bulle signale au maki après un `poll`.
#[derive(Debug, Clone, PartialEq)]
pub enum Signal {
    /// Une réponse est arrivée (le maki peut saluer si la bulle est fermée).
    Answered(String),
    /// Une confirmation est demandée.
    NeedsConfirm,
    /// Un réglage a changé : la surcouche recharge la configuration.
    ConfigChanged,
}

struct WorkHost(Sender<Work>);

impl Host for WorkHost {
    fn confirm(&mut self, q: &str) -> bool {
        let (tx, rx) = mpsc::channel();
        let _ = self.0.send(Work::Confirm(q.to_string(), tx));
        rx.recv().unwrap_or(false)
    }
    fn event(&mut self, e: Event) {
        let _ = self.0.send(Work::Ev(e));
    }
}

pub struct Chat {
    pub open: bool,
    pub input: String,
    pub entries: Vec<Entry>,
    /// Défilement depuis le bas, en lignes.
    pub scroll: usize,
    pub busy: bool,
    pub confirm: Option<String>,
    confirm_tx: Option<Sender<bool>>,
    pub engine: String,
    history: Vec<Value>,
    rx: Option<Receiver<Work>>,
    cfg: Config,
    /// Résultats des commandes « / » lentes (réseau), calculées en arrière-plan.
    side_tx: Sender<Entry>,
    side_rx: Receiver<Entry>,
    signals: Vec<Signal>,
}

impl Chat {
    pub fn new(cfg: Config) -> Self {
        let (side_tx, side_rx) = mpsc::channel();
        let mut entries = vec![];
        let mut history = vec![];
        for (q, r) in memoire::derniers(3) {
            entries.push(Entry::User(q.clone()));
            entries.push(Entry::Maki(r.clone()));
            history.push(json!({"role": "user", "content": q}));
            history.push(json!({"role": "assistant", "content": r}));
        }
        Self {
            open: false,
            input: String::new(),
            entries,
            scroll: 0,
            busy: false,
            confirm: None,
            confirm_tx: None,
            engine: String::new(),
            history,
            rx: None,
            cfg,
            side_tx,
            side_rx,
            signals: vec![],
        }
    }

    pub fn toggle(&mut self) {
        self.open = !self.open;
        self.scroll = 0;
        if self.open && self.entries.is_empty() {
            self.entries.push(Entry::Maki(format!(
                "Salut {} ! Demande-moi ce que tu veux : un fichier, l'état de la machine, \
                 l'actualité, un rappel, ouvrir une appli… (/aide pour les raccourcis)",
                self.cfg.prenom
            )));
        }
    }

    /// Réponse à une confirmation (bouton Oui/Non).
    pub fn answer(&mut self, yes: bool) {
        if let Some(tx) = self.confirm_tx.take() {
            let _ = tx.send(yes);
            let q = self.confirm.take().unwrap_or_default();
            self.entries.push(Entry::Info(format!("{} : {}", if yes { "Accepté" } else { "Refusé" }, q.lines().next().unwrap_or(""))));
        }
    }

    /// Envoie la saisie : commandes « / » traitées sur place, le reste au cerveau.
    pub fn submit(&mut self) {
        let text = self.input.trim().to_string();
        self.input.clear();
        self.scroll = 0;
        if text.is_empty() || self.busy {
            return;
        }
        if let Some(r) = self.local_command(&text) {
            self.entries.push(Entry::User(text));
            self.entries.push(r);
            return;
        }
        self.entries.push(Entry::User(text.clone()));
        self.history.push(json!({"role": "user", "content": text}));
        if self.history.len() > 30 {
            let cut = self.history.len() - 30;
            self.history.drain(..cut);
            while self.history.first().is_some_and(|m| m["role"] != "user") {
                self.history.remove(0);
            }
        }
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        self.busy = true;
        let mut hist = self.history.clone();
        let cfg = self.cfg.clone();
        std::thread::spawn(move || {
            let mut host = WorkHost(tx.clone());
            let r = Brain::new(cfg).reply(&mut hist, &mut host);
            let _ = tx.send(Work::Done(r, hist));
        });
    }

    fn local_command(&mut self, t: &str) -> Option<Entry> {
        let (cmd, rest) = t.split_once(' ').unwrap_or((t, ""));
        Some(match cmd {
            "/aide" => Entry::Info(
                "/reglages · /regle · /modeles · /moteurs · /rappels · /presse · /etat · /sante · /apps · /nouveau (efface) · /retiens <fait>. \
                 Clic sur une réponse = copiée. Échap = fermer."
                    .into(),
            ),
            "/rappels" => Entry::Maki(tools::list_reminders()),
            "/presse" => {
                let l = clipboard::load();
                Entry::Maki(if l.is_empty() {
                    "Presse-papiers vide.".into()
                } else {
                    l.iter().take(8).map(|e| format!("• {}", e.texte.chars().take(80).collect::<String>().replace('\n', " "))).collect::<Vec<_>>().join("\n")
                })
            }
            "/etat" => Entry::Maki(tools::system_status()),
            "/sante" => Entry::Maki(crate::sante::bilan_texte()),
            "/apps" => Entry::Maki(crate::apps::list_text()),
            "/moteurs" => {
                let cfg = self.cfg.clone();
                self.background(move || {
                    Entry::Maki(
                        Brain::new(cfg)
                            .ping_all()
                            .into_iter()
                            .enumerate()
                            .map(|(i, (n, r))| match r {
                                Ok(s) => format!("✓ {} {n} : {s}", i + 1),
                                Err(e) => format!("✗ {} {n} : {e}", i + 1),
                            })
                            .collect::<Vec<_>>()
                            .join("\n"),
                    )
                })
            }
            "/modeles" => {
                // /modeles [n° du moteur] [filtre]
                let mut it = rest.split_whitespace();
                let first = it.next().unwrap_or("");
                let (n, filtre) = match first.parse::<usize>() {
                    Ok(n) => (n, it.collect::<Vec<_>>().join(" ")),
                    Err(_) => (1, rest.trim().to_string()),
                };
                let Some(m) = self.cfg.moteur.get(n.saturating_sub(1)).cloned() else {
                    return Some(Entry::Error(format!("pas de moteur {n} (/reglages)")));
                };
                self.background(move || match crate::brain::list_models(&m) {
                    Ok(list) => {
                        let f = filtre.to_lowercase();
                        let hits: Vec<&String> = list.iter().filter(|x| f.is_empty() || x.to_lowercase().contains(&f)).collect();
                        let shown = hits.iter().take(40).map(|x| x.as_str()).collect::<Vec<_>>().join(", ");
                        Entry::Maki(format!(
                            "{} : {} modèle(s){}.\n{shown}{}\nPour choisir : /regle moteur {n} modele <nom>",
                            m.nom,
                            hits.len(),
                            if f.is_empty() { String::new() } else { format!(" contenant « {f} »") },
                            if hits.len() > 40 { " …" } else { "" }
                        ))
                    }
                    Err(e) => Entry::Error(format!("{} : {e}", m.nom)),
                })
            }
            "/reglages" => Entry::Maki(format!(
                "{}\n\nPour changer : /regle zone libre · /regle moteur 2 modele <nom> · \
                 /regle moteur 3 actif non · /regle sommeil 23,7 · /regle numeros_min 10. \
                 Liste des modèles : /modeles <n° du moteur> [filtre].",
                Config::load().summary()
            )),
            "/regle" => {
                let toks: Vec<&str> = rest.split_whitespace().collect();
                let k = if toks.first() == Some(&"moteur") { 3 } else { 1 };
                if toks.len() <= k {
                    return Some(Entry::Error("usage : /regle <réglage> <valeur> (voir /reglages)".into()));
                }
                let mut cfg = Config::load();
                match cfg.set_path(&toks[..k].join("."), &toks[k..].join(" ")) {
                    Ok(msg) => {
                        self.cfg = cfg;
                        self.signals.push(Signal::ConfigChanged);
                        Entry::Info(format!("Réglé : {msg}"))
                    }
                    Err(e) => Entry::Error(e),
                }
            }
            "/retiens" if !rest.is_empty() => Entry::Maki(crate::memoire::retenir(rest).unwrap_or_else(|e| e)),
            "/nouveau" => {
                self.entries.clear();
                self.history.clear();
                Entry::Info("Nouvelle discussion.".into())
            }
            _ => return None,
        })
    }

    /// Lance une commande lente dans un fil ; son résultat arrivera dans la bulle.
    fn background(&self, f: impl FnOnce() -> Entry + Send + 'static) -> Entry {
        let tx = self.side_tx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(f());
        });
        Entry::Info("Je regarde…".into())
    }

    /// Recharge la configuration (après un changement de réglage).
    pub fn reload(&mut self, cfg: Config) {
        self.cfg = cfg;
    }

    /// Relève le travail du cerveau ; à appeler à chaque image.
    pub fn poll(&mut self) -> Vec<Signal> {
        let mut out = std::mem::take(&mut self.signals);
        while let Ok(e) = self.side_rx.try_recv() {
            self.entries.push(e);
            self.scroll = 0;
        }
        let Some(rx) = &self.rx else { return out };
        let mut done = None;
        while let Ok(w) = rx.try_recv() {
            match w {
                Work::Ev(Event::Engine(e)) => self.engine = e,
                Work::Ev(Event::Tool(_, s)) => self.entries.push(Entry::Info(format!("› {s}"))),
                Work::Ev(Event::ToolDone(..)) => {}
                Work::Confirm(q, tx) => {
                    self.confirm = Some(q);
                    self.confirm_tx = Some(tx);
                    out.push(Signal::NeedsConfirm);
                }
                Work::Done(r, h) => done = Some((r, h)),
            }
        }
        if let Some((r, hist)) = done {
            self.rx = None;
            self.busy = false;
            self.scroll = 0;
            match r {
                Ok(t) => {
                    let question = self.history.last().and_then(|m| m["content"].as_str()).unwrap_or("").to_string();
                    let t = if t.is_empty() { "(réponse vide)".to_string() } else { t };
                    memoire::journaliser(&question, &t);
                    self.history = hist;
                    self.entries.push(Entry::Maki(t.clone()));
                    out.push(Signal::Answered(t));
                }
                Err(e) => self.entries.push(Entry::Error(e)),
            }
            if self.entries.len() > 60 {
                let cut = self.entries.len() - 60;
                self.entries.drain(..cut);
            }
        }
        out
    }
}

/// Mode terminal : `lkatta demander "…"`.
pub fn ask_cli(question: &str) -> Result<(), String> {
    struct Term;
    impl Host for Term {
        fn confirm(&mut self, q: &str) -> bool {
            eprint!("{q} [o/N] ");
            let mut s = String::new();
            let _ = std::io::stdin().read_line(&mut s);
            matches!(s.trim(), "o" | "O" | "oui" | "y")
        }
        fn event(&mut self, e: Event) {
            match e {
                Event::Engine(n) => eprintln!("· moteur : {n}"),
                Event::Tool(_, s) => eprintln!("· {s}"),
                Event::ToolDone(..) => {}
            }
        }
    }
    let mut hist = vec![json!({"role": "user", "content": question})];
    let r = Brain::new(Config::load()).reply(&mut hist, &mut Term)?;
    memoire::journaliser(question, &r);
    println!("{}", brain::clean(&r));
    Ok(())
}
