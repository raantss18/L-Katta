//! Surcouche Wayland du maki (layer-shell, rendu logiciel).
//!
//! Une surface sonde invisible mesure la zone utile de l'écran (hors panneaux) ; une
//! petite surface transparente suit le maki partout dans cette zone en changeant ses
//! marges. Seuls le maki et ses bulles reçoivent les clics ; la bulle de discussion
//! prend le clavier le temps de taper.

pub mod behavior;
pub mod draw;
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
    shm::{Shm, ShmHandler, slot::SlotPool},
};
use tiny_skia::{Pixmap, Transform};
use wayland_client::{
    Connection, QueueHandle,
    globals::registry_queue_init,
    protocol::{wl_keyboard, wl_output, wl_pointer, wl_seat, wl_shm, wl_surface},
};

use crate::chat::{Chat, Signal};
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
    cursor: CursorIcon,
    press: Option<Press>,
    dragging: bool,
    last_drag: Instant,
    /// Relance prévue (changement d'espèce ou de taille).
    restart_at: Option<Instant>,
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
        text: draw::Text::load(),
        maki: {
            let mut m = Maki::new(400.0, s, seed);
            m.sp = espece;
            m
        },
        restart_at: None,
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
        self.interactivity = k;
        self.configured = false;
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
            night,
            day: (6..18).contains(&hour),
            quiet: self.quiet,
            acrobatics: self.cfg.maki.acrobaties,
            show_gap_min: self.cfg.maki.numeros_min.max(1) as f32,
            hovered: self.hovered,
            pointer: self.pointer_world,
            chatting: self.chat.open,
            thinking: self.chat.busy,
            bubble_hover: self.pointer_world.zip(self.bubble_layout.as_ref()).is_some_and(|(p, l)| {
                hit(l.rect, (p.0 - self.origin.0 as f32, p.1 - self.origin.1 as f32))
            }),
            band: if self.cfg.maki.zone == "libre" { 0.0 } else { 0.17 },
            exits: self.exits(),
        }
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
                self.maki.say("D'accord, je te le redis dans 10 minutes.", 3.5);
            }
            "Qu'en faire ?" => {
                if !self.chat.open {
                    self.toggle_chat();
                }
                self.chat.input = format!("{} : {}. Explique-moi simplement et dis-moi quoi faire.", b.msg.title, b.msg.text);
                self.chat.submit();
            }
            "Mettre à jour" => {
                // Le terminal de commande fait le travail (téléchargement, vérification, relance).
                if let Ok(exe) = std::env::current_exe() {
                    let _ = std::process::Command::new(exe).arg("maj").stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn();
                }
                self.maki.say("Je télécharge la nouvelle version…", 6.0);
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
            Msg::Reminder(t) => self.show(Message::new(Kind::Reminder, "Rappel", t)),
            Msg::Hint(t) => self.spontaneous(Message::new(Kind::Info, "", t)),
            Msg::Bubble(m, true) => self.show(m),
            Msg::Bubble(m, false) => self.spontaneous(m),
            Msg::Flip => self.maki.flip(),
            Msg::Wave => self.maki.wave(),
            Msg::Tree => self.maki.tree_show(&ctx),
            Msg::Teleport => self.maki.teleport(&ctx),
            Msg::Climb => self.maki.climb_edge(&ctx),
            Msg::Chat => self.toggle_chat(),
            Msg::Screen => self.other_screen(),
            Msg::Reload => self.reload(),
            Msg::Desktop(id) => self.on_desktop(id),
            Msg::Quiet(v) => {
                self.quiet = v.unwrap_or(!self.quiet);
                let t = if self.quiet { "Je me fais tout discret." } else { "Me revoilà !" };
                self.maki.say(t, 4.0);
            }
            Msg::Hide(v) => {
                self.hidden = v.unwrap_or(!self.hidden);
                if !self.hidden {
                    self.maki.enter(&ctx);
                }
            }
            Msg::Quit => self.exit = true,
        }
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
        let old = std::mem::replace(&mut self.desktop, id.clone());
        if !old.is_empty() {
            self.desk_pos.insert(old, (self.current.clone(), self.maki.pos.0));
        }
        if self.chat.open || !self.maki.visible() || self.hidden {
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
        self.cfg = cfg;
        if changed && self.restart_at.is_none() {
            let e = especes::Espece::parse(&self.cfg.maki.espece).unwrap_or_default();
            if self.chat.open {
                self.toggle_chat();
            }
            self.maki.say(e.presentation(), 3.0);
            let ctx = self.ctx();
            self.maki.teleport(&ctx);
            self.restart_at = Some(Instant::now() + Duration::from_millis(900));
            if let Ok(exe) = std::env::current_exe() {
                let _ = std::process::Command::new("sh")
                    .args(["-c", &format!("sleep 1.6; APPIMAGELAUNCHER_DISABLE=1 exec setsid '{}' >/dev/null 2>&1", std::env::var("APPIMAGE").map(std::path::PathBuf::from).unwrap_or(exe.clone()).display())])
                    .stdin(std::process::Stdio::null())
                    .spawn();
            }
        }
    }

    fn tick(&mut self) -> Duration {
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
        for sig in self.chat.poll() {
            match sig {
                Signal::Answered(t) if !self.chat.open => {
                    let short: String = t.chars().take(220).collect();
                    let short = if t.chars().count() > 220 { format!("{short}…") } else { short };
                    self.show(Message::new(Kind::Answer, "Voilà !", short));
                }
                Signal::Answered(_) => self.maki.react(Kind::Answer),
                Signal::NeedsConfirm if !self.chat.open => self.toggle_chat(),
                Signal::ConfigChanged => self.reload(),
                _ => {}
            }
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
        } else if self.maki.busy() || self.dragging {
            Duration::from_millis(16)
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
        if !self.configured {
            return;
        }
        let s = self.s;
        let (w, h) = self.area();
        let (sw, sh) = (self.sw as f32, self.sh as f32);
        let pose = self.maki.pose();
        let (mx, my) = self.maki.pos;
        let dangle = matches!(pose.kind, PoseKind::Dangle { .. });

        // Rectangle du maki (monde) et position de sa tête.
        let r_maki: R = match pose.kind {
            PoseKind::Dangle { .. } => (mx - 45.0 * s, my - 100.0 * s, 90.0 * s, 160.0 * s),
            PoseKind::Climb => (mx - 130.0 * s, my - 75.0 * s, 260.0 * s, 150.0 * s),
            PoseKind::Sleep => (mx - 70.0 * s, my - 45.0 * s, 130.0 * s, 48.0 * s),
            _ => (mx - 85.0 * s, my - 135.0 * s, 170.0 * s, 140.0 * s),
        };
        let head = match pose.kind {
            PoseKind::Sit => (mx + self.maki.dir * 14.0 * s, my - 88.0 * s),
            PoseKind::Dangle { .. } => (mx, my - 78.0 * s),
            PoseKind::Climb => (mx, my - 60.0 * s),
            PoseKind::Sleep => (mx + self.maki.dir * 24.0 * s, my - 30.0 * s),
            _ => (mx + self.maki.dir * 30.0 * s, my - 72.0 * s),
        };
        let mut lo = (r_maki.0, r_maki.1);
        let mut hi = (r_maki.0 + r_maki.2, r_maki.1 + r_maki.3);
        let mut grow = |r: R| {
            lo = (lo.0.min(r.0), lo.1.min(r.1));
            hi = (hi.0.max(r.0 + r.2), hi.1.max(r.1 + r.3));
        };
        if let Some(t) = &self.maki.tree {
            grow((t.x - 140.0 * s, h - (Tree::height(s) + 70.0 * s) * t.grow.max(0.01), 280.0 * s, (Tree::height(s) + 70.0 * s) * t.grow));
        }
        // Bulles : au-dessus de la tête s'il y a la place, sinon dessous.
        let mut chat_box = None;
        if !self.hidden {
            if let (true, Some(text)) = (self.chat.open, &self.text) {
                let bh = draw::chat_height(text, &self.chat, s) + 16.0 * s;
                let bw = 400.0 * s;
                let above = head.1 - bh - 14.0 * s > 0.0;
                let by = if above { head.1 - 12.0 * s } else { (if dangle { my + 50.0 * s } else { my + 8.0 * s }).min(h - bh) };
                let bx = (head.0 - bw * 0.45).clamp(6.0, (w - bw - 6.0).max(6.0));
                chat_box = Some((bx, by, above, bw, bh));
                grow((bx, if above { by - bh } else { by }, bw, bh));
            } else if self.maki.bubble.is_some() {
                grow(((head.0 - 120.0 * s).max(0.0), head.1 - 230.0 * s, 360.0 * s, 230.0 * s));
            }
        }
        let cx = (lo.0 + hi.0) / 2.0;
        let sx = (cx - sw / 2.0).clamp(0.0, (w - sw).max(0.0)).round();
        let mut sy = (hi.1 - sh + 2.0).clamp(0.0, (h - sh).max(0.0));
        if lo.1 < sy {
            sy = lo.1.max(0.0);
        }
        let sy = sy.round();
        let origin = (sx as i32, sy as i32);
        if origin != self.origin {
            self.origin = origin;
            self.layer.set_margin(origin.1, 0, 0, origin.0);
        }
        let to_local = |p: (f32, f32)| (p.0 - sx, p.1 - sy);

        self.pixmap.fill(tiny_skia::Color::TRANSPARENT);
        self.chat_layout = None;
        self.bubble_layout = None;
        let region = Region::new(&self.compositor).ok();

        if !self.hidden {
            if let Some(t) = &self.maki.tree {
                let (tx, ty) = to_local((t.x, h));
                draw::tree(&mut self.pixmap, Transform::from_translate(tx, ty).pre_scale(s, s), t.grow, t.side, pose.time);
            }
            let (lx, ly) = to_local((mx, my));
            let base = Transform::from_translate(lx, ly).pre_scale(self.maki.dir * s, s);
            self.sprite.fill(tiny_skia::Color::TRANSPARENT);
            draw::maki(&mut self.sprite, base, &pose);
            let lr = (r_maki.0 - sx - 10.0 * s, r_maki.1 - sy - 10.0 * s, r_maki.2 + 20.0 * s, r_maki.3 + 20.0 * s);
            draw::outline(&mut self.sprite, lr, (1.6 * s).round().max(1.0) as i32, (236, 232, 222), 0.85);
            self.pixmap.draw_pixmap(0, 0, self.sprite.as_ref(), &tiny_skia::PixmapPaint::default(), Transform::identity(), None);
            draw::puffs(&mut self.pixmap, (sx, sy), &self.maki.puffs);

            let (hx, hy) = to_local(head);
            if let Some(text) = &self.text {
                if pose.kind == PoseKind::Sleep {
                    let k = (pose.time * 0.6) % 1.0;
                    text.draw(&mut self.pixmap, (11.0 + k * 9.0) * s, hx + 10.0 * s + k * 18.0 * s, hy - 18.0 * s - k * 30.0 * s, "z", (90, 90, 120));
                }
                if let Some((bx, by, above, _, _)) = chat_box {
                    let caret = self.kb_focus && (pose.time * 2.0) as i64 % 2 == 0;
                    let (lbx, lby) = to_local((bx, by));
                    let lay = draw::chat_bubble(&mut self.pixmap, text, &self.chat, hx, hy, above, lbx, lby, s, caret, self.kb_focus, pose.time);
                    self.chat_layout = Some(lay);
                } else if let Some(b) = &self.maki.bubble {
                    // Dessinée à part pour l'apparition et la disparition en fondu.
                    let bold = self.bold.as_ref().unwrap_or(text);
                    let acts = Self::actions_for(&b.msg);
                    self.sprite.fill(tiny_skia::Color::TRANSPARENT);
                    let lay = draw::fancy_bubble(&mut self.sprite, text, bold, b, &acts, hx, hy, s);
                    let opacity = (b.age / 0.25).min(1.0) * (b.ttl / 0.45).clamp(0.0, 1.0);
                    let pp = tiny_skia::PixmapPaint { opacity, ..Default::default() };
                    self.pixmap.draw_pixmap(0, 0, self.sprite.as_ref(), &pp, Transform::identity(), None);
                    self.bubble_layout = Some(lay);
                }
            }
            // Zone cliquable : le maki (serrée) et les bulles.
            let tight = match pose.kind {
                PoseKind::Dangle { .. } => (mx - 28.0 * s, my - 100.0 * s, 56.0 * s, 104.0 * s),
                PoseKind::Climb => (mx - 70.0 * s, my - 45.0 * s, 140.0 * s, 90.0 * s),
                PoseKind::Sleep => (mx - 45.0 * s, my - 38.0 * s, 95.0 * s, 40.0 * s),
                _ => (mx - 45.0 * s, my - 105.0 * s, 95.0 * s, 108.0 * s),
            };
            self.maki_rect = tight;
            if let Some(r) = &region {
                if self.maki.visible() {
                    let (x, y) = to_local((tight.0, tight.1));
                    r.add(x as i32, y as i32, tight.2 as i32, tight.3 as i32);
                }
                for b in [self.chat_layout.as_ref().map(|l| l.rect), self.bubble_layout.as_ref().map(|l| l.rect)].into_iter().flatten() {
                    r.add(b.0 as i32, b.1 as i32, b.2 as i32 + 1, b.3 as i32 + 1);
                }
            }
        }
        let surface = self.layer.wl_surface().clone();
        if let Some(r) = &region {
            surface.set_input_region(Some(r.wl_region()));
        }
        let stride = self.sw as i32 * 4;
        let Ok((buffer, canvas)) = self.pool.create_buffer(self.sw as i32, self.sh as i32, stride, wl_shm::Format::Argb8888) else {
            return;
        };
        for (dst, src) in canvas.chunks_exact_mut(4).zip(self.pixmap.data().chunks_exact(4)) {
            dst[0] = src[2];
            dst[1] = src[1];
            dst[2] = src[0];
            dst[3] = src[3];
        }
        surface.damage_buffer(0, 0, self.sw as i32, self.sh as i32);
        if buffer.attach_to(&surface).is_ok() {
            self.layer.commit();
        }
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
                if let Some((_, p)) = lay.picks.iter().find(|(r, _)| hit(*r, local)) {
                    self.chat.pick(p.clone());
                } else if hit(lay.chip, local) {
                    let a = if self.chat.picker.is_some() { crate::chat::Pick::Close } else { crate::chat::Pick::Open };
                    self.chat.pick(a);
                } else if hit(lay.close, local) {
                    self.toggle_chat();
                } else if lay.yes.is_some_and(|r| hit(r, local)) {
                    self.chat.answer(true);
                } else if lay.no.is_some_and(|r| hit(r, local)) {
                    self.chat.answer(false);
                } else if let Some((_, t)) = lay.copy.iter().find(|(r, _)| hit(*r, local)) {
                    let _ = crate::clipboard::copy(t);
                    self.chat.entries.push(crate::chat::Entry::Info("Réponse copiée dans le presse-papiers.".into()));
                } else {
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
            BTN_LEFT => self.toggle_chat(),
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
        match ev.keysym {
            Keysym::Escape => self.toggle_chat(),
            Keysym::Return | Keysym::KP_Enter => {
                if self.chat.confirm.is_none() {
                    self.chat.submit();
                }
            }
            Keysym::BackSpace => {
                if self.ctrl {
                    let t = self.chat.input.trim_end().to_string();
                    let cut = t.rfind(' ').map(|i| i + 1).unwrap_or(0);
                    self.chat.input.truncate(cut);
                } else {
                    self.chat.input.pop();
                }
            }
            Keysym::Page_Up | Keysym::Up => self.chat.scroll += 3,
            Keysym::Page_Down | Keysym::Down => self.chat.scroll = self.chat.scroll.saturating_sub(3),
            _ if self.ctrl && matches!(ev.keysym, Keysym::v | Keysym::V) => {
                if let Ok(o) = std::process::Command::new("wl-paste").args(["--no-newline", "--type", "text"]).output() {
                    let t = String::from_utf8_lossy(&o.stdout).replace('\n', " ");
                    self.chat.input.push_str(&t);
                }
            }
            _ if self.ctrl && matches!(ev.keysym, Keysym::u | Keysym::U) => self.chat.input.clear(),
            _ => {
                if let Some(t) = ev.utf8 {
                    if self.chat.confirm.is_some() && self.chat.input.is_empty() {
                        match t.as_str() {
                            "o" | "O" | "y" => return self.chat.answer(true),
                            "n" | "N" => return self.chat.answer(false),
                            _ => {}
                        }
                    }
                    if !self.ctrl && t.chars().all(|c| !c.is_control()) && self.chat.input.chars().count() < 2000 {
                        self.chat.input.push_str(&t);
                    }
                }
            }
        }
        self.render();
    }
}

impl CompositorHandler for App {
    fn scale_factor_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: i32) {}
    fn transform_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: wl_output::Transform) {}
    fn frame(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: u32) {}
    fn surface_enter(&mut self, _: &Connection, _: &QueueHandle<Self>, surface: &wl_surface::WlSurface, o: &wl_output::WlOutput) {
        if surface == self.layer.wl_surface() {
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
            if let (Some(p), Some(q)) = (Pixmap::new(self.sw, self.sh), Pixmap::new(self.sw, self.sh)) {
                self.pixmap = p;
                self.sprite = q;
            }
        }
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
        }
    }
    fn leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_keyboard::WlKeyboard, surface: &wl_surface::WlSurface, _: u32) {
        if surface == self.layer.wl_surface() {
            self.kb_focus = false;
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
    }
}

impl PointerHandler for App {
    fn pointer_frame(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_pointer::WlPointer, events: &[PointerEvent]) {
        for ev in events {
            if &ev.surface != self.layer.wl_surface() {
                continue;
            }
            let local = (ev.position.0 as f32, ev.position.1 as f32);
            let world = (local.0 + self.origin.0 as f32, local.1 + self.origin.1 as f32);
            let on_maki = hit(self.maki_rect, world);
            match ev.kind {
                PointerEventKind::Enter { .. } | PointerEventKind::Motion { .. } => {
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
                    } else if self.chat_layout.as_ref().is_some_and(|l| hit(l.input, local)) {
                        CursorIcon::Text
                    } else {
                        CursorIcon::Default
                    };
                    self.set_cursor(c);
                }
                PointerEventKind::Leave { .. } => {
                    self.hovered = false;
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
                        self.on_click(button, local, p.on_maki);
                    }
                }
                PointerEventKind::Axis { vertical, .. } => {
                    let in_picker = self.chat_layout.as_ref().and_then(|l| l.picker).is_some_and(|r| hit(r, local));
                    if in_picker {
                        let d = if vertical.discrete != 0 { vertical.discrete as f64 } else { vertical.absolute / 10.0 };
                        if let Some(p) = &mut self.chat.picker {
                            if d > 0.0 {
                                p.scroll += 2;
                            } else if d < 0.0 {
                                p.scroll = p.scroll.saturating_sub(2);
                            }
                        }
                    } else if self.chat_layout.as_ref().is_some_and(|l| hit(l.rect, local)) {
                        let d = if vertical.discrete != 0 { vertical.discrete as f64 } else { vertical.absolute / 10.0 };
                        if d < 0.0 {
                            self.chat.scroll += 2;
                        } else if d > 0.0 {
                            self.chat.scroll = self.chat.scroll.saturating_sub(2);
                        }
                    }
                }
            }
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

delegate_compositor!(App);
delegate_output!(App);
delegate_shm!(App);
delegate_seat!(App);
delegate_keyboard!(App);
delegate_pointer!(App);
delegate_layer!(App);
delegate_registry!(App);
