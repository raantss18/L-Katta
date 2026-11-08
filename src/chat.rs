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

/// Catégories du sélecteur (clé du catalogue de Tontolo, libellé).
pub const CATS: [(&str, &str); 7] = [
    ("chat", "Discussion"),
    ("code", "Code"),
    ("math", "Maths"),
    ("writing", "Rédaction"),
    ("reasoning", "Raisonnement"),
    ("vision", "Vision"),
    ("search", "Recherche"),
];

/// Un clic dans le sélecteur de modèles.
#[derive(Debug, Clone, PartialEq)]
pub enum Pick {
    Open,
    Close,
    Engine(usize),
    Cat(usize),
    /// `None` = automatique (le moteur choisit).
    Model(Option<String>),
}

/// Sélecteur de modèles affiché dans la bulle.
pub struct Picker {
    /// Indice du moteur (0 = premier).
    pub moteur: usize,
    pub cat: usize,
    pub scroll: usize,
    pub models: Option<Result<Vec<crate::brain::ModelInfo>, String>>,
    rx: Option<Receiver<Result<Vec<crate::brain::ModelInfo>, String>>>,
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
    pub picker: Option<Picker>,
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
            picker: None,
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
                "/modeles (choisir le modèle) · /reglages · /regle · /especes · /moteurs · /maj · /rappels · /presse · /etat · /sante · /apps · /nouveau (efface) · /retiens <fait>. \
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
            "/modeles" if rest.trim().is_empty() => {
                self.open_picker(0);
                return Some(Entry::Info("Choisis un modèle dans la liste (clic), ou « Automatique ».".into()));
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
            "/especes" => Entry::Maki(format!(
                "{}\n\nPour changer : /regle espece sifaka (ou indri, ayeaye, microcebe, vari, catta).",
                crate::mascot::especes::liste()
            )),
            "/maj" => self.background(|| match crate::maj::check() {
                Ok(Some(r)) => Entry::Maki(format!("Version {} disponible (tu as {}). Tape « lkatta maj » ou clique « Mettre à jour » quand je te le propose.", r.version, crate::maj::current())),
                Ok(None) => Entry::Maki(format!("Tu as la dernière version ({}).", crate::maj::current())),
                Err(e) => Entry::Error(format!("Vérification impossible : {e}")),
            }),
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
                        let _ = crate::install::sync_autostart(cfg.demarrage_auto);
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

    /// Moteurs actifs (indice dans la configuration, nom court).
    pub fn engines(&self) -> Vec<(usize, String)> {
        self.cfg
            .moteur
            .iter()
            .enumerate()
            .filter(|(_, m)| m.actif)
            .map(|(i, m)| (i, m.nom.split(" (").next().unwrap_or(&m.nom).to_string()))
            .collect()
    }

    pub fn current_model(&self, n: usize) -> String {
        self.cfg.moteur.get(n).map(|m| m.modele.clone()).unwrap_or_default()
    }

    /// Le moteur sait-il choisir tout seul (Tontolo) ?
    pub fn has_auto(&self, n: usize) -> bool {
        self.cfg.moteur.get(n).is_some_and(|m| m.url == "integre" || m.nom.to_lowercase().contains("tontolo") || m.url.contains(":8765"))
    }

    /// Ouvre le sélecteur sur un moteur et charge sa liste en arrière-plan.
    pub fn open_picker(&mut self, moteur: usize) {
        let moteur = moteur.min(self.cfg.moteur.len().saturating_sub(1));
        let cat = self.picker.as_ref().map(|p| p.cat).unwrap_or(0);
        let (tx, rx) = mpsc::channel();
        if let Some(m) = self.cfg.moteur.get(moteur).cloned() {
            std::thread::spawn(move || {
                let _ = tx.send(crate::brain::catalog(&m));
            });
        }
        self.picker = Some(Picker { moteur, cat, scroll: 0, models: None, rx: Some(rx) });
        self.open = true;
    }

    /// Modèles du moteur affiché, triés par qualité dans la catégorie choisie.
    pub fn picker_rows(&self) -> Vec<&crate::brain::ModelInfo> {
        let Some(p) = &self.picker else { return vec![] };
        let Some(Ok(list)) = &p.models else { return vec![] };
        let cat = CATS[p.cat.min(CATS.len() - 1)].0;
        let mut v: Vec<&crate::brain::ModelInfo> = list.iter().collect();
        if v.iter().any(|m| !m.quality.is_empty()) {
            // D'abord ceux qui peuvent agir (outils) et sont disponibles, puis par qualité.
            v.sort_by(|a, b| {
                (a.pause.is_some(), !a.tools, -a.score(cat))
                    .partial_cmp(&(b.pause.is_some(), !b.tools, -b.score(cat)))
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }
        v
    }

    pub fn pick(&mut self, action: Pick) {
        match action {
            Pick::Open => {
                let first = self.engines().first().map(|(i, _)| *i).unwrap_or(0);
                self.open_picker(first)
            }
            Pick::Close => self.picker = None,
            Pick::Engine(n) => self.open_picker(n),
            Pick::Cat(c) => {
                if let Some(p) = &mut self.picker {
                    p.cat = c;
                    p.scroll = 0;
                }
            }
            Pick::Model(key) => {
                let Some(n) = self.picker.as_ref().map(|p| p.moteur) else { return };
                let value = key.clone().unwrap_or_else(|| "auto".into());
                let mut cfg = Config::load();
                match cfg.set_path(&format!("moteur.{}.modele", n + 1), &value) {
                    Ok(_) => {
                        let nom = self.engines().into_iter().find(|(i, _)| *i == n).map(|(_, s)| s).unwrap_or_default();
                        self.cfg = cfg;
                        self.signals.push(Signal::ConfigChanged);
                        self.picker = None;
                        let what = if key.is_none() { format!("{nom} choisit lui-même le meilleur modèle") } else { format!("{nom} utilisera {value}") };
                        self.entries.push(Entry::Info(format!("C'est noté : {what}.")));
                    }
                    Err(e) => self.entries.push(Entry::Error(e)),
                }
            }
        }
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
        if let Some(p) = &mut self.picker {
            if let Some(rx) = &p.rx {
                if let Ok(r) = rx.try_recv() {
                    p.models = Some(r);
                    p.rx = None;
                }
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picking_a_model_saves_it() {
        let dir = std::env::temp_dir().join(format!("lkatta-pick-{}", std::process::id()));
        // SAFETY : seul test du binaire qui lit ou écrit la configuration.
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", &dir);
            std::env::set_var("XDG_DATA_HOME", &dir);
        }
        let mut chat = Chat::new(Config::load());
        chat.picker = Some(Picker { moteur: 1, cat: 0, scroll: 0, models: Some(Ok(vec![])), rx: None });
        chat.pick(Pick::Model(Some("qwen/qwen3-coder:free".into())));
        assert!(chat.picker.is_none());
        assert_eq!(Config::load().moteur[1].modele, "qwen/qwen3-coder:free");
        assert!(chat.poll().contains(&Signal::ConfigChanged));
        chat.picker = Some(Picker { moteur: 0, cat: 0, scroll: 0, models: Some(Ok(vec![])), rx: None });
        chat.pick(Pick::Model(None));
        assert_eq!(Config::load().moteur[0].modele, "auto");
        let _ = std::fs::remove_dir_all(dir);
    }
}
