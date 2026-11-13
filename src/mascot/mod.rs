//! Surcouche Wayland du maki (layer-shell, rendu logiciel).
//!
//! Une surface sonde invisible mesure la zone utile de l'écran (hors panneaux) ; une
//! petite surface transparente suit le maki partout dans cette zone en changeant ses
//! marges. Seuls le maki et ses bulles reçoivent les clics ; la bulle de discussion
//! prend le clavier le temps de taper.

pub mod behavior;
pub mod draw;
pub mod dessin;
pub mod especes;

use std::time::{Duration, Instant};

use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState, Region},
    delegate_compositor, delegate_keyboard, delegate_layer, delegate_output, delegate_pointer, delegate_registry,
    delegate_seat, delegate_shm,
    output::{OutputHandler, OutputState},
    reexports::{
        calloop::{
            EventLoop, LoopHandle,
            channel::{self, Channel},
            timer::{TimeoutAction, Timer},
        },
        calloop_wayland_source::WaylandSource,
    },
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        Capability, SeatHandler, SeatState,
        keyboard::{KeyEvent, KeyboardHandler, Keysym, Modifiers, RawModifiers},
        pointer::{CursorIcon, PointerEvent, PointerEventKind, PointerHandler, ThemeSpec, ThemedPointer},
    },
    shell::{
        WaylandSurface,
        wlr_layer::{Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface, LayerSurfaceConfigure},
    },
    shm::{Shm, ShmHandler, slot::{Buffer, SlotPool}},
};
use tiny_skia::{Pixmap, Transform};
use wayland_client::{
    Connection, QueueHandle,
    globals::registry_queue_init,
    protocol::{wl_keyboard, wl_output, wl_pointer, wl_seat, wl_shm, wl_surface},
};

use crate::chat::{Chat, Signal};
use wayland_client::Dispatch;
use wayland_protocols::ext::idle_notify::v1::client::{
    ext_idle_notification_v1::{self, ExtIdleNotificationV1},
    ext_idle_notifier_v1::ExtIdleNotifierV1,
};
use crate::conseils::{Kind, Message};
use crate::config::Config;
use crate::ipc::Msg;
use behavior::{Ctx, Maki, Mode, PoseKind, Tree};
use draw::{ChatLayout, R, hit};

const BTN_LEFT: u32 = 0x110;
const BTN_RIGHT: u32 = 0x111;
const BTN_MIDDLE: u32 = 0x112;

struct Press {
    button: u32,
    at: (f32, f32),
    on_maki: bool,
}

/// Un écran et sa sonde (mesure de la zone utile).
struct Screen {
    out: wl_output::WlOutput,
    probe: LayerSurface,
    area: Option<(f32, f32)>,
}

fn make_main(shell: &LayerShell, comp: &CompositorState, qh: &QueueHandle<App>, out: Option<&wl_output::WlOutput>, sw: u32, sh: u32, k: KeyboardInteractivity) -> LayerSurface {
    let layer = shell.create_layer_surface(qh, comp.create_surface(qh), Layer::Top, Some("lkatta"), out);
    layer.set_anchor(Anchor::TOP | Anchor::LEFT);
    layer.set_keyboard_interactivity(k);
    layer.set_exclusive_zone(0);
    layer.set_size(sw, sh);
    layer.commit();
    layer
}

struct App {
    conn: Connection,
    registry_state: RegistryState,
    seat_state: SeatState,
    output_state: OutputState,
    compositor: CompositorState,
    shm: Shm,
    pool: SlotPool,
    /// Deux tampons d'image réutilisés (double tampon). Jamais plus : si le compositeur les
    /// garde tous (écran éteint, session verrouillée), on attend au lieu d'en créer d'autres.
    tampons: Vec<Buffer>,
    layer: LayerSurface,
    layer_shell: LayerShell,
    qh: QueueHandle<App>,
    screens: Vec<Screen>,
    current: Option<wl_output::WlOutput>,
    /// Bureau virtuel courant et place du maki sur chaque bureau (écran, x).
    desktop: String,
    desk_pos: std::collections::HashMap<String, (Option<wl_output::WlOutput>, f32)>,
    pointer: Option<ThemedPointer>,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    loop_handle: LoopHandle<'static, App>,
    kb_focus: bool,
    ctrl: bool,
    shift: bool,
    interactivity: KeyboardInteractivity,
    configured: bool,
    exit: bool,

    sw: u32,
    sh: u32,
    s: f32,
    origin: (i32, i32),
    area: Option<(f32, f32)>,
    pixmap: Pixmap,
    /// Calque du maki (contour clair appliqué à lui seul).
    sprite: Pixmap,
    /// Calque des bulles (fondu) et zone qu'il occupait à l'image précédente.
    bubble_pm: Pixmap,
    bubble_zone: Option<R>,
    /// Ce que contient déjà le calque des bulles : on ne le redessine que s'il change.
    bubble_key: Option<(String, usize, i32, i32, i32)>,
    bubble_lay: Option<draw::BubbleLayout>,
    /// Une image attend encore d'être affichée par le compositeur.
    frame_pending: Option<Instant>,
    /// Un dessin a été demandé pendant qu'une image était en route : il suit sans attendre.
    redessin_attendu: bool,
    /// L'origine de la surface a déjà été placée au moins une fois.
    configured_once: bool,
    text: Option<draw::Text>,

    maki: Maki,
    entered: bool,
    chat: Chat,
    chat_layout: Option<ChatLayout>,
    bubble_layout: Option<draw::BubbleLayout>,
    bold: Option<draw::Text>,
    maki_rect: R,
    cfg: Config,
    quiet: bool,
    hidden: bool,
    last_tick: Instant,
    last_spontaneous: Option<Instant>,

    pointer_world: Option<(f32, f32)>,
    hovered: bool,
    /// Le pointeur est passé sur la petite bulle affichée : quand il la quitte, elle se ferme.
    bulle_survolee: bool,
    /// Capture d'écran en cours : rien n'est dessiné.
    capture: bool,
    cursor: CursorIcon,
    press: Option<Press>,
    dragging: bool,
    last_drag: Instant,
    /// Relance prévue (changement d'espèce ou de taille).
    restart_at: Option<Instant>,
    theme_check: Instant,
    /// Changement de taille de la surface en attente de confirmation du compositeur.
    resizing: bool,
    /// Densité de l'écran (1, 2…) : on dessine à cette échelle pour un rendu net.
    dpr: u32,
    /// Écran actif selon KWin et depuis quand.
    active_out: Option<(String, Instant)>,
    /// Écran où il doit réapparaître après son nuage.
    pending_out: Option<(wl_output::WlOutput, bool)>,
    /// Le pointeur a quitté la surface pendant qu'on le tenait (vers un autre écran ?).
    drag_left: Option<Instant>,
    /// Mesure du temps de rendu (LKATTA_PERF=1) : (images, secondes).
    perf: Option<(u32, f64)>,
    // --- discrétion ---
    /// Fenêtre active (script KWin).
    fenetre: Option<crate::bureau::Fenetre>,
    ne_pas_deranger: bool,
    micro: bool,
    /// Absent (aucune activité clavier/souris) depuis.
    absent: Option<Instant>,
    /// Messages mis de côté pendant une visio, « Ne pas déranger » ou une absence.
    en_attente: Vec<Message>,
    etait_occupe: bool,
    /// Début de la visio / du « Ne pas déranger » en cours (pour compter les notifications).
    occupe_depuis: Option<chrono::DateTime<chrono::Local>>,
    /// Relance (nouvelle espèce ou taille) attendue la fin de la réponse en cours.
    relance_differee: bool,
    /// Premier lancement : moment où ouvrir l'assistant d'accueil.
    accueil_a: Option<Instant>,
    dbg_taille: (u32, u32),
    idle_notifier: Option<ExtIdleNotifierV1>,
    idle: Option<ExtIdleNotificationV1>,
}

pub fn run(cfg: Config, rx: Channel<Msg>) -> Result<(), String> {
    let conn = Connection::connect_to_env().map_err(|e| format!("pas de session Wayland : {e}"))?;
    let (globals, event_queue) = registry_queue_init::<App>(&conn).map_err(|e| e.to_string())?;
    let qh = event_queue.handle();
    let mut event_loop: EventLoop<'static, App> = EventLoop::try_new().map_err(|e| e.to_string())?;
    WaylandSource::new(conn.clone(), event_queue).insert(event_loop.handle()).map_err(|e| e.to_string())?;

    let compositor = CompositorState::bind(&globals, &qh).map_err(|_| "wl_compositor absent")?;
    let layer_shell = LayerShell::bind(&globals, &qh)
        .map_err(|_| "le compositeur ne gère pas wlr-layer-shell (KDE, Sway, Hyprland le gèrent ; GNOME non)")?;
    let shm = Shm::bind(&globals, &qh).map_err(|_| "wl_shm absent")?;

    draw::set_theme(&cfg.maki.theme);
    let espece = especes::Espece::parse(&cfg.maki.espece).unwrap_or_default();
    draw::set_species(espece);
    // Taille : réglage × gabarit de l'espèce (l'indri est grand, le microcèbe minuscule).
    let s = (cfg.maki.taille * espece.pal().size).clamp(0.4, 3.5);
    let (sw, sh) = ((470.0 * s) as u32, (560.0 * s) as u32);
    let layer = make_main(&layer_shell, &compositor, &qh, None, sw, sh, KeyboardInteractivity::None);

    let pool = SlotPool::new((sw * sh * 4) as usize * 2, &shm).map_err(|e| e.to_string())?;
    let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(7);

    let mut app = App {
        conn: conn.clone(),
        registry_state: RegistryState::new(&globals),
        seat_state: SeatState::new(&globals, &qh),
        output_state: OutputState::new(&globals, &qh),
        compositor,
        shm,
        pool,
        tampons: Vec::new(),
        layer,
        layer_shell,
        qh: qh.clone(),
        screens: vec![],
        current: None,
        desktop: crate::kde::current().unwrap_or_default(),
        desk_pos: Default::default(),
        pointer: None,
        keyboard: None,
        loop_handle: event_loop.handle(),
        kb_focus: false,
        ctrl: false,
        shift: false,
        interactivity: KeyboardInteractivity::None,
        configured: false,
        exit: false,
        sw,
        sh,
        s,
        origin: (0, 0),
        area: None,
        pixmap: Pixmap::new(sw, sh).ok_or("taille invalide")?,
        sprite: Pixmap::new(sw, sh).ok_or("taille invalide")?,
        bubble_pm: Pixmap::new(sw, sh).ok_or("taille invalide")?,
        bubble_zone: None,
        bubble_key: None,
        bubble_lay: None,
        frame_pending: None,
        redessin_attendu: false,
        configured_once: false,
        text: draw::Text::load(),
        maki: {
            let mut m = Maki::new(400.0, s, seed);
            m.sp = espece;
            m
        },
        restart_at: None,
        theme_check: Instant::now(),
        dpr: 1,
        resizing: false,
        active_out: None,
        pending_out: None,
        drag_left: None,
        perf: std::env::var("LKATTA_PERF").ok().map(|_| (0, 0.0)),
        fenetre: None,
        ne_pas_deranger: false,
        micro: false,
        absent: None,
        en_attente: vec![],
        etait_occupe: false,
        occupe_depuis: None,
        relance_differee: false,
        accueil_a: (!cfg.accueil).then(|| Instant::now() + Duration::from_secs(4)),
        dbg_taille: (0, 0),
        idle_notifier: globals.bind::<ExtIdleNotifierV1, _, _>(&qh, 1..=1, ()).ok(),
        idle: None,
        entered: false,
        chat: Chat::new(cfg.clone()),
        chat_layout: None,
        bubble_layout: None,
        bold: draw::load_bold(),
        maki_rect: (0.0, 0.0, 0.0, 0.0),
        cfg,
        quiet: false,
        hidden: false,
        last_tick: Instant::now(),
        last_spontaneous: None,
        pointer_world: None,
        hovered: false,
        bulle_survolee: false,
        capture: false,
        cursor: CursorIcon::Default,
        press: None,
        dragging: false,
        last_drag: Instant::now(),
    };
    if app.text.is_none() {
        eprintln!("lkatta : aucune police trouvée, bulles muettes");
    }

    event_loop
        .handle()
        .insert_source(rx, |ev, _, app: &mut App| {
            if let channel::Event::Msg(m) = ev {
                app.on_msg(m);
            }
        })
        .map_err(|e| e.to_string())?;
    event_loop
        .handle()
        .insert_source(Timer::from_duration(Duration::from_millis(50)), |_, _, app: &mut App| {
            TimeoutAction::ToDuration(app.tick())
        })
        .map_err(|e| e.to_string())?;

    while !app.exit {
        event_loop.dispatch(Duration::from_millis(500), &mut app).map_err(|e| e.to_string())?;
    }
    Ok(())
}

impl App {
    fn geom(&self, o: &wl_output::WlOutput) -> Option<(f32, f32, f32, f32)> {
        let i = self.output_state.info(o)?;
        let (x, y) = i.logical_position?;
        let (w, h) = i.logical_size?;
        Some((x as f32, y as f32, w as f32, h as f32))
    }

    /// Écran voisin dans une direction (bords jointifs qui se recouvrent).
    fn neighbor(&self, e: behavior::Exit) -> Option<wl_output::WlOutput> {
        use behavior::Exit;
        let cur = self.current.as_ref()?;
        let (x, y, w, h) = self.geom(cur)?;
        let near = |a: f32, b: f32| (a - b).abs() <= 4.0;
        let overlap = |a0: f32, a1: f32, b0: f32, b1: f32| a0 < b1 && b0 < a1;
        self.screens.iter().map(|s| &s.out).filter(|o| *o != cur).find(|o| {
            let Some((x2, y2, w2, h2)) = self.geom(o) else { return false };
            match e {
                Exit::Right => near(x2, x + w) && overlap(y, y + h, y2, y2 + h2),
                Exit::Left => near(x2 + w2, x) && overlap(y, y + h, y2, y2 + h2),
                Exit::Down => near(y2, y + h) && overlap(x, x + w, x2, x2 + w2),
                Exit::Up => near(y2 + h2, y) && overlap(x, x + w, x2, x2 + w2),
            }
        }).cloned()
    }

    fn exits(&self) -> behavior::Exits {
        use behavior::Exit;
        if !self.cfg.maki.ecrans_multiples {
            return behavior::Exits::default();
        }
        behavior::Exits {
            left: self.neighbor(Exit::Left).is_some(),
            right: self.neighbor(Exit::Right).is_some(),
            up: self.neighbor(Exit::Up).is_some(),
            down: self.neighbor(Exit::Down).is_some(),
        }
    }

    /// Déplace la surface du maki sur un autre écran.
    fn move_to(&mut self, o: wl_output::WlOutput) {
        let k = if self.chat.open { KeyboardInteractivity::OnDemand } else { KeyboardInteractivity::None };
        self.layer = make_main(&self.layer_shell, &self.compositor, &self.qh, Some(&o), self.sw, self.sh, k);
        // Nouvelle surface : tampons neufs (l'ancienne surface rend les siens en disparaissant).
        self.tampons.clear();
        self.interactivity = k;
        self.configured = false;
        self.configured_once = false;
        self.frame_pending = None;
        self.origin = (0, 0);
        self.kb_focus = false;
        self.current = Some(o);
    }

    fn area(&self) -> (f32, f32) {
        let cur = self.current.as_ref().and_then(|c| self.screens.iter().find(|s| &s.out == c)).and_then(|s| s.area);
        cur.or(self.area).unwrap_or_else(|| {
            self.output_state
                .outputs()
                .next()
                .and_then(|o| self.output_state.info(&o))
                .and_then(|i| i.logical_size)
                .map(|(w, h)| (w as f32, h as f32))
                .unwrap_or((1920.0, 1080.0))
        })
    }

    fn ctx(&self) -> Ctx {
        use chrono::Timelike;
        let (w, h) = self.area();
        let hour = chrono::Local::now().hour();
        let [a, b] = self.cfg.maki.sommeil;
        let night = if a == b { false } else if a < b { hour >= a && hour < b } else { hour >= a || hour < b };
        Ctx {
            w,
            h,
            night: night || self.absent.is_some(),
            day: (6..18).contains(&hour),
            quiet: self.quiet || self.occupe(),
            acrobatics: self.cfg.maki.acrobaties,
            show_gap_min: self.cfg.maki.numeros_min.max(1) as f32,
            hovered: self.hovered,
            pointer: self.pointer_world,
            // Bulle ouverte ou réponse en cours : il reste sur place.
            chatting: self.fige(),
            thinking: self.chat.busy,
            bubble_hover: self.pointer_world.zip(self.bubble_layout.as_ref()).is_some_and(|(p, l)| {
                hit(l.rect, self.dev((p.0 - self.origin.0 as f32, p.1 - self.origin.1 as f32)))
            }),
            band: if self.cfg.maki.zone == "libre" { 0.0 } else { 0.17 },
            exits: self.exits(),
            avoid: self.avoid_range(),
            toit: if self.cfg.maki.acrobaties { self.toit() } else { None },
        }
    }

    /// Haut de la fenêtre active sur l'écran du maki, s'il y a la place au-dessus pour lui
    /// (pas en plein écran, pas une fenêtre collée en haut, assez large).
    fn toit(&self) -> Option<(f32, f32, f32)> {
        let f = self.fenetre.as_ref()?;
        if f.plein_ecran || f.rect.2 <= 0.0 {
            return None;
        }
        if !f.ecran.is_empty() && Some(f.ecran.clone()) != self.current_name() {
            return None;
        }
        let g = self.current.as_ref().and_then(|c| self.geom(c))?;
        let (w, h) = self.area();
        let s = self.s;
        let x0 = (f.rect.0 - g.0).max(0.0);
        let x1 = (f.rect.0 + f.rect.2 - g.0).min(w);
        let y = f.rect.1 - g.1;
        (x1 - x0 >= 260.0 * s && y >= 170.0 * s && y <= h - 120.0 * s).then_some((x0, x1, y))
    }

    fn bubble_secs(&self, text: &str) -> f32 {
        self.cfg.maki.bulle_s as f32 + text.split_whitespace().count() as f32 * 0.25
    }

    /// Montre un message : dans la discussion si elle est ouverte, sinon dans une bulle.
    fn show(&mut self, m: Message) {
        if self.chat.open {
            let t = if m.title.is_empty() { m.text.clone() } else { format!("{} — {}", m.title, m.text) };
            self.chat.entries.push(crate::chat::Entry::Maki(t));
            self.maki.react(m.kind);
        } else {
            let secs = self.bubble_secs(&m.text);
            self.maki.show(m, secs);
        }
    }

    /// Remarque spontanée : soumise à la discrétion et à l'intervalle entre deux paroles.
    fn spontaneous(&mut self, m: Message) {
        let gap = Duration::from_secs(self.cfg.maki.parole_min as u64 * 60);
        if self.occupe() || self.absent.is_some() {
            self.mettre_de_cote(m);
            return;
        }
        let busy = self.chat.open || self.maki.bubble.is_some() || self.dragging;
        if !self.quiet && !self.hidden && !busy && self.last_spontaneous.is_none_or(|l| l.elapsed() >= gap) {
            self.last_spontaneous = Some(Instant::now());
            self.show(m);
        }
    }

    /// Boutons proposés sous une bulle selon son genre.
    fn actions_for(m: &Message) -> Vec<&'static str> {
        match m.kind {
            Kind::Hello => vec!["Discuter", "Une astuce"],
            Kind::Tip => vec!["Autre astuce", "Merci"],
            Kind::Reminder => vec!["C'est noté", "Dans 10 min"],
            Kind::Alert => vec!["Qu'en faire ?", "Plus tard"],
            Kind::News if m.link.as_deref().is_some_and(|l| l.starts_with("maj:")) => vec!["Mettre à jour", "Plus tard"],
            Kind::News if m.link.is_some() => vec!["Lire", "OK"],
            Kind::Answer => vec!["Lire en entier", "OK"],
            Kind::Mail => vec!["Résumer", "Plus tard"],
            _ => vec![],
        }
    }

    fn bubble_action(&mut self, label: &str) {
        let Some(b) = self.maki.bubble.take() else { return };
        match label {
            "Discuter" | "Lire en entier" => {
                if !self.chat.open {
                    self.toggle_chat();
                }
            }
            "Une astuce" | "Autre astuce" => self.show(crate::conseils::astuce()),
            "Dans 10 min" => {
                let when = chrono::Local::now() + chrono::Duration::minutes(10);
                let _ = crate::reminders::add(when, &b.msg.text);
                self.maki.say(t!("D'accord, je te le redis dans 10 minutes."), 3.5);
            }
            "Qu'en faire ?" => {
                if !self.chat.open {
                    self.toggle_chat();
                }
                self.chat.input.set(t!("{} : {}. Explique-moi simplement et dis-moi quoi faire.", b.msg.title, b.msg.text));
                self.chat.submit();
            }
            "Résumer" => {
                if !self.chat.open {
                    self.toggle_chat();
                }
                self.chat.input.set(t!("Résume mes nouveaux mails non lus en quelques lignes, du plus important au moins important."));
                self.chat.submit();
            }
            "Mettre à jour" => {
                // Le terminal de commande fait le travail (téléchargement, vérification, relance).
                {
                    let _ = std::process::Command::new(crate::paths::self_exe()).arg("maj").env("APPIMAGELAUNCHER_DISABLE", "1").stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn();
                }
                self.maki.say(t!("Je télécharge la nouvelle version…"), 6.0);
            }
            "Lire" => {
                if let Some(l) = &b.msg.link {
                    let _ = std::process::Command::new("xdg-open").arg(l).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn();
                }
            }
            _ => self.maki.set_expr(behavior::Expr::Happy, 1.5),
        }
    }

    fn on_msg(&mut self, m: Msg) {
        let ctx = self.ctx();
        match m {
            Msg::Say(t) => self.show(Message::new(Kind::Info, "", t)),
            Msg::Reminder(t) => self.show(Message::new(Kind::Reminder, t!("Rappel"), t)),
            Msg::Hint(t) => self.spontaneous(Message::new(Kind::Info, "", t)),
            // Occupé ou absent : astuces, nouvelles et mails attendent ; les rappels passent.
            Msg::Bubble(m, _) if (self.occupe() || self.absent.is_some()) && matches!(m.kind, Kind::Mail | Kind::News | Kind::Tip | Kind::Alert) => {
                self.mettre_de_cote(m)
            }
            Msg::Bubble(m, true) => self.show(m),
            Msg::Bubble(m, false) => self.spontaneous(m),
            Msg::Flip | Msg::Tree | Msg::Teleport | Msg::Climb | Msg::Screen if self.fige() => {}
            Msg::Flip => self.maki.flip(),
            Msg::Wave => self.maki.wave(),
            Msg::Tree => self.maki.tree_show(&ctx),
            Msg::Teleport => self.maki.teleport(&ctx),
            Msg::Climb => self.maki.climb_edge(&ctx),
            Msg::Chat => self.toggle_chat(),
            Msg::Raccourci => {
                self.toggle_chat();
                if self.chat.open {
                    crate::bureau::focaliser_bulle();
                }
            }
            Msg::Screen => self.other_screen(),
            Msg::Reload => self.reload(),
            Msg::ActiveOutput(n) => self.on_active_output(n),
            Msg::Fenetre(f) => {
                if std::env::var_os("LKATTA_DEBUG").is_some() {
                    eprintln!("fenêtre : {:?} (occupé : {})", f, f.visio() || f.plein_ecran);
                }
                // Une autre fenêtre (ou le bureau) devient active, ou la fenêtre active bouge :
                // on travaille ailleurs, la petite bulle se ferme.
                let autre = self.fenetre.as_ref().is_none_or(|g| g.classe != f.classe || g.titre != f.titre || g.rect != f.rect);
                if autre {
                    self.fermer_bulle_auto();
                }
                // Il était sur une autre fenêtre : il en descend (la même qui bouge, il la suit).
                if self.fenetre.as_ref().is_none_or(|g| g.classe != f.classe || g.titre != f.titre) {
                    self.maki.quitter_toit();
                }
                self.fenetre = Some(f);
            }
            Msg::Occupe { ne_pas_deranger, micro } => {
                self.ne_pas_deranger = ne_pas_deranger;
                self.micro = micro;
            }
            Msg::Desktop(id) => self.on_desktop(id),
            Msg::Quiet(v) => {
                self.quiet = v.unwrap_or(!self.quiet);
                let t = if self.quiet { t!("Je me fais tout discret.") } else { t!("Me revoilà !") };
                self.maki.say(t, 4.0);
            }
            Msg::Hide(v) => {
                self.hidden = v.unwrap_or(!self.hidden);
                if !self.hidden {
                    self.maki.enter(&ctx);
                }
            }
            Msg::Capture(v) => self.capture = v,
            Msg::Quit => self.exit = true,
        }
    }

    /// La fenêtre active est en plein écran sur l'écran du maki ?
    /// La bulle est ouverte ou il réfléchit : il ne bouge pas (ni marche, ni numéro, ni
    /// changement d'écran ou de bureau, ni relance).
    fn fige(&self) -> bool {
        self.chat.open || self.chat.busy
    }

    fn plein_ecran_ici(&self) -> bool {
        self.fenetre.as_ref().is_some_and(|f| f.plein_ecran && (f.ecran.is_empty() || Some(f.ecran.clone()) == self.current_name()))
    }

    /// Visio, présentation, « Ne pas déranger » ou plein écran : il se fait oublier.
    fn occupe(&self) -> bool {
        self.ne_pas_deranger || self.micro || self.plein_ecran_ici() || self.fenetre.as_ref().is_some_and(|f| f.visio())
    }

    /// Zone horizontale de la fenêtre active sur l'écran du maki (à éviter), sauf si elle
    /// prend presque toute la largeur.
    fn avoid_range(&self) -> Option<(f32, f32)> {
        let f = self.fenetre.as_ref()?;
        let g = self.current.as_ref().and_then(|c| self.geom(c))?;
        if !f.ecran.is_empty() && Some(f.ecran.clone()) != self.current_name() {
            return None;
        }
        let (x0, x1) = (f.rect.0 - g.0, f.rect.0 + f.rect.2 - g.0);
        (f.rect.2 > 0.0 && f.rect.2 < g.2 * 0.8).then_some((x0.max(0.0), x1.min(g.2)))
    }

    /// Fin d'une période occupée ou d'une absence : il montre ce qu'il a gardé pour plus tard.
    fn liberer_attente(&mut self, titre: &str) {
        if self.en_attente.is_empty() {
            return;
        }
        let lignes: Vec<String> = self.en_attente.drain(..).map(|m| if m.title.is_empty() { m.text } else { m.title }).collect();
        let texte = lignes.iter().take(4).map(|l| format!("• {}", l.lines().next().unwrap_or(""))).collect::<Vec<_>>().join("\n");
        self.show(Message::new(Kind::Info, titre, texte));
    }

    /// Notifications arrivées depuis `d` : une ligne de plus dans « Pendant ton absence ».
    fn notifs_depuis(&mut self, d: chrono::DateTime<chrono::Local>) {
        let l: Vec<crate::notifs::Notif> = crate::notifs::load().into_iter().filter(|n| chrono::DateTime::parse_from_rfc3339(&n.quand).is_ok_and(|q| q > d)).collect();
        if l.len() >= 2 {
            self.mettre_de_cote(Message::new(Kind::Info, t!("{} notifications ({}) : /notifs pour un résumé", l.len(), crate::notifs::compte(&l)), ""));
        }
    }

    fn mettre_de_cote(&mut self, m: Message) {
        if !self.en_attente.iter().any(|x| x.title == m.title && x.text == m.text) {
            self.en_attente.push(m);
        }
        self.en_attente.truncate(8);
    }

    fn on_idle(&mut self, idle: bool) {
        if idle {
            if self.absent.is_none() {
                self.absent = Some(Instant::now());
            }
        } else if let Some(t) = self.absent.take() {
            if let Ok(d) = chrono::Duration::from_std(t.elapsed()) {
                if t.elapsed() > Duration::from_secs(10 * 60) {
                    self.notifs_depuis(chrono::Local::now() - d);
                }
            }
            if t.elapsed() > Duration::from_secs(10 * 60) && !self.occupe() {
                self.maki.wave();
                if self.en_attente.is_empty() {
                    self.maki.say(t!("Te revoilà !"), 4.0);
                } else {
                    self.liberer_attente(t!("Pendant ton absence"));
                }
            } else if !self.occupe() {
                self.liberer_attente(t!("Pendant ton absence"));
            }
        }
    }

    fn output_named(&self, name: &str) -> Option<wl_output::WlOutput> {
        self.screens.iter().map(|s| s.out.clone()).find(|o| self.output_state.info(o).and_then(|i| i.name).as_deref() == Some(name))
    }

    fn current_name(&self) -> Option<String> {
        self.current.as_ref().and_then(|o| self.output_state.info(o)).and_then(|i| i.name)
    }

    fn on_active_output(&mut self, name: String) {
        self.active_out = Some((name.clone(), Instant::now()));
        // On l'emportait vers cet écran : il y réapparaît, relâché du haut.
        if self.dragging && self.drag_left.is_some_and(|t| t.elapsed() < Duration::from_secs(3)) && self.current_name().as_deref() != Some(name.as_str()) {
            if let Some(o) = self.output_named(&name) {
                self.dragging = false;
                self.press = None;
                self.drag_left = None;
                self.pending_out = Some((o, true));
                self.maki.vanish_to(-1.0, true);
            }
        }
    }

    /// Rejoint en douceur l'écran où l'on travaille : par un saut si les écrans se touchent,
    /// sinon dans un petit nuage. Jamais quand il parle, qu'on discute ou qu'on le tient.
    fn follow_active_output(&mut self) {
        if !self.cfg.maki.suivre || self.screens.len() < 2 || self.hidden || self.dragging || self.fige() {
            return;
        }
        let Some((name, since)) = self.active_out.clone() else { return };
        if since.elapsed() < Duration::from_secs(4) || self.current_name().as_deref() == Some(name.as_str()) {
            return;
        }
        if self.maki.bubble.is_some() || self.pending_out.is_some() || !matches!(self.maki.mode, Mode::Sit | Mode::Walk { .. } | Mode::Sleep) {
            return;
        }
        let Some(target) = self.output_named(&name) else { return };
        let ctx = self.ctx();
        use behavior::Exit;
        for e in [Exit::Left, Exit::Right, Exit::Down, Exit::Up] {
            if self.neighbor(e).as_ref() == Some(&target) && self.cfg.maki.ecrans_multiples {
                self.maki.travel(&ctx, e);
                self.active_out = Some((name, Instant::now()));
                return;
            }
        }
        self.pending_out = Some((target, false));
        self.maki.teleport(&ctx);
    }

    /// Va sur un autre écran : par un bord voisin si possible, sinon en se téléportant.
    fn other_screen(&mut self) {
        use behavior::Exit;
        let ctx = self.ctx();
        for e in [Exit::Right, Exit::Left, Exit::Down, Exit::Up] {
            if self.neighbor(e).is_some() {
                self.maki.travel(&ctx, e);
                return;
            }
        }
        let other = self.screens.iter().map(|s| s.out.clone()).find(|o| Some(o) != self.current.as_ref());
        if let Some(o) = other {
            self.move_to(o);
            self.maki.enter(&self.ctx());
        }
    }

    /// Changement de bureau virtuel : le maki retrouve la place qu'il avait sur ce bureau.
    fn on_desktop(&mut self, id: String) {
        if id == self.desktop || self.dragging {
            return;
        }
        self.fermer_bulle_auto();
        let old = std::mem::replace(&mut self.desktop, id.clone());
        if !old.is_empty() {
            self.desk_pos.insert(old, (self.current.clone(), self.maki.pos.0));
        }
        if self.fige() || !self.maki.visible() || self.hidden {
            return;
        }
        let ctx = self.ctx();
        match self.desk_pos.get(&id).cloned() {
            Some((out, x)) => {
                if out.is_some() && out != self.current {
                    self.move_to(out.unwrap());
                }
                self.maki.teleport_at(x);
            }
            None => self.maki.teleport(&ctx),
        }
    }

    /// Ferme la petite bulle (astuce, nouvelle, réponse, clic droit…) quand on s'occupe
    /// d'autre chose. Elle a eu le temps d'être vue : 1 s, 3 s pour un rappel ou une alerte.
    fn fermer_bulle_auto(&mut self) {
        let Some(b) = &self.maki.bubble else { return };
        let min = if matches!(b.msg.kind, Kind::Reminder | Kind::Alert) { 3.0 } else { 1.0 };
        if b.age >= min {
            self.maki.bubble = None;
            self.bulle_survolee = false;
        }
    }

    fn toggle_chat(&mut self) {
        self.chat.toggle();
        self.maki.bubble = None;
        // Jamais « Exclusive » : la bulle ne vole pas le clavier, elle le reçoit quand on
        // clique dedans (OnDemand) et le rend dès qu'on clique ailleurs.
        let want = if self.chat.open { KeyboardInteractivity::OnDemand } else { KeyboardInteractivity::None };
        self.set_interactivity(want);
        if self.chat.open {
            self.maki.wave();
        }
    }

    fn set_interactivity(&mut self, k: KeyboardInteractivity) {
        if self.interactivity != k {
            self.interactivity = k;
            self.layer.set_keyboard_interactivity(k);
            self.layer.commit();
        }
    }

    /// Relit la configuration ; une nouvelle espèce ou taille demande de relancer le maki,
    /// ce qu'il fait lui-même dans un nuage.
    fn reload(&mut self) {
        let cfg = Config::load();
        let changed = cfg.maki.espece != self.cfg.maki.espece || (cfg.maki.taille - self.cfg.maki.taille).abs() > 0.01;
        self.chat.reload(cfg.clone());
        draw::set_theme(&cfg.maki.theme);
        let (espece, taille) = (self.cfg.maki.espece.clone(), self.cfg.maki.taille);
        self.cfg = cfg;
        if changed && (self.chat.busy || self.chat.en_accueil()) {
            // Relance plus tard : on garde l'allure actuelle pour la reconnaître alors.
            self.cfg.maki.espece = espece;
            self.cfg.maki.taille = taille;
            self.relance_differee = true;
            return;
        }
        if changed && self.restart_at.is_none() {
            let e = especes::Espece::parse(&self.cfg.maki.espece).unwrap_or_default();
            if self.chat.open {
                self.toggle_chat();
            }
            self.maki.say(e.presentation(), 3.0);
            let ctx = self.ctx();
            self.maki.teleport(&ctx);
            self.restart_at = Some(Instant::now() + Duration::from_millis(900));
            {
                // Relance après l'arrêt ; une erreur éventuelle est gardée dans relance.log.
                let log = crate::paths::data_dir().join("relance.log");
                let _ = std::process::Command::new("sh")
                    .args(["-c", &format!(
                        "sleep 1.6; APPIMAGELAUNCHER_DISABLE=1 exec setsid '{}' >'{}' 2>&1",
                        crate::paths::self_exe().display(),
                        log.display()
                    )])
                    .stdin(std::process::Stdio::null())
                    .spawn();
            }
        }
    }

    fn tick(&mut self) -> Duration {
        // Thème « auto » : suit un changement de thème KDE.
        if self.cfg.maki.theme == "auto" && self.theme_check.elapsed() > Duration::from_secs(5) {
            self.theme_check = Instant::now();
            draw::set_theme("auto");
        }
        if self.restart_at.is_some_and(|t| Instant::now() >= t) {
            self.exit = true;
        }
        let dt = self.last_tick.elapsed().as_secs_f32().min(0.1);
        self.last_tick = Instant::now();
        let ctx = self.ctx();
        if self.configured && !self.entered && self.area.is_some() {
            self.entered = true;
            self.maki.enter(&ctx);
        }
        // La réponse arrive : il la « dit », debout.
        if !self.chat.flux.is_empty() {
            self.maki.set_attitude(behavior::Attitude::Parle, 0.4);
        }
        for sig in self.chat.poll() {
            match sig {
                Signal::Answered(t) if !self.chat.open => {
                    let short: String = t.chars().take(220).collect();
                    let short = if t.chars().count() > 220 { format!("{short}…") } else { short };
                    self.show(Message::new(Kind::Answer, t!("Voilà !"), short));
                }
                Signal::Answered(_) => self.maki.react(Kind::Answer),
                Signal::NeedsConfirm if !self.chat.open => self.toggle_chat(),
                Signal::ConfigChanged => self.reload(),
                _ => {}
            }
        }
        // Fin d'une visio / d'un « Ne pas déranger » : ce qui attendait s'affiche.
        let occupe = self.occupe();
        if occupe && !self.etait_occupe {
            self.occupe_depuis = Some(chrono::Local::now());
        }
        if self.etait_occupe && !occupe && self.absent.is_none() {
            if let Some(d) = self.occupe_depuis.take() {
                self.notifs_depuis(d);
            }
            self.liberer_attente(t!("Pendant que tu étais occupé·e"));
        }
        self.etait_occupe = occupe;
        if self.accueil_a.is_some_and(|t| Instant::now() >= t) && self.configured_once && !self.occupe() && self.maki.mode != Mode::Hidden {
            self.accueil_a = None;
            self.chat.open_accueil();
            self.maki.bubble = None;
            self.set_interactivity(KeyboardInteractivity::OnDemand);
            self.maki.wave();
        }
        if self.relance_differee && !self.chat.busy && !self.chat.en_accueil() {
            self.relance_differee = false;
            self.reload();
        }
        self.follow_active_output();
        // Nuage terminé : il réapparaît sur l'écran visé.
        if self.maki.mode == Mode::Hidden {
            if let Some((o, from_top)) = self.pending_out.take() {
                self.move_to(o);
                let c2 = self.ctx();
                // Réapparaît sur un bord (zone de travail libre), en tombant du haut s'il était tenu.
                let x = c2.w * if fastrand::bool() { 0.08 } else { 0.92 };
                self.maki.vanish_to(x, from_top);
                return Duration::from_millis(16);
            }
        }
        // Tenu puis sorti de la surface sans changer d'écran : on le relâche.
        if self.dragging && self.drag_left.is_some_and(|t| t.elapsed() > Duration::from_secs(3)) {
            self.dragging = false;
            self.press = None;
            self.drag_left = None;
            let c = self.ctx();
            self.maki.release(&c);
        }
        if let Some(e) = self.maki.exit.take() {
            let gx = self.current.as_ref().and_then(|c| self.geom(c)).map(|g| g.0).unwrap_or(0.0) + self.maki.pos.0;
            match self.neighbor(e) {
                Some(n) => {
                    let nx = self.geom(&n).map(|g| g.0).unwrap_or(0.0);
                    self.move_to(n);
                    let c2 = self.ctx();
                    self.maki.arrive(&c2, e, gx - nx);
                }
                None => self.maki.enter(&ctx),
            }
            return Duration::from_millis(16);
        }
        if !self.dragging {
            self.maki.step(dt, ctx);
        } else {
            self.maki.step(dt, Ctx { pointer: None, ..ctx });
        }
        self.render();
        if self.hidden {
            Duration::from_millis(500)
        } else if self.dragging || matches!(self.maki.mode, Mode::Air | Mode::Held | Mode::Crouch { .. } | Mode::Land | Mode::PoofIn | Mode::PoofOut | Mode::Hang) {
            // Mouvements rapides : 60 images/s.
            Duration::from_millis(16)
        } else if self.maki.busy() {
            // Marche, escalade, liane qui se déroule : 30 images/s suffisent à l'œil.
            Duration::from_millis(33)
        } else if self.chat.open {
            Duration::from_millis(if self.chat.busy { 120 } else { 250 })
        } else if self.maki.mode == Mode::Sleep {
            Duration::from_millis(300)
        } else {
            Duration::from_millis(90)
        }
    }

    /// Place la surface autour de ce qu'il faut montrer, puis dessine.
    fn render(&mut self) {
        if !self.configured || self.resizing {
            return;
        }
        let t0 = Instant::now();
        self.render_inner();
        if let Some(p) = &mut self.perf {
            p.0 += 1;
            p.1 += t0.elapsed().as_secs_f64();
            if p.0 >= 300 {
                eprintln!("lkatta perf : {:.2} ms par image ({} images, surface {}×{} ×{})", p.1 * 1000.0 / p.0 as f64, p.0, self.sw, self.sh, self.dpr);
                *p = (0, 0.0);
            }
        }
    }

    fn render_inner(&mut self) {
        // L'image précédente n'est pas encore affichée : on ne l'enterre pas sous une autre
        // (sur une machine modeste, cela évite les à-coups). Garde-fou : 100 ms.
        if self.frame_pending.is_some_and(|t| t.elapsed() < Duration::from_millis(100)) {
            // Redessinée dès que le compositeur aura affiché la précédente.
            self.redessin_attendu = true;
            return;
        }
        // Écran éteint ou session verrouillée : le compositeur garde nos deux tampons et
        // n'appelle plus. Rien à dessiner tant que l'un d'eux n'est pas revenu.
        if self.tampons.len() >= 2 && self.tampons.iter().all(|b| b.canvas(&mut self.pool).is_none()) {
            self.redessin_attendu = true;
            return;
        }
        self.redessin_attendu = false;
        let s = self.s;
        let k = self.dpr as f32;
        // Plein écran : il se cache, sauf si on discute avec lui.
        let cache = self.hidden || (self.plein_ecran_ici() && !self.fige());
        let u = self.cfg.maki.texte.clamp(0.8, 2.2);
        let (w, h) = self.area();
        let (sw, sh) = (self.sw as f32, self.sh as f32);
        let pose = self.maki.pose();
        let (mx, my) = self.maki.pos;
        let dangle = matches!(pose.kind, PoseKind::Dangle { .. });

        // Rectangle du maki (monde) et position de sa tête.
        let r_maki: R = match pose.kind {
            PoseKind::Dangle { .. } => (mx - 45.0 * s, my - 100.0 * s, 90.0 * s, 160.0 * s),
            PoseKind::Climb => (mx - 130.0 * s, my - 75.0 * s, 260.0 * s, 150.0 * s),
            PoseKind::Grimpe => (mx - 60.0 * s, my - 130.0 * s, 120.0 * s, 175.0 * s),
            PoseKind::Sleep => (mx - 70.0 * s, my - 45.0 * s, 130.0 * s, 48.0 * s),
            _ => (mx - 85.0 * s, my - 135.0 * s, 170.0 * s, 140.0 * s),
        };
        let head = match pose.kind {
            PoseKind::Sit => (mx + self.maki.dir * 14.0 * s, my - 88.0 * s),
            PoseKind::Dangle { .. } => (mx, my - 78.0 * s),
            PoseKind::Climb => (mx, my - 60.0 * s),
            PoseKind::Grimpe => (mx + self.maki.dir * 4.0 * s, my - 95.0 * s),
            PoseKind::Sleep => (mx + self.maki.dir * 24.0 * s, my - 30.0 * s),
            _ => (mx + self.maki.dir * 30.0 * s, my - 72.0 * s),
        };
        let mut lo = (r_maki.0, r_maki.1);
        let mut hi = (r_maki.0 + r_maki.2, r_maki.1 + r_maki.3);
        let mut grow = |r: R| {
            lo = (lo.0.min(r.0), lo.1.min(r.1));
            hi = (hi.0.max(r.0 + r.2), hi.1.max(r.1 + r.3));
        };
        if let Some(v) = &self.maki.vine {
            grow((v.x - 30.0 * s, 0.0, 60.0 * s, v.bottom * v.grow.max(0.01)));
        }
        if let Some(t) = &self.maki.tree {
            grow((t.x - 140.0 * s, h - (Tree::height(s) + 70.0 * s) * t.grow.max(0.01), 280.0 * s, (Tree::height(s) + 70.0 * s) * t.grow));
        }
        // Bulles : au-dessus de la tête s'il y a la place, sinon dessous.
        let mut chat_box = None;
        if !cache {
            if let (true, Some(text)) = (self.chat.open, &self.text) {
                let bh = draw::chat_height(text, &self.chat, u) + 16.0 * u;
                let bw = 400.0 * u;
                // La bulle passe au-dessus du haut du personnage (oreilles comprises).
                let top = r_maki.1 + 20.0 * s;
                let above = top - bh - 14.0 * u > 0.0;
                let by = if above { top - 6.0 * u } else { (if dangle { my + 50.0 * s } else { my + 8.0 * s }).min(h - bh) };
                let bx = (head.0 - bw * 0.45).clamp(6.0, (w - bw - 6.0).max(6.0));
                chat_box = Some((bx, by, above, bw, bh));
                grow((bx, if above { by - bh } else { by }, bw, bh));
            } else if self.maki.bubble.is_some() {
                grow(((head.0 - 140.0 * u).max(0.0), r_maki.1 + 20.0 * s - 250.0 * u, 420.0 * u, 250.0 * u));
            }
        }
        // La surface s'agrandit (ou se réduit) pour tout contenir : rien n'est coupé.
        let need = (((hi.0 - lo.0) + 24.0).max(470.0 * s).min(w), ((hi.1 - lo.1) + 24.0).max(560.0 * s).min(h));
        let (nw, nh) = (need.0.ceil() as u32, need.1.ceil() as u32);
        let grow_needed = nw > self.sw || nh > self.sh;
        let shrink_ok = self.sw > nw + 200 || self.sh > nh + 200;
        if grow_needed || shrink_ok {
            self.layer.set_size(nw, nh);
            self.layer.commit();
            self.resizing = true;
            return;
        }
        // La surface ne bouge que si le contenu approche de son bord : KWin n'a pas à refaire
        // sa mise en page à chaque image.
        let (ox, oy) = (self.origin.0 as f32, self.origin.1 as f32);
        let m = 16.0;
        // Après un agrandissement, l'ancienne position peut faire déborder la surface sous le
        // bas de l'écran : KWin la remonterait d'office et tout serait dessiné trop haut.
        let inside = ox + sw <= w + 0.5 && oy + sh <= h + 0.5;
        let fits = inside && lo.0 >= ox + m && hi.0 <= ox + sw - m && lo.1 >= oy + m.min(lo.1.max(0.0)) && hi.1 <= oy + sh - 2.0;
        let (sx, sy) = if fits && self.configured_once {
            (ox, oy)
        } else {
            let cx = (lo.0 + hi.0) / 2.0;
            let sx = (cx - sw / 2.0).clamp(0.0, (w - sw).max(0.0)).round();
            let mut sy = (hi.1 - sh + 2.0).clamp(0.0, (h - sh).max(0.0));
            if lo.1 < sy {
                sy = lo.1.max(0.0);
            }
            (sx, sy.round())
        };
        self.configured_once = true;
        let origin = (sx as i32, sy as i32);
        if std::env::var_os("LKATTA_DEBUG").is_some() && (origin != self.origin || (nw, nh) != self.dbg_taille) {
            self.dbg_taille = (nw, nh);
            eprintln!(
                "géométrie : écran {:?} zone {w}x{h} · surface {sw}x{sh} (voulue {nw}x{nh}) · origine {origin:?} (avant {:?}, tient {fits}) · contenu y {:.0}..{:.0} · maki {:?} {:?}",
                self.current_name(), self.origin, lo.1, hi.1, self.maki.pos, self.maki.mode
            );
        }
        if origin != self.origin {
            self.origin = origin;
            self.layer.set_margin(origin.1, 0, 0, origin.0);
        }
        let to_local = |p: (f32, f32)| ((p.0 - sx) * k, (p.1 - sy) * k);

        self.pixmap.fill(tiny_skia::Color::TRANSPARENT);
        self.chat_layout = None;
        self.bubble_layout = None;
        let region = Region::new(&self.compositor).ok();

        if !cache && !self.capture {
            if let Some(t) = &self.maki.tree {
                let (tx, ty) = to_local((t.x, h));
                draw::tree(&mut self.pixmap, Transform::from_translate(tx, ty).pre_scale(s * k, s * k), t.grow, t.side, pose.time);
            }
            if let Some(v) = &self.maki.vine {
                let (vx, vt) = to_local((v.x, 0.0));
                let (_, vb) = to_local((v.x, v.bottom));
                draw::vine(&mut self.pixmap, vx, vt, vb, v.grow, s * k, pose.time);
            }
            let (lx, ly) = to_local((mx, my));
            let base = Transform::from_translate(lx, ly).pre_scale(self.maki.dir * s * k, s * k);
            // Calque du maki : effacé, dessiné et posé seulement dans son rectangle.
            let lr = ((r_maki.0 - sx - 10.0 * s) * k, (r_maki.1 - sy - 10.0 * s) * k, (r_maki.2 + 20.0 * s) * k, (r_maki.3 + 20.0 * s) * k);
            let pad = 70.0 * s * k;
            let zone = (lr.0 - pad, lr.1 - pad, lr.2 + 2.0 * pad, lr.3 + 2.0 * pad);
            draw::clear_rect(&mut self.sprite, zone);
            draw::maki(&mut self.sprite, base, &pose);
            draw::outline(&mut self.sprite, lr, (1.6 * s * k).round().max(1.0) as i32, (236, 232, 222), 0.85);
            draw::blend_over(&mut self.pixmap, &self.sprite, zone, 1.0);
            draw::puffs(&mut self.pixmap, (sx, sy), k, &self.maki.puffs);

            let (hx, hy) = to_local(head);
            // Pointe des bulles : juste au-dessus du personnage.
            let (_, ty) = to_local((head.0, r_maki.1 + 20.0 * s));
            if let Some(text) = &self.text {
                if pose.kind == PoseKind::Sleep {
                    let d = (pose.time * 0.6) % 1.0;
                    text.draw(&mut self.pixmap, (11.0 + d * 9.0) * s * k, hx + (10.0 + d * 18.0) * s * k, hy - (18.0 + d * 30.0) * s * k, "z", (90, 90, 120));
                }
                if let Some((bx, by, above, _, _)) = chat_box {
                    let caret = self.kb_focus && (pose.time * 2.0) as i64 % 2 == 0;
                    let (lbx, lby) = to_local((bx, by));
                    let ay = if above { ty } else { hy };
                    let lay = draw::chat_bubble(&mut self.pixmap, text, &self.chat, hx, ay, above, lbx, lby, u * k, caret, self.kb_focus, pose.time);
                    self.chat_layout = Some(lay);
                } else if let Some(b) = &self.maki.bubble {
                    // Dessinée à part pour l'apparition et la disparition en fondu.
                    let bold = self.bold.as_ref().unwrap_or(text);
                    let acts = Self::actions_for(&b.msg);
                    // Redessinée seulement si son contenu change (texte qui s'écrit, glissement
                    // d'arrivée, position) ; sinon le calque est simplement reposé.
                    let key = (
                        format!("{}|{}", b.msg.title, b.msg.text),
                        b.revealed().min(b.full()),
                        ((b.age / 0.28).min(1.0) * 20.0) as i32,
                        hx as i32,
                        ty as i32,
                    );
                    let lay = if self.bubble_key.as_ref() == Some(&key) && self.bubble_lay.is_some() {
                        self.bubble_lay.clone().unwrap()
                    } else {
                        if let Some(z) = self.bubble_zone.take() {
                            draw::clear_rect(&mut self.bubble_pm, z);
                        }
                        let l = draw::fancy_bubble(&mut self.bubble_pm, text, bold, b, &acts, hx, ty, u * k);
                        self.bubble_key = Some(key);
                        self.bubble_lay = Some(l.clone());
                        l
                    };
                    let opacity = (b.age / 0.25).min(1.0) * (b.ttl / 0.45).clamp(0.0, 1.0);
                    let r = lay.rect;
                    let m = 24.0 * u * k;
                    // Bulle + pointe vers le personnage.
                    let zone = (r.0 - m, r.1 - m, r.2 + 2.0 * m, (ty - r.1).max(r.3) + 2.0 * m);
                    draw::blend_over(&mut self.pixmap, &self.bubble_pm, zone, opacity);
                    self.bubble_zone = Some(zone);
                    self.bubble_layout = Some(lay);
                }
            }
            // Zone cliquable : le maki (serrée) et les bulles.
            let tight = match pose.kind {
                PoseKind::Dangle { .. } => (mx - 28.0 * s, my - 100.0 * s, 56.0 * s, 104.0 * s),
                PoseKind::Climb => (mx - 70.0 * s, my - 45.0 * s, 140.0 * s, 90.0 * s),
                PoseKind::Grimpe => (mx - 30.0 * s, my - 110.0 * s, 60.0 * s, 115.0 * s),
                PoseKind::Sleep => (mx - 45.0 * s, my - 38.0 * s, 95.0 * s, 40.0 * s),
                _ => (mx - 45.0 * s, my - 105.0 * s, 95.0 * s, 108.0 * s),
            };
            self.maki_rect = tight;
            if let Some(r) = &region {
                // La région d'entrée est en px logiques de la surface.
                if self.maki.visible() {
                    r.add((tight.0 - sx) as i32, (tight.1 - sy) as i32, tight.2 as i32, tight.3 as i32);
                }
                for b in [self.chat_layout.as_ref().map(|l| l.rect), self.bubble_layout.as_ref().map(|l| l.rect)].into_iter().flatten() {
                    r.add((b.0 / k) as i32, (b.1 / k) as i32, (b.2 / k) as i32 + 2, (b.3 / k) as i32 + 2);
                }
            }
        }
        let surface = self.layer.wl_surface().clone();
        if let Some(r) = &region {
            surface.set_input_region(Some(r.wl_region()));
        }
        let (bw, bh) = ((self.sw * self.dpr) as i32, (self.sh * self.dpr) as i32);
        if self.pixmap.width() as i32 != bw || self.pixmap.height() as i32 != bh {
            self.resize_pixmaps();
            return;
        }
        let Some(i) = self.tampon_libre(bw, bh) else {
            // Les deux tampons sont encore à l'écran : on redessinera quand l'un revient.
            self.redessin_attendu = true;
            return;
        };
        let buffer = &self.tampons[i];
        let Some(canvas) = buffer.canvas(&mut self.pool) else {
            return;
        };
        surface.set_buffer_scale(self.dpr as i32);
        // RGBA (tiny-skia) → BGRA (Wayland ARGB8888) : échange des octets 0 et 2, par mots.
        for (dst, src) in canvas.chunks_exact_mut(4).zip(self.pixmap.data().chunks_exact(4)) {
            let v = u32::from_le_bytes([src[0], src[1], src[2], src[3]]);
            let w = (v & 0xFF00_FF00) | ((v & 0xFF) << 16) | ((v >> 16) & 0xFF);
            dst.copy_from_slice(&w.to_le_bytes());
        }
        surface.damage_buffer(0, 0, bw, bh);
        if buffer.attach_to(&surface).is_ok() {
            surface.frame(&self.qh, surface.clone());
            self.frame_pending = Some(Instant::now());
            self.layer.commit();
        }
    }

    /// Un tampon libre de la bonne taille : réutilisé, ou créé s'il y en a moins de deux.
    /// La réserve de mémoire partagée est refaite à neuf quand la taille change, pour ne
    /// jamais garder la place d'anciennes tailles.
    fn tampon_libre(&mut self, bw: i32, bh: i32) -> Option<usize> {
        let taille = (bw as usize) * (bh as usize) * 4;
        if self.tampons.iter().any(|b| b.height() != bh || b.stride() != bw * 4) {
            self.tampons.clear();
        }
        if self.tampons.is_empty() && self.pool.len() > taille * 3 {
            if let Ok(p) = SlotPool::new(taille * 2, &self.shm) {
                self.pool = p;
            }
        }
        if let Some(i) = self.tampons.iter().position(|b| b.canvas(&mut self.pool).is_some()) {
            return Some(i);
        }
        if self.tampons.len() >= 2 {
            return None;
        }
        let (b, _) = self.pool.create_buffer(bw, bh, bw * 4, wl_shm::Format::Argb8888).ok()?;
        self.tampons.push(b);
        Some(self.tampons.len() - 1)
    }

    fn resize_pixmaps(&mut self) {
        let (w, h) = (self.sw * self.dpr, self.sh * self.dpr);
        if let (Some(p), Some(q), Some(b)) = (Pixmap::new(w, h), Pixmap::new(w, h), Pixmap::new(w, h)) {
            self.pixmap = p;
            self.sprite = q;
            self.bubble_pm = b;
            self.bubble_zone = None;
            self.bubble_key = None;
        }
    }

    /// Point de la surface (px logiques) → px de l'image dessinée.
    fn dev(&self, p: (f32, f32)) -> (f32, f32) {
        (p.0 * self.dpr as f32, p.1 * self.dpr as f32)
    }

    fn set_cursor(&mut self, c: CursorIcon) {
        if self.cursor != c {
            self.cursor = c;
            if let Some(p) = &self.pointer {
                let _ = p.set_cursor(&self.conn, c);
            }
        }
    }

    fn on_click(&mut self, button: u32, local: (f32, f32), on_maki: bool) {
        let ctx = self.ctx();
        if let Some(lay) = self.chat_layout.clone() {
            if hit(lay.rect, local) {
                if button != BTN_LEFT {
                    return;
                }
                if let Some(r) = lay.stop.filter(|r| hit(*r, local)) {
                    let _ = r;
                    self.chat.arreter();
                } else if let Some((_, a)) = lay.histo.iter().rev().find(|(r, _)| hit(*r, local)) {
                    self.chat.histo_act(a.clone());
                    self.set_interactivity(KeyboardInteractivity::OnDemand);
                } else if let Some((_, c)) = lay.cotes.iter().find(|(r, _)| hit(*r, local)) {
                    self.chat.basculer_cote(*c);
                } else if hit(lay.histo_btn, local) || (lay.fil.2 > 0.0 && hit(lay.fil, local)) {
                    self.chat.basculer_histo();
                    self.set_interactivity(KeyboardInteractivity::OnDemand);
                } else if let Some((_, a)) = lay.picks_panel.iter().find(|(r, _)| hit(*r, local)) {
                    self.chat.settings_act(a.clone());
                } else if let Some((_, p)) = lay.picks.iter().find(|(r, _)| hit(*r, local)) {
                    self.chat.pick(p.clone());
                } else if let Some((_, k)) = lay.presse.iter().find(|(r, _)| hit(*r, local)) {
                    self.chat.action_presse(*k);
                } else if lay.mode.2 > 0.0 && hit(lay.mode, local) {
                    self.chat.mode_suivant();
                } else if hit(lay.gear, local) {
                    self.chat.histo = None;
                    if self.chat.panneau.is_some() {
                        self.chat.settings_act(crate::chat::Act::Fermer);
                    } else {
                        self.chat.open_settings(0);
                    }
                } else if hit(lay.chip, local) && self.chat.histo.is_some() {
                    self.chat.histo = None;
                } else if hit(lay.chip, local) && self.chat.panneau.is_some() {
                    self.chat.settings_act(crate::chat::Act::Fermer);
                } else if hit(lay.chip, local) {
                    let a = if self.chat.picker.is_some() { crate::chat::Pick::Close } else { crate::chat::Pick::Open };
                    self.chat.pick(a);
                } else if hit(lay.close, local) {
                    self.toggle_chat();
                } else if lay.yes.is_some_and(|r| hit(r, local)) {
                    self.chat.answer(true);
                } else if lay.no.is_some_and(|r| hit(r, local)) {
                    self.chat.answer(false);
                    self.maki.set_attitude(behavior::Attitude::Boude, 2.4);
                } else if let Some((_, u)) = lay.links.iter().find(|(r, _)| hit(*r, local)) {
                    // Lien d'une réponse : ouvert dans le navigateur (adresses web seulement).
                    if u.starts_with("https://") || u.starts_with("http://") {
                        let _ = std::process::Command::new("xdg-open").arg(u).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn();
                        self.chat.entries.push(crate::chat::Entry::Info(t!("Ouvert : {}", u)));
                    }
                } else if let Some((_, t)) = lay.copy.iter().find(|(r, _)| hit(*r, local)) {
                    let _ = crate::clipboard::copy(t);
                    self.chat.entries.push(crate::chat::Entry::Info(t!("Réponse copiée dans le presse-papiers.").into()));
                } else {
                    if hit(lay.input, local) {
                        // Clic dans la saisie : le curseur va au caractère le plus proche.
                        let i = lay.input_x.iter().enumerate().min_by(|a, b| (a.1 - local.0).abs().total_cmp(&(b.1 - local.0).abs())).map(|(i, _)| i);
                        if let Some(i) = i {
                            self.chat.input.placer(lay.input_off + i, self.shift);
                        }
                    }
                    self.set_interactivity(KeyboardInteractivity::OnDemand);
                }
                return;
            }
        }
        if let Some(lay) = self.bubble_layout.clone() {
            if hit(lay.rect, local) {
                if button != BTN_LEFT {
                    return;
                }
                let labels = self.maki.bubble.as_ref().map(|b| Self::actions_for(&b.msg)).unwrap_or_default();
                if let Some(i) = lay.actions.iter().position(|r| hit(*r, local)) {
                    if let Some(l) = labels.get(i) {
                        self.bubble_action(l);
                    }
                } else if hit(lay.close, local) {
                    self.maki.bubble = None;
                } else if let Some(b) = &mut self.maki.bubble {
                    // Premier clic : affiche tout le texte ; ensuite : ferme.
                    if b.revealed() < b.full() {
                        b.age = 60.0;
                    } else if labels.is_empty() {
                        self.maki.bubble = None;
                    }
                }
                return;
            }
        }
        if !on_maki {
            return;
        }
        match button {
            BTN_LEFT => {
                self.toggle_chat();
                // Ouverte d'un clic : elle prend le clavier, pour se refermer quand on clique ailleurs.
                if self.chat.open {
                    crate::bureau::focaliser_bulle();
                }
            }
            BTN_MIDDLE => match fastrand::u8(0..4) {
                0 => self.maki.tree_show(&ctx),
                1 => self.maki.teleport(&ctx),
                2 => self.maki.climb_edge(&ctx),
                _ => self.maki.flip(),
            },
            BTN_RIGHT => self.on_msg(Msg::Quiet(None)),
            _ => {}
        }
    }

    fn on_key(&mut self, ev: KeyEvent) {
        if !self.chat.open || !self.kb_focus {
            return;
        }
        // Un champ du panneau de réglages a le focus : la saisie va dans ce champ.
        if self.chat.has_field_focus() {
            match ev.keysym {
                Keysym::Escape => self.chat.field_escape(),
                Keysym::Return | Keysym::KP_Enter => self.chat.field_enter(),
                Keysym::BackSpace => self.chat.field_backspace(),
                _ if self.ctrl && matches!(ev.keysym, Keysym::v | Keysym::V) => {
                    if let Ok(o) = std::process::Command::new("wl-paste").args(["--no-newline", "--type", "text"]).output() {
                        self.chat.field_input(&String::from_utf8_lossy(&o.stdout));
                    }
                }
                _ => {
                    if let Some(t) = ev.utf8.filter(|t| !self.ctrl && t.chars().all(|c| !c.is_control())) {
                        self.chat.field_input(&t);
                    }
                }
            }
            self.render();
            return;
        }
        match ev.keysym {
            // Pendant qu'il travaille, Échap arrête ses actions (la bulle reste ouverte).
            Keysym::Escape if self.chat.busy && !crate::controle::arrete() => self.chat.arreter(),
            Keysym::Escape => self.toggle_chat(),
            Keysym::Return | Keysym::KP_Enter => {
                if self.chat.confirm.is_none() {
                    self.chat.submit();
                }
            }
            Keysym::BackSpace => self.chat.input.retour_arriere(self.ctrl),
            Keysym::Delete | Keysym::KP_Delete => self.chat.input.supprimer(self.ctrl),
            Keysym::Left | Keysym::KP_Left => self.chat.input.gauche(self.ctrl, self.shift),
            Keysym::Right | Keysym::KP_Right => self.chat.input.droite(self.ctrl, self.shift),
            Keysym::Home | Keysym::KP_Home => self.chat.input.debut(self.shift),
            Keysym::End | Keysym::KP_End => self.chat.input.fin(self.shift),
            Keysym::Page_Up | Keysym::Up => self.chat.scroll += 3,
            Keysym::Page_Down | Keysym::Down => self.chat.scroll = self.chat.scroll.saturating_sub(3),
            _ if self.ctrl && matches!(ev.keysym, Keysym::v | Keysym::V) => {
                if let Ok(o) = std::process::Command::new("wl-paste").args(["--no-newline", "--type", "text"]).output() {
                    self.chat.input.inserer(&String::from_utf8_lossy(&o.stdout));
                }
            }
            _ if self.ctrl && matches!(ev.keysym, Keysym::a | Keysym::A) => self.chat.input.tout_selectionner(),
            _ if self.ctrl && matches!(ev.keysym, Keysym::c | Keysym::C) => {
                if let Some(t) = self.chat.input.texte_selectionne() {
                    let _ = crate::clipboard::copy(&t);
                }
            }
            _ if self.ctrl && matches!(ev.keysym, Keysym::x | Keysym::X) => {
                if let Some(t) = self.chat.input.couper() {
                    let _ = crate::clipboard::copy(&t);
                }
            }
            _ if self.ctrl && matches!(ev.keysym, Keysym::u | Keysym::U) => {
                self.chat.input.prendre();
            }
            _ => {
                if let Some(t) = ev.utf8 {
                    if self.chat.confirm.is_some() && self.chat.input.is_empty() {
                        match t.as_str() {
                            "o" | "O" | "y" => return self.chat.answer(true),
                            "n" | "N" => {
                                self.maki.set_attitude(behavior::Attitude::Boude, 2.4);
                                return self.chat.answer(false);
                            }
                            _ => {}
                        }
                    }
                    if !self.ctrl && t.chars().all(|c| !c.is_control()) {
                        self.chat.input.inserer(&t);
                    }
                }
            }
        }
        self.render();
    }
}

impl CompositorHandler for App {
    fn scale_factor_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, surface: &wl_surface::WlSurface, f: i32) {
        if surface == self.layer.wl_surface() {
            let f = f.clamp(1, 4) as u32;
            if f != self.dpr {
                self.dpr = f;
                self.resize_pixmaps();
                self.render();
            }
        }
    }
    fn transform_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: wl_output::Transform) {}
    fn frame(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: u32) {
        self.frame_pending = None;
        if self.redessin_attendu {
            self.render();
        }
    }
    fn surface_enter(&mut self, _: &Connection, _: &QueueHandle<Self>, surface: &wl_surface::WlSurface, o: &wl_output::WlOutput) {
        // Une surface layer-shell appartient à l'écran pour lequel elle est créée (move_to).
        // Elle peut « toucher » l'écran voisin quand elle grandit jusqu'au bord (bulle
        // ouverte) : ce n'est pas un changement d'écran, sinon il prendrait le sol de l'autre.
        if surface == self.layer.wl_surface() && self.current.is_none() {
            self.current = Some(o.clone());
        }
    }
    fn surface_leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: &wl_output::WlOutput) {}
}

impl OutputHandler for App {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }
    fn new_output(&mut self, _: &Connection, qh: &QueueHandle<Self>, o: wl_output::WlOutput) {
        // Sonde invisible : couvre la zone utile de cet écran (hors panneaux).
        let probe = self.layer_shell.create_layer_surface(qh, self.compositor.create_surface(qh), Layer::Background, Some("lkatta-zone"), Some(&o));
        probe.set_anchor(Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
        probe.set_size(0, 0);
        probe.set_exclusive_zone(0);
        probe.set_keyboard_interactivity(KeyboardInteractivity::None);
        if let Ok(r) = Region::new(&self.compositor) {
            probe.wl_surface().set_input_region(Some(r.wl_region()));
        }
        probe.commit();
        self.screens.push(Screen { out: o, probe, area: None });
    }
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, o: wl_output::WlOutput) {
        self.screens.retain(|s| s.out != o);
        if self.current.as_ref() == Some(&o) {
            if let Some(other) = self.screens.first().map(|s| s.out.clone()) {
                self.move_to(other);
                self.maki.enter(&self.ctx());
            }
        }
    }
}

impl LayerShellHandler for App {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, l: &LayerSurface) {
        if l == &self.layer {
            self.exit = true;
        }
    }

    fn configure(&mut self, _: &Connection, _: &QueueHandle<Self>, l: &LayerSurface, c: LayerSurfaceConfigure, _: u32) {
        if let Some(scr) = self.screens.iter_mut().find(|s| &s.probe == l) {
            if c.new_size.0 > 0 && c.new_size.1 > 0 {
                scr.area = Some((c.new_size.0 as f32, c.new_size.1 as f32));
                if self.area.is_none() {
                    self.area = scr.area;
                }
            }
            return;
        }
        if l != &self.layer {
            return;
        }
        if c.new_size.0 > 0 && c.new_size.1 > 0 && (c.new_size.0, c.new_size.1) != (self.sw, self.sh) {
            self.sw = c.new_size.0;
            self.sh = c.new_size.1;
            self.resize_pixmaps();
        }
        self.resizing = false;
        if !self.configured {
            self.configured = true;
            self.render();
        }
    }
}

impl SeatHandler for App {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }
    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
    fn new_capability(&mut self, _: &Connection, qh: &QueueHandle<Self>, seat: wl_seat::WlSeat, cap: Capability) {
        if cap == Capability::Pointer && self.pointer.is_none() {
            let surface = self.compositor.create_surface(qh);
            self.pointer = self.seat_state.get_pointer_with_theme(qh, &seat, self.shm.wl_shm(), surface, ThemeSpec::default()).ok();
        }
        // Absence : prévenu après 5 minutes sans clavier ni souris.
        if self.idle.is_none() {
            if let Some(n) = &self.idle_notifier {
                self.idle = Some(n.get_idle_notification(5 * 60 * 1000, &seat, qh, ()));
            }
        }
        if cap == Capability::Keyboard && self.keyboard.is_none() {
            self.keyboard = self
                .seat_state
                .get_keyboard_with_repeat(qh, &seat, None, self.loop_handle.clone(), Box::new(|app: &mut App, _, ev| app.on_key(ev)))
                .ok();
        }
    }
    fn remove_capability(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat, cap: Capability) {
        if cap == Capability::Pointer {
            if let Some(p) = self.pointer.take() {
                p.pointer().release();
            }
        }
        if cap == Capability::Keyboard {
            if let Some(k) = self.keyboard.take() {
                k.release();
            }
        }
    }
    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl KeyboardHandler for App {
    fn enter(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_keyboard::WlKeyboard, surface: &wl_surface::WlSurface, _: u32, _: &[u32], _: &[Keysym]) {
        if surface == self.layer.wl_surface() {
            self.kb_focus = true;
            self.chat.retour_attendu = false;
        }
    }
    fn leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_keyboard::WlKeyboard, surface: &wl_surface::WlSurface, _: u32) {
        if surface == self.layer.wl_surface() {
            self.kb_focus = false;
            // Clic ailleurs (autre fenêtre, bureau) : la bulle se referme.
            if self.chat.fermable() {
                self.toggle_chat();
            }
        }
    }
    fn press_key(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_keyboard::WlKeyboard, _: u32, ev: KeyEvent) {
        self.on_key(ev);
    }
    fn repeat_key(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_keyboard::WlKeyboard, _: u32, ev: KeyEvent) {
        self.on_key(ev);
    }
    fn release_key(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_keyboard::WlKeyboard, _: u32, _: KeyEvent) {}
    fn update_modifiers(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_keyboard::WlKeyboard, _: u32, m: Modifiers, _: RawModifiers, _: u32) {
        self.ctrl = m.ctrl;
        self.shift = m.shift;
    }
}

impl PointerHandler for App {
    fn pointer_frame(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_pointer::WlPointer, events: &[PointerEvent]) {
        let mut redessiner = false;
        for ev in events {
            if &ev.surface != self.layer.wl_surface() {
                continue;
            }
            let local = (ev.position.0 as f32, ev.position.1 as f32);
            // Les zones des bulles sont en px de l'image (densité de l'écran).
            let dl = (local.0 * self.dpr as f32, local.1 * self.dpr as f32);
            let world = (local.0 + self.origin.0 as f32, local.1 + self.origin.1 as f32);
            let on_maki = hit(self.maki_rect, world);
            match ev.kind {
                PointerEventKind::Enter { .. } | PointerEventKind::Motion { .. } => {
                    // Petite bulle survolée puis quittée : lue, elle se ferme.
                    let sur_bulle = self.maki.bubble.is_some() && self.bubble_layout.as_ref().is_some_and(|l| hit(l.rect, dl));
                    if sur_bulle {
                        self.bulle_survolee = true;
                    } else if self.bulle_survolee && !on_maki {
                        self.bulle_survolee = false;
                        self.fermer_bulle_auto();
                    }
                    self.drag_left = None;
                    self.pointer_world = Some(world);
                    self.hovered = on_maki && !self.dragging;
                    if let Some(p) = &self.press {
                        if p.button == BTN_LEFT && p.on_maki {
                            let moved = ((world.0 - p.at.0).powi(2) + (world.1 - p.at.1).powi(2)).sqrt();
                            if !self.dragging && moved > 6.0 {
                                self.dragging = true;
                                self.maki.bubble = None;
                                self.maki.grab(world);
                                self.last_drag = Instant::now();
                            } else if self.dragging {
                                let dt = self.last_drag.elapsed().as_secs_f32();
                                self.last_drag = Instant::now();
                                self.maki.drag_to(world, dt);
                            }
                        }
                    }
                    let c = if self.dragging {
                        CursorIcon::Grabbing
                    } else if on_maki {
                        CursorIcon::Grab
                    } else if self.chat_layout.as_ref().is_some_and(|l| l.links.iter().any(|(r, _)| hit(*r, dl))) {
                        CursorIcon::Pointer
                    } else if self.chat_layout.as_ref().is_some_and(|l| hit(l.input, dl)) {
                        CursorIcon::Text
                    } else {
                        CursorIcon::Default
                    };
                    self.set_cursor(c);
                }
                PointerEventKind::Leave { .. } => {
                    self.hovered = false;
                    if self.bulle_survolee {
                        self.bulle_survolee = false;
                        self.fermer_bulle_auto();
                    }
                    if self.dragging {
                        self.drag_left = Some(Instant::now());
                    }
                    self.cursor = CursorIcon::Default;
                    if !self.dragging {
                        self.press = None;
                        self.pointer_world = None;
                    }
                }
                PointerEventKind::Press { button, .. } => {
                    self.press = Some(Press { button, at: world, on_maki });
                }
                PointerEventKind::Release { button, .. } => {
                    let press = self.press.take();
                    if self.dragging {
                        self.dragging = false;
                        let ctx = self.ctx();
                        self.maki.release(&ctx);
                        continue;
                    }
                    if let Some(p) = press.filter(|p| p.button == button) {
                        self.on_click(button, dl, p.on_maki);
                        // Le résultat du clic se voit tout de suite (pas au prochain tour).
                        redessiner = true;
                    }
                }
                PointerEventKind::Axis { vertical, .. } => {
                    let in_picker = self.chat_layout.as_ref().and_then(|l| l.picker).is_some_and(|r| hit(r, dl));
                    if in_picker {
                        let d = if vertical.discrete != 0 { vertical.discrete as f64 } else { vertical.absolute / 10.0 };
                        if let Some(p) = &mut self.chat.picker {
                            if d > 0.0 {
                                p.scroll += 2;
                            } else if d < 0.0 {
                                p.scroll = p.scroll.saturating_sub(2);
                            }
                        }
                        if let Some(h) = &mut self.chat.histo {
                            if d > 0.0 {
                                h.scroll = (h.scroll + 2).min(500);
                            } else if d < 0.0 {
                                h.scroll = h.scroll.saturating_sub(2);
                            }
                        }
                        let (content, view) = self.chat_layout.as_ref().map(|l| (l.panel_content, l.panel_view)).unwrap_or((0.0, 0.0));
                        if let Some(p) = &mut self.chat.panneau {
                            let max = (content - view).max(0.0);
                            p.scroll = (p.scroll + if d > 0.0 { 40.0 } else if d < 0.0 { -40.0 } else { 0.0 }).clamp(0.0, max);
                        }
                    } else if self.chat_layout.as_ref().is_some_and(|l| hit(l.rect, dl)) {
                        let d = if vertical.discrete != 0 { vertical.discrete as f64 } else { vertical.absolute / 10.0 };
                        if d < 0.0 {
                            self.chat.scroll += 2;
                        } else if d > 0.0 {
                            self.chat.scroll = self.chat.scroll.saturating_sub(2);
                        }
                    }
                    redessiner = true;
                }
            }
        }
        if redessiner {
            self.render();
        }
    }
}

impl ShmHandler for App {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

impl ProvidesRegistryState for App {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState, SeatState];
}

impl Dispatch<ExtIdleNotifierV1, ()> for App {
    fn event(_: &mut Self, _: &ExtIdleNotifierV1, _: <ExtIdleNotifierV1 as wayland_client::Proxy>::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

impl Dispatch<ExtIdleNotificationV1, ()> for App {
    fn event(app: &mut Self, _: &ExtIdleNotificationV1, ev: ext_idle_notification_v1::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
        match ev {
            ext_idle_notification_v1::Event::Idled => app.on_idle(true),
            ext_idle_notification_v1::Event::Resumed => app.on_idle(false),
            _ => {}
        }
    }
}

delegate_compositor!(App);
delegate_output!(App);
delegate_shm!(App);
delegate_seat!(App);
delegate_keyboard!(App);
delegate_pointer!(App);
delegate_layer!(App);
delegate_registry!(App);
