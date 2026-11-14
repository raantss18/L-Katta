//! Lire le texte de la fenêtre active par l'accessibilité du bureau (AT-SPI), sans capture
//! d'image : plus rapide, plus juste pour le texte, et rien ne part vers un modèle de vision.
//!
//! Marche avec les applications Qt (KDE), GTK (GNOME), Firefox, Chromium, LibreOffice… dès que
//! l'accessibilité est active dans la session (`org.a11y.Status.IsEnabled`). Elle est souvent
//! coupée par défaut : L Katta le dit, propose de l'activer (réglage de session, réversible), et
//! se rabat sinon sur la capture d'écran.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use zbus::{Connection, OwnedObjectPath, OwnedVariant};

/// Nœuds visités au plus, texte gardé au plus, temps au plus.
const MAX_NOEUDS: usize = 4000;
const MAX_TEXTE: usize = 16_000;
const MAX_DUREE: Duration = Duration::from_secs(4);

// États AT-SPI (AtspiStateType) : numéro de bit dans le tableau de deux u32.
const ETAT_ACTIF: u32 = 1;
const ETAT_VISIBLE_ECRAN: u32 = 25; // SHOWING

const ACC: &str = "org.a11y.atspi.Accessible";

fn a_etat(etats: &[u32], n: u32) -> bool {
    etats.get((n / 32) as usize).is_some_and(|w| w & (1 << (n % 32)) != 0)
}

/// L'accessibilité est-elle active dans la session ?
pub fn active() -> bool {
    zbus::block_on(active_async())
}

/// Active (ou coupe) l'accessibilité de la session. Les applications ouvertes ensuite exposent
/// leur texte ; certaines déjà ouvertes doivent être relancées.
pub fn activer(oui: bool) -> Result<(), String> {
    zbus::block_on(async {
        let c = Connection::session().await.map_err(|e| e.to_string())?;
        c.call_method(Some("org.a11y.Bus"), "/org/a11y/bus", Some("org.freedesktop.DBus.Properties"), "Set", &("org.a11y.Status", "IsEnabled", zbus::Variant::from(oui)))
            .await
            .map(|_| ())
            .map_err(|e| e.to_string())
    })
}

async fn bus_a11y() -> Result<Connection, String> {
    let s = Connection::session().await.map_err(|e| e.to_string())?;
    let r = s.call_method(Some("org.a11y.Bus"), "/org/a11y/bus", Some("org.a11y.Bus"), "GetAddress", &()).await.map_err(|_| t!("le bus d'accessibilité est absent").to_string())?;
    let adresse: String = r.body().deserialize().map_err(|e| e.to_string())?;
    zbus::connection::Builder::address(adresse.as_str()).build().await.map_err(|e| e.to_string())
}

struct Lecteur {
    c: Connection,
    fin: Instant,
}

impl Lecteur {
    async fn appel<B: serde::de::DeserializeOwned + zbus::zvariant::Type>(&self, bus: &str, chemin: &str, iface: &str, m: &str, args: &(impl serde::Serialize + zbus::zvariant::DynamicType)) -> Option<B> {
        let r = self.c.call_method(Some(bus), chemin, Some(iface), m, args).await.ok()?;
        r.body().deserialize::<B>().ok()
    }
    async fn prop(&self, bus: &str, chemin: &str, iface: &str, nom: &str) -> Option<OwnedVariant> {
        self.appel::<OwnedVariant>(bus, chemin, "org.freedesktop.DBus.Properties", "Get", &(iface, nom)).await
    }
    async fn enfants(&self, bus: &str, chemin: &str) -> Vec<(String, OwnedObjectPath)> {
        self.appel::<Vec<(String, OwnedObjectPath)>>(bus, chemin, ACC, "GetChildren", &()).await.unwrap_or_default()
    }
    async fn etats(&self, bus: &str, chemin: &str) -> Vec<u32> {
        self.appel::<Vec<u32>>(bus, chemin, ACC, "GetState", &()).await.unwrap_or_default()
    }
    async fn nom(&self, bus: &str, chemin: &str) -> String {
        self.prop(bus, chemin, ACC, "Name").await.and_then(|v| String::try_from(v).ok()).unwrap_or_default()
    }

    /// Texte visible sous ce nœud, dans l'ordre de lecture (profondeur d'abord).
    async fn texte(&self, bus: &str, racine: &str) -> String {
        let mut out: Vec<String> = vec![];
        let mut total = 0;
        let mut pile = vec![(bus.to_string(), racine.to_string())];
        let mut vus = 0;
        while let Some((b, p)) = pile.pop() {
            vus += 1;
            if vus > MAX_NOEUDS || total > MAX_TEXTE || Instant::now() > self.fin {
                break;
            }
            let etats = self.etats(&b, &p).await;
            if !etats.is_empty() && !a_etat(&etats, ETAT_VISIBLE_ECRAN) && p != racine {
                continue;
            }
            let ifaces: Vec<String> = self.appel(&b, &p, ACC, "GetInterfaces", &()).await.unwrap_or_default();
            let enfants = self.enfants(&b, &p).await;
            let mut ligne = String::new();
            if ifaces.iter().any(|i| i == "org.a11y.atspi.Text") {
                let n = self.prop(&b, &p, "org.a11y.atspi.Text", "CharacterCount").await.and_then(|v| i32::try_from(v).ok()).unwrap_or(0);
                if n > 0 {
                    ligne = self.appel::<String>(&b, &p, "org.a11y.atspi.Text", "GetText", &(0i32, n.min(8000))).await.unwrap_or_default();
                }
            }
            if ligne.trim().is_empty() && enfants.is_empty() {
                ligne = self.nom(&b, &p).await;
            }
            let ligne = ligne.replace('\u{FFFC}', "").trim().to_string();
            if !ligne.is_empty() && out.last() != Some(&ligne) {
                total += ligne.chars().count();
                out.push(ligne);
            }
            // Enfants empilés à l'envers : le premier est lu d'abord.
            for (eb, ep) in enfants.into_iter().rev() {
                pile.push((eb, ep.to_string()));
            }
        }
        let t = out.join("\n");
        t.chars().take(MAX_TEXTE).collect()
    }
}

/// Fenêtre lue : (application, titre, texte).
pub struct Lu {
    pub application: String,
    pub titre: String,
    pub texte: String,
}

/// Dernière fenêtre active connue du gestionnaire de fenêtres (titre, classe) : la bulle de
/// L Katta prend le clavier, et certaines applications cessent alors de se dire « actives ».
static DERNIERE: std::sync::Mutex<(String, String)> = std::sync::Mutex::new((String::new(), String::new()));

pub fn noter_fenetre(titre: &str, classe: &str) {
    if classe.to_lowercase().contains("lkatta") || titre.is_empty() {
        return;
    }
    if let Ok(mut d) = DERNIERE.lock() {
        *d = (titre.to_string(), classe.to_string());
    }
}

pub fn derniere_fenetre() -> (String, String) {
    DERNIERE.lock().map(|d| d.clone()).unwrap_or_default()
}

/// Texte de la fenêtre active : celle qui se dit active, sinon celle dont le titre est celui de
/// la dernière fenêtre active connue.
pub fn fenetre_active(titre: Option<&str>) -> Result<Lu, String> {
    let titre = titre.map(str::to_string).filter(|t| !t.is_empty()).or_else(|| Some(derniere_fenetre().0).filter(|t| !t.is_empty()));
    lire(move |_, nom, etats| {
        if a_etat(etats, ETAT_ACTIF) {
            2
        } else if titre.as_deref().is_some_and(|t| !nom.is_empty() && (t.contains(nom) || nom.contains(t))) {
            1
        } else {
            0
        }
    })
}

/// Texte de la première fenêtre d'une application donnée par son numéro de processus (essais).
#[cfg(test)]
pub fn fenetre_du_processus(pid: u32) -> Result<Lu, String> {
    lire(move |p, _, _| if p == pid { 2 } else { 0 })
}

/// `note` donne à chaque fenêtre une préférence (0 : jamais) ; la meilleure est lue.
fn lire(note: impl Fn(u32, &str, &[u32]) -> u8) -> Result<Lu, String> {
    zbus::block_on(async move {
        let c = bus_a11y().await?;
        let l = Lecteur { c: c.clone(), fin: Instant::now() + MAX_DUREE };
        let dbus = zbus::fdo::DBusProxy::new(&c).await.map_err(|e| e.to_string())?;
        let apps = l.enfants("org.a11y.atspi.Registry", "/org/a11y/atspi/accessible/root").await;
        let mut pids: HashMap<String, u32> = HashMap::new();
        let mut meilleur: Option<(u8, String, String, String, String)> = None;
        for (bus, chemin) in apps {
            let pid = match pids.get(&bus) {
                Some(p) => *p,
                None => {
                    let p = match zbus::names::BusName::try_from(bus.as_str()) {
                        Ok(n) => dbus.get_connection_unix_process_id(n).await.unwrap_or(0),
                        Err(_) => 0,
                    };
                    pids.insert(bus.clone(), p);
                    p
                }
            };
            // Jamais L Katta lui-même.
            if pid == std::process::id() {
                continue;
            }
            let app = l.nom(&bus, chemin.as_str()).await;
            for (fb, fp) in l.enfants(&bus, chemin.as_str()).await {
                let etats = l.etats(&fb, fp.as_str()).await;
                let titre = l.nom(&fb, fp.as_str()).await;
                let n = note(pid, &titre, &etats);
                if n > 0 && meilleur.as_ref().is_none_or(|m| n > m.0) {
                    meilleur = Some((n, fb, fp.to_string(), app.clone(), titre));
                }
            }
            if meilleur.as_ref().is_some_and(|m| m.0 == 2) {
                break;
            }
        }
        if let Some((_, b, p, application, titre)) = meilleur {
            let texte = l.texte(&b, &p).await;
            return Ok(Lu { application, titre, texte });
        }
        Err(if active_async().await {
            t!("aucune fenêtre active n'expose son texte (application non accessible, ou ouverte avant l'activation de l'accessibilité : relance-la)").to_string()
        } else {
            t!("l'accessibilité du bureau est coupée").to_string()
        })
    })
}

/// État de l'accessibilité, relu au plus toutes les 5 s (pour le panneau de réglages).
pub fn active_cache() -> bool {
    static C: std::sync::Mutex<Option<(Instant, bool)>> = std::sync::Mutex::new(None);
    let mut g = match C.lock() {
        Ok(g) => g,
        Err(_) => return false,
    };
    if let Some((t, v)) = *g {
        if t.elapsed() < Duration::from_secs(5) {
            return v;
        }
    }
    let v = active();
    *g = Some((Instant::now(), v));
    v
}

async fn active_async() -> bool {
    async {
        let c = Connection::session().await.ok()?;
        let r = c.call_method(Some("org.a11y.Bus"), "/org/a11y/bus", Some("org.freedesktop.DBus.Properties"), "Get", &("org.a11y.Status", "IsEnabled")).await.ok()?;
        let v: OwnedVariant = r.body().deserialize().ok()?;
        bool::try_from(v).ok()
    }
    .await
    .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_bits() {
        let e = [1u32 << 1, 0];
        assert!(a_etat(&e, ETAT_ACTIF));
        assert!(!a_etat(&e, ETAT_VISIBLE_ECRAN));
        assert!(a_etat(&[1 << 25, 0], ETAT_VISIBLE_ECRAN));
        assert!(!a_etat(&[], ETAT_ACTIF));
    }

    /// Lit une vraie fenêtre KWrite lancée hors écran (accessibilité forcée pour elle seule).
    #[test]
    #[ignore = "lance KWrite hors écran ; nécessite une session"]
    fn reads_a_real_window() {
        let f = std::env::temp_dir().join("lkatta-atspi.txt");
        std::fs::write(&f, "Bonjour depuis le maki\nDeuxième ligne").unwrap();
        let mut ch = std::process::Command::new("kwrite")
            .arg(&f)
            .env("QT_QPA_PLATFORM", "offscreen")
            .env("QT_LINUX_ACCESSIBILITY_ALWAYS_ON", "1")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
        std::thread::sleep(Duration::from_secs(4));
        let r = fenetre_du_processus(ch.id());
        let _ = ch.kill();
        let lu = r.unwrap();
        assert!(lu.texte.contains("Bonjour depuis le maki"), "{}", lu.texte);
    }
}
