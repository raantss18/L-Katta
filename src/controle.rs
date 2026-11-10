//! Manipuler l'ordinateur pour l'utilisateur : fenêtres (kdotool), clavier (ydotool, lancé à
//! la demande, souris coupée), son (wpctl), lecteurs multimédias (MPRIS), luminosité.
//!
//! Garde-fous, appliqués par le code (un modèle qui désobéit ne les franchit pas) :
//! 1. Interrupteur général : `controle` dans les règles strictes (l'administrateur peut
//!    l'interdire pour tous les comptes) et le réglage `controle` (« demander », « confiance »,
//!    « non »).
//! 2. Trois niveaux : regarder (sans demander) ; réversible (activer, réduire, agrandir, son,
//!    lecteur, luminosité : demandé, sauf en « confiance ») ; risqué (fermer une fenêtre, taper,
//!    raccourcis : TOUJOURS demandé, avec la fenêtre et le texte exacts).
//! 3. La bonne fenêtre : avant de taper, il active la fenêtre visée et vérifie que c'est bien
//!    elle qui a le clavier ; jamais sa propre bulle.
//! 4. Fenêtres interdites (mots de passe, banques, mobile money, pages de connexion), touches
//!    interdites (Ctrl+Alt+Suppr…), jamais de secret tapé.
//! 5. 25 actions au plus par demande ; Échap dans la bulle arrête tout de suite.

use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use crate::config::which;

/// Échap pressé dans la bulle : plus aucune action jusqu'à la prochaine demande.
pub static ARRET: AtomicBool = AtomicBool::new(false);
static COMPTE: AtomicUsize = AtomicUsize::new(0);
const MAX_ACTIONS: usize = 25;

/// Une nouvelle demande commence : compteur à zéro, arrêt levé.
pub fn nouvelle_demande() {
    ARRET.store(false, Ordering::Relaxed);
    COMPTE.store(0, Ordering::Relaxed);
}

pub fn arreter() {
    ARRET.store(true, Ordering::Relaxed);
}

pub fn arrete() -> bool {
    ARRET.load(Ordering::Relaxed)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Niveau {
    Lecture,
    Reversible,
    Risque,
}

/// Une fenêtre vue par KWin.
#[derive(Debug, Clone, PartialEq)]
pub struct Fenetre {
    pub id: String,
    pub titre: String,
    pub classe: String,
}

fn kdotool(args: &[&str]) -> Result<String, String> {
    if !which("kdotool") {
        return Err(t!("kdotool n'est pas installé (paquet kdotool) : je ne peux pas manipuler les fenêtres.").into());
    }
    let o = Command::new("kdotool").args(args).stderr(Stdio::null()).output().map_err(|e| e.to_string())?;
    Ok(String::from_utf8_lossy(&o.stdout).trim().to_string())
}

/// Fenêtres ouvertes (sauf le maki lui-même et les panneaux du bureau).
pub fn fenetres() -> Result<Vec<Fenetre>, String> {
    let ids = kdotool(&["search", "--class", "."])?;
    let mut v = vec![];
    for id in ids.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let titre = kdotool(&["getwindowname", id]).unwrap_or_default();
        let classe = kdotool(&["getwindowclassname", id]).unwrap_or_default();
        let c = classe.to_lowercase();
        if c.contains("lkatta") || c == "plasmashell" || c.contains("org.kde.plasmashell") || titre.trim().is_empty() {
            continue;
        }
        v.push(Fenetre { id: id.to_string(), titre, classe });
    }
    Ok(v)
}

pub fn active() -> Option<Fenetre> {
    let id = kdotool(&["getactivewindow"]).ok().filter(|s| !s.is_empty())?;
    Some(Fenetre { titre: kdotool(&["getwindowname", &id]).unwrap_or_default(), classe: kdotool(&["getwindowclassname", &id]).unwrap_or_default(), id })
}

/// La fenêtre dont le titre ou l'application contient `nom` (la plus courte d'abord : la
/// plus précise).
pub fn trouver(nom: &str) -> Result<Fenetre, String> {
    let n = crate::semantique::simplifier(nom.trim());
    if n.is_empty() {
        return Err(t!("Quelle fenêtre ? (un mot de son titre ou le nom de l'application)").into());
    }
    let mut l: Vec<Fenetre> = fenetres()?.into_iter().filter(|f| crate::semantique::simplifier(&format!("{} {}", f.titre, f.classe)).contains(&n)).collect();
    l.sort_by_key(|f| f.titre.len());
    l.into_iter().next().ok_or_else(|| t!("Aucune fenêtre ouverte ne correspond à « {} ».", nom))
}

/// Fenêtre où il ne faut jamais taper ni envoyer de touches (règles strictes).
pub fn interdite(f: &Fenetre, regles: &crate::regles::Regles) -> Option<String> {
    let t = crate::semantique::simplifier(&format!("{} {}", f.titre, f.classe));
    regles
        .fenetres_interdites
        .iter()
        .find(|m| !m.trim().is_empty() && t.contains(&crate::semantique::simplifier(m)))
        .map(|m| t!("la fenêtre « {} » est protégée par les règles strictes ({})", f.titre, m))
}

/// Combinaisons jamais envoyées (session, redémarrage, consoles).
pub fn touches_interdites(combo: &str, regles: &crate::regles::Regles) -> bool {
    let c = combo.to_lowercase().replace(' ', "");
    let base = ["ctrl+alt+delete", "ctrl+alt+del", "ctrl+alt+suppr", "ctrl+alt+backspace", "alt+sysrq", "alt+print+", "ctrl+alt+f"];
    base.iter().any(|b| c.starts_with(b)) || regles.touches_interdites.iter().any(|t| !t.is_empty() && c == t.to_lowercase().replace(' ', ""))
}

/// Code de touche Linux (clavier QWERTY américain, la disposition des combinaisons).
fn code(nom: &str) -> Option<u16> {
    let n = nom.trim().to_lowercase();
    let lettres = "qwertyuiop";
    let rang2 = "asdfghjkl";
    let rang3 = "zxcvbnm";
    if n.chars().count() == 1 {
        let c = n.chars().next()?;
        if let Some(i) = lettres.find(c) {
            return Some(16 + i as u16);
        }
        if let Some(i) = rang2.find(c) {
            return Some(30 + i as u16);
        }
        if let Some(i) = rang3.find(c) {
            return Some(44 + i as u16);
        }
        if let Some(d) = c.to_digit(10) {
            return Some(if d == 0 { 11 } else { 1 + d as u16 });
        }
    }
    Some(match n.as_str() {
        "ctrl" | "control" | "ctrl_l" => 29,
        "shift" | "maj" => 42,
        "alt" => 56,
        "altgr" => 100,
        "super" | "meta" | "win" => 125,
        "enter" | "entree" | "entrée" | "return" => 28,
        "esc" | "escape" | "echap" | "échap" => 1,
        "tab" => 15,
        "backspace" | "retour" => 14,
        "delete" | "del" | "suppr" => 111,
        "space" | "espace" => 57,
        "up" | "haut" => 103,
        "down" | "bas" => 108,
        "left" | "gauche" => 105,
        "right" | "droite" => 106,
        "home" | "debut" | "début" => 102,
        "end" | "fin" => 107,
        "pageup" => 104,
        "pagedown" => 109,
        "print" | "impr" => 99,
        "minus" | "-" => 12,
        "plus" | "=" => 13,
        f if f.starts_with('f') && f.len() <= 3 => match f[1..].parse::<u16>().ok()? {
            k @ 1..=10 => 58 + k,
            11 => 87,
            12 => 88,
            _ => return None,
        },
        _ => return None,
    })
}

/// « ctrl+shift+t » → séquence ydotool (appuis puis relâchements dans l'ordre inverse).
pub fn sequence(combo: &str) -> Result<Vec<String>, String> {
    let codes: Vec<u16> = combo.split('+').map(|k| code(k).ok_or_else(|| t!("touche inconnue : « {} »", k.trim()))).collect::<Result<_, String>>()?;
    if codes.is_empty() {
        return Err(t!("aucune touche").into());
    }
    let mut v: Vec<String> = codes.iter().map(|c| format!("{c}:1")).collect();
    v.extend(codes.iter().rev().map(|c| format!("{c}:0")));
    Ok(v)
}

/// Prise du démon ydotool lancé par L Katta (clavier seulement, souris coupée).
fn prise() -> std::path::PathBuf {
    crate::paths::runtime_dir().join("ydotool.sock")
}

fn ydotool(args: &[String]) -> Result<(), String> {
    if !which("ydotool") || !which("ydotoold") {
        return Err(t!("ydotool n'est pas installé (paquet ydotool) : je ne peux pas taper au clavier.").into());
    }
    let p = prise();
    if !p.exists() {
        crate::paths::mourir_avec_parent(&mut Command::new("ydotoold"))
            .arg(format!("--socket-path={}", p.display()))
            .arg("--mouse-off")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| e.to_string())?;
        for _ in 0..20 {
            std::thread::sleep(Duration::from_millis(100));
            if p.exists() {
                break;
            }
        }
    }
    let ok = Command::new("ydotool").env("YDOTOOL_SOCKET", &p).args(args).stdout(Stdio::null()).stderr(Stdio::null()).status().map(|s| s.success()).unwrap_or(false);
    if ok { Ok(()) } else { Err(t!("ydotool n'a pas pu envoyer les touches (accès à /dev/uinput ?).").into()) }
}

/// Active la fenêtre et vérifie qu'elle a bien le clavier.
fn viser(f: &Fenetre) -> Result<(), String> {
    kdotool(&["windowactivate", &f.id])?;
    for _ in 0..10 {
        std::thread::sleep(Duration::from_millis(80));
        if active().is_some_and(|a| a.id == f.id) {
            return Ok(());
        }
    }
    Err(t!("La fenêtre « {} » n'a pas pris le clavier : je n'envoie rien.", f.titre))
}

/// Tape un texte dans une fenêtre : collé (toutes les lettres, accents compris, quelle que
/// soit la disposition du clavier), puis le presse-papiers d'avant est remis.
pub fn taper(f: &Fenetre, texte: &str) -> Result<String, String> {
    viser(f)?;
    let avant = Command::new("wl-paste").args(["--no-newline"]).stderr(Stdio::null()).output().ok().filter(|o| o.status.success()).map(|o| o.stdout);
    crate::clipboard::copy(texte).map_err(|e| e.to_string())?;
    std::thread::sleep(Duration::from_millis(120));
    let terminal = ["konsole", "terminal", "kitty", "alacritty", "wezterm", "foot", "yakuake"].iter().any(|t| f.classe.to_lowercase().contains(t));
    let combo = if terminal { "ctrl+shift+v" } else { "ctrl+v" };
    let r = ydotool(&std::iter::once("key".to_string()).chain(sequence(combo)?).collect::<Vec<_>>());
    std::thread::sleep(Duration::from_millis(250));
    if let Some(b) = avant {
        let _ = crate::clipboard::copy(&String::from_utf8_lossy(&b));
    }
    r.map(|_| t!("Texte tapé dans « {} » ({} caractères).", f.titre, texte.chars().count()))
}

pub fn touches(f: &Fenetre, combo: &str) -> Result<String, String> {
    let seq = sequence(combo)?;
    viser(f)?;
    ydotool(&std::iter::once("key".to_string()).chain(seq).collect::<Vec<_>>()).map(|_| t!("« {} » envoyé à « {} ».", combo, f.titre))
}

/// Action sur une fenêtre : activer, reduire, agrandir, fermer, bureau (n).
pub fn fenetre(f: &Fenetre, action: &str, bureau: Option<u32>) -> Result<String, String> {
    match action {
        "activer" => kdotool(&["windowactivate", &f.id]).map(|_| t!("« {} » est au premier plan.", f.titre)),
        "reduire" => kdotool(&["windowminimize", &f.id]).map(|_| t!("« {} » est réduite.", f.titre)),
        "agrandir" => kdotool(&["windowstate", "--toggle", "maximized", &f.id]).map(|_| t!("« {} » : taille basculée (agrandie / normale).", f.titre)),
        "fermer" => kdotool(&["windowclose", &f.id]).map(|_| t!("« {} » : fermeture demandée (l'application peut demander d'enregistrer).", f.titre)),
        "bureau" => {
            let n = bureau.ok_or(t!("quel bureau ? (numéro)"))?;
            kdotool(&["set_desktop_for_window", &f.id, &n.to_string()]).map(|_| t!("« {} » est sur le bureau {}.", f.titre, n))
        }
        a => Err(t!("action de fenêtre inconnue : {}", a)),
    }
}

/// Son : volume (0–150 %), plus, moins, muet.
pub fn son(action: &str, valeur: Option<u32>) -> Result<String, String> {
    if !which("wpctl") {
        return Err(t!("wpctl (PipeWire) n'est pas installé.").into());
    }
    let cible = "@DEFAULT_AUDIO_SINK@";
    let (args, msg): (Vec<String>, String) = match action {
        "volume" => {
            let v = valeur.ok_or(t!("quel volume ? (0 à 150)"))?.min(150);
            (vec!["set-volume".into(), cible.into(), format!("{v}%")], t!("Volume à {} %.", v))
        }
        "plus" => (vec!["set-volume".into(), "-l".into(), "1.5".into(), cible.into(), "10%+".into()], t!("Volume monté.").into()),
        "moins" => (vec!["set-volume".into(), cible.into(), "10%-".into()], t!("Volume baissé.").into()),
        "muet" => (vec!["set-mute".into(), cible.into(), "toggle".into()], t!("Son coupé ou rétabli.").into()),
        a => return Err(t!("action de son inconnue : {}", a)),
    };
    let ok = Command::new("wpctl").args(&args).status().map(|s| s.success()).unwrap_or(false);
    if ok { Ok(msg) } else { Err(t!("wpctl a échoué.").into()) }
}

/// Lecteur multimédia (MPRIS) : lecture, pause, suivant, precedent.
pub fn lecteur(action: &str) -> Result<String, String> {
    let methode = match action {
        "lecture" | "pause" | "lecture_pause" => "PlayPause",
        "suivant" => "Next",
        "precedent" | "précédent" => "Previous",
        a => return Err(t!("action de lecteur inconnue : {}", a)),
    };
    let noms = Command::new("qdbus6").stderr(Stdio::null()).output().map(|o| String::from_utf8_lossy(&o.stdout).to_string()).unwrap_or_default();
    let lecteur = noms.lines().map(str::trim).find(|l| l.starts_with("org.mpris.MediaPlayer2.")).ok_or(t!("Aucun lecteur multimédia ouvert."))?.to_string();
    let ok = Command::new("qdbus6").args([&lecteur, "/org/mpris/MediaPlayer2", &format!("org.mpris.MediaPlayer2.Player.{methode}")]).status().map(|s| s.success()).unwrap_or(false);
    if ok { Ok(t!("Lecteur : {}.", action)) } else { Err(t!("Le lecteur n'a pas répondu.").into()) }
}

/// Luminosité de l'écran (0–100 %), par le service d'énergie de KDE.
pub fn luminosite(pourcent: u32) -> Result<String, String> {
    let svc = "org.kde.Solid.PowerManagement";
    let chemin = "/org/kde/Solid/PowerManagement/Actions/BrightnessControl";
    let max: u32 = Command::new("qdbus6").args([svc, chemin, "brightnessMax"]).output().ok().and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse().ok()).ok_or(t!("Luminosité non réglable ici."))?;
    let v = (max * pourcent.min(100) / 100).max(1);
    let ok = Command::new("qdbus6").args([svc, chemin, "setBrightness", &v.to_string()]).status().map(|s| s.success()).unwrap_or(false);
    if ok { Ok(t!("Luminosité à {} %.", pourcent.min(100))) } else { Err(t!("Le réglage de luminosité a échoué.").into()) }
}

/// Vérifie qu'une action est permise maintenant (interrupteurs, arrêt, quota par demande).
pub fn permis(regles: &crate::regles::Regles, cfg: &crate::config::Config, niveau: Niveau) -> Result<(), String> {
    if !regles.controle {
        return Err(t!("Le contrôle de l'ordinateur est interdit par les règles strictes de cette machine.").into());
    }
    if cfg.controle == "non" && niveau != Niveau::Lecture {
        return Err(t!("Le contrôle de l'ordinateur est coupé dans les réglages (Assistant › Contrôle de l'ordinateur).").into());
    }
    if arrete() {
        return Err(t!("Arrêté par l'utilisateur (Échap) : plus aucune action pour cette demande.").into());
    }
    if niveau != Niveau::Lecture && COMPTE.fetch_add(1, Ordering::Relaxed) >= MAX_ACTIONS {
        return Err(t!("Limite de {} actions par demande atteinte : je m'arrête là.", MAX_ACTIONS));
    }
    Ok(())
}

/// Faut-il demander l'accord ? (Toujours pour le risqué ; pour le réversible, sauf en confiance.)
pub fn demander(cfg: &crate::config::Config, niveau: Niveau) -> bool {
    match niveau {
        Niveau::Lecture => false,
        Niveau::Reversible => cfg.controle != "confiance",
        Niveau::Risque => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_and_guards() {
        assert_eq!(sequence("ctrl+s").unwrap(), vec!["29:1", "31:1", "31:0", "29:0"]);
        assert_eq!(sequence("Ctrl + Shift + T").unwrap(), vec!["29:1", "42:1", "20:1", "20:0", "42:0", "29:0"]);
        assert_eq!(sequence("Entrée").unwrap(), vec!["28:1", "28:0"]);
        assert_eq!(sequence("f5").unwrap(), vec!["63:1", "63:0"]);
        assert!(sequence("ctrl+€").is_err());
        let r = crate::regles::Regles::default();
        assert!(touches_interdites("Ctrl+Alt+Delete", &r));
        assert!(touches_interdites("ctrl+alt+f2", &r));
        assert!(!touches_interdites("ctrl+s", &r));
        let f = |t: &str, c: &str| Fenetre { id: "x".into(), titre: t.into(), classe: c.into() };
        assert!(interdite(&f("Coffre — KeePassXC", "org.keepassxc.KeePassXC"), &r).is_some());
        assert!(interdite(&f("BNI Madagascar — Connexion - Firefox", "firefox"), &r).is_some());
        assert!(interdite(&f("Document sans titre — LibreOffice Writer", "libreoffice-writer"), &r).is_none());
        nouvelle_demande();
        let mut cfg = crate::config::Config::default();
        assert!(permis(&r, &cfg, Niveau::Risque).is_ok());
        arreter();
        assert!(permis(&r, &cfg, Niveau::Reversible).is_err());
        nouvelle_demande();
        cfg.controle = "non".into();
        assert!(permis(&r, &cfg, Niveau::Reversible).is_err() && permis(&r, &cfg, Niveau::Lecture).is_ok());
        cfg.controle = "confiance".into();
        assert!(!demander(&cfg, Niveau::Reversible) && demander(&cfg, Niveau::Risque));
    }
}

#[cfg(test)]
mod essai_reel {
    /// Lecture seule : `cargo test liste_les_fenetres -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn liste_les_fenetres() {
        let l = super::fenetres().unwrap();
        for f in &l {
            println!("{} | {}", f.titre, f.classe);
        }
        println!("active : {:?}", super::active().map(|f| f.titre));
        assert!(!l.is_empty());
    }
}
