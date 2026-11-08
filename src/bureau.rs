//! Ce qui se passe sur le bureau, pour rester discret :
//! - un petit script KWin signale la fenêtre active (plein écran, position, application,
//!   titre, écran) au service D-Bus `org.lkatta.Maki` de L Katta ;
//! - une veille légère regarde « Ne pas déranger » (Plasma) et l'usage du micro (visio).

use std::process::{Command, Stdio};
use std::time::Duration;

use crate::config::which;
use crate::ipc::Msg;
use crate::paths;

/// Fenêtre active, telle que KWin la décrit (coordonnées globales, px logiques).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Fenetre {
    pub plein_ecran: bool,
    pub rect: (f32, f32, f32, f32),
    pub classe: String,
    pub titre: String,
    pub ecran: String,
}

impl Fenetre {
    pub fn from_json(s: &str) -> Option<Fenetre> {
        let v: serde_json::Value = serde_json::from_str(s).ok()?;
        let n = |k: &str| v.get(k).and_then(|x| x.as_f64()).unwrap_or(0.0) as f32;
        let t = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
        Some(Fenetre {
            plein_ecran: v.get("plein").and_then(|x| x.as_bool()).unwrap_or(false),
            rect: (n("x"), n("y"), n("w"), n("h")),
            classe: t("classe"),
            titre: t("titre"),
            ecran: t("ecran"),
        })
    }

    /// Une visio ou une présentation, d'après l'application ou le titre.
    pub fn visio(&self) -> bool {
        let c = format!("{} {}", self.classe, self.titre).to_lowercase();
        ["zoom", "teams", "meet -", "google meet", "jitsi", "skype", "webex", "discord", "obs", "présentation", "presentation", "impress", "slideshow", "diaporama"]
            .iter()
            .any(|k| c.contains(k))
    }
}

const SCRIPT: &str = r#"// L Katta : signale la fenêtre active pour rester discret (chargé et retiré par L Katta).
function signaler() {
    var w = workspace.activeWindow;
    // Le maki lui-même, les panneaux, les notifications : on garde la fenêtre précédente.
    if (w && (String(w.resourceClass) === "lkatta" || w.dock || w.notification || w.criticalNotification || w.onScreenDisplay)) return;
    // Le bureau vide : rien à éviter.
    if (w && w.desktopWindow) w = null;
    var g = w ? w.frameGeometry : null;
    callDBus("org.lkatta.Maki", "/Maki", "org.lkatta.Maki", "Fenetre", JSON.stringify({
        plein: w ? !!w.fullScreen : false,
        x: g ? g.x : 0, y: g ? g.y : 0, w: g ? g.width : 0, h: g ? g.height : 0,
        classe: w ? String(w.resourceClass) : "", titre: w ? String(w.caption) : "",
        ecran: (w && w.output) ? String(w.output.name) : ""}));
}
var suivies = [];
workspace.windowActivated.connect(function (w) {
    if (w && suivies.indexOf(w) < 0) {
        suivies.push(w);
        w.frameGeometryChanged.connect(signaler);
        w.fullScreenChanged.connect(signaler);
    }
    signaler();
});
signaler();
"#;

fn qdbus(args: &[&str]) -> Option<String> {
    let bin = if which("qdbus6") { "qdbus6" } else { "qdbus" };
    let o = Command::new(bin).args(args).stderr(Stdio::null()).output().ok()?;
    o.status.success().then(|| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

/// Charge (ou recharge) le script KWin.
pub fn charger_script() {
    let path = paths::data_dir().join("kwin-lkatta.js");
    if paths::write_private(&path, SCRIPT.as_bytes()).is_err() {
        return;
    }
    let p = path.to_string_lossy().into_owned();
    if qdbus(&["org.kde.KWin", "/Scripting", "org.kde.kwin.Scripting.isScriptLoaded", "lkatta"]).as_deref() == Some("true") {
        let _ = qdbus(&["org.kde.KWin", "/Scripting", "org.kde.kwin.Scripting.unloadScript", "lkatta"]);
    }
    let _ = qdbus(&["org.kde.KWin", "/Scripting", "org.kde.kwin.Scripting.loadScript", &p, "lkatta"]);
    let _ = qdbus(&["org.kde.KWin", "/Scripting", "org.kde.kwin.Scripting.start"]);
}

pub fn retirer_script() {
    let _ = qdbus(&["org.kde.KWin", "/Scripting", "org.kde.kwin.Scripting.unloadScript", "lkatta"]);
}

struct Service {
    tx: Box<dyn Fn(Msg) + Send + Sync>,
}

#[zbus::interface(name = "org.lkatta.Maki")]
impl Service {
    /// La fenêtre active, en JSON (KWin envoie les nombres entiers en `int` : un seul texte
    /// évite toute surprise de signature).
    #[zbus(name = "Fenetre")]
    fn fenetre(&self, json: String) {
        if let Some(f) = Fenetre::from_json(&json) {
            (self.tx)(Msg::Fenetre(f));
        }
    }
}

/// Démarre le service D-Bus (fil dédié) puis charge le script KWin.
pub fn demarrer(tx: impl Fn(Msg) + Send + Sync + 'static) {
    std::thread::spawn(move || {
        let r: zbus::Result<()> = zbus::block_on(async move {
            let _conn = zbus::connection::Builder::session()
                .name("org.lkatta.Maki")
                .serve_at("/Maki", Service { tx: Box::new(tx) })
                .build()
                .await?;
            charger_script();
            std::future::pending::<()>().await;
            Ok(())
        });
        if let Err(e) = r {
            eprintln!("lkatta : service D-Bus indisponible ({e}) ; discrétion réduite");
        }
    });
}

// --- raccourci global (KGlobalAccel) ----------------------------------------------------

const ACTION: &str = "['lkatta', 'discuter', 'L Katta', 'Ouvrir la bulle de discussion']";

/// « Meta+K », « Ctrl+Alt+Espace », « Meta+F2 »… en code de touche Qt (0 = invalide).
pub fn qt_key(seq: &str) -> Option<i32> {
    let mut code = 0i32;
    let mut key = None;
    for part in seq.split('+').map(str::trim).filter(|p| !p.is_empty()) {
        match part.to_lowercase().as_str() {
            "meta" | "super" | "win" => code |= 0x1000_0000,
            "ctrl" | "control" => code |= 0x0400_0000,
            "alt" => code |= 0x0800_0000,
            "shift" | "maj" => code |= 0x0200_0000,
            "space" | "espace" => key = Some(0x20),
            p if p.len() == 1 && p.chars().all(|c| c.is_ascii_alphanumeric()) => key = Some(p.to_ascii_uppercase().as_bytes()[0] as i32),
            p if p.starts_with('f') && p[1..].parse::<i32>().is_ok_and(|n| (1..=24).contains(&n)) => key = Some(0x0100_0030 + p[1..].parse::<i32>().unwrap() - 1),
            _ => return None,
        }
    }
    key.map(|k| code | k)
}

fn kga(method: &str, args: &[&str]) -> Option<String> {
    let o = Command::new("gdbus")
        .args(["call", "--session", "--dest", "org.kde.kglobalaccel", "--object-path", "/kglobalaccel", "--method", &format!("org.kde.KGlobalAccel.{method}")])
        .args(args)
        .stderr(Stdio::null())
        .output()
        .ok()?;
    o.status.success().then(|| String::from_utf8_lossy(&o.stdout).into_owned())
}

/// Enregistre le raccourci dans Plasma et attend qu'on le presse (fil dédié).
pub fn raccourci(seq: &str, tx: impl Fn(Msg) + Send + 'static) {
    if !which("gdbus") {
        return;
    }
    let seq = seq.trim().to_string();
    std::thread::spawn(move || {
        if seq.is_empty() || seq == "aucun" {
            let _ = kga("unRegister", &[ACTION]);
            return;
        }
        let Some(k) = qt_key(&seq) else {
            eprintln!("lkatta : raccourci « {seq} » incompris (ex. Meta+K)");
            return;
        };
        let keys = format!("[{k}]");
        let _ = kga("doRegister", &[ACTION]);
        // Par défaut, puis actif (2 = présent, 4 = sans relire l'ancien : la config fait foi).
        let _ = kga("setShortcut", &[ACTION, &keys, "1"]);
        let _ = kga("setShortcut", &[ACTION, &keys, "6"]);
        let Ok(mut child) = paths::mourir_avec_parent(&mut Command::new("gdbus"))
            .args(["monitor", "--session", "--dest", "org.kde.kglobalaccel", "--object-path", "/component/lkatta"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
        else {
            return;
        };
        let out = child.stdout.take().expect("stdout");
        use std::io::{BufRead, BufReader};
        for line in BufReader::new(out).lines().map_while(Result::ok) {
            if line.contains("globalShortcutPressed") && line.contains("'discuter'") {
                tx(Msg::Raccourci);
            }
        }
        let _ = child.wait();
    });
}

/// Donne le clavier à la bulle (ouverte par le raccourci) : un petit script KWin active la
/// surface du maki. Elle reste « OnDemand » : un clic ailleurs rend le clavier.
pub fn focaliser_bulle() {
    std::thread::spawn(|| {
        std::thread::sleep(Duration::from_millis(120));
        const JS: &str = r#"var l = workspace.windowList(), best = null, a = 0;
for (var i = 0; i < l.length; i++) {
    var w = l[i], g = w.frameGeometry;
    if (String(w.resourceClass) === "lkatta" && g.width * g.height > a) { best = w; a = g.width * g.height; }
}
if (best) workspace.activeWindow = best;
"#;
        let path = paths::data_dir().join("kwin-lkatta-focus.js");
        if paths::write_private(&path, JS.as_bytes()).is_err() {
            return;
        }
        let p = path.to_string_lossy().into_owned();
        let _ = qdbus(&["org.kde.KWin", "/Scripting", "org.kde.kwin.Scripting.unloadScript", "lkatta-focus"]);
        let _ = qdbus(&["org.kde.KWin", "/Scripting", "org.kde.kwin.Scripting.loadScript", &p, "lkatta-focus"]);
        let _ = qdbus(&["org.kde.KWin", "/Scripting", "org.kde.kwin.Scripting.start"]);
        std::thread::sleep(Duration::from_millis(400));
        let _ = qdbus(&["org.kde.KWin", "/Scripting", "org.kde.kwin.Scripting.unloadScript", "lkatta-focus"]);
    });
}

/// « Ne pas déranger » de Plasma activé ?
pub fn ne_pas_deranger() -> bool {
    Command::new("gdbus")
        .args(["call", "--session", "--dest", "org.freedesktop.Notifications", "--object-path", "/org/freedesktop/Notifications", "--method", "org.freedesktop.DBus.Properties.Get", "org.freedesktop.Notifications", "Inhibited"])
        .stderr(Stdio::null())
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).contains("true"))
        .unwrap_or(false)
}

/// Le micro est-il utilisé (enregistrement en cours : visio, appel) ?
pub fn micro_utilise() -> bool {
    if !which("pactl") {
        return false;
    }
    Command::new("pactl")
        .args(["list", "source-outputs", "short"])
        .stderr(Stdio::null())
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).lines().any(|l| !l.trim().is_empty()))
        .unwrap_or(false)
}

/// Veille légère : « Ne pas déranger » et micro, toutes les 3 s, signalés quand ils changent.
pub fn veiller(tx: impl Fn(Msg) + Send + 'static) {
    std::thread::spawn(move || {
        let mut last = (false, false);
        loop {
            let now = (ne_pas_deranger(), micro_utilise());
            if now != last {
                last = now;
                tx(Msg::Occupe { ne_pas_deranger: now.0, micro: now.1 });
            }
            std::thread::sleep(Duration::from_secs(3));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_calls_and_presentations() {
        let f = |c: &str, t: &str| Fenetre { classe: c.into(), titre: t.into(), ..Default::default() };
        assert!(f("zoom", "Réunion Zoom").visio());
        assert!(f("firefox", "Meet - abc-defg-hij").visio());
        assert!(f("libreoffice-impress", "cours.odp").visio());
        assert!(!f("org.kde.dolphin", "Documents").visio());
        assert_eq!(qt_key("Meta+K"), Some(0x1000_004B));
        assert_eq!(qt_key("ctrl+alt+espace"), Some(0x0C00_0020));
        assert_eq!(qt_key("Meta+F2"), Some(0x1000_0000 | 0x0100_0031));
        assert_eq!(qt_key("Meta+Truc"), None);
        let j = Fenetre::from_json(r#"{"plein":true,"x":0,"y":0,"w":1920,"h":1080,"classe":"mpv","titre":"film","ecran":"eDP-1"}"#).unwrap();
        assert!(j.plein_ecran && j.rect.2 == 1920.0 && j.ecran == "eDP-1");
    }
}
