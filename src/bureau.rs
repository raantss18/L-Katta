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
        let j = Fenetre::from_json(r#"{"plein":true,"x":0,"y":0,"w":1920,"h":1080,"classe":"mpv","titre":"film","ecran":"eDP-1"}"#).unwrap();
        assert!(j.plein_ecran && j.rect.2 == 1920.0 && j.ecran == "eDP-1");
    }
}
