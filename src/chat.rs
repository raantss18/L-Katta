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
    pub input: crate::saisie::Saisie,
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
    pub panneau: Option<Panneau>,
    /// Actions faites pendant la question en cours (journalisées avec la réponse).
    etapes: Vec<String>,
    /// Réponse en train d'arriver (brute, avant nettoyage).
    pub flux: String,
    /// Façon de traiter les demandes (discussion, recherche approfondie, agent).
    pub mode: brain::Mode,
    /// Une page ou un fichier vient d'être ouvert pour qu'on y copie quelque chose : la
    /// bulle reste ouverte quand elle perd le clavier, jusqu'au retour.
    pub retour_attendu: bool,
}

impl Chat {
    pub fn new(cfg: Config) -> Self {
        let (side_tx, side_rx) = mpsc::channel();
        let mut entries = vec![];
        let mut history = vec![];
        let mut echanges = memoire::derniers(3);
        if let Some(e) = memoire::en_cours_repris() {
            echanges.push(e);
            if echanges.len() > 3 {
                echanges.remove(0);
            }
        }
        for e in echanges {
            entries.push(Entry::User(e.q.clone()));
            for s in &e.etapes {
                entries.push(Entry::Info(s.clone()));
            }
            if e.r == memoire::INTERROMPU {
                entries.push(Entry::Error(crate::lang::trd(&e.r).to_string()));
            } else {
                entries.push(Entry::Maki(e.r.clone()));
            }
            history.push(json!({"role": "user", "content": e.q}));
            let r = if e.etapes.is_empty() { e.r } else { format!("[actions faites : {}]\n{}", e.etapes.join(" ; "), e.r) };
            history.push(json!({"role": "assistant", "content": r}));
        }
        Self {
            open: false,
            input: Default::default(),
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
            panneau: None,
            etapes: vec![],
            flux: String::new(),
            mode: brain::Mode::Discussion,
            retour_attendu: false,
        }
    }

    pub fn toggle(&mut self) {
        self.open = !self.open;
        self.scroll = 0;
        if self.open && self.entries.is_empty() {
            self.entries.push(Entry::Maki(t!(
                "Salut {} ! Demande-moi ce que tu veux : un fichier, l'état de la machine, l'actualité, un rappel, ouvrir une appli… (/aide pour les raccourcis)",
                self.cfg.prenom
            )));
        }
    }

    /// Réponse à une confirmation (bouton Oui/Non).
    pub fn answer(&mut self, yes: bool) {
        if let Some(tx) = self.confirm_tx.take() {
            let _ = tx.send(yes);
            let q = self.confirm.take().unwrap_or_default();
            self.entries.push(Entry::Info(format!("{} : {}", if yes { t!("Accepté") } else { t!("Refusé") }, q.lines().next().unwrap_or(""))));
        }
    }

    /// Envoie la saisie : commandes « / » traitées sur place, le reste au cerveau.
    pub fn submit(&mut self) {
        let text = self.input.prendre().trim().to_string();
        self.scroll = 0;
        if text.is_empty() || self.busy {
            return;
        }
        // « /cherche question » et « /agent tâche » : ce mode pour cette seule demande.
        let (cmd, rest) = text.split_once(' ').map(|(c, r)| (c, r.trim())).unwrap_or((text.as_str(), ""));
        let ponctuel = match cmd {
            "/cherche" | "/recherche" | "/research" | "/deep" if !rest.is_empty() => Some(brain::Mode::Recherche),
            "/agent" if !rest.is_empty() => Some(brain::Mode::Agent),
            _ => None,
        };
        if let Some(mode) = ponctuel {
            let q = rest.to_string();
            self.entries.push(Entry::User(text.clone()));
            self.envoyer(q, mode);
            return;
        }
        if let Some(r) = self.local_command(&text) {
            self.entries.push(Entry::User(text));
            self.entries.push(r);
            return;
        }
        self.entries.push(Entry::User(text.clone()));
        let mode = self.mode;
        self.envoyer(text, mode);
    }

    /// Change de mode (puce de la bulle) : discussion → recherche → agent → discussion.
    pub fn mode_suivant(&mut self) {
        self.mode = match self.mode {
            brain::Mode::Discussion => brain::Mode::Recherche,
            brain::Mode::Recherche => brain::Mode::Agent,
            brain::Mode::Agent => brain::Mode::Discussion,
        };
        self.entries.push(Entry::Info(mode_aide(self.mode).into()));
        self.scroll = 0;
    }

    /// Envoie une demande au cerveau, dans un fil.
    fn envoyer(&mut self, text: String, mode: brain::Mode) {
        self.history.push(json!({"role": "user", "content": text}));
        if self.history.len() > 30 {
            let cut = self.history.len() - 30;
            self.history.drain(..cut);
            while self.history.first().is_some_and(|m| m["role"] != "user") {
                self.history.remove(0);
            }
        }
        memoire::en_cours_debut(&text);
        self.etapes.clear();
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        self.busy = true;
        let mut hist = self.history.clone();
        let cfg = self.cfg.clone();
        std::thread::spawn(move || {
            let mut host = WorkHost(tx.clone());
            let r = Brain::new(cfg).with_mode(mode).reply(&mut hist, &mut host);
            let _ = tx.send(Work::Done(r, hist));
        });
    }

    fn local_command(&mut self, t: &str) -> Option<Entry> {
        let (cmd, rest) = t.split_once(' ').unwrap_or((t, ""));
        // Noms anglais des commandes.
        let cmd = match cmd {
            "/help" => "/aide",
            "/settings" => "/reglages",
            "/models" => "/modeles",
            "/history" => "/historique",
            "/about" => "/apropos",
            "/mail" => "/mails",
            "/calendar" => "/agenda",
            "/species" => "/especes",
            "/update" => "/maj",
            "/new" => "/nouveau",
            "/remember" => "/retiens",
            "/reminders" => "/rappels",
            "/clipboard" => "/presse",
            "/status" => "/etat",
            "/health" => "/sante",
            "/engines" => "/moteurs",
            "/set" => "/regle",
            "/welcome" | "/setup" => "/accueil",
            "/recherche" | "/research" | "/deep" => "/cherche",
            c => c,
        };
        Some(match cmd {
            "/aide" => Entry::Info(
                t!("/historique [mots] · /mails · /agenda · /modeles (choisir le modèle) · /reglages · /regle · /especes · /moteurs · /maj · /rappels · /presse · /etat · /sante · /apps · /nouveau (efface) · /retiens <fait> · /accueil (premiers réglages) · /cherche [question] (recherche approfondie) · /agent [tâche] (mode agent) · /local (modèle hors ligne). Clic sur une réponse = copiée. Flèches, Maj, Ctrl+A/C/X/V dans la saisie. Échap = fermer.")
                    .into(),
            ),
            "/rappels" => Entry::Maki(tools::list_reminders()),
            "/cherche" => {
                self.mode = if self.mode == brain::Mode::Recherche { brain::Mode::Discussion } else { brain::Mode::Recherche };
                Entry::Info(mode_aide(self.mode).into())
            }
            "/agent" => {
                self.mode = if self.mode == brain::Mode::Agent { brain::Mode::Discussion } else { brain::Mode::Agent };
                Entry::Info(mode_aide(self.mode).into())
            }
            "/local" => {
                let m = crate::local::machine();
                let c = crate::local::conseil(&m);
                let etat = match crate::local::installes() {
                    Some(l) if l.iter().any(|x| x == c.tag || x.starts_with(&format!("{}:", c.tag))) => t!("Il est déjà installé : il me sert de secours sans Internet.").to_string(),
                    Some(_) => t!("Ollama tourne ; le modèle n'est pas encore là : réglages › Modèles d'IA › Télécharger.").to_string(),
                    None if crate::local::ollama_installe() => t!("Ollama est installé mais arrêté ; je le démarrerai au téléchargement (réglages › Modèles d'IA).").to_string(),
                    None => t!("Il faut d'abord Ollama (ollama.com) ; ensuite réglages › Modèles d'IA › Télécharger.").to_string(),
                };
                Entry::Maki(t!("Ta machine : {}.
Modèle local conseillé : {} ({} Go à télécharger).
{}", crate::local::resume(&m), c.tag, format!("{:.1}", c.poids_go), etat))
            }
            "/accueil" => {
                self.open_accueil();
                return Some(Entry::Info(t!("Assistant de premier lancement ouvert.").into()));
            }
            "/apropos" => Entry::Maki(crate::apropos::texte()),
            "/historique" | "/hist" => Entry::Maki(if rest.trim().is_empty() { memoire::recents(8) } else { memoire::chercher(rest) }),
            "/presse" => {
                let l = clipboard::load();
                Entry::Maki(if l.is_empty() {
                    t!("Presse-papiers vide.").into()
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
                return Some(Entry::Info(t!("Choisis un modèle dans la liste (clic), ou « Automatique ».").into()));
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
                    return Some(Entry::Error(t!("pas de moteur {} (/reglages)", n)));
                };
                self.background(move || match crate::brain::list_models(&m) {
                    Ok(list) => {
                        let f = filtre.to_lowercase();
                        let hits: Vec<&String> = list.iter().filter(|x| f.is_empty() || x.to_lowercase().contains(&f)).collect();
                        let shown = hits.iter().take(40).map(|x| x.as_str()).collect::<Vec<_>>().join(", ");
                        Entry::Maki(t!(
                            "{} : {} modèle(s){}.\n{}{}\nPour choisir : /regle moteur {} modele <nom>",
                            m.nom,
                            hits.len(),
                            if f.is_empty() { String::new() } else { t!(" contenant « {} »", f) },
                            shown,
                            if hits.len() > 40 { " …" } else { "" },
                            n
                        ))
                    }
                    Err(e) => Entry::Error(format!("{} : {e}", m.nom)),
                })
            }
            "/mails" => {
                let comptes = self.cfg.mail.clone();
                if comptes.is_empty() {
                    self.open_settings(4);
                    return Some(Entry::Info(t!("Aucun compte mail : branche-le dans l'onglet Mails (deux copier-coller).").into()));
                }
                self.background(move || {
                    let mut out = vec![];
                    for c in &comptes {
                        match crate::mail::list(c, true, 8) {
                            Ok(l) if l.is_empty() => out.push(t!("{} : aucun mail non lu.", c.adresse)),
                            Ok(l) => {
                                out.push(t!("{} — {} non lu(s) :", c.adresse, l.len()));
                                for e in l {
                                    let who = e.de.split('<').next().unwrap_or(&e.de).trim().trim_matches('"').to_string();
                                    out.push(format!("• {who} : {}", e.sujet));
                                }
                            }
                            Err(e) => out.push(format!("{} : {e}", c.adresse)),
                        }
                    }
                    Entry::Maki(out.join("\n"))
                })
            }
            "/agenda" => {
                let sources = self.cfg.agenda.clone();
                if sources.is_empty() {
                    self.open_settings(4);
                    return Some(Entry::Info(t!("Aucun agenda : branche-le dans l'onglet Mails (un copier-coller).").into()));
                }
                self.background(move || match crate::agenda::upcoming(&sources, 7) {
                    Ok(evs) => Entry::Maki(crate::agenda::text(&evs)),
                    Err(e) => Entry::Error(e),
                })
            }
            "/especes" => Entry::Maki(t!(
                "{}\n\nPour changer : /regle espece sifaka (ou indri, ayeaye, microcebe, vari, catta).",
                crate::mascot::especes::liste()
            )),
            "/maj" => self.background(|| match crate::maj::check() {
                Ok(Some(r)) => Entry::Maki(t!("Version {} disponible (tu as {}). Tape « lkatta maj » ou clique « Mettre à jour » quand je te le propose.", r.version, crate::maj::current())),
                Ok(None) => Entry::Maki(t!("Tu as la dernière version ({}).", crate::maj::current())),
                Err(e) => Entry::Error(t!("Vérification impossible : {}", e)),
            }),
            "/reglages" if rest.trim().is_empty() => {
                self.open_settings(0);
                return Some(Entry::Info(t!("Réglages ouverts (roue dentée en haut de la bulle).").into()));
            }
            "/reglages" => Entry::Maki(t!(
                "{}\n\nPour changer : /regle zone libre · /regle moteur 2 modele <nom> · /regle moteur 3 actif non · /regle sommeil 23,7 · /regle numeros_min 10. Liste des modèles : /modeles <n° du moteur> [filtre].",
                Config::load().summary()
            )),
            "/regle" => {
                let toks: Vec<&str> = rest.split_whitespace().collect();
                let k = if toks.first() == Some(&"moteur") { 3 } else { 1 };
                if toks.len() <= k {
                    return Some(Entry::Error(t!("usage : /regle <réglage> <valeur> (voir /reglages)").into()));
                }
                let mut cfg = Config::load();
                match cfg.set_path(&toks[..k].join("."), &toks[k..].join(" ")) {
                    Ok(msg) => {
                        let _ = crate::install::sync_autostart(cfg.demarrage_auto);
                        self.cfg = cfg;
                        self.signals.push(Signal::ConfigChanged);
                        Entry::Info(t!("Réglé : {}", msg))
                    }
                    Err(e) => Entry::Error(e),
                }
            }
            "/retiens" if !rest.is_empty() => Entry::Maki(crate::memoire::retenir(rest).unwrap_or_else(|e| e)),
            "/nouveau" => {
                self.entries.clear();
                self.history.clear();
                Entry::Info(t!("Nouvelle discussion.").into())
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
        self.panneau = None;
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
                        let what = if key.is_none() { t!("{} choisit lui-même le meilleur modèle", nom) } else { t!("{} utilisera {}", nom, value) };
                        self.entries.push(Entry::Info(t!("C'est noté : {}.", what)));
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
        Entry::Info(t!("Je regarde…").into())
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
        self.poll_panel();
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
                Work::Ev(Event::Delta(d)) => {
                    if d.is_empty() {
                        self.flux.clear();
                    } else {
                        self.flux.push_str(&d);
                        self.scroll = 0;
                    }
                }
                Work::Ev(Event::Tool(_, s)) => {
                    self.flux.clear();
                    let s = format!("› {s}");
                    memoire::en_cours_etape(&s);
                    self.etapes.push(s.clone());
                    self.entries.push(Entry::Info(s));
                }
                Work::Ev(Event::ToolDone(_, apercu)) => {
                    if !apercu.is_empty() {
                        let s = format!("  ↳ {apercu}");
                        memoire::en_cours_etape(&s);
                        self.etapes.push(s.clone());
                        self.entries.push(Entry::Info(s));
                    }
                }
                Work::Confirm(q, tx) => {
                    self.confirm = Some(q);
                    self.confirm_tx = Some(tx);
                    out.push(Signal::NeedsConfirm);
                }
                Work::Done(r, h) => done = Some((r, h)),
            }
        }
        if let Some((r, hist)) = done {
            self.flux.clear();
            self.rx = None;
            self.busy = false;
            self.scroll = 0;
            match r {
                Ok(t) => {
                    let question = self.history.last().and_then(|m| m["content"].as_str()).unwrap_or("").to_string();
                    let t = if t.is_empty() { t!("(réponse vide)").to_string() } else { t };
                    memoire::journaliser_etapes(&question, &t, &self.etapes);
                    self.history = hist;
                    self.entries.push(Entry::Maki(t.clone()));
                    out.push(Signal::Answered(t));
                }
                Err(e) => {
                    let question = self.history.last().and_then(|m| m["content"].as_str()).unwrap_or("").to_string();
                    memoire::journaliser_etapes(&question, &format!("(erreur) {e}"), &self.etapes);
                    self.entries.push(Entry::Error(e));
                }
            }
            memoire::en_cours_fin();
            if self.entries.len() > 60 {
                let cut = self.entries.len() - 60;
                self.entries.drain(..cut);
            }
        }
        out
    }
}

/// Ce que fait un mode, en une phrase.
pub fn mode_aide(m: brain::Mode) -> &'static str {
    match m {
        brain::Mode::Discussion => t!("Mode discussion : je réponds simplement, avec un outil au besoin."),
        brain::Mode::Recherche => t!("Mode recherche approfondie : je cherche sur plusieurs sources, je les lis et je réponds en les citant. (Encore /cherche pour revenir à la discussion.)"),
        brain::Mode::Agent => t!("Mode agent : je fais un plan, j'enchaîne les actions jusqu'au bout et je te rends compte. Je te demande toujours avant de modifier quoi que ce soit. (Encore /agent pour revenir à la discussion.)"),
    }
}

/// Libellé court du mode (puce de la bulle).
pub fn mode_nom(m: brain::Mode) -> &'static str {
    match m {
        brain::Mode::Discussion => t!("Discussion"),
        brain::Mode::Recherche => t!("Recherche"),
        brain::Mode::Agent => t!("Agent"),
    }
}

/// Mode terminal : `lkatta demander "…"`.
pub fn ask_cli(question: &str, mode: brain::Mode) -> Result<(), String> {
    /// Écrit la réponse au fil de l'eau (sans les blocs de réflexion).
    #[derive(Default)]
    struct Term {
        brut: String,
        ecrit: usize,
    }
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
                Event::Delta(d) if d.is_empty() => {
                    if self.ecrit > 0 {
                        println!();
                    }
                    self.brut.clear();
                    self.ecrit = 0;
                }
                Event::Delta(d) => {
                    use std::io::Write;
                    self.brut.push_str(&d);
                    let v = crate::flux::visible(&self.brut);
                    if v.len() > self.ecrit && v.is_char_boundary(self.ecrit) {
                        print!("{}", &v[self.ecrit..]);
                        let _ = std::io::stdout().flush();
                        self.ecrit = v.len();
                    }
                }
            }
        }
    }
    let mut hist = vec![json!({"role": "user", "content": question})];
    let mut term = Term::default();
    let r = Brain::new(Config::load()).with_mode(mode).reply(&mut hist, &mut term)?;
    memoire::journaliser(question, &r);
    if term.ecrit == 0 {
        println!("{}", brain::clean(&r));
    } else {
        println!();
        // Ajouté après le flux (liste des sources d'une recherche approfondie).
        if let Some(i) = r.rfind(&format!("\n\n**{}**", t!("Sources"))) {
            println!("{}", r[i..].trim().replace("**", ""));
        }
    }
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
        // Panneau de réglages : interrupteur, pas, choix.
        chat.open_settings(1);
        let before = Config::load();
        chat.settings_act(Act::Pas("maki.bulle_s", 1.0, 3.0, 60.0));
        chat.settings_act(Act::Bascule("maki.acrobaties"));
        chat.settings_act(Act::Choix("maki.theme", "sombre".into()));
        chat.settings_act(Act::Pas("maki.sommeil.1", 5.0, 0.0, 23.0));
        let after = Config::load();
        assert_eq!(after.maki.bulle_s, before.maki.bulle_s + 1);
        assert_eq!(after.maki.acrobaties, !before.maki.acrobaties);
        assert_eq!(after.maki.theme, "sombre");
        assert_eq!(after.maki.sommeil[0], 23);
        assert!(chat.panneau.as_ref().unwrap().message.as_ref().is_some_and(|m| m.0));
        // Saisie dans un champ.
        chat.settings_act(Act::Focus(Champ::Prenom));
        chat.field_backspace();
        chat.field_input("x");
        assert!(chat.has_field_focus());
        // Assistant de premier lancement : prénom gardé en passant à l'étape suivante, fini une fois pour toutes.
        assert!(!Config::load().accueil);
        chat.open_accueil();
        assert!(chat.en_accueil());
        chat.settings_act(Act::Etape(1));
        chat.panneau.as_mut().unwrap().champs.insert(Champ::Prenom, "Hery".into());
        chat.settings_act(Act::Etape(2));
        assert_eq!(Config::load().prenom, "Hery");
        chat.settings_act(Act::Etape(99));
        assert_eq!(chat.panneau.as_ref().unwrap().accueil, Some(ETAPES.len() - 1));
        chat.settings_act(Act::FinAccueil);
        assert!(!chat.en_accueil() && chat.panneau.is_none());
        assert!(Config::load().accueil);
        let _ = std::fs::remove_dir_all(dir);
    }
}

// --- panneau de réglages ------------------------------------------------------------------

/// Libellés traduits à l'affichage (`crate::lang::trd`).
pub const ONGLETS: [&str; 7] = ["Assistant", "Bulles", "Apparence", "Démarrage", "Mails", "Modèles d'IA", "À propos"];

/// Champ de saisie du panneau.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Champ {
    Prenom,
    MailAdresse,
    MailPasse,
    Agenda,
    Contacts,
    Cle(&'static str),
}

impl Champ {
    pub fn secret(&self) -> bool {
        matches!(self, Champ::MailPasse | Champ::Cle(_))
    }
}

/// Un clic dans le panneau.
#[derive(Debug, Clone, PartialEq)]
pub enum Act {
    Onglet(usize),
    Fermer,
    Focus(Champ),
    /// Bascule un réglage oui/non (chemin du réglage).
    Bascule(&'static str),
    /// Ajoute `pas` à un réglage numérique, borné.
    Pas(&'static str, f64, f64, f64),
    /// Fixe un réglage à une valeur.
    Choix(&'static str, String),
    Enregistrer(Champ),
    OuvrirFichier(&'static str),
    ChoisirModele,
    VerifierMaj,
    TesterMail(usize),
    RetirerMail(usize),
    ObtenirCle(&'static str),
    OuvrirLien(&'static str),
    /// Ouvre une adresse (mail, WhatsApp, site) sans autre consigne.
    Ouvrir(&'static str),
    RetirerCle(&'static str),
    RafraichirCatalogue,
    /// Télécharge le modèle local conseillé (Ollama).
    TelechargerLocal(&'static str),
    /// Assistant de premier lancement : aller à l'étape n.
    Etape(usize),
    /// Assistant de premier lancement terminé.
    FinAccueil,
}

/// Étapes de l'assistant de premier lancement (libellés traduits à l'affichage).
pub const ETAPES: [&str; 6] = ["Langue", "Toi", "Personnage", "Discrétion", "Cerveau", "Mails et agenda"];

pub struct Panneau {
    pub onglet: usize,
    /// Défilement vertical, en pixels logiques (avant mise à l'échelle).
    pub scroll: f32,
    pub champs: std::collections::HashMap<Champ, String>,
    pub focus: Option<Champ>,
    /// Dernier message (succès ?, texte).
    pub message: Option<(bool, String)>,
    rx: Option<Receiver<(bool, String)>>,
    /// Hauteur du contenu mesurée au dernier dessin (px logiques) : la bulle s'y ajuste.
    pub contenu: std::cell::Cell<f32>,
    /// Assistant de premier lancement : étape affichée (sinon, réglages ordinaires).
    pub accueil: Option<usize>,
}

impl Chat {
    pub fn cfg(&self) -> &Config {
        &self.cfg
    }

    pub fn open_settings(&mut self, onglet: usize) {
        self.picker = None;
        let mut champs = std::collections::HashMap::new();
        champs.insert(Champ::Prenom, self.cfg.prenom.clone());
        champs.insert(Champ::Agenda, self.cfg.agenda.first().cloned().unwrap_or_default());
        champs.insert(Champ::Contacts, self.cfg.contacts.first().cloned().unwrap_or_default());
        self.panneau = Some(Panneau { onglet, scroll: 0.0, champs, focus: None, message: None, rx: None, contenu: std::cell::Cell::new(0.0), accueil: None });
        self.open = true;
    }

    /// Ouvre l'assistant de premier lancement (bulle ouverte, première étape).
    pub fn open_accueil(&mut self) {
        self.open_settings(4);
        if let Some(p) = &mut self.panneau {
            p.accueil = Some(0);
        }
        if self.entries.is_empty() {
            self.entries.push(Entry::Maki(t!("Bonjour ! Je suis ton maki. Réglons deux ou trois choses ensemble.").into()));
        }
    }

    pub fn en_accueil(&self) -> bool {
        self.panneau.as_ref().is_some_and(|p| p.accueil.is_some())
    }

    /// La bulle peut-elle se fermer d'elle-même (clic ailleurs) ? Pas pendant une question
    /// Oui/Non, ni pendant l'accueil, ni quand on attend le retour d'une page ouverte.
    pub fn fermable(&self) -> bool {
        self.open && self.confirm.is_none() && !self.en_accueil() && !self.retour_attendu
    }

    /// Fin de l'assistant (terminé ou fermé) : on ne le reproposera plus (/accueil le relance).
    fn fin_accueil(&mut self) {
        self.panneau = None;
        let mut cfg = Config::load();
        if cfg.set_path("accueil", "oui").is_ok() {
            self.cfg = cfg;
        }
        self.signals.push(Signal::ConfigChanged);
        self.entries.push(Entry::Maki(t!(
            "C'est réglé, {} ! Clique sur moi pour me parler, ou fais {}. Tout se change plus tard dans les réglages (roue dentée).",
            self.cfg.prenom,
            self.cfg.maki.raccourci
        )));
        self.scroll = 0;
    }

    pub fn has_field_focus(&self) -> bool {
        self.panneau.as_ref().is_some_and(|p| p.focus.is_some())
    }

    pub fn field_input(&mut self, t: &str) {
        if let Some(p) = &mut self.panneau {
            if let Some(f) = p.focus.clone() {
                let v = p.champs.entry(f).or_default();
                if v.chars().count() < 500 {
                    v.push_str(&t.replace(['\n', '\r'], ""));
                }
            }
        }
    }

    pub fn field_backspace(&mut self) {
        if let Some(p) = &mut self.panneau {
            if let Some(f) = p.focus.clone() {
                p.champs.entry(f).or_default().pop();
            }
        }
    }

    pub fn field_escape(&mut self) {
        if let Some(p) = &mut self.panneau {
            p.focus = None;
        }
    }

    pub fn field_enter(&mut self) {
        if let Some(f) = self.panneau.as_ref().and_then(|p| p.focus.clone()) {
            let f = if f == Champ::MailAdresse { Champ::MailPasse } else { f };
            self.settings_act(Act::Enregistrer(f));
        }
    }

    fn say_panel(&mut self, ok: bool, t: impl Into<String>) {
        if let Some(p) = &mut self.panneau {
            p.message = Some((ok, t.into()));
        }
    }

    /// Travail lent (réseau, trousseau) dans un fil ; le résultat s'affiche dans le panneau.
    fn panel_background(&mut self, wait: &str, f: impl FnOnce() -> (bool, String) + Send + 'static) {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(f());
        });
        if let Some(p) = &mut self.panneau {
            p.rx = Some(rx);
            p.message = Some((true, wait.into()));
        }
    }

    fn apply(&mut self, path: &str, value: &str) {
        let mut cfg = Config::load();
        match cfg.set_path(path, value) {
            Ok(_) => {
                if path == "demarrage_auto" {
                    let _ = crate::install::sync_autostart(cfg.demarrage_auto);
                }
                crate::lang::regler(&cfg.langue);
                self.cfg = cfg;
                self.signals.push(Signal::ConfigChanged);
                let msg = match path {
                    "maki.espece" | "maki.taille" if self.en_accueil() => t!("Enregistré : il prendra cette allure à la fin de l'accueil.").to_string(),
                    "maki.espece" | "maki.taille" => t!("Enregistré : le maki se transforme…").to_string(),
                    _ => t!("Enregistré.").to_string(),
                };
                self.say_panel(true, msg);
            }
            Err(e) => self.say_panel(false, e),
        }
    }

    pub fn settings_act(&mut self, act: Act) {
        let field = |p: &Option<Panneau>, c: &Champ| p.as_ref().and_then(|p| p.champs.get(c).cloned()).unwrap_or_default();
        match act {
            Act::Onglet(i) => {
                if let Some(p) = &mut self.panneau {
                    p.onglet = i.min(ONGLETS.len() - 1);
                    p.scroll = 0.0;
                    p.focus = None;
                    p.message = None;
                }
            }
            Act::Fermer if self.en_accueil() => self.fin_accueil(),
            Act::Fermer => self.panneau = None,
            Act::Etape(i) => {
                // Prénom tapé sans « Entrée » : on le garde quand même.
                let prenom = field(&self.panneau, &Champ::Prenom);
                if !prenom.trim().is_empty() && prenom.trim() != self.cfg.prenom {
                    self.apply("prenom", prenom.trim());
                }
                if let Some(p) = &mut self.panneau {
                    p.accueil = Some(i.min(ETAPES.len() - 1));
                    p.onglet = 4;
                    p.scroll = 0.0;
                    p.focus = None;
                    p.message = None;
                }
            }
            Act::FinAccueil => {
                let prenom = field(&self.panneau, &Champ::Prenom);
                if !prenom.trim().is_empty() && prenom.trim() != self.cfg.prenom {
                    self.apply("prenom", prenom.trim());
                }
                self.fin_accueil()
            }
            Act::Focus(c) => {
                if let Some(p) = &mut self.panneau {
                    p.focus = Some(c);
                }
            }
            Act::Bascule(path) => {
                let v = !self.cfg.get_bool(path);
                self.apply(path, if v { "oui" } else { "non" });
            }
            Act::Pas(path, step, min, max) => {
                let v = (self.cfg.get_num(path) + step).clamp(min, max);
                let v = if step.fract() == 0.0 { format!("{}", v.round() as i64) } else { format!("{:.1}", v) };
                self.apply(path, &v);
            }
            Act::Choix(path, v) => self.apply(path, &v),
            Act::Enregistrer(c) => {
                let v = field(&self.panneau, &c);
                match c.clone() {
                    Champ::Prenom if !v.trim().is_empty() => self.apply("prenom", v.trim()),
                    Champ::Agenda => {
                        self.apply("agenda", v.trim());
                        let sources = self.cfg.agenda.clone();
                        if !sources.is_empty() {
                            self.panel_background(t!("Je lis ton agenda…"), move || match crate::agenda::upcoming(&sources, 7) {
                                Ok(e) => (true, t!("Agenda branché : {} événement(s) dans les 7 prochains jours.", e.len())),
                                Err(e) => (false, t!("Agenda illisible : {}", e)),
                            });
                        }
                    }
                    Champ::Contacts => self.apply("contacts", v.trim()),
                    Champ::MailAdresse | Champ::MailPasse => {
                        let adresse = field(&self.panneau, &Champ::MailAdresse).trim().to_string();
                        let mut pass = field(&self.panneau, &Champ::MailPasse);
                        // Mot de passe d'application Google : 16 lettres affichées par groupes de 4.
                        let compact: String = pass.chars().filter(|c| !c.is_whitespace()).collect();
                        if compact.len() == 16 && compact.chars().all(|c| c.is_ascii_alphabetic()) {
                            pass = compact;
                        }
                        if !adresse.contains('@') || pass.is_empty() {
                            self.say_panel(false, t!("Adresse et mot de passe nécessaires."));
                            return;
                        }
                        let mut cfg = Config::load();
                        if !cfg.mail.iter().any(|m| m.adresse == adresse) {
                            cfg.mail.push(crate::mail::Compte::guess(&adresse));
                            if let Err(e) = cfg.save() {
                                self.say_panel(false, e.to_string());
                                return;
                            }
                        }
                        self.cfg = cfg.clone();
                        if let Some(p) = &mut self.panneau {
                            p.champs.remove(&Champ::MailPasse);
                            p.champs.remove(&Champ::MailAdresse);
                            p.focus = None;
                        }
                        let compte = cfg.mail.iter().find(|m| m.adresse == adresse).cloned().unwrap();
                        self.panel_background(t!("J'enregistre le mot de passe et je teste la connexion…"), move || {
                            if let Err(e) = crate::mail::set_password(&adresse, &pass) {
                                return (false, e);
                            }
                            match crate::mail::list(&compte, true, 3) {
                                Ok(l) => (true, t!("{} : connexion réussie ({} non lu(s) récent(s)).", adresse, l.len())),
                                Err(e) => (false, t!("Enregistré, mais la connexion échoue : {}", e)),
                            }
                        });
                    }
                    Champ::Cle(id) => {
                        let key = v.trim().to_string();
                        if let Some(p) = &mut self.panneau {
                            p.champs.remove(&Champ::Cle(id));
                            p.focus = None;
                        }
                        self.panel_background(t!("J'enregistre la clé et je cherche les modèles…"), move || {
                            if let Err(e) = crate::cles::set_cle(id, &key) {
                                return (false, e);
                            }
                            match crate::moteur::refresh() {
                                Ok(_) => {
                                    let n = crate::moteur::catalogue().modeles.iter().filter(|m| m.provider == id).count();
                                    if n > 0 { (true, t!("Clé enregistrée : {} modèles ajoutés.", n)) } else { (false, t!("Clé enregistrée, mais aucun modèle trouvé : la clé est-elle valide ?").into()) }
                                }
                                Err(e) => (false, t!("Clé enregistrée ; catalogue : {}", e)),
                            }
                        });
                    }
                    _ => {}
                }
            }
            Act::OuvrirFichier(f) => {
                self.retour_attendu = true;
                let path = crate::paths::config_dir().join(f);
                if !path.exists() {
                    let _ = crate::paths::write_private(&path, b"");
                }
                let _ = std::process::Command::new("xdg-open").arg(&path).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn();
                self.say_panel(true, t!("{} ouvert dans ton éditeur ; enregistre-le, c'est pris en compte à la question suivante.", f));
            }
            Act::ChoisirModele => {
                self.panneau = None;
                self.open_picker(0);
            }
            Act::VerifierMaj => self.panel_background(t!("Je regarde sur GitHub…"), || match crate::maj::check() {
                Ok(Some(r)) => (true, t!("Version {} disponible : « lkatta maj » ou le bouton de la bulle.", r.version)),
                Ok(None) => (true, t!("Tu as la dernière version ({}).", crate::maj::current())),
                Err(e) => (false, t!("Vérification impossible : {}", e)),
            }),
            Act::TesterMail(i) => {
                if let Some(c) = self.cfg.mail.get(i).cloned() {
                    self.panel_background(t!("Je teste la connexion…"), move || match crate::mail::list(&c, true, 3) {
                        Ok(l) => (true, t!("{} : connexion réussie ({} non lu(s) récent(s)).", c.adresse, l.len())),
                        Err(e) => (false, format!("{} : {e}", c.adresse)),
                    });
                }
            }
            Act::RetirerMail(i) => {
                let mut cfg = Config::load();
                if i < cfg.mail.len() {
                    let c = cfg.mail.remove(i);
                    crate::mail::remove_password(&c.adresse);
                    let _ = cfg.save();
                    self.cfg = cfg;
                    self.say_panel(true, t!("{} retiré (mot de passe effacé du trousseau).", c.adresse));
                }
            }
            Act::ObtenirCle(id) => {
                self.retour_attendu = true;
                if let Some(f) = crate::cles::fournisseur(id) {
                    let _ = std::process::Command::new("xdg-open").arg(f.key_url).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn();
                    if let Some(p) = &mut self.panneau {
                        p.focus = Some(Champ::Cle(id));
                    }
                    self.say_panel(true, t!("Page de {} ouverte : crée la clé, puis colle-la ici (Ctrl+V) et Entrée.", f.nom));
                }
            }
            Act::OuvrirLien(url) => {
                self.retour_attendu = true;
                let _ = std::process::Command::new("xdg-open").arg(url).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn();
                self.say_panel(true, t!("Page ouverte dans ton navigateur : copie ce qu'il faut, puis colle-le ici (Ctrl+V)."));
            }
            Act::Ouvrir(url) => {
                self.retour_attendu = true;
                let _ = std::process::Command::new("xdg-open").arg(url).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn();
                self.say_panel(true, t!("Ouvert : {}", url));
            }
            Act::RetirerCle(id) => {
                crate::cles::remove_cle(id);
                self.panel_background(t!("Je retire la clé…"), move || {
                    let _ = crate::moteur::refresh();
                    (true, t!("Clé retirée du trousseau.").into())
                });
            }
            Act::TelechargerLocal(tag) => self.panel_background(t!("Je télécharge le modèle local (plusieurs minutes, tu peux continuer à travailler)…"), move || match crate::local::telecharger(tag) {
                Ok(m) => (true, m),
                Err(e) => (false, t!("Téléchargement impossible : {}", e)),
            }),
            Act::RafraichirCatalogue => self.panel_background(t!("Je rafraîchis la liste des modèles…"), || match crate::moteur::refresh() {
                Ok(n) => (true, t!("{} modèles gratuits disponibles.", n)),
                Err(e) => (false, e),
            }),
        }
    }

    pub fn poll_panel(&mut self) {
        if let Some(p) = &mut self.panneau {
            if let Some(rx) = &p.rx {
                if let Ok(m) = rx.try_recv() {
                    p.message = Some(m);
                    p.rx = None;
                }
            }
        }
    }
}
