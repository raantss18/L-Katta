//! Comportement et physique du maki, sans Wayland (testable).
//!
//! Repère « monde » : pixels logiques de la zone utilisable de l'écran, y vers le bas,
//! `pos` = point entre les pattes (sol à y = h). Le maki marche, bondit, est lancé,
//! tombe et se redresse en l'air, grimpe aux bords de l'écran et à un arbre qu'il fait
//! pousser, se suspend en haut de l'écran puis se laisse tomber, disparaît dans un nuage
//! et réapparaît ailleurs.

use std::f32::consts::{PI, TAU};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Edge {
    Left,
    Right,
}

/// Sortie vers un écran voisin.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Exit {
    Left,
    Right,
    Up,
    Down,
}

/// Écrans voisins disponibles depuis l'écran courant.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Exits {
    pub left: bool,
    pub right: bool,
    pub up: bool,
    pub down: bool,
}

impl Exits {
    pub fn any(&self) -> bool {
        self.left || self.right || self.up || self.down
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Launch {
    pub vx: f32,
    pub vy: f32,
    pub spin: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Mode {
    Sit,
    Walk { to: f32 },
    /// Série de bonds jusqu'à `to` (la course du maki catta).
    Bound { to: f32 },
    /// Accroupi avant un saut (anticipation).
    Crouch { then: Launch },
    /// Vol libre : saut, chute, lancer.
    Air,
    /// Amorti après l'atterrissage.
    Land,
    /// Tenu par la souris.
    Held,
    /// Suspendu au bord haut de l'écran.
    Hang,
    /// Agrippé à un bord de l'écran ou au tronc, grimpe jusqu'à `to_y`.
    Climb { edge: Edge, to_y: f32 },
    /// Assis sur la branche de l'arbre.
    Perch,
    PoofOut,
    Hidden,
    PoofIn,
    Sleep,
    Wave,
    Stretch,
    Scratch,
    /// Réfléchit (une question est en cours).
    Think,
    /// Lève la patte, l'air de dire « j'ai une idée ».
    Point,
    /// Numéros propres aux espèces.
    Sing,
    Call,
    Tap,
    Peek,
}

/// Expression du visage.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum Expr {
    #[default]
    Normal,
    /// Yeux plissés de contentement.
    Happy,
    /// Sourcils froncés, oreilles en arrière.
    Worried,
    /// Grands yeux.
    Surprised,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TreeState {
    Growing,
    Standing,
    Shrinking,
}

#[derive(Debug, Clone, Copy)]
pub struct Tree {
    pub x: f32,
    /// 0 → 1 (avec léger dépassement pendant la pousse).
    pub grow: f32,
    pub state: TreeState,
    /// Côté de la branche (+1 droite, −1 gauche).
    pub side: f32,
    t: f32,
}

impl Tree {
    pub fn height(s: f32) -> f32 {
        330.0 * s
    }
    pub fn branch_y(h: f32, s: f32) -> f32 {
        h - 245.0 * s
    }
    pub fn branch_x(&self, s: f32) -> f32 {
        self.x + self.side * 78.0 * s
    }
}

/// Liane qui se déroule depuis le haut de l'écran pour qu'il grimpe.
#[derive(Debug, Clone, Copy)]
pub struct Vine {
    pub x: f32,
    /// Bas de la liane (le sol).
    pub bottom: f32,
    /// 0 → 1 : déroulée.
    pub grow: f32,
    pub shrinking: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct Puff {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub r: f32,
    pub life: f32,
    pub max: f32,
    pub leaf: bool,
}

#[derive(Debug, Clone)]
pub struct Bubble {
    pub msg: crate::conseils::Message,
    /// Temps restant (s).
    pub ttl: f32,
    /// Âge (s) : apparition, texte qui s'écrit.
    pub age: f32,
}

impl Bubble {
    /// Nombre de caractères déjà « écrits » (effet machine à écrire).
    pub fn revealed(&self) -> usize {
        ((self.age - 0.15).max(0.0) * 60.0) as usize
    }
    pub fn full(&self) -> usize {
        self.msg.title.chars().count() + self.msg.text.chars().count()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Ctx {
    pub w: f32,
    pub h: f32,
    pub night: bool,
    pub quiet: bool,
    pub acrobatics: bool,
    /// Minutes minimum entre deux numéros (arbre, téléportation, escalade…).
    pub show_gap_min: f32,
    pub hovered: bool,
    pub pointer: Option<(f32, f32)>,
    /// La bulle de discussion est ouverte : le maki reste là.
    pub chatting: bool,
    pub thinking: bool,
    /// Le pointeur est sur la bulle : elle reste affichée.
    pub bubble_hover: bool,
    /// Il fait jour (6 h – 18 h) : les espèces nocturnes ont les paupières lourdes.
    pub day: bool,
    /// Largeur relative des bandes de bord où il se promène (0 = tout l'écran).
    pub band: f32,
    /// Zone horizontale occupée par la fenêtre active (à éviter), en px de l'écran.
    pub avoid: Option<(f32, f32)>,
    pub exits: Exits,
}

/// Attitude du corps entier (émotion), en plus de l'expression du visage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Attitude {
    #[default]
    Aucune,
    /// Debout, il parle (bouche qui bouge, une main qui accompagne).
    Parle,
    /// Il danse (une jambe levée, notes de musique).
    Danse,
    /// Bras croisés, sourcils froncés.
    Boude,
    /// Mains sur les hanches, sourire : travail fait.
    Fier,
}

/// Paramètres de dessin.
#[derive(Debug, Clone, Copy, Default)]
pub struct Pose {
    pub kind: PoseKind,
    pub walk_phase: f32,
    pub wave: f32,
    pub spin: f32,
    pub lean: f32,
    /// Écrasement (+) / étirement (−).
    pub squash: f32,
    pub tail: (f32, f32),
    pub blink: bool,
    pub time: f32,
    /// Échelle d'apparition (nuage).
    pub pop: f32,
    /// Angle de balancier quand il est suspendu.
    pub swing: f32,
    pub scratch: f32,
    pub look_up: f32,
    pub expr: Expr,
    /// Patte levée (idée, nouvelle).
    pub point: bool,
    /// Paupières lourdes (espèce nocturne en plein jour).
    pub sleepy: bool,
    pub sing: bool,
    pub call: bool,
    /// Doigt qui tapote (aye-aye).
    pub tap: f32,
    pub attitude: Attitude,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum PoseKind {
    #[default]
    Sit,
    Walk,
    /// Pattes écartées en vol.
    Fly,
    Climb,
    /// Suspendu (bras levés si `arms_up`).
    Dangle { arms_up: bool },
    Sleep,
    Stretch,
    /// Debout (propithèque, indri).
    Upright,
    /// Grimpe à une liane ou à un tronc, une main après l'autre.
    Grimpe,
}

pub struct Maki {
    pub pos: (f32, f32),
    pub vel: (f32, f32),
    pub dir: f32,
    pub mode: Mode,
    pub t: f32,
    pub clock: f32,
    pub s: f32,
    pub spin: f32,
    spin_v: f32,
    squash: f32,
    squash_v: f32,
    tail: (f32, f32),
    tail_v: (f32, f32),
    swing: f32,
    swing_v: f32,
    lean: f32,
    phase: f32,
    hop_to: Option<f32>,
    bounces: u8,
    pub tree: Option<Tree>,
    pub vine: Option<Vine>,
    pub puffs: Vec<Puff>,
    pub bubble: Option<Bubble>,
    next_idea: f32,
    next_show: f32,
    teleport_to: Option<(f32, bool)>,
    rng: fastrand::Rng,
    last_ground: f32,
    /// Sortie demandée vers un écran voisin (lue par la surcouche).
    pub exit: Option<Exit>,
    /// Traverse le sol (vers l'écran du dessous).
    through: bool,
    /// Grimpe le bord pour passer à l'écran du dessus.
    going_up: bool,
    expr: Expr,
    expr_until: f32,
    attitude: Attitude,
    attitude_until: f32,
    /// Temps passé sous le pointeur (caresse).
    petted: f32,
    /// Espèce : allure et démarche.
    pub sp: super::especes::Espece,
    /// Bonds de danse restants (propithèque).
    dance: u8,
    day: bool,
}

const MARGIN: f32 = 26.0;
/// Distance entre la prise (nuque / mains) et les pattes quand il pend.
pub const DANGLE: f32 = 92.0;

fn approach(v: f32, target: f32, rate: f32, dt: f32) -> f32 {
    v + (target - v) * (1.0 - (-rate * dt).exp())
}

impl Maki {
    pub fn new(x: f32, s: f32, seed: u64) -> Self {
        Self {
            pos: (x, 0.0),
            vel: (0.0, 0.0),
            dir: 1.0,
            mode: Mode::Hidden,
            t: 0.0,
            clock: 0.0,
            s,
            spin: 0.0,
            spin_v: 0.0,
            squash: 0.0,
            squash_v: 0.0,
            tail: (0.0, 0.0),
            tail_v: (0.0, 0.0),
            swing: 0.0,
            swing_v: 0.0,
            lean: 0.0,
            phase: 0.0,
            hop_to: None,
            bounces: 0,
            tree: None,
            vine: None,
            puffs: vec![],
            bubble: None,
            next_idea: 6.0,
            next_show: 120.0,
            teleport_to: None,
            rng: fastrand::Rng::with_seed(seed),
            last_ground: 0.0,
            exit: None,
            through: false,
            going_up: false,
            expr: Expr::Normal,
            expr_until: 0.0,
            attitude: Attitude::Aucune,
            attitude_until: 0.0,
            petted: 0.0,
            sp: super::especes::Espece::Catta,
            dance: 0,
            day: true,
        }
    }

    fn style(&self) -> super::especes::Style {
        self.sp.style()
    }

    fn set(&mut self, m: Mode) {
        // Les espèces qui vont debout se déplacent par bonds.
        let m = match m {
            Mode::Walk { to } if self.style().demarche == super::especes::Demarche::Bonds => Mode::Bound { to },
            other => other,
        };
        self.mode = m;
        self.t = 0.0;
        match m {
            Mode::Walk { to } | Mode::Bound { to } => {
                if (to - self.pos.0).abs() > 1.0 {
                    self.dir = (to - self.pos.0).signum();
                }
            }
            Mode::Crouch { then } if then.vx.abs() > 1.0 => self.dir = then.vx.signum(),
            _ => {}
        }
    }

    /// Un point au hasard où il a le droit d'être : bandes de bord, ou tout l'écran.
    fn spot(&mut self, ctx: &Ctx, near_left: Option<bool>) -> f32 {
        let (w, m) = (ctx.w, MARGIN * 3.0);
        if ctx.band <= 0.0 {
            return m + self.rng.f32() * (w - 2.0 * m).max(1.0);
        }
        let bw = (w * ctx.band).max(120.0 * self.s).min(w / 2.0);
        // La fenêtre active déborde sur une bande : il prend l'autre côté.
        let blocked = |left: bool| {
            ctx.avoid.is_some_and(|(a, b)| if left { a < bw } else { b > w - bw })
        };
        let mut left = near_left.unwrap_or_else(|| self.rng.bool());
        if blocked(left) && !blocked(!left) {
            left = !left;
        }
        let x = m + self.rng.f32() * (bw - m).max(1.0);
        if left { x } else { w - x }
    }

    fn in_band(&self, ctx: &Ctx) -> bool {
        let bw = (ctx.w * ctx.band).max(120.0 * self.s);
        let in_band = ctx.band <= 0.0 || self.pos.0 <= bw || self.pos.0 >= ctx.w - bw;
        // Devant la fenêtre active : pas à sa place (sauf si elle couvre tout).
        let covered = ctx.avoid.is_some_and(|(a, b)| self.pos.0 > a - 40.0 * self.s && self.pos.0 < b + 40.0 * self.s);
        let free_side = ctx.avoid.is_some_and(|(a, b)| a > bw || b < ctx.w - bw);
        in_band && !(covered && free_side)
    }

    /// Entrée en scène : apparaît suspendu en haut de l'écran puis se laisse tomber.
    pub fn enter(&mut self, ctx: &Ctx) {
        let x = self.spot(ctx, None);
        self.teleport_to = Some((x, true));
        self.set(Mode::Hidden);
        self.t = 10.0;
    }

    pub fn grounded(&self) -> bool {
        matches!(
            self.mode,
            Mode::Sit | Mode::Walk { .. } | Mode::Land | Mode::Wave | Mode::Stretch | Mode::Scratch | Mode::Think | Mode::Sleep | Mode::Crouch { .. } | Mode::Bound { .. }
        )
    }

    pub fn visible(&self) -> bool {
        !matches!(self.mode, Mode::Hidden)
    }

    /// Animation rapide nécessaire ?
    pub fn busy(&self) -> bool {
        !matches!(self.mode, Mode::Sit | Mode::Sleep | Mode::Perch | Mode::Hidden | Mode::Think)
            || !self.puffs.is_empty()
            || self.tree.is_some_and(|t| t.state != TreeState::Standing)
            || self.squash.abs() > 0.01
            || self.tail_v.0.abs() + self.tail_v.1.abs() > 3.0
            || self.bubble.as_ref().is_some_and(|b| b.age < 0.6 || b.revealed() < b.full() || b.ttl < 0.5)
    }

    pub fn say(&mut self, text: impl Into<String>, secs: f32) {
        self.show(crate::conseils::Message::new(crate::conseils::Kind::Info, "", text), secs);
    }

    /// Affiche une bulle et réagit selon son genre.
    pub fn show(&mut self, msg: crate::conseils::Message, secs: f32) {
        let kind = msg.kind;
        let reading = (msg.title.chars().count() + msg.text.chars().count()) as f32 / 60.0;
        self.bubble = Some(Bubble { msg, ttl: secs + reading, age: 0.0 });
        self.hush();
        self.react(kind);
    }

    /// Il a quelque chose à dire : il cesse d'aller partout et s'assoit (s'il est en l'air
    /// ou agrippé, il finira son geste et s'assiéra en arrivant).
    fn hush(&mut self) {
        self.hop_to = None;
        self.dance = 0;
        self.going_up = false;
        if matches!(
            self.mode,
            Mode::Sleep | Mode::Walk { .. } | Mode::Bound { .. } | Mode::Stretch | Mode::Scratch | Mode::Tap | Mode::Peek | Mode::Sing | Mode::Call
        ) {
            self.vel.0 = 0.0;
            self.set(Mode::Sit);
        }
        if let Mode::Crouch { then } = self.mode {
            if then.vx.abs() > 1.0 {
                self.set(Mode::Sit);
            }
        }
    }

    /// Attitude pour quelques secondes (vue seulement quand il est assis au calme).
    pub fn set_attitude(&mut self, a: Attitude, secs: f32) {
        self.attitude = a;
        self.attitude_until = self.clock + secs;
    }

    pub fn set_expr(&mut self, e: Expr, secs: f32) {
        self.expr = e;
        self.expr_until = self.clock + secs;
    }

    /// Réaction du corps et du visage à ce qu'il annonce.
    pub fn react(&mut self, kind: crate::conseils::Kind) {
        use crate::conseils::Kind as K;
        let calm = matches!(self.mode, Mode::Sit | Mode::Perch | Mode::Think | Mode::Wave | Mode::Point | Mode::Stretch | Mode::Scratch);
        match kind {
            K::Hello => {
                self.set_expr(Expr::Happy, 2.5);
                if calm && self.mode != Mode::Perch {
                    self.set(Mode::Wave);
                }
            }
            K::Tip | K::News | K::Mail => {
                self.set_expr(Expr::Surprised, 1.2);
                if calm && self.mode != Mode::Perch {
                    self.set(Mode::Point);
                }
            }
            K::Reminder => {
                self.set_expr(Expr::Surprised, 1.8);
                if calm && self.mode != Mode::Perch {
                    self.set(Mode::Wave);
                }
            }
            K::Alert => {
                self.set_expr(Expr::Worried, 5.0);
                if calm && self.mode != Mode::Perch {
                    self.set(Mode::Scratch);
                }
            }
            K::Answer => {
                // Travail fait : mains sur les hanches, fier de lui.
                self.set_expr(Expr::Happy, 0.6);
                self.set_attitude(Attitude::Fier, 2.6);
                if calm && self.mode != Mode::Perch {
                    self.set(Mode::Sit);
                }
            }
            K::Info => {
                if calm && self.mode != Mode::Perch {
                    self.set(Mode::Wave);
                }
            }
        }
    }

    pub fn wave(&mut self) {
        if matches!(self.mode, Mode::Sit | Mode::Perch | Mode::Think | Mode::Sleep) {
            self.set(Mode::Wave);
        }
    }

    pub fn flip(&mut self) {
        if self.grounded() {
            let vx = self.dir * 60.0 * self.s;
            self.set(Mode::Crouch { then: Launch { vx, vy: -720.0 * self.s, spin: -self.dir * TAU } });
        }
    }

    pub fn teleport(&mut self, ctx: &Ctx) {
        if !self.visible() {
            return;
        }
        let x = self.spot(ctx, None);
        self.teleport_to = Some((x, self.rng.bool()));
        self.set(Mode::PoofOut);
        self.burst(self.pos.0, self.pos.1 - 40.0 * self.s, 14, false);
    }

    /// Destination de la prochaine réapparition (après un nuage ou un changement d'écran).
    pub fn vanish_to(&mut self, x: f32, from_top: bool) {
        self.teleport_to = Some((x, from_top));
        self.hop_to = None;
        self.dance = 0;
        self.vel = (0.0, 0.0);
        self.spin = 0.0;
        self.set(Mode::Hidden);
    }

    /// Disparaît et réapparaît au sol à l'abscisse donnée.
    pub fn teleport_at(&mut self, x: f32) {
        if !self.visible() {
            return;
        }
        self.teleport_to = Some((x, false));
        self.set(Mode::PoofOut);
        self.burst(self.pos.0, self.pos.1 - 40.0 * self.s, 14, false);
    }

    pub fn tree_show(&mut self, ctx: &Ctx) {
        if !matches!(self.mode, Mode::Sit | Mode::Walk { .. } | Mode::Wave | Mode::Stretch | Mode::Scratch) || self.tree.is_some() {
            return;
        }
        let s = self.s;
        let (x, side) = if ctx.band > 0.0 {
            // Contre le bord, branche vers l'extérieur : le milieu de l'écran reste libre.
            if self.pos.0 < ctx.w / 2.0 { (150.0 * s, -1.0) } else { (ctx.w - 150.0 * s, 1.0) }
        } else {
            let want = self.pos.0 + self.dir * 150.0 * s;
            let x = if want > 140.0 * s && want < ctx.w - 140.0 * s { want } else { self.pos.0 - self.dir * 150.0 * s };
            let x = x.clamp(140.0 * s, (ctx.w - 140.0 * s).max(140.0 * s));
            (x, if x < ctx.w / 2.0 { 1.0 } else { -1.0 })
        };
        self.tree = Some(Tree { x, grow: 0.0, state: TreeState::Growing, side, t: 0.0 });
        self.burst(x, ctx.h - 20.0 * s, 10, true);
        // Il va au pied du tronc (côté maki) puis grimpe.
        self.set(Mode::Sit);
    }

    pub fn climb_edge(&mut self, ctx: &Ctx) {
        if !self.grounded() {
            return;
        }
        let to = if self.pos.0 < ctx.w / 2.0 { MARGIN } else { ctx.w - MARGIN };
        self.hop_to = None;
        self.set(Mode::Bound { to });
    }

    /// Part vers un écran voisin (à pied, en sautant dans le vide ou en grimpant).
    pub fn travel(&mut self, ctx: &Ctx, e: Exit) {
        if !self.grounded() {
            return;
        }
        let s = self.s;
        self.hop_to = None;
        match e {
            Exit::Left if ctx.exits.left => self.set(Mode::Bound { to: -200.0 * s }),
            Exit::Right if ctx.exits.right => self.set(Mode::Bound { to: ctx.w + 200.0 * s }),
            Exit::Down if ctx.exits.down => {
                // Petit saut… et il passe à travers le sol.
                self.through = true;
                self.set(Mode::Crouch { then: Launch { vx: self.dir * 40.0 * s, vy: -380.0 * s, spin: 0.0 } });
            }
            Exit::Up if ctx.exits.up => {
                self.going_up = true;
                let to = if self.pos.0 < ctx.w / 2.0 { MARGIN } else { ctx.w - MARGIN };
                self.set(Mode::Bound { to });
            }
            _ => {}
        }
    }

    /// Arrivée depuis un autre écran.
    pub fn arrive(&mut self, ctx: &Ctx, from: Exit, x: f32) {
        let s = self.s;
        self.exit = None;
        self.through = false;
        self.going_up = false;
        self.hop_to = None;
        self.spin = 0.0;
        match from {
            // Sorti par la gauche : entre par la droite du nouvel écran, et inversement.
            Exit::Left => {
                self.pos = (ctx.w + 30.0 * s, ctx.h);
                let to = self.spot(ctx, Some(false));
                self.set(Mode::Bound { to });
            }
            Exit::Right => {
                self.pos = (-30.0 * s, ctx.h);
                let to = self.spot(ctx, Some(true));
                self.set(Mode::Bound { to });
            }
            // Tombé de l'écran du dessus : relâché du haut de celui-ci.
            Exit::Down => {
                self.pos = (x.clamp(MARGIN, ctx.w - MARGIN), 40.0 * s);
                self.vel = (0.0, 200.0 * s);
                self.spin_v = if self.rng.bool() { 4.0 } else { -4.0 };
                self.bounces = 0;
                self.set(Mode::Air);
            }
            // Monté depuis l'écran du dessous : jaillit du bas.
            Exit::Up => {
                self.pos = (x.clamp(MARGIN, ctx.w - MARGIN), ctx.h);
                self.vel = (0.0, -1100.0 * s);
                self.spin_v = -TAU * 1.2;
                self.bounces = 0;
                self.set(Mode::Air);
            }
        }
    }

    /// Le maki est attrapé par la souris (prise = position du pointeur).
    pub fn grab(&mut self, p: (f32, f32)) {
        if let Some(t) = &mut self.tree {
            if matches!(self.mode, Mode::Perch | Mode::Climb { .. }) {
                t.state = TreeState::Shrinking;
            }
        }
        self.set(Mode::Held);
        self.hop_to = None;
        self.spin = 0.0;
        self.spin_v = 0.0;
        self.pos = (p.0, p.1 + DANGLE * self.s);
        self.squash_v -= 3.0;
    }

    pub fn drag_to(&mut self, p: (f32, f32), dt: f32) {
        let new = (p.0, p.1 + DANGLE * self.s);
        if dt > 0.0 {
            let v = ((new.0 - self.pos.0) / dt, (new.1 - self.pos.1) / dt);
            self.vel = (approach(self.vel.0, v.0, 18.0, dt), approach(self.vel.1, v.1, 18.0, dt));
            // Balancier : la vitesse horizontale de la main fait osciller le corps.
            self.swing_v -= v.0 * 0.0009 * self.s.recip();
        }
        self.pos = new;
    }

    /// Relâché : lancé avec la vitesse de la main, ou agrippé à un bord proche.
    pub fn release(&mut self, ctx: &Ctx) {
        let s = self.s;
        let grip_y = self.pos.1 - DANGLE * s;
        let speed = (self.vel.0.powi(2) + self.vel.1.powi(2)).sqrt();
        if grip_y < 30.0 && speed < 900.0 {
            self.pos.1 = DANGLE * s;
            self.set(Mode::Hang);
            return;
        }
        for (edge, near) in [(Edge::Left, self.pos.0 < 50.0), (Edge::Right, self.pos.0 > ctx.w - 50.0)] {
            if near && speed < 900.0 && self.pos.1 < ctx.h - 40.0 * s {
                self.pos.0 = if edge == Edge::Left { MARGIN } else { ctx.w - MARGIN };
                self.dir = if edge == Edge::Left { -1.0 } else { 1.0 };
                let to = (self.pos.1 - 120.0 * s).max(160.0 * s);
                self.start_vine(ctx.h);
                if let Some(v) = &mut self.vine {
                    v.grow = 1.0; // il attrape une liane au vol
                }
                self.set(Mode::Climb { edge, to_y: to });
                return;
            }
        }
        self.vel = (self.vel.0.clamp(-2200.0, 2200.0), self.vel.1.clamp(-2200.0, 1500.0));
        self.spin_v = self.vel.0 * 0.006 / s;
        self.bounces = 0;
        self.set(Mode::Air);
    }

    fn burst(&mut self, x: f32, y: f32, n: usize, leaf: bool) {
        let s = self.s;
        for i in 0..n {
            let a = i as f32 / n as f32 * TAU + self.rng.f32() * 0.6;
            let sp = (60.0 + self.rng.f32() * 120.0) * s;
            self.puffs.push(Puff {
                x,
                y,
                vx: a.cos() * sp,
                vy: a.sin() * sp - 40.0 * s,
                r: (8.0 + self.rng.f32() * 10.0) * s,
                life: 0.0,
                max: 0.5 + self.rng.f32() * 0.4,
                leaf,
            });
        }
    }

    fn launch(&mut self, l: Launch) {
        self.vel = (l.vx, l.vy);
        self.spin_v = l.spin / 0.9;
        self.bounces = 0;
        self.squash_v -= 5.0;
        self.set(Mode::Air);
    }

    fn ground_y(&self, ctx: &Ctx) -> f32 {
        ctx.h
    }

    pub fn step(&mut self, dt: f32, ctx: Ctx) {
        let dt = dt.min(0.05);
        let s = self.s;
        self.day = ctx.day;
        self.clock += dt;
        self.t += dt;
        let (w, h) = (ctx.w, ctx.h);
        let prev_vel = self.vel;

        if let Some(b) = &mut self.bubble {
            b.age += dt;
            if !ctx.bubble_hover {
                b.ttl -= dt;
            }
            if b.ttl <= 0.0 {
                self.bubble = None;
            }
        }
        // Caresse : la souris posée sur lui le rend tout content.
        if ctx.hovered && !matches!(self.mode, Mode::Held | Mode::Air) {
            self.petted += dt;
            if self.petted > 1.2 {
                self.set_expr(Expr::Happy, 0.6);
                // Une longue caresse : il danse de joie.
                if self.petted > 2.6 && self.clock > self.attitude_until && self.mode == Mode::Sit {
                    self.set_attitude(Attitude::Danse, 3.2);
                }
                self.tail_v.0 += (self.clock * 6.0).sin() * 40.0 * dt;
            }
        } else {
            self.petted = 0.0;
        }
        self.update_tree(dt, ctx);
        if let Some(v) = &mut self.vine {
            if v.shrinking {
                v.grow -= dt / 0.6;
            } else if v.grow < 1.0 {
                v.grow = (v.grow + dt / 0.7).min(1.0);
            }
            if v.grow <= 0.0 {
                self.vine = None;
            }
        }
        if self.vine.is_some_and(|v| !v.shrinking) && !matches!(self.mode, Mode::Climb { .. } | Mode::Hang | Mode::Air | Mode::Held) {
            self.drop_vine();
        }
        for p in &mut self.puffs {
            p.life += dt;
            p.x += p.vx * dt;
            p.y += p.vy * dt;
            p.vx *= 1.0 - 2.5 * dt;
            p.vy = p.vy * (1.0 - 2.5 * dt) + if p.leaf { 160.0 * s * dt } else { -30.0 * s * dt };
        }
        self.puffs.retain(|p| p.life < p.max);

        match self.mode {
            Mode::Hidden => {
                if self.t > 0.55 {
                    if let Some((x, top)) = self.teleport_to.take() {
                        if top {
                            self.pos = (x, DANGLE * s);
                            self.swing_v = 2.5;
                            self.set(Mode::Hang);
                        } else {
                            self.pos = (x, h);
                            self.set(Mode::PoofIn);
                        }
                        self.burst(x, self.pos.1 - 45.0 * s, 14, false);
                    } else {
                        self.pos = (self.pos.0.clamp(MARGIN, w - MARGIN), h);
                        self.set(Mode::PoofIn);
                    }
                }
            }
            Mode::PoofOut if self.t > 0.32 => self.set(Mode::Hidden),
            Mode::PoofIn if self.t > 0.38 => self.set(Mode::Sit),
            // Il a quelque chose à dire, ou on discute avec lui : il ne repart pas.
            Mode::Bound { .. } | Mode::Walk { .. } if self.bubble.is_some() || ctx.chatting => {
                self.vel.0 = 0.0;
                self.set(Mode::Sit);
            }
            Mode::Walk { to } => {
                let d = to - self.pos.0;
                let st = self.style();
                let vmax = st.vitesse * s;
                let mut want = d.signum() * vmax.min(d.abs() * 2.2);
                // Microcèbe : petites courses entrecoupées d'arrêts aux aguets.
                if st.demarche == super::especes::Demarche::Trottine && (self.t % 1.1) > 0.7 {
                    want = 0.0;
                }
                self.vel.0 = approach(self.vel.0, want, 5.0, dt);
                self.pos.0 += self.vel.0 * dt;
                self.phase += self.vel.0.abs() * dt / (30.0 * s) * TAU;
                if d.abs() < 1.5 && self.vel.0.abs() < 6.0 || ctx.hovered {
                    self.vel.0 = 0.0;
                    self.set(Mode::Sit);
                    self.next_idea = self.clock + 15.0 + self.rng.f32() * 40.0;
                }
            }
            Mode::Bound { to } => {
                if to < 0.0 && ctx.exits.left && self.pos.0 <= MARGIN + 1.0 {
                    self.leave(Exit::Left);
                    return;
                }
                if to > w && ctx.exits.right && self.pos.0 >= w - MARGIN - 1.0 {
                    self.leave(Exit::Right);
                    return;
                }
                let d = to - self.pos.0;
                if d.abs() < 14.0 * s {
                    self.pos.0 = to;
                    self.after_bound(ctx);
                } else {
                    let st = self.style();
                    // Vitesse choisie pour retomber pile sur la cible (durée du vol = 2·vy/g).
                    let flight = 2.0 * st.bond_vy / 2100.0;
                    let vx = d.signum() * (d.abs() / flight).min(st.bond_vx * s);
                    self.hop_to = Some(to);
                    self.set(Mode::Crouch { then: Launch { vx, vy: -st.bond_vy * s, spin: 0.0 } });
                }
            }
            Mode::Crouch { then } => {
                let dur = if self.hop_to.is_some() { 0.07 } else { 0.2 };
                if self.t >= dur {
                    self.launch(then);
                }
            }
            Mode::Air => self.air(dt, ctx),
            Mode::Land => {
                if self.t > 0.22 {
                    self.set(Mode::Sit);
                    self.next_idea = self.clock + 4.0 + self.rng.f32() * 10.0;
                }
            }
            Mode::Held => {
                self.swing_v += (-self.swing * 40.0 - self.swing_v * 3.0) * dt;
                self.swing += self.swing_v * dt;
            }
            Mode::Hang => {
                self.swing_v += (-self.swing * 18.0 - self.swing_v * 1.2) * dt;
                self.swing += self.swing_v * dt;
                self.phase += dt * 6.0;
                if self.t > 1.6 && !ctx.chatting {
                    // Il lâche prise.
                    self.vel = (self.swing_v * 40.0 * s, 0.0);
                    self.spin_v = self.rng.f32() * 6.0 - 3.0;
                    self.bounces = 0;
                    self.set(Mode::Air);
                }
            }
            Mode::Climb { edge, to_y } => {
                let d = to_y - self.pos.1;
                // Une main après l'autre : la montée avance par tractions.
                let pull = 0.25 + 0.75 * (self.phase.sin().max(0.0));
                let v = 110.0 * s * pull;
                let ready = self.tree.is_some() || self.vine.is_none_or(|v| v.grow >= 1.0);
                if !ready {
                    // Il attend que la liane soit déroulée.
                } else if d.abs() > 2.0 {
                    self.pos.1 += d.signum() * (v * dt).min(d.abs());
                    self.phase += dt * 7.0;
                } else if self.going_up && ctx.exits.up {
                    self.leave(Exit::Up);
                    return;
                } else if let Some(t) = self.tree {
                    // Sur l'arbre : passe sur la branche.
                    if t.state == TreeState::Standing && self.t > 0.4 {
                        self.pos = (t.branch_x(s), Tree::branch_y(h, s));
                        self.dir = t.side;
                        self.squash_v += 4.0;
                        self.set(Mode::Perch);
                        self.next_idea = self.clock + 7.0 + self.rng.f32() * 6.0;
                    }
                } else if (self.t > 1.2 + (self.clock % 1.0) || self.bubble.is_some()) && !ctx.chatting {
                    // Saute du mur vers l'intérieur de l'écran avec une vrille.
                    let away = if edge == Edge::Left { 1.0 } else { -1.0 };
                    self.dir = away;
                    self.drop_vine();
                    self.launch(Launch { vx: away * 330.0 * s, vy: -330.0 * s, spin: -away * TAU });
                }
            }
            Mode::Perch => {
                if self.clock > self.next_idea && !ctx.chatting {
                    let side = self.tree.map(|t| t.side).unwrap_or(1.0);
                    if let Some(t) = &mut self.tree {
                        t.state = TreeState::Shrinking;
                        t.t = 0.0;
                    }
                    self.hop_to = None;
                    self.set(Mode::Crouch { then: Launch { vx: side * 230.0 * s, vy: -470.0 * s, spin: -side * TAU * 1.0 } });
                    self.mode = Mode::Crouch { then: Launch { vx: side * 230.0 * s, vy: -470.0 * s, spin: -side * TAU } };
                }
            }
            Mode::Wave if self.t > 2.2 => self.set(Mode::Sit),
            Mode::Stretch if self.t > 2.0 => self.set(Mode::Sit),
            Mode::Scratch if self.t > 1.6 => self.set(Mode::Sit),
            Mode::Point if self.t > 2.6 => self.set(Mode::Sit),
            Mode::Sing if self.t > 4.5 => self.set(Mode::Sit),
            Mode::Call if self.t > 2.0 => self.set(Mode::Sit),
            Mode::Tap if self.t > 3.2 => self.set(Mode::Sit),
            Mode::Peek => {
                // Regarde à gauche, à droite, à gauche…
                let k = (self.t / 0.55) as i32;
                self.dir = if k % 2 == 0 { 1.0 } else { -1.0 };
                self.set_expr(Expr::Surprised, 0.2);
                if self.t > 2.4 {
                    self.set(Mode::Sit);
                }
            }
            Mode::Think if !ctx.thinking => self.set(Mode::Sit),
            _ => {}
        }

        // Ressorts : écrasement, queue (inertie), inclinaison.
        let acc = ((self.vel.0 - prev_vel.0) / dt.max(1e-3), (self.vel.1 - prev_vel.1) / dt.max(1e-3));
        let squash_target = match self.mode {
            Mode::Crouch { .. } => 0.22,
            Mode::Air => -(self.vel.1.abs() / (3500.0 * s)).min(0.12),
            _ => 0.0,
        };
        self.squash_v += ((squash_target - self.squash) * 220.0 - self.squash_v * 16.0) * dt;
        self.squash = (self.squash + self.squash_v * dt).clamp(-0.3, 0.42);
        let tail_target = (-self.vel.0 * self.dir * 0.03 / s, self.vel.1 * 0.02 / s);
        let tail_force = (-acc.0 * self.dir * 0.004 / s, -acc.1 * 0.004 / s);
        self.tail_v.0 += ((tail_target.0 - self.tail.0) * 55.0 - self.tail_v.0 * 6.0 + tail_force.0.clamp(-400.0, 400.0)) * dt;
        self.tail_v.1 += ((tail_target.1 - self.tail.1) * 55.0 - self.tail_v.1 * 6.0 + tail_force.1.clamp(-400.0, 400.0)) * dt;
        self.tail.0 = (self.tail.0 + self.tail_v.0 * dt).clamp(-30.0, 30.0);
        self.tail.1 = (self.tail.1 + self.tail_v.1 * dt).clamp(-30.0, 30.0);
        let lean_target = match self.mode {
            Mode::Walk { .. } => (acc.0 * self.dir * 0.0006 / s).clamp(-0.15, 0.15),
            _ => 0.0,
        };
        self.lean = approach(self.lean, lean_target, 8.0, dt);

        if matches!(self.mode, Mode::Sit) {
            self.decide(ctx);
        }
        if matches!(self.mode, Mode::Sit | Mode::Perch | Mode::Think) {
            if let Some((px, _)) = ctx.pointer {
                if (px - self.pos.0).abs() > 30.0 && (px - self.pos.0).abs() < 700.0 && self.mode != Mode::Perch {
                    self.dir = (px - self.pos.0).signum();
                }
            }
        }
        if self.grounded() && !matches!(self.mode, Mode::Crouch { .. }) {
            self.pos.1 = self.ground_y(&ctx);
            self.last_ground = self.clock;
        }
        if !matches!(self.mode, Mode::Held | Mode::Climb { .. } | Mode::Hang | Mode::Air) {
            self.pos.0 = self.pos.0.clamp(MARGIN, (w - MARGIN).max(MARGIN));
        }
    }

    /// Fait descendre une liane devant lui (à son bord) avant de grimper.
    fn start_vine(&mut self, ground: f32) {
        let s = self.s;
        self.vine = Some(Vine { x: self.pos.0 + self.dir * 13.0 * s, bottom: ground, grow: 0.0, shrinking: false });
    }

    fn drop_vine(&mut self) {
        if let Some(v) = &mut self.vine {
            v.shrinking = true;
        }
    }

    fn leave(&mut self, e: Exit) {
        self.exit = Some(e);
        self.through = false;
        self.going_up = false;
        self.hop_to = None;
        self.vel = (0.0, 0.0);
        self.set(Mode::Hidden);
        self.vine = None;
        self.t = -1e6; // attend que la surcouche le fasse arriver ailleurs
    }

    fn after_bound(&mut self, ctx: Ctx) {
        let s = self.s;
        self.hop_to = None;
        if self.dance > 0 {
            // Danse du propithèque : bonds de côté, aller-retour.
            self.dance -= 1;
            let back = if self.dance % 2 == 0 { 1.0 } else { -1.0 };
            let to = (self.pos.0 + back * 80.0 * s).clamp(MARGIN * 3.0, ctx.w - MARGIN * 3.0);
            self.set(Mode::Bound { to });
            return;
        }
        // Arrivé au bord : il s'agrippe et grimpe.
        if self.pos.0 <= MARGIN + 2.0 || self.pos.0 >= ctx.w - MARGIN - 2.0 {
            let edge = if self.pos.0 <= MARGIN + 2.0 { Edge::Left } else { Edge::Right };
            self.dir = if edge == Edge::Left { -1.0 } else { 1.0 };
            let to = if self.going_up { 30.0 * s } else { (ctx.h - (220.0 + self.rng.f32() * 260.0) * s).max(150.0 * s) };
            self.start_vine(ctx.h);
            self.set(Mode::Climb { edge, to_y: to });
        } else if let Some(t) = self.tree.filter(|t| t.state == TreeState::Standing) {
            let trunk = t.x - t.side * 9.0 * s;
            if (self.pos.0 - trunk).abs() < 20.0 * s {
                let edge = if t.side > 0.0 { Edge::Right } else { Edge::Left };
                self.pos.0 = trunk;
                self.dir = t.side;
                self.set(Mode::Climb { edge, to_y: Tree::branch_y(ctx.h, s) + 6.0 * s });
            } else {
                self.set(Mode::Bound { to: trunk });
            }
        } else {
            self.set(Mode::Land);
        }
    }

    fn air(&mut self, dt: f32, ctx: Ctx) {
        let s = self.s;
        let g = 2100.0 * s;
        self.vel.1 += g * dt;
        self.pos.0 += self.vel.0 * dt;
        self.pos.1 += self.vel.1 * dt;
        self.spin += self.spin_v * dt;
        self.phase = 1.0;
        let ground = self.ground_y(&ctx);
        // Redressement félin avant de toucher le sol.
        let to_ground = ground - self.pos.1;
        let eta = if self.vel.1 > 0.0 { to_ground / self.vel.1.max(1.0) } else { 1.0 };
        if eta < 0.28 || self.spin_v.abs() < 0.5 {
            let target = (self.spin / TAU).round() * TAU;
            self.spin = approach(self.spin, target, 14.0, dt);
            self.spin_v *= 1.0 - (8.0 * dt).min(1.0);
        }
        // Vers un écran voisin.
        if self.through && self.pos.1 > ground + 160.0 * s {
            self.leave(Exit::Down);
            return;
        }
        let beyond = self.hop_to.map(|t| (t < 0.0, t > ctx.w)).unwrap_or((false, false));
        if (beyond.0 && ctx.exits.left && self.pos.0 < -10.0) || (beyond.1 && ctx.exits.right && self.pos.0 > ctx.w + 10.0) {
            self.leave(if beyond.0 { Exit::Left } else { Exit::Right });
            return;
        }
        // Bords de l'écran : rebond ou s'agrippe (sauf s'il part vers un écran voisin).
        for edge in [Edge::Left, Edge::Right] {
            if (edge == Edge::Left && beyond.0 && ctx.exits.left) || (edge == Edge::Right && beyond.1 && ctx.exits.right) {
                continue;
            }
            let (hit, x) = match edge {
                Edge::Left => (self.pos.0 < MARGIN, MARGIN),
                Edge::Right => (self.pos.0 > ctx.w - MARGIN, ctx.w - MARGIN),
            };
            if hit {
                self.pos.0 = x;
                if self.vel.0.abs() > 380.0 * s && self.pos.1 < ground - 140.0 * s && self.rng.f32() < 0.7 {
                    self.spin = 0.0;
                    self.spin_v = 0.0;
                    self.dir = if edge == Edge::Left { -1.0 } else { 1.0 };
                    self.vel = (0.0, 0.0);
                    self.squash_v += 4.0;
                    let to = (self.pos.1 - 60.0 * s).max(150.0 * s);
                    self.start_vine(ground);
                    if let Some(v) = &mut self.vine {
                        v.grow = 1.0;
                    }
                    self.set(Mode::Climb { edge, to_y: to });
                    return;
                }
                self.vel.0 = -self.vel.0 * 0.45;
                self.spin_v = -self.spin_v * 0.5;
            }
        }
        if self.pos.1 - 110.0 * s < 0.0 && self.vel.1 < 0.0 {
            self.pos.1 = 110.0 * s;
            self.vel.1 = -self.vel.1 * 0.3;
        }
        if self.pos.1 >= ground && !self.through {
            self.pos.1 = ground;
            let impact = self.vel.1;
            self.squash_v += (impact / (260.0 * s)).min(11.0);
            if impact > 900.0 * s && self.bounces < 1 && self.hop_to.is_none() {
                self.bounces += 1;
                self.vel.1 = -impact * 0.28;
                self.vel.0 *= 0.6;
                return;
            }
            self.vel = (0.0, 0.0);
            self.spin = 0.0;
            self.spin_v = 0.0;
            if let Some(to) = self.hop_to {
                self.set(Mode::Bound { to });
            } else {
                self.set(Mode::Land);
            }
        }
    }

    fn update_tree(&mut self, dt: f32, ctx: Ctx) {
        let s = self.s;
        let Some(t) = &mut self.tree else { return };
        t.t += dt;
        match t.state {
            TreeState::Growing => {
                let k = (t.t / 0.9).min(1.0);
                // Pousse élastique.
                t.grow = 1.0 - (1.0 - k).powi(3) + (k * PI).sin() * 0.08;
                if k >= 1.0 {
                    t.grow = 1.0;
                    t.state = TreeState::Standing;
                    let trunk = t.x - t.side * 9.0 * s;
                    if matches!(self.mode, Mode::Sit | Mode::Walk { .. }) {
                        self.mode = Mode::Bound { to: trunk };
                        self.t = 0.0;
                        self.dir = (trunk - self.pos.0).signum();
                    }
                }
            }
            TreeState::Standing => {
                // L'arbre repart s'il n'est plus utilisé.
                if !matches!(self.mode, Mode::Bound { .. } | Mode::Crouch { .. } | Mode::Air | Mode::Climb { .. } | Mode::Perch) && t.t > 3.0 {
                    t.state = TreeState::Shrinking;
                    t.t = 0.0;
                }
            }
            TreeState::Shrinking => {
                t.grow = (1.0 - t.t / 0.8).max(0.0);
                if t.grow <= 0.0 {
                    self.tree = None;
                }
            }
        }
        let _ = ctx;
    }

    fn decide(&mut self, ctx: Ctx) {
        if ctx.chatting {
            if ctx.thinking {
                self.set(Mode::Think);
            }
            return;
        }
        if ctx.night && self.bubble.is_none() && !self.style().nocturne {
            self.set(Mode::Sleep);
            return;
        }
        if self.clock < self.next_idea || ctx.hovered || self.bubble.is_some() {
            return;
        }
        let r = self.rng.f32();
        let s = self.s;
        self.next_idea = self.clock + 30.0 + self.rng.f32() * if ctx.quiet { 180.0 } else { 90.0 };
        let show_ok = ctx.acrobatics && !ctx.quiet && self.clock >= self.next_show;
        if show_ok && r < 0.12 {
            self.next_show = self.clock + ctx.show_gap_min * 60.0 * (0.7 + self.rng.f32() * 0.6);
            match self.rng.u8(0..4) {
                0 => self.tree_show(&ctx),
                1 => self.teleport(&ctx),
                2 => self.climb_edge(&ctx),
                _ => self.flip(),
            }
            return;
        }
        // Hors des bandes de bord (lâché au milieu…) : il libère la zone de travail.
        if !self.in_band(&ctx) {
            let to = self.spot(&ctx, Some(self.pos.0 < ctx.w / 2.0));
            self.set(Mode::Walk { to });
            return;
        }
        // De temps en temps, il va voir un autre écran.
        if ctx.exits.any() && !ctx.quiet && ctx.acrobatics && r > 0.985 {
            let opts: Vec<Exit> = [(ctx.exits.left, Exit::Left), (ctx.exits.right, Exit::Right), (ctx.exits.up, Exit::Up), (ctx.exits.down, Exit::Down)]
                .into_iter()
                .filter(|(ok, _)| *ok)
                .map(|(_, e)| e)
                .collect();
            let e = opts[self.rng.usize(0..opts.len())];
            self.travel(&ctx, e);
            return;
        }
        let calm = if ctx.quiet { 0.8 } else { 0.55 };
        if r < calm {
            return;
        }
        let k = self.rng.f32();
        let to = if ctx.band > 0.0 {
            self.spot(&ctx, Some(self.pos.0 < ctx.w / 2.0))
        } else {
            let span = (ctx.w * 0.4).max(120.0);
            (self.pos.0 + (self.rng.f32() * 2.0 - 1.0) * span).clamp(MARGIN * 3.0, ctx.w - MARGIN * 3.0)
        };
        if k < 0.5 {
            self.set(Mode::Walk { to });
        } else if k < 0.56 && !ctx.quiet {
            self.set(Mode::Bound { to });
        } else if k < 0.75 {
            self.numero(&ctx);
        } else if k < 0.9 {
            self.set(Mode::Scratch);
        } else {
            self.dir = -self.dir;
            let _ = s;
        }
    }

    /// Le numéro favori de l'espèce.
    pub fn numero(&mut self, ctx: &Ctx) {
        use super::especes::Numero;
        if !self.grounded() {
            return;
        }
        match self.style().numero {
            Numero::Soleil => self.set(Mode::Stretch),
            Numero::Danse => {
                self.dance = 4;
                let to = (self.pos.0 + self.dir * 80.0 * self.s).clamp(MARGIN * 3.0, ctx.w - MARGIN * 3.0);
                self.set(Mode::Bound { to });
            }
            Numero::Chant => self.set(Mode::Sing),
            Numero::Cri => {
                self.set_expr(Expr::Surprised, 2.0);
                self.set(Mode::Call);
            }
            Numero::Tapote => self.set(Mode::Tap),
            Numero::Guette => self.set(Mode::Peek),
        }
    }

    pub fn pose(&self) -> Pose {
        let mut p = Pose {
            time: self.clock,
            tail: (self.tail.0 + (self.clock * 1.3).sin() * 3.0, self.tail.1),
            squash: self.squash,
            lean: self.lean,
            spin: self.spin,
            blink: (self.clock % 4.3) < 0.12,
            pop: 1.0,
            swing: self.swing,
            walk_phase: self.phase,
            ..Default::default()
        };
        p.expr = if self.clock < self.expr_until {
            self.expr
        } else if matches!(self.mode, Mode::Air | Mode::Held) && self.vel.1.abs() > 600.0 * self.s {
            Expr::Surprised
        } else {
            Expr::Normal
        };
        let upright = self.style().demarche == super::especes::Demarche::Bonds;
        p.sleepy = self.style().nocturne && self.day && !matches!(self.mode, Mode::Air | Mode::Held);
        p.kind = match self.mode {
            Mode::Walk { .. } => PoseKind::Walk,
            Mode::Crouch { .. } | Mode::Land | Mode::Bound { .. } if upright => PoseKind::Upright,
            Mode::Air if upright && (self.hop_to.is_some() || self.dance > 0) => PoseKind::Upright,
            Mode::Sing => PoseKind::Upright,
            Mode::Crouch { .. } | Mode::Land | Mode::Bound { .. } => PoseKind::Walk,
            Mode::Air => PoseKind::Fly,
            Mode::Climb { .. } => PoseKind::Grimpe,
            Mode::Held => PoseKind::Dangle { arms_up: false },
            Mode::Hang => PoseKind::Dangle { arms_up: true },
            Mode::Sleep => PoseKind::Sleep,
            Mode::Stretch => PoseKind::Stretch,
            _ => PoseKind::Sit,
        };
        match self.mode {
            Mode::Wave => p.wave = (self.t * 9.0).sin(),
            Mode::Point => p.point = true,
            Mode::Sing => {
                p.sing = true;
                p.look_up = 0.6;
            }
            Mode::Call => p.call = true,
            Mode::Tap => p.tap = (self.t * 14.0).sin(),
            Mode::Scratch => p.scratch = (self.t * 22.0).sin(),
            Mode::Think => p.scratch = (self.clock * 9.0).sin() * 0.6,
            Mode::Stretch => p.squash = 0.0,
            Mode::Sleep => p.blink = true,
            Mode::PoofOut => p.pop = (1.0 - self.t / 0.32).max(0.0),
            Mode::PoofIn => {
                let k = (self.t / 0.38).min(1.0);
                p.pop = k * (1.0 + (1.0 - k) * 1.2 * (k * PI).sin());
            }
            Mode::Hidden => p.pop = 0.0,
            Mode::Perch => p.look_up = (self.clock * 0.7).sin() * 0.5,
            _ => {}
        }
        if matches!(self.mode, Mode::Stretch) {
            p.walk_phase = (self.t / 2.0).min(1.0);
        }
        // Assis au calme : l'attitude du moment, ou debout à parler quand une bulle est là.
        if self.mode == Mode::Sit {
            p.attitude = if self.clock < self.attitude_until {
                self.attitude
            } else if self.bubble.as_ref().is_some_and(|b| b.age < 6.0) {
                Attitude::Parle
            } else {
                Attitude::Aucune
            };
        }
        p
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> Ctx {
        Ctx {
            w: 1600.0,
            h: 900.0,
            night: false,
            quiet: false,
            acrobatics: true,
            show_gap_min: 5.0,
            hovered: false,
            pointer: None,
            chatting: false,
            thinking: false,
            bubble_hover: false,
            day: true,
            band: 0.0,
            exits: Exits::default(),
            avoid: None,
        }
    }

    fn run(m: &mut Maki, secs: f32, c: Ctx) {
        for _ in 0..(secs / 0.016) as usize {
            m.step(0.016, c);
        }
    }

    #[test]
    fn spots_avoid_the_active_window() {
        let mut m = Maki::new(400.0, 1.0, 7);
        // Fenêtre active sur la moitié gauche : il choisit toujours la bande de droite.
        let c = Ctx { band: 0.15, avoid: Some((0.0, 800.0)), ..ctx() };
        for _ in 0..50 {
            assert!(m.spot(&c, Some(true)) > 800.0);
        }
        // Devant la fenêtre, avec un côté libre : pas à sa place.
        m.pos.0 = 100.0;
        assert!(!m.in_band(&c));
        // Fenêtre maximisée (pas de zone à éviter) : la bande de gauche lui va.
        assert!(m.in_band(&Ctx { band: 0.15, ..ctx() }));
    }

    #[test]
    fn enters_from_top_and_lands() {
        let mut m = Maki::new(400.0, 1.0, 3);
        m.enter(&ctx());
        run(&mut m, 1.0, ctx());
        assert_eq!(m.mode, Mode::Hang);
        assert!(m.pos.1 < 200.0);
        run(&mut m, 4.0, Ctx { acrobatics: false, ..ctx() });
        assert!(m.grounded(), "{:?}", m.mode);
        assert_eq!(m.pos.1, 900.0);
        assert!(m.spin.abs() < 0.01);
    }

    #[test]
    fn thrown_lands_on_screen() {
        let mut m = Maki::new(400.0, 1.0, 5);
        m.mode = Mode::Sit;
        m.pos = (400.0, 900.0);
        m.grab((700.0, 300.0));
        m.drag_to((760.0, 280.0), 0.016);
        m.vel = (1500.0, -600.0);
        m.release(&ctx());
        run(&mut m, 6.0, Ctx { acrobatics: false, ..ctx() });
        assert!(m.pos.0 >= MARGIN && m.pos.0 <= 1600.0 - MARGIN);
        assert!(!matches!(m.mode, Mode::Air | Mode::Held));
    }

    #[test]
    fn walk_arrives_without_overshoot() {
        let mut m = Maki::new(300.0, 1.0, 1);
        m.mode = Mode::Sit;
        m.pos = (300.0, 900.0);
        m.set(Mode::Walk { to: 500.0 });
        run(&mut m, 8.0, Ctx { acrobatics: false, quiet: true, ..ctx() });
        assert!((m.pos.0 - 500.0).abs() < 3.0 || m.mode != Mode::Sit, "x={}", m.pos.0);
    }

    #[test]
    fn tree_show_climbs_and_jumps_down() {
        let mut m = Maki::new(600.0, 1.0, 9);
        m.mode = Mode::Sit;
        m.pos = (600.0, 900.0);
        m.tree_show(&ctx());
        let mut perched = false;
        for _ in 0..(30.0 / 0.016) as usize {
            m.step(0.016, Ctx { acrobatics: false, ..ctx() });
            perched |= m.mode == Mode::Perch;
        }
        assert!(perched, "le maki n'est jamais monté sur la branche");
        assert!(m.tree.is_none());
        assert!(m.grounded());
    }

    #[test]
    fn teleport_reappears() {
        let mut m = Maki::new(600.0, 1.0, 11);
        m.mode = Mode::Sit;
        m.pos = (600.0, 900.0);
        m.teleport(&ctx());
        run(&mut m, 6.0, Ctx { acrobatics: false, ..ctx() });
        assert!(m.visible() && m.grounded());
    }

    #[test]
    fn edge_climb_then_jump() {
        let mut m = Maki::new(200.0, 1.0, 2);
        m.mode = Mode::Sit;
        m.pos = (200.0, 900.0);
        m.climb_edge(&ctx());
        let mut climbed = false;
        for _ in 0..(20.0 / 0.016) as usize {
            m.step(0.016, Ctx { acrobatics: false, ..ctx() });
            climbed |= matches!(m.mode, Mode::Climb { .. });
        }
        assert!(climbed);
        assert!(m.grounded());
    }

    #[test]
    fn quiet_means_no_show() {
        let mut m = Maki::new(600.0, 1.0, 4);
        m.mode = Mode::Sit;
        m.pos = (600.0, 900.0);
        for _ in 0..(1200.0 / 0.05) as usize {
            m.step(0.05, Ctx { quiet: true, ..ctx() });
            assert!(m.tree.is_none() && !matches!(m.mode, Mode::Hidden | Mode::Climb { .. }));
        }
    }
}

#[cfg(test)]
mod tests_ecrans {
    use super::*;

    fn ctx() -> Ctx {
        Ctx {
            w: 1600.0,
            h: 900.0,
            night: false,
            quiet: false,
            acrobatics: false,
            show_gap_min: 5.0,
            hovered: false,
            pointer: None,
            chatting: false,
            thinking: false,
            bubble_hover: false,
            day: true,
            band: 0.18,
            exits: Exits { left: false, right: true, up: false, down: true },
            avoid: None,
        }
    }

    fn sitting(x: f32) -> Maki {
        let mut m = Maki::new(x, 1.0, 21);
        m.mode = Mode::Sit;
        m.pos = (x, 900.0);
        m
    }

    #[test]
    fn stays_on_the_edges() {
        let mut m = sitting(800.0);
        for _ in 0..(600.0 / 0.05) as usize {
            m.step(0.05, Ctx { exits: Exits::default(), ..ctx() });
        }
        assert!(m.pos.0 <= 1600.0 * 0.18 + 1.0 || m.pos.0 >= 1600.0 * 0.82 - 1.0, "x={}", m.pos.0);
    }

    #[test]
    fn leaves_by_the_right_and_falls_through() {
        let mut m = sitting(1500.0);
        m.travel(&ctx(), Exit::Right);
        for _ in 0..600 {
            m.step(0.016, ctx());
            if m.exit.is_some() {
                break;
            }
        }
        assert_eq!(m.exit, Some(Exit::Right));
        let mut m = sitting(400.0);
        m.travel(&ctx(), Exit::Down);
        for _ in 0..600 {
            m.step(0.016, ctx());
            if m.exit.is_some() {
                break;
            }
        }
        assert_eq!(m.exit, Some(Exit::Down));
        // Arrivée par le haut de l'écran du dessous, puis atterrissage.
        m.arrive(&ctx(), Exit::Down, 400.0);
        for _ in 0..400 {
            m.step(0.016, ctx());
        }
        assert!(m.grounded() && m.exit.is_none());
    }
}

#[cfg(test)]
mod tests_especes {
    use super::*;
    use crate::mascot::especes::Espece;

    fn ctx() -> Ctx {
        Ctx {
            w: 1600.0,
            h: 900.0,
            night: false,
            quiet: false,
            acrobatics: false,
            show_gap_min: 5.0,
            hovered: false,
            pointer: None,
            chatting: false,
            thinking: false,
            bubble_hover: false,
            day: true,
            band: 0.0,
            exits: Exits::default(),
            avoid: None,
        }
    }

    fn sitting(e: Espece) -> Maki {
        let mut m = Maki::new(800.0, 1.0, 13);
        m.sp = e;
        m.mode = Mode::Sit;
        m.pos = (800.0, 900.0);
        m
    }

    #[test]
    fn sifaka_hops_upright_and_dances() {
        let mut m = sitting(Espece::Sifaka);
        m.set(Mode::Walk { to: 1000.0 });
        assert!(matches!(m.mode, Mode::Bound { .. }));
        let mut upright = false;
        for _ in 0..400 {
            m.step(0.016, ctx());
            upright |= m.pose().kind == PoseKind::Upright;
        }
        assert!(upright);
        let mut m = sitting(Espece::Sifaka);
        m.numero(&ctx());
        for _ in 0..1500 {
            m.step(0.016, ctx());
        }
        assert!(m.grounded() && m.dance == 0, "{:?}", m.mode);
    }

    #[test]
    fn nocturnal_species_stay_awake_at_night_and_are_sleepy_by_day() {
        let mut m = sitting(Espece::AyeAye);
        for _ in 0..100 {
            m.step(0.05, Ctx { night: true, day: false, ..ctx() });
        }
        assert_ne!(m.mode, Mode::Sleep);
        m.step(0.05, ctx());
        assert!(m.pose().sleepy);
        let mut c = sitting(Espece::Catta);
        c.step(0.05, Ctx { night: true, ..ctx() });
        assert_eq!(c.mode, Mode::Sleep);
    }

    #[test]
    fn every_numero_ends() {
        for e in Espece::TOUTES {
            let mut m = sitting(e);
            m.numero(&ctx());
            for _ in 0..800 {
                m.step(0.016, ctx());
            }
            assert!(m.grounded(), "{e:?} : {:?}", m.mode);
        }
    }
}

#[cfg(test)]
mod tests_sobriete {
    use super::*;
    use crate::conseils::{Kind, Message};

    fn ctx() -> Ctx {
        Ctx {
            w: 1600.0,
            h: 900.0,
            night: false,
            quiet: false,
            acrobatics: true,
            show_gap_min: 15.0,
            hovered: false,
            pointer: None,
            chatting: false,
            thinking: false,
            bubble_hover: false,
            day: true,
            band: 0.17,
            exits: Exits::default(),
            avoid: None,
        }
    }

    #[test]
    fn stops_to_speak() {
        let mut m = Maki::new(100.0, 1.0, 5);
        m.mode = Mode::Sit;
        m.pos = (100.0, 900.0);
        m.set(Mode::Walk { to: 250.0 });
        for _ in 0..20 {
            m.step(0.016, ctx());
        }
        let x = m.pos.0;
        m.show(Message::new(Kind::Reminder, "Rappel", "Appeler Hery"), 8.0);
        for _ in 0..300 {
            m.step(0.016, ctx());
            assert!(m.grounded(), "il ne doit pas bondir en parlant : {:?}", m.mode);
        }
        assert!((m.pos.0 - x).abs() < 1.0, "il a continué de marcher en parlant");
    }

    #[test]
    fn calm_by_default() {
        // En dix minutes, pas plus de quelques déplacements.
        let mut m = Maki::new(100.0, 1.0, 9);
        m.mode = Mode::Sit;
        m.pos = (100.0, 900.0);
        let mut moves = 0;
        let mut was = false;
        for _ in 0..(600.0 / 0.05) as usize {
            m.step(0.05, ctx());
            let moving = matches!(m.mode, Mode::Walk { .. } | Mode::Bound { .. } | Mode::Air | Mode::Climb { .. });
            if moving && !was {
                moves += 1;
            }
            was = moving;
        }
        assert!(moves <= 10, "{moves} déplacements en 10 min");
    }
}

#[cfg(test)]
mod tests_vanish {
    use super::*;

    #[test]
    fn vanish_then_reappear_on_ground_or_from_top() {
        let ctx = Ctx {
            w: 1536.0,
            h: 864.0,
            night: false,
            quiet: false,
            acrobatics: false,
            show_gap_min: 15.0,
            hovered: false,
            pointer: None,
            chatting: false,
            thinking: false,
            bubble_hover: false,
            day: true,
            band: 0.17,
            exits: Exits::default(),
            avoid: None,
        };
        let mut m = Maki::new(100.0, 1.0, 3);
        m.mode = Mode::Sit;
        m.vanish_to(1400.0, false);
        for _ in 0..120 {
            m.step(0.016, ctx);
        }
        assert!(m.visible() && (m.pos.0 - 1400.0).abs() < 1.0 && m.pos.1 == 864.0);
        m.vanish_to(120.0, true);
        for _ in 0..400 {
            m.step(0.016, ctx);
        }
        assert!(m.grounded(), "{:?}", m.mode);
    }
}

#[cfg(test)]
mod tests_liane {
    use super::*;

    #[test]
    fn edge_climb_uses_a_vine_that_rolls_back() {
        let ctx = Ctx {
            w: 1600.0,
            h: 900.0,
            night: false,
            quiet: false,
            acrobatics: false,
            show_gap_min: 15.0,
            hovered: false,
            pointer: None,
            chatting: false,
            thinking: false,
            bubble_hover: false,
            day: true,
            band: 0.0,
            exits: Exits::default(),
            avoid: None,
        };
        let mut m = Maki::new(200.0, 1.0, 2);
        m.mode = Mode::Sit;
        m.pos = (200.0, 900.0);
        m.climb_edge(&ctx);
        let (mut had_vine, mut climbed_while_unrolled) = (false, false);
        for _ in 0..(25.0 / 0.016) as usize {
            m.step(0.016, ctx);
            if let Some(v) = m.vine {
                had_vine = true;
                if matches!(m.mode, Mode::Climb { .. }) && m.pos.1 < 880.0 {
                    climbed_while_unrolled |= v.grow >= 1.0;
                    assert!(v.grow >= 1.0, "il grimpe avant que la liane soit déroulée");
                }
            }
        }
        assert!(had_vine && climbed_while_unrolled);
        assert!(m.vine.is_none() && m.grounded(), "{:?}", m.mode);
        assert_eq!(m.pose().kind, PoseKind::Sit);
    }
}
