//! Fenêtres vues par le protocole Wayland standard `wlr-foreign-toplevel-management`
//! (Hyprland, Sway, niri, COSMIC, Wayfire, labwc, river…). Le maki reçoit les événements sur sa
//! connexion Wayland et tient la liste ici ; les outils (autres fils) la lisent et déposent des
//! actions que le maki exécute à sa prochaine image.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Fenetre {
    pub id: u32,
    pub titre: String,
    pub classe: String,
    pub active: bool,
    pub plein_ecran: bool,
    pub agrandie: bool,
    pub reduite: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Action {
    Activer,
    Reduire,
    Agrandir,
    Fermer,
}

static LISTE: Mutex<Vec<Fenetre>> = Mutex::new(Vec::new());
static ACTIONS: Mutex<Vec<(u32, Action)>> = Mutex::new(Vec::new());
static DISPONIBLE: AtomicBool = AtomicBool::new(false);

/// Le compositeur offre-t-il le protocole (le maki l'a lié) ?
pub fn disponible() -> bool {
    DISPONIBLE.load(Ordering::Relaxed)
}

pub fn marquer_disponible() {
    DISPONIBLE.store(true, Ordering::Relaxed);
}

pub fn liste() -> Vec<Fenetre> {
    LISTE.lock().map(|l| l.clone()).unwrap_or_default()
}

pub fn active() -> Option<Fenetre> {
    liste().into_iter().find(|f| f.active)
}

/// Mise à jour d'une fenêtre (événement « done ») ; `None` : elle est fermée.
pub fn mettre_a_jour(id: u32, f: Option<Fenetre>) {
    if let Ok(mut l) = LISTE.lock() {
        l.retain(|x| x.id != id);
        if let Some(f) = f {
            l.push(f);
        }
    }
}

pub fn demander(id: u32, a: Action) {
    if let Ok(mut q) = ACTIONS.lock() {
        q.push((id, a));
    }
}

pub fn prendre_actions() -> Vec<(u32, Action)> {
    ACTIONS.lock().map(|mut q| std::mem::take(&mut *q)).unwrap_or_default()
}

/// États du protocole (tableau de u32 en octets natifs) : (agrandie, réduite, active, plein écran).
pub fn etats(octets: &[u8]) -> (bool, bool, bool, bool) {
    let mut e = (false, false, false, false);
    for c in octets.chunks_exact(4) {
        match u32::from_ne_bytes([c[0], c[1], c[2], c[3]]) {
            0 => e.0 = true,
            1 => e.1 = true,
            2 => e.2 = true,
            3 => e.3 = true,
            _ => {}
        }
    }
    e
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn states_and_list() {
        let b: Vec<u8> = [2u32, 3].iter().flat_map(|v| v.to_ne_bytes()).collect();
        assert_eq!(etats(&b), (false, false, true, true));
        assert_eq!(etats(&[]), (false, false, false, false));
        mettre_a_jour(900_001, Some(Fenetre { id: 900_001, titre: "Kate".into(), active: true, ..Default::default() }));
        assert!(liste().iter().any(|f| f.id == 900_001 && f.active));
        mettre_a_jour(900_001, None);
        assert!(!liste().iter().any(|f| f.id == 900_001));
        demander(7, Action::Fermer);
        assert!(prendre_actions().contains(&(7, Action::Fermer)));
    }
}
