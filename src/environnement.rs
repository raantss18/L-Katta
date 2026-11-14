//! Tour d'horizon de l'environnement de l'utilisateur : bureau, compositeur, applications par
//! défaut, terminal et outils. Détecté au premier démarrage, enregistré dans
//! `~/.config/lkatta/environnement.toml` (que l'utilisateur peut corriger), et résumé dans le
//! prompt système : L Katta parle de *ses* applications et se sert de *ses* outils.

use serde::{Deserialize, Serialize};

use crate::config::which;
use crate::paths;

/// Une application par défaut : identifiant .desktop et nom lisible.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Appli {
    pub id: String,
    pub nom: String,
    /// Commande pour lancer l'application (champs %f, %U retirés).
    pub commande: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Environnement {
    /// « KDE », « GNOME », « Hyprland », « sway »… (XDG_CURRENT_DESKTOP).
    pub bureau: String,
    /// « wayland » ou « x11 ».
    pub session: String,
    /// Famille de compositeur pour les outils : kde, hyprland, sway, niri, gnome, wlroots, x11, autre.
    pub compositeur: String,
    pub navigateur: Appli,
    pub mails: Appli,
    pub pdf: Appli,
    pub images: Appli,
    pub texte: Appli,
    pub fichiers: Appli,
    pub videos: Appli,
    pub musique: Appli,
    /// Terminal préféré (programme) : konsole, kitty, gnome-terminal…
    pub terminal: String,
    /// Outil de capture d'écran : spectacle, grim, gnome-screenshot.
    pub capture: String,
    /// Outil des fenêtres : kdotool, hyprctl, swaymsg, aucun.
    pub fenetres: String,
    /// L'utilisateur a corrigé ce fichier : ne plus rien remplacer à la détection suivante.
    pub verrouille: bool,
}

pub fn fichier() -> std::path::PathBuf {
    paths::config_dir().join("environnement.toml")
}

fn sortie(prog: &str, args: &[&str]) -> String {
    std::process::Command::new(prog)
        .args(args)
        .stderr(std::process::Stdio::null())
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

/// Famille du compositeur, d'après les variables posées par chacun.
pub fn compositeur_de(bureau: &str, session: &str, var: impl Fn(&str) -> bool) -> &'static str {
    let b = bureau.to_lowercase();
    if session == "x11" {
        "x11"
    } else if var("HYPRLAND_INSTANCE_SIGNATURE") || b.contains("hyprland") {
        "hyprland"
    } else if var("SWAYSOCK") || b.contains("sway") {
        "sway"
    } else if var("NIRI_SOCKET") || b.contains("niri") {
        "niri"
    } else if b.contains("kde") || b.contains("plasma") {
        "kde"
    } else if b.contains("gnome") || b.contains("unity") || b.contains("pantheon") {
        "gnome"
    } else if b.contains("cosmic") {
        "cosmic"
    } else if ["wayfire", "labwc", "river", "hikari", "dwl", "qtile"].iter().any(|w| b.contains(w)) {
        "wlroots"
    } else {
        "autre"
    }
}

/// Application associée à un type de fichier ou de lien (`xdg-mime`), avec son nom.
fn par_defaut(mime: &str, apps: &[crate::apps::App]) -> Appli {
    let id = sortie("xdg-mime", &["query", "default", mime]);
    let id = id.lines().next().unwrap_or("").trim_end_matches(".desktop").to_string();
    if id.is_empty() {
        return Appli::default();
    }
    match apps.iter().find(|a| a.id == id) {
        Some(a) => Appli { id, nom: a.nom.clone(), commande: a.exec.clone() },
        None => Appli { nom: id.rsplit('.').next().unwrap_or(&id).to_string(), id, commande: String::new() },
    }
}

/// Terminaux connus, dans l'ordre où on les essaie quand rien n'est configuré.
pub const TERMINAUX: &[&str] = &[
    "konsole", "gnome-terminal", "kgx", "ptyxis", "kitty", "alacritty", "foot", "wezterm", "ghostty", "xfce4-terminal", "tilix", "terminator",
    "mate-terminal", "lxterminal", "qterminal", "xterm",
];

/// Terminal de l'utilisateur : $TERMINAL, réglage du bureau, sinon le premier installé
/// (celui du bureau d'abord).
fn terminal(compositeur: &str) -> String {
    let nom = |c: &str| c.split_whitespace().next().unwrap_or("").rsplit('/').next().unwrap_or("").to_string();
    if let Ok(t) = std::env::var("TERMINAL") {
        if !t.trim().is_empty() && which(&nom(&t)) {
            return nom(&t);
        }
    }
    let du_bureau = match compositeur {
        "kde" => {
            let t = sortie("kreadconfig6", &["--file", "kdeglobals", "--group", "General", "--key", "TerminalApplication"]);
            if t.is_empty() { "konsole".to_string() } else { nom(&t) }
        }
        "gnome" => {
            let t = sortie("gsettings", &["get", "org.gnome.desktop.default-applications.terminal", "exec"]);
            let t = t.trim_matches('\'').to_string();
            if t.is_empty() { "gnome-terminal".to_string() } else { nom(&t) }
        }
        _ => String::new(),
    };
    if !du_bureau.is_empty() && which(&du_bureau) {
        return du_bureau;
    }
    TERMINAUX.iter().find(|t| which(t)).map(|t| t.to_string()).unwrap_or_default()
}

/// Détecte l'environnement de cette session.
pub fn detecter() -> Environnement {
    let bureau = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default().replace(':', " / ");
    let session = std::env::var("XDG_SESSION_TYPE").unwrap_or_default();
    let compositeur = compositeur_de(&bureau, &session, |v| std::env::var_os(v).is_some()).to_string();
    let apps = crate::apps::index();
    let capture = ["spectacle", "grim", "gnome-screenshot"].into_iter().find(|o| which(o)).unwrap_or("").to_string();
    let fenetres = match compositeur.as_str() {
        "kde" if which("kdotool") => "kdotool",
        "hyprland" if which("hyprctl") => "hyprctl",
        "sway" if which("swaymsg") => "swaymsg",
        _ => "aucun",
    }
    .to_string();
    Environnement {
        navigateur: par_defaut("x-scheme-handler/https", &apps),
        mails: par_defaut("x-scheme-handler/mailto", &apps),
        pdf: par_defaut("application/pdf", &apps),
        images: par_defaut("image/png", &apps),
        texte: par_defaut("text/plain", &apps),
        fichiers: par_defaut("inode/directory", &apps),
        videos: par_defaut("video/mp4", &apps),
        musique: par_defaut("audio/mpeg", &apps),
        terminal: terminal(&compositeur),
        capture,
        fenetres,
        bureau,
        session,
        compositeur,
        verrouille: false,
    }
}

pub fn charger() -> Option<Environnement> {
    toml::from_str(&std::fs::read_to_string(fichier()).ok()?).ok()
}

/// Environnement connu, détecté (et enregistré) s'il manque ou si la session a changé de
/// bureau ; un fichier corrigé par l'utilisateur (`verrouille = true`) est gardé tel quel.
static COURANT: std::sync::OnceLock<Environnement> = std::sync::OnceLock::new();

/// Déjà connu (sans attendre la détection) ? Pour le dessin, qui ne doit jamais se figer.
pub fn si_pret() -> Option<Environnement> {
    COURANT.get().cloned()
}

/// Lance la détection en arrière-plan (au démarrage).
pub fn preparer() {
    std::thread::spawn(|| {
        let _ = courant();
    });
}

pub fn courant() -> Environnement {
    COURANT.get_or_init(|| {
        let bureau = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default().replace(':', " / ");
        match charger() {
            Some(e) if e.verrouille || e.bureau == bureau => e,
            _ => {
                let e = detecter();
                let _ = enregistrer(&e);
                e
            }
        }
    })
    .clone()
}

pub fn enregistrer(e: &Environnement) -> Result<(), String> {
    let t = toml::to_string_pretty(e).map_err(|e| e.to_string())?;
    let entete = "# Ce que L Katta a trouvé sur cette machine (corrige si besoin, puis mets verrouille = true).\n";
    paths::write_private(&fichier(), format!("{entete}{t}").as_bytes()).map_err(|e| e.to_string())
}

impl Environnement {
    /// Lignes « rôle : application » trouvées.
    pub fn applis(&self) -> Vec<(&'static str, &Appli)> {
        [
            ("Navigateur", &self.navigateur),
            ("Mails", &self.mails),
            ("PDF", &self.pdf),
            ("Images", &self.images),
            ("Textes", &self.texte),
            ("Fichiers", &self.fichiers),
            ("Vidéos", &self.videos),
            ("Musique", &self.musique),
        ]
        .into_iter()
        .filter(|(_, a)| !a.nom.is_empty())
        .collect()
    }

    /// Section du prompt système : ce que l'assistant doit savoir de cet environnement.
    pub fn pour_prompt(&self) -> String {
        let mut l = vec![format!("- Bureau : {} ({}) ; outils de fenêtres : {}", if self.bureau.is_empty() { "inconnu" } else { &self.bureau }, self.session, self.fenetres)];
        for (role, a) in self.applis() {
            l.push(format!("- {role} : {}", a.nom));
        }
        if !self.terminal.is_empty() {
            l.push(format!("- Terminal : {}", self.terminal));
        }
        if !self.capture.is_empty() {
            l.push(format!("- Capture d'écran : {}", self.capture));
        }
        format!(
            "## Applications et outils de l'utilisateur\n\n{}\n\nParle de ces applications par leur nom ; pour ouvrir un fichier, utilise l'outil ouvrir (il prend l'application par défaut).",
            l.join("\n")
        )
    }

    /// Résumé court (accueil, /machine).
    pub fn resume(&self) -> String {
        let mut l = vec![t!("Bureau : {}", if self.bureau.is_empty() { t!("inconnu") } else { &self.bureau })];
        for (role, a) in self.applis() {
            l.push(format!("{} : {}", crate::lang::trd(role), a.nom));
        }
        if !self.terminal.is_empty() {
            l.push(t!("Terminal : {}", self.terminal));
        }
        l.join("\n")
    }
}

/// Commande pour lancer `script` dans le terminal de l'utilisateur (programme, arguments).
pub fn commande_terminal(terminal: &str, titre: &str, script: &str) -> Option<(String, Vec<String>)> {
    let s = script.to_string();
    let a: Vec<String> = match terminal {
        "konsole" => vec!["-p".into(), format!("tabtitle={titre}"), "-e".into(), "bash".into(), "-c".into(), s],
        "gnome-terminal" | "mate-terminal" => vec![format!("--title={titre}"), "--".into(), "bash".into(), "-c".into(), s],
        "kgx" | "ptyxis" => vec!["--".into(), "bash".into(), "-c".into(), s],
        "kitty" => vec!["--title".into(), titre.into(), "bash".into(), "-c".into(), s],
        "alacritty" => vec!["--title".into(), titre.into(), "-e".into(), "bash".into(), "-c".into(), s],
        "foot" => vec![format!("--title={titre}"), "bash".into(), "-c".into(), s],
        "wezterm" => vec!["start".into(), "--".into(), "bash".into(), "-c".into(), s],
        "ghostty" => vec![format!("--title={titre}"), "-e".into(), "bash".into(), "-c".into(), s],
        "xfce4-terminal" | "terminator" | "lxterminal" | "qterminal" => vec!["--title".into(), titre.into(), "-e".into(), format!("bash -c {}", shell_quote(&s))],
        "tilix" => vec!["-t".into(), titre.into(), "-e".into(), format!("bash -c {}", shell_quote(&s))],
        "xterm" => vec!["-T".into(), titre.into(), "-e".into(), "bash".into(), "-c".into(), s],
        _ => return None,
    };
    Some((terminal.to_string(), a))
}

fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn families_and_terminals() {
        let aucun = |_: &str| false;
        assert_eq!(compositeur_de("KDE", "wayland", aucun), "kde");
        assert_eq!(compositeur_de("Hyprland", "wayland", aucun), "hyprland");
        assert_eq!(compositeur_de("", "wayland", |v| v == "SWAYSOCK"), "sway");
        assert_eq!(compositeur_de("ubuntu / GNOME", "wayland", aucun), "gnome");
        assert_eq!(compositeur_de("XFCE", "x11", aucun), "x11");
        assert_eq!(compositeur_de("labwc", "wayland", aucun), "wlroots");
        let (p, a) = commande_terminal("kitty", "L Katta", "paru -Syu").unwrap();
        assert_eq!((p.as_str(), a.last().map(String::as_str)), ("kitty", Some("paru -Syu")));
        let (_, a) = commande_terminal("xfce4-terminal", "L Katta", "echo 'a'").unwrap();
        assert_eq!(a.last().unwrap(), "bash -c 'echo '\\''a'\\'''");
        assert!(commande_terminal("inconnu", "t", "x").is_none());
        let e = Environnement { bureau: "KDE".into(), session: "wayland".into(), navigateur: Appli { nom: "Vivaldi".into(), ..Default::default() }, terminal: "konsole".into(), ..Default::default() };
        let p = e.pour_prompt();
        assert!(p.contains("Navigateur : Vivaldi") && p.contains("Terminal : konsole"));
        let t = toml::to_string(&e).unwrap();
        assert_eq!(toml::from_str::<Environnement>(&t).unwrap(), e);
    }
}
