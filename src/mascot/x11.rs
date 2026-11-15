//! Affichage X11 (prototype) : le maki dans une fenêtre ARGB sans décoration, au-dessus de
//! tout (override-redirect), qui ne reçoit les clics que sur lui (extension SHAPE). Même
//! comportement (`behavior.rs`) et même dessin (`draw.rs`) que sous Wayland.
//!
//! `lkatta x11-essai [--capture image.png] [--duree s]` : essai autonome (le maki complet,
//! bulles et discussion, viendra quand `mod.rs` sera séparé en scène + affichage ; voir
//! docs/TODO.md).

use std::time::{Duration, Instant};

use tiny_skia::{Pixmap, Transform};
use x11rb::connection::Connection;
use x11rb::protocol::Event;
use x11rb::protocol::shape::{self, ConnectionExt as _};
use x11rb::protocol::xproto::{self, ConnectionExt as _};
use x11rb::wrapper::ConnectionExt as _;
#[allow(unused_imports)]
use x11rb::connection::RequestConnection as _;

use super::behavior::{Ctx, Exits, Maki, Mode};
use super::draw;

/// Taille de la fenêtre du maki (px).
const LARGEUR: u16 = 260;
const HAUTEUR: u16 = 280;

/// Visuel 32 bits (ARGB) de l'écran : la transparence passe par lui.
fn visuel_argb(ecran: &xproto::Screen) -> Option<xproto::Visualid> {
    ecran
        .allowed_depths
        .iter()
        .filter(|d| d.depth == 32)
        .flat_map(|d| d.visuals.iter())
        .find(|v| v.class == xproto::VisualClass::TRUE_COLOR && v.red_mask == 0x00ff_0000)
        .map(|v| v.visual_id)
}

/// RGBA prémultiplié (tiny-skia) → BGRA prémultiplié (X11 32 bits, petit-boutiste).
pub fn vers_bgra(rgba: &[u8]) -> Vec<u8> {
    let mut v = rgba.to_vec();
    for p in v.chunks_exact_mut(4) {
        p.swap(0, 2);
    }
    v
}

/// Rectangle d'entrée (zone cliquable) : le maki seulement, le reste laisse passer les clics.
fn zone_entree(c: &impl Connection, w: xproto::Window, r: (i16, i16, u16, u16)) -> Result<(), String> {
    c.shape_rectangles(shape::SO::SET, shape::SK::INPUT, xproto::ClipOrdering::UNSORTED, w, 0, 0, &[xproto::Rectangle { x: r.0, y: r.1, width: r.2, height: r.3 }])
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn essai(capture: Option<&str>, duree: f32) -> Result<(), String> {
    let (c, n) = x11rb::connect(None).map_err(|e| t!("pas de serveur X ({})", e))?;
    let ecran = c.setup().roots[n].clone();
    let visuel = visuel_argb(&ecran).ok_or(t!("le serveur X n'a pas de visuel 32 bits (transparence impossible)"))?;
    c.shape_query_version().map_err(|e| e.to_string())?.reply().map_err(|_| t!("le serveur X n'a pas l'extension SHAPE"))?;
    let carte = c.generate_id().map_err(|e| e.to_string())?;
    c.create_colormap(xproto::ColormapAlloc::NONE, carte, ecran.root, visuel).map_err(|e| e.to_string())?;
    let w = c.generate_id().map_err(|e| e.to_string())?;
    let (x, y) = (ecran.width_in_pixels.saturating_sub(LARGEUR + 40) as i16, ecran.height_in_pixels.saturating_sub(HAUTEUR + 40) as i16);
    let attrs = xproto::CreateWindowAux::new()
        .background_pixel(0)
        .border_pixel(0)
        .colormap(carte)
        .override_redirect(1)
        .event_mask(xproto::EventMask::BUTTON_PRESS | xproto::EventMask::EXPOSURE);
    c.create_window(32, w, ecran.root, x, y, LARGEUR, HAUTEUR, 0, xproto::WindowClass::INPUT_OUTPUT, visuel, &attrs).map_err(|e| e.to_string())?;
    // Nom et rôle, pour les gestionnaires de fenêtres qui les lisent.
    c.change_property8(xproto::PropMode::REPLACE, w, xproto::AtomEnum::WM_NAME, xproto::AtomEnum::STRING, b"L Katta").map_err(|e| e.to_string())?;
    c.change_property8(xproto::PropMode::REPLACE, w, xproto::AtomEnum::WM_CLASS, xproto::AtomEnum::STRING, b"lkatta\0lkatta\0").map_err(|e| e.to_string())?;
    let gc = c.generate_id().map_err(|e| e.to_string())?;
    c.create_gc(gc, w, &xproto::CreateGCAux::new()).map_err(|e| e.to_string())?;
    c.map_window(w).map_err(|e| e.to_string())?;
    c.flush().map_err(|e| e.to_string())?;

    let mut maki = Maki::new(LARGEUR as f32 / 2.0, 1.0, 11);
    maki.mode = Mode::Sit;
    let sol = HAUTEUR as f32 - 12.0;
    maki.pos = (LARGEUR as f32 / 2.0, sol);
    let ctx = Ctx {
        w: LARGEUR as f32,
        h: sol,
        night: false,
        quiet: true,
        acrobatics: false,
        show_gap_min: 999.0,
        hovered: false,
        pointer: None,
        chatting: true,
        thinking: false,
        bubble_hover: false,
        day: true,
        band: 0.0,
        exits: Exits::default(),
        avoid: None,
        toit: None,
        toits: Default::default(),
    };
    let mut pm = Pixmap::new(LARGEUR as u32, HAUTEUR as u32).ok_or("image")?;
    let debut = Instant::now();
    let mut dernier = Instant::now();
    let mut clics = 0;
    loop {
        while let Some(ev) = c.poll_for_event().map_err(|e| e.to_string())? {
            if let Event::ButtonPress(_) = ev {
                clics += 1;
                maki.wave();
            }
        }
        let dt = dernier.elapsed().as_secs_f32();
        dernier = Instant::now();
        maki.step(dt, ctx);
        maki.pos = (LARGEUR as f32 / 2.0, sol);
        if !matches!(maki.mode, Mode::Sit | Mode::Wave | Mode::Think | Mode::Stretch | Mode::Scratch) {
            maki.mode = Mode::Sit;
        }
        pm.fill(tiny_skia::Color::TRANSPARENT);
        let pose = maki.pose();
        draw::maki(&mut pm, Transform::from_translate(maki.pos.0, maki.pos.1).pre_scale(1.2, 1.2), &pose);
        c.put_image(xproto::ImageFormat::Z_PIXMAP, w, gc, LARGEUR, HAUTEUR, 0, 0, 0, 32, &vers_bgra(pm.data())).map_err(|e| e.to_string())?;
        // Seul le maki reçoit les clics (zone recalculée : il peut se lever, saluer…).
        zone_entree(&c, w, ((LARGEUR / 2 - 70) as i16, (HAUTEUR - 170) as i16, 140, 165))?;
        // Toujours au-dessus (un gestionnaire peut remonter une autre fenêtre).
        c.configure_window(w, &xproto::ConfigureWindowAux::new().stack_mode(xproto::StackMode::ABOVE)).map_err(|e| e.to_string())?;
        c.flush().map_err(|e| e.to_string())?;
        let t = debut.elapsed().as_secs_f32();
        if t >= duree {
            break;
        }
        std::thread::sleep(Duration::from_millis(33));
    }
    if let Some(f) = capture {
        // Image de notre fenêtre (avant composition) et état vu par le serveur.
        let img = c.get_image(xproto::ImageFormat::Z_PIXMAP, w, 0, 0, LARGEUR, HAUTEUR, !0).map_err(|e| e.to_string())?.reply().map_err(|e| e.to_string())?;
        let mut rgba = vers_bgra(&img.data);
        let opaques = rgba.chunks_exact(4).filter(|p| p[3] > 0).count();
        let mut out = Pixmap::new(LARGEUR as u32, HAUTEUR as u32).ok_or("image")?;
        out.data_mut().copy_from_slice(&rgba.drain(..).collect::<Vec<u8>>());
        out.save_png(f).map_err(|e| e.to_string())?;
        let a = c.get_window_attributes(w).map_err(|e| e.to_string())?.reply().map_err(|e| e.to_string())?;
        let entree = c.shape_get_rectangles(w, shape::SK::INPUT).map_err(|e| e.to_string())?.reply().map_err(|e| e.to_string())?;
        println!(
            "fenêtre X11 : visible={:?}, au-dessus (override-redirect)={}, pixels du maki={}, zone cliquable={:?}, clics={}",
            a.map_state,
            a.override_redirect,
            opaques,
            entree.rectangles.iter().map(|r| (r.x, r.y, r.width, r.height)).collect::<Vec<_>>(),
            clics
        );
    }
    c.destroy_window(w).map_err(|e| e.to_string())?;
    c.flush().map_err(|e| e.to_string())?;
    Ok(())
}

// --- affichage X11 du vrai maki ---------------------------------------------------------------

use xkbcommon::xkb;

/// Atomes des conventions de bureau (EWMH) lus ou envoyés par le maki.
struct Atomes {
    actif: u32,
    liste: u32,
    etat: u32,
    plein: u32,
    max_v: u32,
    max_h: u32,
    cache: u32,
    nom: u32,
    zone: u32,
    bureau: u32,
    fermer: u32,
    change_etat: u32,
    cadre: u32,
    // Glisser-déposer (protocole XDND, version 5).
    xdnd_aware: u32,
    xdnd_enter: u32,
    xdnd_position: u32,
    xdnd_status: u32,
    xdnd_leave: u32,
    xdnd_drop: u32,
    xdnd_finished: u32,
    xdnd_selection: u32,
    xdnd_types: u32,
    xdnd_copie: u32,
    uri_list: u32,
    depot: u32,
    /// Fenêtres dans l'ordre d'empilement, et bureau virtuel de chacune.
    pile: u32,
    bureau_fen: u32,
}

impl Atomes {
    fn new(c: &impl Connection) -> Result<Atomes, String> {
        let noms = [
            "_NET_ACTIVE_WINDOW",
            "_NET_CLIENT_LIST",
            "_NET_WM_STATE",
            "_NET_WM_STATE_FULLSCREEN",
            "_NET_WM_STATE_MAXIMIZED_VERT",
            "_NET_WM_STATE_MAXIMIZED_HORZ",
            "_NET_WM_STATE_HIDDEN",
            "_NET_WM_NAME",
            "_NET_WORKAREA",
            "_NET_CURRENT_DESKTOP",
            "_NET_CLOSE_WINDOW",
            "WM_CHANGE_STATE",
            "_NET_FRAME_EXTENTS",
            "XdndAware",
            "XdndEnter",
            "XdndPosition",
            "XdndStatus",
            "XdndLeave",
            "XdndDrop",
            "XdndFinished",
            "XdndSelection",
            "XdndTypeList",
            "XdndActionCopy",
            "text/uri-list",
            "LKATTA_DEPOT",
            "_NET_CLIENT_LIST_STACKING",
            "_NET_WM_DESKTOP",
        ];
        // Toutes les demandes d'abord, puis les réponses : un seul aller-retour.
        let cookies: Vec<_> = noms.iter().map(|n| c.intern_atom(false, n.as_bytes())).collect::<Result<_, _>>().map_err(|e| e.to_string())?;
        let v: Vec<u32> = cookies.into_iter().map(|k| k.reply().map(|r| r.atom)).collect::<Result<_, _>>().map_err(|e| e.to_string())?;
        Ok(Atomes {
            actif: v[0],
            liste: v[1],
            etat: v[2],
            plein: v[3],
            max_v: v[4],
            max_h: v[5],
            cache: v[6],
            nom: v[7],
            zone: v[8],
            bureau: v[9],
            fermer: v[10],
            change_etat: v[11],
            cadre: v[12],
            xdnd_aware: v[13],
            xdnd_enter: v[14],
            xdnd_position: v[15],
            xdnd_status: v[16],
            xdnd_leave: v[17],
            xdnd_drop: v[18],
            xdnd_finished: v[19],
            xdnd_selection: v[20],
            xdnd_types: v[21],
            xdnd_copie: v[22],
            uri_list: v[23],
            depot: v[24],
            pile: v[25],
            bureau_fen: v[26],
        })
    }
}

/// Clavier par libxkbcommon : la vraie disposition (AltGr, niveaux) et la composition (touches
/// mortes : ^ puis e → ê ; touche Compose).
struct Clavier {
    etat: xkb::State,
    compose: Option<xkb::compose::State>,
}

impl Clavier {
    /// D'après la disposition annoncée par le serveur (`_XKB_RULES_NAMES`, posée par setxkbmap
    /// et les bureaux) ; sans elle, rien (on garde la table simple du serveur).
    fn new(c: &impl Connection, racine: xproto::Window) -> Option<Clavier> {
        let a = c.intern_atom(true, b"_XKB_RULES_NAMES").ok()?.reply().ok()?.atom;
        if a == 0 {
            return None;
        }
        let p = c.get_property(false, racine, a, xproto::AtomEnum::STRING, 0, 1024).ok()?.reply().ok()?;
        let champs: Vec<String> = p.value.split(|b| *b == 0).map(|s| String::from_utf8_lossy(s).into_owned()).collect();
        let g = |i: usize| champs.get(i).cloned().unwrap_or_default();
        if g(2).is_empty() {
            return None;
        }
        Clavier::depuis_noms(&g(0), &g(1), &g(2), &g(3), &g(4))
    }

    /// Clavier d'après les noms XKB (règles, modèle, disposition, variante, options).
    fn depuis_noms(regles: &str, modele: &str, dispo: &str, variante: &str, options: &str) -> Option<Clavier> {
        let ctx = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
        let options = Some(options.to_string()).filter(|o| !o.is_empty());
        let carte = xkb::Keymap::new_from_names(&ctx, regles, modele, dispo, variante, options, xkb::KEYMAP_COMPILE_NO_FLAGS)?;
        let locale = ["LC_ALL", "LC_CTYPE", "LANG"].iter().filter_map(|v| std::env::var_os(v)).find(|v| !v.is_empty()).unwrap_or_else(|| "C".into());
        let compose = xkb::compose::Table::new_from_locale(&ctx, &locale, xkb::compose::COMPILE_NO_FLAGS)
            .ok()
            .map(|t| xkb::compose::State::new(&t, xkb::compose::STATE_NO_FLAGS));
        Some(Clavier { etat: xkb::State::new(&carte), compose })
    }

    /// Une touche (code X = code xkb) avec l'état des modificateurs de l'événement. `None` :
    /// rien à taper (touche morte en attente, séquence annulée).
    fn touche(&mut self, code: u8, etat: u16) -> Option<(xkb::Keysym, Option<String>)> {
        // Modificateurs réels (Maj, Verr. maj, Ctrl, Mod1..Mod5) et groupe (disposition) XKB.
        self.etat.update_mask((etat & 0xff) as u32, 0, 0, 0, 0, ((etat >> 13) & 3) as u32);
        let kc = xkb::Keycode::new(code as u32);
        let sym = self.etat.key_get_one_sym(kc);
        let utf8 = Some(self.etat.key_get_utf8(kc)).filter(|s| !s.is_empty());
        if let Some(cs) = self.compose.as_mut() {
            if let xkb::compose::FeedResult::Accepted = cs.feed(sym) {
                match cs.status() {
                    xkb::compose::Status::Composing => return None,
                    xkb::compose::Status::Composed => {
                        let r = (cs.keysym().unwrap_or(sym), cs.utf8());
                        cs.reset();
                        return Some(r);
                    }
                    xkb::compose::Status::Cancelled => {
                        cs.reset();
                        return None;
                    }
                    xkb::compose::Status::Nothing => {}
                }
            }
        }
        Some((sym, utf8))
    }
}

/// Affichage X11 : une fenêtre ARGB au-dessus de tout, qui suit le contenu (maki, bulles).
pub struct X11Aff {
    c: x11rb::rust_connection::RustConnection,
    racine: xproto::Window,
    win: xproto::Window,
    gc: xproto::Gcontext,
    taille: (u32, u32),
    pos: (i32, i32),
    zones: Vec<(i32, i32, i32, i32)>,
    /// La position doit être (re)donnée au serveur (nouvel écran).
    pub(super) a_placer: bool,
    /// Table simple des touches du serveur, si libxkbcommon ne sait pas lire la disposition :
    /// (premier code, symboles par touche, symboles).
    touches: (u8, u8, Vec<u32>),
    clavier: Option<Clavier>,
    at: Atomes,
    /// Fenêtre active suivie (changements de titre, d'état, de position).
    suivie: xproto::Window,
    curseurs: Option<x11rb::cursor::Handle>,
    curseurs_charges: std::collections::HashMap<&'static str, xproto::Cursor>,
    // Lectures à refaire après un lot d'événements.
    lire_fenetres: bool,
    lire_active: bool,
    lire_zone: bool,
    /// Glisser en cours au-dessus du maki : (fenêtre source, version XDND, fichiers offerts).
    glisse: Option<(xproto::Window, u32, bool)>,
}

/// Une fenêtre vue par le gestionnaire de fenêtres (EWMH).
struct Vue {
    f: crate::toplevels::Fenetre,
}

impl X11Aff {
    pub(super) fn redimensionner(&mut self, w: u32, h: u32) {
        if self.taille != (w, h) {
            self.taille = (w, h);
            let _ = self.c.configure_window(self.win, &xproto::ConfigureWindowAux::new().width(w).height(h));
        }
    }

    pub(super) fn deplacer(&mut self, x: i32, y: i32) {
        if self.pos != (x, y) || self.a_placer {
            self.pos = (x, y);
            self.a_placer = false;
            let _ = self.c.configure_window(self.win, &xproto::ConfigureWindowAux::new().x(x).y(y).stack_mode(xproto::StackMode::ABOVE));
        }
    }

    pub(super) fn zones(&mut self, z: &[(i32, i32, i32, i32)]) {
        if self.zones == z {
            return;
        }
        self.zones = z.to_vec();
        let r: Vec<xproto::Rectangle> = z.iter().map(|z| xproto::Rectangle { x: z.0 as i16, y: z.1 as i16, width: z.2.max(0) as u16, height: z.3.max(0) as u16 }).collect();
        let _ = self.c.shape_rectangles(shape::SO::SET, shape::SK::INPUT, xproto::ClipOrdering::UNSORTED, self.win, 0, 0, &r);
    }

    pub(super) fn presenter(&mut self, pm: &Pixmap) {
        // Essais : LKATTA_X11_CAPTURE=fichier.png garde la dernière image envoyée.
        if let Some(f) = std::env::var_os("LKATTA_X11_CAPTURE") {
            let _ = pm.save_png(f);
        }
        let (w, h) = (pm.width() as u16, pm.height() as u16);
        let _ = self.c.put_image(xproto::ImageFormat::Z_PIXMAP, self.win, self.gc, w, h, 0, 0, 0, 32, &vers_bgra(pm.data()));
        let _ = self.c.flush();
    }

    /// La bulle prend le clavier (clic dans la bulle) ou le rend au reste du bureau.
    pub(super) fn clavier(&mut self, oui: bool) {
        let cible = if oui { self.win } else { u32::from(xproto::InputFocus::POINTER_ROOT) };
        let _ = self.c.set_input_focus(xproto::InputFocus::POINTER_ROOT, cible, x11rb::CURRENT_TIME);
        let _ = self.c.flush();
    }

    /// Forme du pointeur au-dessus du maki et des bulles (thème de curseurs du bureau).
    pub(super) fn curseur(&mut self, nom: &'static str) {
        let id = match self.curseurs_charges.get(nom) {
            Some(id) => *id,
            None => {
                let Some(h) = self.curseurs.as_ref() else { return };
                let Ok(id) = h.load_cursor(&self.c, nom) else { return };
                self.curseurs_charges.insert(nom, id);
                id
            }
        };
        let _ = self.c.change_window_attributes(self.win, &xproto::ChangeWindowAttributesAux::new().cursor(id));
        let _ = self.c.flush();
    }

    /// Symbole d'une touche (avec Maj ou Verr. maj : le deuxième symbole de la touche).
    fn symbole(&self, code: u8, maj: bool) -> u32 {
        let (premier, par, ref t) = self.touches;
        let i = (code.saturating_sub(premier) as usize) * par as usize;
        let base = t.get(i).copied().unwrap_or(0);
        let haut = t.get(i + 1).copied().unwrap_or(0);
        if maj && haut != 0 { haut } else { base }
    }

    fn propriete32(&self, w: xproto::Window, a: u32, max: u32) -> Vec<u32> {
        self.c
            .get_property(false, w, a, xproto::AtomEnum::ANY, 0, max)
            .ok()
            .and_then(|k| k.reply().ok())
            .and_then(|r| r.value32().map(|v| v.collect()))
            .unwrap_or_default()
    }

    fn texte(&self, w: xproto::Window, a: u32) -> String {
        self.c
            .get_property(false, w, a, xproto::AtomEnum::ANY, 0, 1024)
            .ok()
            .and_then(|k| k.reply().ok())
            .map(|r| String::from_utf8_lossy(&r.value).into_owned())
            .unwrap_or_default()
    }

    /// Fenêtres gérées par le gestionnaire de fenêtres et fenêtre active.
    fn fenetres(&self) -> (Vec<Vue>, xproto::Window) {
        let active = self.propriete32(self.racine, self.at.actif, 1).first().copied().unwrap_or(0);
        let mut v = vec![];
        for id in self.propriete32(self.racine, self.at.liste, 4096) {
            let mut titre = self.texte(id, self.at.nom);
            if titre.is_empty() {
                titre = self.texte(id, u32::from(xproto::AtomEnum::WM_NAME));
            }
            // WM_CLASS : « instance\0Classe\0 » ; la classe parle mieux.
            let wc = self.texte(id, u32::from(xproto::AtomEnum::WM_CLASS));
            let classe = wc.split('\0').filter(|s| !s.is_empty()).last().unwrap_or("").to_string();
            let etats = self.propriete32(id, self.at.etat, 64);
            let a = |x: u32| etats.contains(&x);
            v.push(Vue {
                f: crate::toplevels::Fenetre {
                    id,
                    titre,
                    classe,
                    active: id == active,
                    plein_ecran: a(self.at.plein),
                    agrandie: a(self.at.max_v) && a(self.at.max_h),
                    reduite: a(self.at.cache),
                },
            });
        }
        (v, active)
    }

    /// Rectangle d'une fenêtre (coordonnées globales, cadre du gestionnaire compris).
    fn rect(&self, w: xproto::Window) -> Option<(f32, f32, f32, f32)> {
        let g = self.c.get_geometry(w).ok()?.reply().ok()?;
        let t = self.c.translate_coordinates(w, self.racine, 0, 0).ok()?.reply().ok()?;
        let cadre = self.propriete32(w, self.at.cadre, 4);
        let (l, r, h, b) = if cadre.len() == 4 { (cadre[0], cadre[1], cadre[2], cadre[3]) } else { (0, 0, 0, 0) };
        Some((
            t.dst_x as f32 - l as f32,
            t.dst_y as f32 - h as f32,
            g.width as f32 + (l + r) as f32,
            g.height as f32 + (h + b) as f32,
        ))
    }

    /// Fenêtres visibles du bureau courant, de la plus basse à la plus haute (le maki saute de
    /// l'une à l'autre) : ni réduites, ni en plein écran.
    fn toits(&self, vues: &[Vue]) -> Vec<(f32, f32, f32, f32)> {
        let courant = self.propriete32(self.racine, self.at.bureau, 1).first().copied();
        let mut pile = self.propriete32(self.racine, self.at.pile, 4096);
        if pile.is_empty() {
            pile = vues.iter().map(|v| v.f.id).collect();
        }
        pile.into_iter()
            .filter(|id| vues.iter().any(|v| v.f.id == *id && !v.f.reduite && !v.f.plein_ecran))
            .filter(|id| {
                let b = self.propriete32(*id, self.at.bureau_fen, 1).first().copied();
                courant.is_none() || b.is_none() || b == Some(u32::MAX) || b == courant
            })
            .filter_map(|id| self.rect(id))
            .take(40)
            .collect()
    }

    /// Suivre la fenêtre active : son titre, son état (plein écran) et ses déplacements.
    fn suivre(&mut self, w: xproto::Window) {
        if w == self.suivie {
            return;
        }
        if self.suivie != 0 {
            let _ = self.c.change_window_attributes(self.suivie, &xproto::ChangeWindowAttributesAux::new().event_mask(xproto::EventMask::NO_EVENT));
        }
        self.suivie = w;
        if w != 0 {
            let m = xproto::EventMask::PROPERTY_CHANGE | xproto::EventMask::STRUCTURE_NOTIFY;
            let _ = self.c.change_window_attributes(w, &xproto::ChangeWindowAttributesAux::new().event_mask(m));
        }
    }

    /// Zone utile du bureau courant (hors panneaux), coordonnées globales.
    fn zone_utile(&self) -> Option<(f32, f32, f32, f32)> {
        let n = self.propriete32(self.racine, self.at.bureau, 1).first().copied().unwrap_or(0) as usize;
        let z = self.propriete32(self.racine, self.at.zone, 4 * 64);
        let r = z.get(4 * n..4 * n + 4).or_else(|| z.get(0..4))?;
        Some((r[0] as i32 as f32, r[1] as i32 as f32, r[2] as f32, r[3] as f32))
    }

    /// Message XDND à la fenêtre source d'un glisser.
    fn xdnd_envoyer(&self, source: xproto::Window, type_: u32, d: [u32; 5]) {
        let ev = xproto::ClientMessageEvent::new(32, source, type_, d);
        let _ = self.c.send_event(false, source, xproto::EventMask::NO_EVENT, ev);
        let _ = self.c.flush();
    }

    /// Types offerts par la source : dans le message, ou dans sa propriété XdndTypeList.
    fn xdnd_types(&self, source: xproto::Window, d: &[u32; 5]) -> Vec<u32> {
        if d[1] & 1 != 0 { self.propriete32(source, self.at.xdnd_types, 256) } else { d[2..5].iter().copied().filter(|t| *t != 0).collect() }
    }

    /// Action demandée par un outil, envoyée au gestionnaire de fenêtres (EWMH).
    pub(super) fn action(&self, id: xproto::Window, a: crate::toplevels::Action) {
        use crate::toplevels::Action;
        let (type_, d) = match a {
            // Source 2 : un outil de l'utilisateur (le gestionnaire obéit sans voler le focus).
            Action::Activer => (self.at.actif, [2, x11rb::CURRENT_TIME, 0, 0, 0]),
            Action::Reduire => (self.at.change_etat, [3, 0, 0, 0, 0]),
            Action::Agrandir => (self.at.etat, [2, self.at.max_v, self.at.max_h, 2, 0]),
            Action::Fermer => (self.at.fermer, [x11rb::CURRENT_TIME, 2, 0, 0, 0]),
        };
        let ev = xproto::ClientMessageEvent::new(32, id, type_, d);
        let m = xproto::EventMask::SUBSTRUCTURE_REDIRECT | xproto::EventMask::SUBSTRUCTURE_NOTIFY;
        let _ = self.c.send_event(false, self.racine, m, ev);
        let _ = self.c.flush();
    }
}

/// Zone utile d'un écran : l'intersection avec la zone de travail, si elle en garde au moins
/// la moitié (sinon la zone de travail parle d'un autre écran) ; renvoie (taille, décalage).
pub(super) fn zone_ecran(g: (f32, f32, f32, f32), z: Option<(f32, f32, f32, f32)>) -> ((f32, f32), (f32, f32)) {
    let entier = ((g.2, g.3), (0.0, 0.0));
    let Some(z) = z else { return entier };
    let (x0, y0) = (g.0.max(z.0), g.1.max(z.1));
    let (x1, y1) = ((g.0 + g.2).min(z.0 + z.2), (g.1 + g.3).min(z.1 + z.3));
    let (w, h) = (x1 - x0, y1 - y0);
    if w < g.2 * 0.5 || h < g.3 * 0.5 {
        return entier;
    }
    ((w, h), (x0 - g.0, y0 - g.1))
}

/// Écrans X11 (RandR « moniteurs » : un par écran physique, nom et géométrie).
fn ecrans(c: &impl Connection, racine: xproto::Window, larg: u16, haut: u16) -> Vec<(String, (f32, f32, f32, f32))> {
    use x11rb::protocol::randr::ConnectionExt as _;
    let mut v = vec![];
    if let Ok(Ok(r)) = c.randr_get_monitors(racine, true).map(|r| r.reply()) {
        for m in r.monitors {
            let nom = c.get_atom_name(m.name).ok().and_then(|r| r.reply().ok()).map(|n| String::from_utf8_lossy(&n.name).into_owned()).unwrap_or_default();
            v.push((nom, (m.x as f32, m.y as f32, m.width as f32, m.height as f32)));
        }
    }
    if v.is_empty() {
        v.push(("X11".into(), (0.0, 0.0, larg as f32, haut as f32)));
    }
    v
}

/// Lance le maki sur un serveur X11 (sessions XFCE, Cinnamon, MATE, i3, KDE en X11…).
pub fn run(cfg: crate::config::Config, rx: smithay_client_toolkit::reexports::calloop::channel::Channel<crate::ipc::Msg>) -> Result<(), String> {
    use smithay_client_toolkit::reexports::calloop::{self, EventLoop, generic::Generic, Interest, Mode as CMode, PostAction, timer::{TimeoutAction, Timer}};
    use std::os::fd::AsFd;
    let (c, n) = x11rb::connect(None).map_err(|e| t!("pas de serveur X ({})", e))?;
    let ecran = c.setup().roots[n].clone();
    let visuel = visuel_argb(&ecran).ok_or(t!("le serveur X n'a pas de visuel 32 bits (transparence impossible)"))?;
    c.shape_query_version().map_err(|e| e.to_string())?.reply().map_err(|_| t!("le serveur X n'a pas l'extension SHAPE"))?;
    let carte = c.generate_id().map_err(|e| e.to_string())?;
    c.create_colormap(xproto::ColormapAlloc::NONE, carte, ecran.root, visuel).map_err(|e| e.to_string())?;

    draw::set_theme(&cfg.maki.theme);
    let espece = super::especes::Espece::parse(&cfg.maki.espece).unwrap_or_default();
    draw::set_species(espece);
    let s = (cfg.maki.taille * espece.pal().size).clamp(0.4, 3.5);
    let (sw, sh) = ((470.0 * s) as u32, (560.0 * s) as u32);
    let w = c.generate_id().map_err(|e| e.to_string())?;
    let masque = xproto::EventMask::BUTTON_PRESS
        | xproto::EventMask::BUTTON_RELEASE
        | xproto::EventMask::POINTER_MOTION
        | xproto::EventMask::ENTER_WINDOW
        | xproto::EventMask::LEAVE_WINDOW
        | xproto::EventMask::KEY_PRESS
        | xproto::EventMask::FOCUS_CHANGE
        | xproto::EventMask::EXPOSURE;
    let attrs = xproto::CreateWindowAux::new().background_pixel(0).border_pixel(0).colormap(carte).override_redirect(1).event_mask(masque);
    c.create_window(32, w, ecran.root, 0, 0, sw as u16, sh as u16, 0, xproto::WindowClass::INPUT_OUTPUT, visuel, &attrs).map_err(|e| e.to_string())?;
    c.change_property8(xproto::PropMode::REPLACE, w, xproto::AtomEnum::WM_NAME, xproto::AtomEnum::STRING, b"L Katta").map_err(|e| e.to_string())?;
    c.change_property8(xproto::PropMode::REPLACE, w, xproto::AtomEnum::WM_CLASS, xproto::AtomEnum::STRING, b"lkatta\0lkatta\0").map_err(|e| e.to_string())?;
    let gc = c.generate_id().map_err(|e| e.to_string())?;
    c.create_gc(gc, w, &xproto::CreateGCAux::new()).map_err(|e| e.to_string())?;
    // Rien de cliquable tant que rien n'est dessiné.
    c.shape_rectangles(shape::SO::SET, shape::SK::INPUT, xproto::ClipOrdering::UNSORTED, w, 0, 0, &[]).map_err(|e| e.to_string())?;
    c.map_window(w).map_err(|e| e.to_string())?;
    let at = Atomes::new(&c)?;
    // Le maki accepte les fichiers glissés (XDND version 5).
    c.change_property32(xproto::PropMode::REPLACE, w, at.xdnd_aware, xproto::AtomEnum::ATOM, &[5]).map_err(|e| e.to_string())?;
    // Racine : fenêtre active, liste des fenêtres, zone de travail, bureau courant.
    c.change_window_attributes(ecran.root, &xproto::ChangeWindowAttributesAux::new().event_mask(xproto::EventMask::PROPERTY_CHANGE)).map_err(|e| e.to_string())?;
    let setup = c.setup();
    let (premier, dernier) = (setup.min_keycode, setup.max_keycode);
    let touches = c
        .get_keyboard_mapping(premier, dernier - premier + 1)
        .map_err(|e| e.to_string())?
        .reply()
        .map(|r| (premier, r.keysyms_per_keycode, r.keysyms))
        .unwrap_or((premier, 1, vec![]));
    let clavier = Clavier::new(&c, ecran.root);
    let curseurs = x11rb::resource_manager::new_from_default(&c).ok().and_then(|db| x11rb::cursor::Handle::new(&c, n, &db).ok()?.reply().ok());
    c.flush().map_err(|e| e.to_string())?;
    let liste = ecrans(&c, ecran.root, ecran.width_in_pixels, ecran.height_in_pixels);
    // Le descripteur de la connexion, pour réveiller la boucle quand X parle.
    let fd = c.stream().as_fd().try_clone_to_owned().map_err(|e| e.to_string())?;
    let aff = X11Aff {
        c,
        racine: ecran.root,
        win: w,
        gc,
        taille: (sw, sh),
        pos: (0, 0),
        zones: vec![],
        a_placer: true,
        touches,
        clavier,
        at,
        suivie: 0,
        curseurs,
        curseurs_charges: Default::default(),
        lire_fenetres: true,
        lire_active: true,
        lire_zone: false,
        glisse: None,
    };
    let zone = aff.zone_utile();

    let mut boucle: EventLoop<'static, super::App> = EventLoop::try_new().map_err(|e| e.to_string())?;
    let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(7);
    let mut app = super::App::nouvelle(cfg, None, Some(aff), boucle.handle(), (sw, sh, s), espece, seed)?;
    for (i, (nom, g)) in liste.into_iter().enumerate() {
        let (area, decalage) = zone_ecran(g, zone);
        app.screens.push(super::Screen { out: super::Sortie::X(i as u32), probe: None, area: Some(area), geom: Some(g), nom, decalage });
    }
    app.current = app.screens.first().map(|s| s.out.clone());
    app.area = app.screens.first().and_then(|s| s.area);
    app.configured = true;
    app.configured_once = false;
    if std::env::var_os("LKATTA_DEBUG").is_some() {
        eprintln!("x11 : clavier {} · zone de travail {:?}", if app.x11.as_ref().is_some_and(|x| x.clavier.is_some()) { "xkb + composition" } else { "table simple" }, zone);
    }

    boucle
        .handle()
        .insert_source(rx, |ev, _, app: &mut super::App| {
            if let calloop::channel::Event::Msg(m) = ev {
                app.on_msg(m);
            }
        })
        .map_err(|e| e.to_string())?;
    boucle
        .handle()
        .insert_source(Timer::from_duration(Duration::from_millis(50)), |_, _, app: &mut super::App| {
            // Des événements ont pu être lus pendant une réponse attendue ailleurs : le
            // descripteur ne se réveillerait pas pour eux.
            evenements(app);
            TimeoutAction::ToDuration(app.tick())
        })
        .map_err(|e| e.to_string())?;
    boucle
        .handle()
        .insert_source(Generic::new(fd, Interest::READ, CMode::Level), |_, _, app: &mut super::App| {
            evenements(app);
            Ok(PostAction::Continue)
        })
        .map_err(|e| e.to_string())?;
    while !app.exit {
        boucle.dispatch(Duration::from_millis(500), &mut app).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Événements X11 → la même logique que sous Wayland (pointeur, clavier, fenêtre active).
fn evenements(app: &mut super::App) {
    use smithay_client_toolkit::seat::pointer::{AxisScroll, PointerEventKind as K};
    let mut lus = vec![];
    if let Some(x) = app.x11.as_ref() {
        loop {
            match x.c.poll_for_event() {
                Ok(Some(ev)) => lus.push(ev),
                Ok(None) => break,
                // Serveur X parti (session fermée) : le maki s'arrête au lieu de tourner à vide.
                Err(e) => {
                    eprintln!("lkatta : serveur X perdu ({e}), arrêt");
                    app.exit = true;
                    return;
                }
            }
        }
    }
    let mut redessiner = false;
    for ev in lus {
        match ev {
            Event::ButtonPress(e) | Event::ButtonRelease(e) => {
                let appui = e.response_type & 0x7f == xproto::BUTTON_PRESS_EVENT;
                let pos = (e.event_x as f32, e.event_y as f32);
                let kind = match e.detail {
                    4 | 5 if appui => {
                        let d = if e.detail == 4 { -1 } else { 1 };
                        let v = AxisScroll { absolute: d as f64 * 10.0, discrete: d, ..Default::default() };
                        K::Axis { time: e.time, horizontal: AxisScroll::default(), vertical: v, source: None }
                    }
                    4 | 5 => continue,
                    b => {
                        let button = match b {
                            1 => super::BTN_LEFT,
                            2 => super::BTN_MIDDLE,
                            3 => super::BTN_RIGHT,
                            _ => continue,
                        };
                        if appui { K::Press { time: e.time, button, serial: 0 } } else { K::Release { time: e.time, button, serial: 0 } }
                    }
                };
                redessiner |= app.pointeur(&kind, pos);
            }
            Event::MotionNotify(e) => redessiner |= app.pointeur(&K::Motion { time: e.time }, (e.event_x as f32, e.event_y as f32)),
            Event::EnterNotify(e) => redessiner |= app.pointeur(&K::Enter { serial: 0 }, (e.event_x as f32, e.event_y as f32)),
            Event::LeaveNotify(e) => redessiner |= app.pointeur(&K::Leave { serial: 0 }, (e.event_x as f32, e.event_y as f32)),
            Event::KeyPress(e) => {
                let etat = u16::from(e.state);
                app.ctrl = etat & u16::from(xproto::KeyButMask::CONTROL) != 0;
                app.shift = etat & u16::from(xproto::KeyButMask::SHIFT) != 0;
                let Some(x) = app.x11.as_mut() else { continue };
                let (keysym, utf8) = match x.clavier.as_mut() {
                    Some(k) => match k.touche(e.detail, etat) {
                        Some(r) => r,
                        // Touche morte : on attend la lettre suivante.
                        None => continue,
                    },
                    None => {
                        let maj = etat & (u16::from(xproto::KeyButMask::SHIFT) | u16::from(xproto::KeyButMask::LOCK)) != 0;
                        let k = smithay_client_toolkit::seat::keyboard::Keysym::from(x.symbole(e.detail, maj));
                        (k, k.key_char().map(|c| c.to_string()))
                    }
                };
                let utf8 = utf8.filter(|t| t.chars().all(|c| !c.is_control() || c == '\r'));
                app.on_key(smithay_client_toolkit::seat::keyboard::KeyEvent { time: e.time, raw_code: e.detail as u32, keysym, utf8 });
                redessiner = true;
            }
            Event::FocusIn(_) => app.kb_focus = true,
            Event::FocusOut(_) => app.kb_focus = false,
            Event::Expose(_) => redessiner = true,
            Event::MappingNotify(_) => {
                // Disposition changée (setxkbmap, réglages du bureau).
                if let Some(x) = app.x11.as_mut() {
                    x.clavier = Clavier::new(&x.c, x.racine);
                }
            }
            Event::PropertyNotify(e) => {
                if let Some(x) = app.x11.as_mut() {
                    if e.window == x.racine {
                        if e.atom == x.at.actif || e.atom == x.at.liste {
                            x.lire_fenetres = true;
                            x.lire_active = true;
                        } else if e.atom == x.at.zone || e.atom == x.at.bureau {
                            x.lire_zone = true;
                        }
                    } else if e.window == x.suivie && (e.atom == x.at.etat || e.atom == x.at.nom || e.atom == u32::from(xproto::AtomEnum::WM_NAME)) {
                        x.lire_fenetres = true;
                        x.lire_active = true;
                    }
                }
            }
            Event::ClientMessage(e) => xdnd(app, &e),
            Event::SelectionNotify(e) => {
                let Some(x) = app.x11.as_mut() else { continue };
                if e.requestor != x.win || e.selection != x.at.xdnd_selection {
                    continue;
                }
                let source = x.glisse.take().map(|g| g.0);
                let donnees = if e.property == 0 {
                    vec![]
                } else {
                    let r = x.c.get_property(true, x.win, e.property, xproto::AtomEnum::ANY, 0, 1 << 18).ok().and_then(|k| k.reply().ok());
                    r.map(|r| r.value).unwrap_or_default()
                };
                if let Some(s) = source {
                    let ok = u32::from(!donnees.is_empty());
                    x.xdnd_envoyer(s, x.at.xdnd_finished, [x.win, ok, if ok == 1 { x.at.xdnd_copie } else { 0 }, 0, 0]);
                }
                app.depot_sortie();
                app.fichiers_deposes(&donnees);
            }
            Event::ConfigureNotify(e) => {
                if let Some(x) = app.x11.as_mut() {
                    if e.window == x.suivie {
                        x.lire_active = true;
                    }
                }
            }
            _ => {}
        }
    }
    redessiner |= bureau(app);
    if redessiner {
        app.render();
    }
}

/// Glisser-déposer XDND : la source annonce le glisser, le promène, puis le lâche ; on demande
/// alors la liste des fichiers (`text/uri-list`) par la sélection XdndSelection.
fn xdnd(app: &mut super::App, e: &xproto::ClientMessageEvent) {
    let Some(x) = app.x11.as_mut() else { return };
    if e.window != x.win || e.format != 32 {
        return;
    }
    let d = e.data.as_data32();
    let source = d[0];
    let t = e.type_;
    if t == x.at.xdnd_enter {
        let version = d[1] >> 24;
        let fichiers = x.xdnd_types(source, &d).contains(&x.at.uri_list);
        x.glisse = Some((source, version, fichiers));
        if fichiers {
            app.depot_entree();
        }
    } else if t == x.at.xdnd_position {
        let fichiers = x.glisse.is_some_and(|g| g.0 == source && g.2);
        // Accepté (bit 0), et des positions encore (bit 1) : la réponse ne dépend pas de l'endroit.
        let action = if fichiers { x.at.xdnd_copie } else { 0 };
        x.xdnd_envoyer(source, x.at.xdnd_status, [x.win, if fichiers { 3 } else { 2 }, 0, 0, action]);
    } else if t == x.at.xdnd_leave {
        x.glisse = None;
        app.depot_sortie();
    } else if t == x.at.xdnd_drop {
        let fichiers = x.glisse.is_some_and(|g| g.0 == source && g.2);
        if !fichiers {
            x.glisse = None;
            x.xdnd_envoyer(source, x.at.xdnd_finished, [x.win, 0, 0, 0, 0]);
            app.depot_sortie();
            return;
        }
        // Heure du lâcher (version ≥ 1) ; la réponse arrive en SelectionNotify.
        let quand = if x.glisse.is_some_and(|g| g.1 >= 1) { d[2] } else { x11rb::CURRENT_TIME };
        let _ = x.c.convert_selection(x.win, x.at.xdnd_selection, x.at.uri_list, x.at.depot, quand);
        let _ = x.c.flush();
    }
}

/// Fenêtres et zone de travail relues après un lot d'événements. Renvoie vrai s'il faut
/// redessiner.
fn bureau(app: &mut super::App) -> bool {
    let Some(x) = app.x11.as_mut() else { return false };
    let mut redessiner = false;
    if std::mem::take(&mut x.lire_zone) {
        let z = x.zone_utile();
        x.a_placer = true;
        for s in app.screens.iter_mut() {
            if let Some(g) = s.geom {
                let (area, d) = zone_ecran(g, z);
                s.area = Some(area);
                s.decalage = d;
            }
        }
        app.area = app.current.as_ref().and_then(|c| app.screens.iter().find(|s| &s.out == c)).and_then(|s| s.area);
        redessiner = true;
    }
    let Some(x) = app.x11.as_mut() else { return redessiner };
    let fenetres = std::mem::take(&mut x.lire_fenetres);
    let active = std::mem::take(&mut x.lire_active);
    if !fenetres && !active {
        return redessiner;
    }
    let (vues, id) = x.fenetres();
    x.suivre(id);
    let rect = if id != 0 { x.rect(id) } else { None };
    let toits = x.toits(&vues);
    let _ = x.c.flush();
    if fenetres {
        crate::toplevels::marquer_disponible();
        let ids: Vec<u32> = vues.iter().map(|v| v.f.id).collect();
        for f in crate::toplevels::liste() {
            if !ids.contains(&f.id) {
                crate::toplevels::mettre_a_jour(f.id, None);
            }
        }
        for v in &vues {
            crate::toplevels::mettre_a_jour(v.f.id, Some(v.f.clone()));
        }
    }
    // Un pont du bureau (extension GNOME, script KWin) donne déjà la fenêtre active, applications
    // Wayland comprises : on ne le contredit pas.
    if crate::bureau::pont_actif() {
        return true;
    }
    let f = vues.into_iter().find(|v| v.f.id == id).map(|v| v.f);
    let rect = rect.unwrap_or((0.0, 0.0, 0.0, 0.0));
    let centre = (rect.0 + rect.2 / 2.0, rect.1 + rect.3 / 2.0);
    let ecran = app
        .screens
        .iter()
        .find(|s| s.geom.is_some_and(|g| centre.0 >= g.0 && centre.0 < g.0 + g.2 && centre.1 >= g.1 && centre.1 < g.1 + g.3))
        .map(|s| s.nom.clone())
        .unwrap_or_default();
    let f = f.unwrap_or_default();
    app.on_fenetre(crate::bureau::Fenetre { plein_ecran: f.plein_ecran, rect, classe: f.classe, titre: f.titre, ecran, toits, ..Default::default() });
    true
}

#[cfg(test)]
mod tests {
    #[test]
    fn french_keyboard_dead_keys_and_altgr() {
        // Disposition française (codes X = evdev + 8) ; composition de la locale fr_FR.UTF-8.
        let Some(mut k) = super::Clavier::depuis_noms("evdev", "pc105", "fr", "", "") else { return };
        let (_, a) = k.touche(24, 0).unwrap();
        assert_eq!(a.as_deref(), Some("a"));
        let (_, a) = k.touche(24, 1).unwrap();
        assert_eq!(a.as_deref(), Some("A"));
        // AltGr (Mod5) + e → €.
        let (_, e) = k.touche(26, 0x80).unwrap();
        assert_eq!(e.as_deref(), Some("€"));
        if k.compose.is_some() {
            // ^ (touche morte) puis e → ê ; Maj+^ (¨) puis i → ï.
            assert!(k.touche(34, 0).is_none());
            assert_eq!(k.touche(26, 0).unwrap().1.as_deref(), Some("ê"));
            assert!(k.touche(34, 1).is_none());
            assert_eq!(k.touche(31, 0).unwrap().1.as_deref(), Some("ï"));
        }
    }

    #[test]
    fn work_area() {
        // Écran 1920×1080 en (0,0), panneau de 40 px en haut : zone 1920×1040 décalée de 40.
        let g = (0.0, 0.0, 1920.0, 1080.0);
        assert_eq!(super::zone_ecran(g, Some((0.0, 40.0, 1920.0, 1040.0))), ((1920.0, 1040.0), (0.0, 40.0)));
        assert_eq!(super::zone_ecran(g, None), ((1920.0, 1080.0), (0.0, 0.0)));
        // Zone de travail d'un autre écran (au-dessus) : on garde l'écran entier.
        assert_eq!(super::zone_ecran((0.0, 1080.0, 1920.0, 1080.0), Some((0.0, 0.0, 1920.0, 1100.0))), ((1920.0, 1080.0), (0.0, 0.0)));
    }

    #[test]
    fn swaps_channels() {
        assert_eq!(super::vers_bgra(&[1, 2, 3, 4, 5, 6, 7, 8]), vec![3, 2, 1, 4, 7, 6, 5, 8]);
    }
}
