//! Dessin vectoriel du maki catta et des bulles (tiny-skia + ab_glyph).
//! Repère du personnage : origine entre les pattes au sol, y vers le bas, regard à droite.

use ab_glyph::{Font, FontVec, PxScale, ScaleFont, point};
use tiny_skia::{
    Color, FillRule, LineCap, Paint, PathBuilder, Pixmap, PremultipliedColorU8, Rect, Stroke, Transform,
};

use super::behavior::Pose;

use super::especes::{Espece, Oreilles, Pal, Queue};

thread_local! {
    static PAL: std::cell::Cell<Pal> = std::cell::Cell::new(Espece::Catta.pal());
    static SLEEPY: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Choisit l'espèce dessinée par les appels suivants.
pub fn set_species(e: Espece) {
    PAL.with(|p| p.set(e.pal()));
}

fn pal() -> Pal {
    PAL.with(|p| p.get())
}

fn paint(c: (u8, u8, u8)) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color_rgba8(c.0, c.1, c.2, 255);
    p.anti_alias = true;
    p
}

fn oval(pm: &mut Pixmap, t: Transform, cx: f32, cy: f32, rx: f32, ry: f32, c: (u8, u8, u8)) {
    if let Some(r) = Rect::from_xywh(cx - rx, cy - ry, rx * 2.0, ry * 2.0) {
        if let Some(path) = PathBuilder::from_oval(r) {
            pm.fill_path(&path, &paint(c), FillRule::Winding, t, None);
        }
    }
}

fn line(pm: &mut Pixmap, t: Transform, a: (f32, f32), b: (f32, f32), w: f32, c: (u8, u8, u8)) {
    let mut pb = PathBuilder::new();
    pb.move_to(a.0, a.1);
    pb.line_to(b.0, b.1);
    if let Some(path) = pb.finish() {
        let stroke = Stroke { width: w, line_cap: LineCap::Round, ..Default::default() };
        pm.stroke_path(&path, &paint(c), &stroke, t, None);
    }
}

fn poly(pm: &mut Pixmap, t: Transform, pts: &[(f32, f32)], c: (u8, u8, u8)) {
    let mut pb = PathBuilder::new();
    pb.move_to(pts[0].0, pts[0].1);
    for p in &pts[1..] {
        pb.line_to(p.0, p.1);
    }
    pb.close();
    if let Some(path) = pb.finish() {
        pm.fill_path(&path, &paint(c), FillRule::Winding, t, None);
    }
}

fn bezier(p: [(f32, f32); 4], u: f32) -> (f32, f32) {
    let v = 1.0 - u;
    let a = v * v * v;
    let b = 3.0 * v * v * u;
    let c = 3.0 * v * u * u;
    let d = u * u * u;
    (a * p[0].0 + b * p[1].0 + c * p[2].0 + d * p[3].0, a * p[0].1 + b * p[1].1 + c * p[2].1 + d * p[3].1)
}

/// Queue de l'espèce (annelée ou unie), allongée ou raccourcie selon elle.
fn tail(pm: &mut Pixmap, t: Transform, ctrl: [(f32, f32); 4]) {
    let c = pal();
    let k = c.tail_len;
    let o = ctrl[0];
    let ctrl = ctrl.map(|p| (o.0 + (p.0 - o.0) * k, o.1 + (p.1 - o.1) * k));
    let n = if k < 0.3 { 4 } else { 26 };
    let mut prev = bezier(ctrl, 0.0);
    for i in 1..=n {
        let u = i as f32 / n as f32;
        let cur = bezier(ctrl, u);
        let col = match c.queue {
            Queue::Annelee(a, b) => {
                if i > n - 2 || ((i - 1) / 2) % 2 == 1 { b } else { a }
            }
            Queue::Unie(a, tip) => {
                if u > 0.85 { tip } else { a }
            }
        };
        // Queue touffue : plus large au milieu (aye-aye, vari).
        let w = if c.tail_w > 11.0 { c.tail_w * (0.7 + 0.5 * (u * std::f32::consts::PI).sin()) } else { c.tail_w - 3.5 * u };
        line(pm, t, prev, cur, w, col);
        prev = cur;
    }
}

fn leg(pm: &mut Pixmap, t: Transform, hip: (f32, f32), phase: f32, swing: f32, c: (u8, u8, u8)) {
    let pl = pal();
    let fx = hip.0 + phase.sin() * swing;
    let lift = (phase.cos()).max(0.0) * swing * 0.5;
    let knee = ((hip.0 + fx) / 2.0 + 2.0, hip.1 / 2.0 - 2.0);
    line(pm, t, hip, knee, 7.0, c);
    line(pm, t, knee, (fx, -lift - 2.0), 6.0, c);
    oval(pm, t, fx + 2.0, -lift - 1.5, 4.5, 2.6, pl.black);
}

fn head(pm: &mut Pixmap, t: Transform, hc: (f32, f32), blink: bool, expr: super::behavior::Expr) {
    use super::behavior::Expr;
    let c = pal();
    let (x, y) = hc;
    let m = c.muzzle;
    // Collerette (vari, indri) derrière la tête.
    if c.ruff {
        let ruff = if c.fur_dark.0 < 60 && c.fur.0 > 200 { c.cream } else { c.fur_dark };
        for k in 0..8 {
            let a = k as f32 * std::f32::consts::FRAC_PI_4;
            oval(pm, t, x - 2.0 + a.cos() * 11.0, y + 2.0 + a.sin() * 11.0, 8.0, 8.0, ruff);
        }
    }
    // Oreilles : dressées, ou couchées en arrière quand il s'inquiète.
    let (ex, ey) = if expr == Expr::Worried { (-6.0, 7.0) } else { (0.0, 0.0) };
    match c.ears {
        Oreilles::Pointues => {
            poly(pm, t, &[(x - 11.0, y - 6.0), (x - 8.0 + ex, y - 23.0 + ey), (x + 1.0, y - 11.0)], c.ear_color);
            poly(pm, t, &[(x - 8.5, y - 9.0), (x - 7.5 + ex * 0.8, y - 18.5 + ey * 0.8), (x - 2.0, y - 11.0)], c.cream);
        }
        Oreilles::Rondes => {
            oval(pm, t, x - 8.0 + ex * 0.5, y - 11.0 + ey * 0.5, 6.5, 6.0, c.ear_color);
        }
        Oreilles::Grandes => {
            let et = t.pre_concat(rot(-20.0 + ex * 2.0, x - 6.0, y - 10.0));
            oval(pm, et, x - 7.0, y - 22.0, 9.0, 14.0, c.ear_color);
            oval(pm, et, x - 6.0, y - 21.0, 5.5, 10.0, (c.ear_color.0.saturating_add(40), c.ear_color.1.saturating_add(30), c.ear_color.2.saturating_add(30)));
        }
    }
    oval(pm, t, x, y, 14.0, 13.5, c.fur);
    if let Some(cap) = c.cap {
        oval(pm, t, x - 3.0, y - 7.0, 11.0, 7.0, cap);
    }
    // Visage (clair ou sombre) et museau.
    let face = if c.face_dark { c.black } else { c.cream };
    oval(pm, t, x + 4.0, y + 3.0, 11.5, 10.0, face);
    poly(pm, t, &[(x + 6.0, y - 1.0), (x + 6.0 + 16.0 * m, y + 5.0), (x + 4.0 + 16.0 * m, y + 8.5), (x + 6.0, y + 10.0)], c.black);
    oval(pm, t, x + 5.5 + 16.0 * m, y + 5.5, 2.4, 2.2, (12, 12, 12));
    if expr == Expr::Happy {
        let mut pb = PathBuilder::new();
        pb.move_to(x + 11.0, y + 9.0);
        pb.quad_to(x + 14.0, y + 11.5, x + 17.0, y + 9.5);
        if let Some(path) = pb.finish() {
            let col = if c.face_dark { (120, 110, 100) } else { c.cream };
            pm.stroke_path(&path, &paint(col), &Stroke { width: 1.2, line_cap: LineCap::Round, ..Default::default() }, t, None);
        }
    }
    // Masque sombre autour de l'œil (inutile sur un visage déjà sombre).
    let er = 3.6 * c.eye_r;
    if !c.face_dark {
        oval(pm, t, x + 5.5, y - 1.0, er + 2.4, er + 1.8, c.black);
    }
    let lid = if c.face_dark { (70, 64, 60) } else { c.cream };
    let stroke = |pm: &mut Pixmap, pts: &[(f32, f32)], col: (u8, u8, u8), w: f32| {
        let mut pb = PathBuilder::new();
        pb.move_to(pts[0].0, pts[0].1);
        pb.quad_to(pts[1].0, pts[1].1, pts[2].0, pts[2].1);
        if let Some(path) = pb.finish() {
            pm.stroke_path(&path, &paint(col), &Stroke { width: w, line_cap: LineCap::Round, ..Default::default() }, t, None);
        }
    };
    if blink {
        line(pm, t, (x + 6.0 - er, y - 0.5), (x + 6.0 + er, y - 0.5), 1.6, lid);
    } else if expr == Expr::Happy {
        stroke(pm, &[(x + 6.0 - er * 0.95, y + 0.6), (x + 5.8, y - 1.0 - er * 1.1), (x + 6.0 + er * 0.95, y + 0.6)], c.eye, 1.9);
    } else {
        let r = if expr == Expr::Surprised { er * 1.2 } else { er };
        let pr = if expr == Expr::Surprised { r * 0.3 } else { r * 0.47 };
        oval(pm, t, x + 6.0, y - 1.0, r, r, c.eye);
        oval(pm, t, x + 6.6, y - 1.0, pr, pr + 0.2, (10, 10, 10));
        oval(pm, t, x + 6.0 + r * 0.38, y - 1.0 - r * 0.33, 0.8 * c.eye_r, 0.8 * c.eye_r, (255, 255, 255));
        // Animal nocturne en plein jour : paupière lourde.
        if SLEEPY.with(|s| s.get()) && expr != Expr::Surprised {
            let lt = Rect::from_xywh(x + 6.0 - r - 0.5, y - 1.0 - r - 0.5, r * 2.0 + 1.0, r + 0.5);
            if let Some(rc) = lt {
                pm.fill_path(&PathBuilder::from_rect(rc), &paint(c.black), FillRule::Winding, t, None);
            }
            line(pm, t, (x + 6.0 - r, y - 1.0), (x + 6.0 + r, y - 1.0), 1.2, lid);
        }
    }
    if expr == Expr::Worried {
        line(pm, t, (x + 1.0, y - 8.6), (x + 9.5, y - 6.2), 1.8, if c.face_dark { (200, 196, 190) } else { (40, 36, 36) });
    }
}

fn rot(deg: f32, x: f32, y: f32) -> Transform {
    Transform::from_rotate_at(deg, x, y)
}

fn scale_at(sx: f32, sy: f32, x: f32, y: f32) -> Transform {
    Transform::from_translate(x, y).pre_scale(sx, sy).pre_translate(-x, -y)
}

fn arm(pm: &mut Pixmap, t: Transform, a: (f32, f32), b: (f32, f32), c: (u8, u8, u8)) {
    let pl = pal();
    line(pm, t, a, b, 6.2, c);
    oval(pm, t, b.0, b.1, 4.2, 4.0, pl.black);
}

/// Dessine le maki. `base` place l'origine (entre les pattes) avec l'échelle et la direction.
pub fn maki(pm: &mut Pixmap, base: Transform, p: &Pose) {
    let c = pal();
    use super::behavior::PoseKind;
    if p.pop <= 0.01 {
        return;
    }
    let sq = p.squash;
    let mut t = base
        .pre_concat(scale_at(1.0 + sq * 0.6, 1.0 - sq, 0.0, 0.0))
        .pre_concat(rot(p.lean.to_degrees(), 0.0, 0.0))
        .pre_concat(rot(p.spin.to_degrees(), 0.0, -40.0))
        .pre_concat(scale_at(p.pop, p.pop, 0.0, -40.0));
    let (tx, ty) = p.tail;
    SLEEPY.with(|x| x.set(p.sleepy));
    match p.kind {
        PoseKind::Sleep => {
            tail(pm, t, [(-26.0, -14.0), (-56.0, 6.0), (24.0, 10.0), (40.0, -2.0)]);
            oval(pm, t, 0.0, -16.0, 30.0, 16.0, c.fur);
            oval(pm, t, 4.0, -9.0, 20.0, 7.0, c.cream);
            head(pm, t.pre_concat(rot(18.0, 24.0, -18.0)), (24.0, -18.0), true, super::behavior::Expr::Normal);
        }
        PoseKind::Sit => {
            if c.tail_up {
                tail(pm, t, [(-16.0, -10.0), (-52.0, -2.0), (-70.0 + tx * 0.5, -34.0 + ty * 0.5), (-56.0 + tx, -78.0 + ty)]);
            } else {
                tail(pm, t, [(-16.0, -10.0), (-40.0, -2.0), (-62.0 + tx * 0.5, 0.0), (-84.0 + tx, -6.0 + ty * 0.3)]);
            }
            oval(pm, t, -8.0, -15.0, 15.0, 12.0, c.fur_dark);
            oval(pm, t.pre_concat(rot(-58.0, 0.0, -38.0)), 0.0, -38.0, 25.0, 15.0, c.fur);
            oval(pm, t, 7.0, -36.0, 7.5, 15.0, c.cream);
            oval(pm, t, 2.0, -2.0, 9.0, 3.2, c.black);
            line(pm, t, (10.0, -40.0), (13.0, -3.0), 6.5, c.fur_dark);
            oval(pm, t, 15.0, -2.0, 4.5, 2.6, c.black);
            let busy_arm = p.wave != 0.0 || p.scratch != 0.0 || p.point || p.tap != 0.0;
            if !busy_arm {
                line(pm, t, (14.0, -42.0), (19.0, -3.0), 6.5, c.fur);
                oval(pm, t, 21.0, -2.0, 4.5, 2.6, c.black);
            }
            head(pm, t.pre_concat(rot(-p.look_up * 25.0, 12.0, -70.0)), (12.0, -70.0), p.blink, p.expr);
            if p.wave != 0.0 {
                let ang = (-62.0 + p.wave * 24.0f32).to_radians();
                let sh = (14.0, -44.0);
                let elbow = (sh.0 + 14.0, sh.1 - 4.0);
                line(pm, t, sh, elbow, 6.5, c.fur);
                arm(pm, t, elbow, (elbow.0 + ang.cos() * 18.0, elbow.1 + ang.sin() * 18.0), c.fur);
            } else if p.point {
                // Patte levée bien haut, avec une petite étincelle d'idée.
                let sh = (14.0, -44.0);
                let elbow = (24.0, -58.0);
                line(pm, t, sh, elbow, 6.5, c.fur);
                arm(pm, t, elbow, (27.0, -80.0), c.fur);
                let k = (p.time * 6.0).sin() * 0.3 + 1.0;
                for a in 0..4 {
                    let ang = a as f32 * std::f32::consts::FRAC_PI_2 + p.time;
                    line(pm, t, (33.0 + ang.cos() * 3.0, -92.0 + ang.sin() * 3.0), (33.0 + ang.cos() * 7.0 * k, -92.0 + ang.sin() * 7.0 * k), 1.6, (250, 200, 60));
                }
            } else if p.tap != 0.0 {
                // Aye-aye : le long doigt fin tapote le sol pour écouter.
                let sh = (14.0, -44.0);
                let hand = (30.0, -14.0);
                line(pm, t, sh, hand, 6.0, c.fur);
                let tip = (44.0, -1.0 - p.tap.max(0.0) * 5.0);
                line(pm, t, hand, tip, 1.6, c.black);
                if p.tap > 0.6 {
                    for k in 0..3 {
                        let r = 4.0 + k as f32 * 3.0;
                        line(pm, t, (tip.0 - r, tip.1 - r * 0.6), (tip.0 - r * 0.6, tip.1 - r), 1.2, (200, 190, 170));
                    }
                }
            } else if p.scratch != 0.0 {
                // Se gratte la tête.
                let sh = (14.0, -44.0);
                let elbow = (24.0, -64.0);
                line(pm, t, sh, elbow, 6.5, c.fur);
                arm(pm, t, elbow, (10.0 + p.scratch * 3.0, -84.0 + p.scratch.abs() * 2.0), c.fur);
            }
        }
        PoseKind::Dangle { arms_up } => {
            // Suspendu par les mains (ou la peau du cou) : corps vertical qui se balance.
            let g = -super::behavior::DANGLE;
            t = t.pre_concat(rot(p.swing.to_degrees(), 0.0, g));
            let kick = (p.time * 5.0).sin() * 4.0;
            tail(pm, t, [(-2.0, -24.0), (-14.0 + tx * 0.4, 6.0), (16.0 + tx, 26.0 + ty * 0.3), (2.0 + tx * 1.4, 52.0)]);
            line(pm, t, (-7.0, -26.0), (-10.0 - kick, -3.0), 6.5, c.fur_dark);
            oval(pm, t, -10.0 - kick, -2.0, 4.0, 3.0, c.black);
            oval(pm, t, 0.0, -44.0, 16.0, 25.0, c.fur);
            oval(pm, t, 4.0, -42.0, 8.0, 17.0, c.cream);
            line(pm, t, (7.0, -26.0), (10.0 + kick, -3.0), 6.5, c.fur);
            oval(pm, t, 10.0 + kick, -2.0, 4.0, 3.0, c.black);
            head(pm, t, (3.0, -74.0), p.blink, p.expr);
            if arms_up {
                arm(pm, t, (-9.0, -60.0), (-7.0, g + 2.0), c.fur_dark);
                arm(pm, t, (11.0, -60.0), (9.0, g + 2.0), c.fur);
            } else {
                arm(pm, t, (-10.0, -58.0), (-15.0 + kick, -36.0), c.fur_dark);
                arm(pm, t, (12.0, -58.0), (17.0 - kick, -36.0), c.fur);
            }
        }
        PoseKind::Grimpe => {
            // Agrippé à une liane (ou au tronc) juste devant lui : corps vertical, une main
            // après l'autre, pieds calés, tête levée vers le haut.
            let a = p.walk_phase.sin();
            tail(pm, t, [(-6.0, -30.0), (-16.0 + tx * 0.3, -6.0), (-22.0 + tx * 0.6, 14.0), (-12.0 + tx, 34.0)]);
            line(pm, t, (-2.0, -26.0), (-6.0, -16.0 + 3.0 * a), 7.0, c.fur_dark);
            line(pm, t, (-6.0, -16.0 + 3.0 * a), (10.0, -10.0 + 4.0 * a), 6.0, c.fur_dark);
            oval(pm, t, 11.0, -10.0 + 4.0 * a, 3.6, 4.4, c.black);
            arm(pm, t, (4.0, -64.0), (12.0, -94.0 - 8.0 * a), c.fur_dark);
            let bt = t.pre_concat(rot(-8.0, 0.0, -48.0));
            oval(pm, bt, 0.0, -48.0, 15.0, 26.0, c.fur);
            oval(pm, bt, 6.0, -46.0, 6.5, 17.0, c.cream);
            line(pm, t, (4.0, -28.0), (-2.0, -20.0 - 3.0 * a), 7.0, c.fur);
            line(pm, t, (-2.0, -20.0 - 3.0 * a), (10.0, -22.0 - 4.0 * a), 6.0, c.fur);
            oval(pm, t, 11.0, -22.0 - 4.0 * a, 3.6, 4.4, c.black);
            head(pm, t.pre_concat(rot(-22.0, 4.0, -82.0)), (4.0, -82.0), p.blink, p.expr);
            arm(pm, t, (7.0, -62.0), (13.0, -78.0 + 8.0 * a), c.fur);
        }
        PoseKind::Upright => {
            // Debout (propithèque, indri) : bras levés en équilibre, jambes fléchies.
            let k = p.walk_phase.sin() * 0.5 + 0.5;
            let bend = 6.0 + k * 6.0;
            tail(pm, t, [(-4.0, -22.0), (-16.0 + tx * 0.3, -8.0), (-32.0 + tx * 0.6, 2.0), (-56.0 + tx, -2.0 + ty * 0.3)]);
            line(pm, t, (-6.0, -26.0), (-2.0 + bend, -14.0), 7.0, c.fur_dark);
            line(pm, t, (-2.0 + bend, -14.0), (-8.0, -2.0), 6.5, c.fur_dark);
            oval(pm, t, -6.0, -1.5, 5.0, 2.6, c.black);
            oval(pm, t, 0.0, -46.0, 17.0, 25.0, c.fur);
            oval(pm, t, 5.0, -44.0, 8.0, 17.0, c.cream);
            line(pm, t, (6.0, -26.0), (10.0 + bend, -14.0), 7.0, c.fur);
            line(pm, t, (10.0 + bend, -14.0), (6.0, -2.0), 6.5, c.fur);
            oval(pm, t, 8.0, -1.5, 5.0, 2.6, c.black);
            let up = 4.0 * (p.time * 3.0).sin();
            arm(pm, t, (-8.0, -60.0), (-24.0, -84.0 + up), c.fur_dark);
            head(pm, t.pre_concat(rot(-p.look_up * 25.0, 3.0, -76.0)), (3.0, -76.0), p.blink, p.expr);
            arm(pm, t, (10.0, -60.0), (28.0, -82.0 - up), c.fur);
        }
        PoseKind::Stretch => {
            // Étirement : avant bas, arrière haut, pattes avant tendues.
            let k = (p.walk_phase * std::f32::consts::PI).sin();
            tail(pm, t, [(-24.0, -44.0 - k * 6.0), (-50.0, -60.0), (-56.0 + tx, -104.0), (-40.0 + tx, -122.0)]);
            line(pm, t, (-15.0, -36.0 - k * 4.0), (-17.0, -2.0), 7.0, c.fur_dark);
            let bt = t.pre_concat(rot(k * 16.0, 0.0, -38.0));
            oval(pm, bt, 0.0, -40.0, 28.0 + k * 4.0, 14.0, c.fur);
            oval(pm, bt, 2.0, -31.0, 19.0, 5.0, c.cream);
            line(pm, t, (18.0, -36.0 + k * 10.0), (24.0 + k * 22.0, -2.0), 6.5, c.fur);
            oval(pm, t, 26.0 + k * 22.0, -2.0, 4.5, 2.6, c.black);
            line(pm, t, (-12.0, -34.0 - k * 4.0), (-12.0, -2.0), 7.0, c.fur);
            head(pm, t.pre_concat(rot(k * 14.0, 31.0, -50.0)), (33.0, -52.0 + k * 16.0), k > 0.6, p.expr);
        }
        PoseKind::Walk | PoseKind::Fly | PoseKind::Climb => {
            if p.kind == PoseKind::Climb {
                t = t.pre_concat(rot(-90.0, 0.0, 0.0));
            }
            let ph = p.walk_phase;
            let pi = std::f32::consts::PI;
            let (sw, f1, f2) = match p.kind {
                PoseKind::Fly => (11.0, pi / 2.0, -pi / 2.0),
                PoseKind::Climb => (10.0, ph, ph + pi),
                _ => (7.0, ph, ph + pi),
            };
            let moving = p.kind != PoseKind::Walk || sw > 0.0;
            if c.tail_up {
                tail(pm, t, [(-24.0, -42.0), (-52.0 + tx * 0.3, -48.0 + ty * 0.4), (-64.0 + tx * 0.7, -92.0 + ty * 0.7), (-46.0 + tx, -114.0 + ty)]);
            } else {
                tail(pm, t, [(-24.0, -42.0), (-48.0 + tx * 0.3, -46.0 + ty * 0.4), (-72.0 + tx * 0.7, -44.0 + ty * 0.7), (-96.0 + tx, -34.0 + ty)]);
            }
            leg(pm, t, (-15.0, -34.0), f2, sw, c.fur_dark);
            leg(pm, t, (17.0, -34.0), f1, sw, c.fur_dark);
            let bob = if moving { (ph * 2.0).sin().abs() * 1.5 } else { 0.0 };
            oval(pm, t, 0.0, -40.0 - bob, 27.0, 15.0, c.fur);
            oval(pm, t, 2.0, -31.0 - bob, 19.0, 6.0, c.cream);
            leg(pm, t, (-12.0, -32.0), f1, sw, c.fur);
            leg(pm, t, (20.0, -32.0), f2, sw, c.fur);
            head(pm, t, (31.0, -55.0 - bob), p.blink, p.expr);
        }
    }
    // Chant (indri) : des notes s'envolent ; cri (vari) : traits qui rayonnent du museau.
    if p.sing {
        for n in 0..3 {
            let ph = (p.time * 0.8 + n as f32 * 0.33) % 1.0;
            let (nx, ny) = (24.0 + n as f32 * 9.0 + (ph * 6.0).sin() * 4.0, -92.0 - ph * 40.0);
            let a = (1.0 - ph).clamp(0.0, 1.0);
            if a > 0.05 {
                let col = (250, 214, 90);
                oval(pm, t, nx, ny, 3.4, 2.6, col);
                line(pm, t, (nx + 3.0, ny), (nx + 3.0, ny - 11.0), 1.4, col);
                line(pm, t, (nx + 3.0, ny - 11.0), (nx + 8.0, ny - 8.0), 1.4, col);
            }
        }
    }
    if p.call {
        let k = (p.time * 10.0).sin() * 2.0;
        for (dx, dy) in [(1.0, -0.5), (1.0, 0.0), (1.0, 0.5)] {
            let (bx, by) = (36.0, -66.0);
            line(pm, t, (bx + dx * 6.0, by + dy * 6.0), (bx + dx * (16.0 + k), by + dy * (16.0 + k)), 2.0, (240, 120, 60));
        }
    }
}

fn paint_a(c: (u8, u8, u8), a: f32) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color(Color::from_rgba8(c.0, c.1, c.2, (a.clamp(0.0, 1.0) * 255.0) as u8));
    p.anti_alias = true;
    p
}

fn disc(pm: &mut Pixmap, t: Transform, x: f32, y: f32, r: f32, paint: &Paint) {
    if let Some(path) = PathBuilder::from_circle(x, y, r.max(0.1)) {
        pm.fill_path(&path, paint, FillRule::Winding, t, None);
    }
}

/// L'arbre que le maki fait pousser pour y grimper (origine : pied du tronc, au sol).
pub fn tree(pm: &mut Pixmap, base: Transform, grow: f32, side: f32, time: f32) {
    if grow <= 0.01 {
        return;
    }
    let t = base.pre_concat(scale_at(grow, grow, 0.0, 0.0));
    let sway = t.pre_concat(rot((time * 1.1).sin() * 1.4, 0.0, -230.0));
    const BARK: (u8, u8, u8) = (112, 80, 52);
    const BARK_D: (u8, u8, u8) = (82, 57, 37);
    let leaves = [(58u8, 116u8, 60u8), (78, 142, 70), (101, 166, 86)];
    // Feuillage du fond.
    for (x, y, r, c) in [(-side * 70.0, -292.0, 44.0, 0), (side * 20.0, -330.0, 62.0, 0), (side * 104.0, -290.0, 34.0, 0)] {
        disc(pm, sway, x, y, r, &paint(leaves[c]));
    }
    // Tronc évasé.
    poly(pm, t, &[(-19.0, 0.0), (-11.0, -90.0), (-9.0, -250.0), (-4.0, -300.0), (5.0, -300.0), (10.0, -250.0), (12.0, -90.0), (21.0, 0.0)], BARK);
    poly(pm, t, &[(4.0, 0.0), (6.0, -120.0), (6.0, -250.0), (10.0, -250.0), (12.0, -90.0), (21.0, 0.0)], BARK_D);
    for (y, w) in [(-60.0, 7.0), (-140.0, 6.0), (-205.0, 5.0)] {
        line(pm, t, (-4.0, y), (-4.0 + w * 0.6, y - w), 1.6, BARK_D);
    }
    // La branche où s'assoit le maki, et une autre en face.
    line(pm, t, (side * 6.0, -230.0), (side * 104.0, -241.0), 9.0, BARK);
    line(pm, t, (side * 6.0, -226.0), (side * 104.0, -238.0), 3.0, BARK_D);
    line(pm, t, (-side * 6.0, -262.0), (-side * 70.0, -296.0), 7.0, BARK);
    // Feuillage du devant (au-dessus de la branche, laisse le maki visible).
    for (x, y, r, c) in [
        (-side * 40.0, -318.0, 40.0, 1),
        (side * 40.0, -356.0, 44.0, 1),
        (side * 112.0, -268.0, 18.0, 2),
        (-side * 82.0, -300.0, 24.0, 2),
        (0.0, -376.0, 30.0, 2),
    ] {
        disc(pm, sway, x, y, r, &paint(leaves[c]));
    }
}

/// Nuages de téléportation et feuilles.
pub fn puffs(pm: &mut Pixmap, origin: (f32, f32), dpr: f32, list: &[super::behavior::Puff]) {
    for p in list {
        let k = (p.life / p.max).clamp(0.0, 1.0);
        let t = Transform::from_scale(dpr, dpr).pre_translate(-origin.0, -origin.1);
        if p.leaf {
            let pt = t.pre_concat(rot(p.life * 300.0, p.x, p.y));
            let r = Rect::from_xywh(p.x - p.r * 0.5, p.y - p.r * 0.25, p.r, p.r * 0.5).unwrap();
            if let Some(path) = PathBuilder::from_oval(r) {
                pm.fill_path(&path, &paint_a((86, 150, 72), 1.0 - k), FillRule::Winding, pt, None);
            }
        } else {
            disc(pm, t, p.x, p.y, p.r * (0.6 + k * 0.8), &paint_a((242, 240, 236), (1.0 - k) * 0.9));
        }
    }
}

// --- texte -------------------------------------------------------------------

pub struct Text {
    font: FontVec,
}

fn blend(px: &mut PremultipliedColorU8, c: (u8, u8, u8), a: f32) {
    let a = a.clamp(0.0, 1.0);
    let inv = 1.0 - a;
    let mix = |s: u8, d: u8| ((s as f32 * a) + d as f32 * inv).round().min(255.0) as u8;
    let na = mix(255, px.alpha());
    let nr = mix(c.0, px.red()).min(na);
    let ng = mix(c.1, px.green()).min(na);
    let nb = mix(c.2, px.blue()).min(na);
    if let Some(v) = PremultipliedColorU8::from_rgba(nr, ng, nb, na) {
        *px = v;
    }
}

/// Police grasse (titres des bulles).
pub fn load_bold() -> Option<Text> {
    let mut c = vec![];
    if let Ok(o) = std::process::Command::new("fc-match").args(["-f", "%{file}", "sans:style=Bold"]).output() {
        c.push(String::from_utf8_lossy(&o.stdout).to_string());
    }
    c.extend(["/usr/share/fonts/noto/NotoSans-Bold.ttf", "/usr/share/fonts/TTF/DejaVuSans-Bold.ttf"].map(String::from));
    c.into_iter().filter_map(|p| std::fs::read(p.trim()).ok()).find_map(|b| FontVec::try_from_vec(b).ok()).map(|font| Text { font })
}

impl Text {

    /// Charge la police sans empattement du système (fontconfig), sinon une police connue.
    pub fn load() -> Option<Text> {
        let mut candidates = vec![];
        if let Ok(o) = std::process::Command::new("fc-match").args(["-f", "%{file}", "sans:style=Regular"]).output() {
            candidates.push(String::from_utf8_lossy(&o.stdout).to_string());
        }
        candidates.extend(
            [
                "/usr/share/fonts/noto/NotoSans-Regular.ttf",
                "/usr/share/fonts/TTF/DejaVuSans.ttf",
                "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
                "/usr/share/fonts/liberation/LiberationSans-Regular.ttf",
            ]
            .map(String::from),
        );
        candidates
            .into_iter()
            .filter_map(|p| std::fs::read(p.trim()).ok())
            .find_map(|b| FontVec::try_from_vec(b).ok())
            .map(|font| Text { font })
    }

    pub fn width(&self, size: f32, s: &str) -> f32 {
        let sf = self.font.as_scaled(PxScale::from(size));
        let mut w = 0.0;
        let mut prev = None;
        for c in s.chars() {
            let g = sf.glyph_id(c);
            if let Some(p) = prev {
                w += sf.kern(p, g);
            }
            w += sf.h_advance(g);
            prev = Some(g);
        }
        w
    }

    pub fn line_height(&self, size: f32) -> f32 {
        let sf = self.font.as_scaled(PxScale::from(size));
        sf.ascent() - sf.descent() + sf.line_gap()
    }

    /// Coupe un texte en lignes de largeur `max_w`.
    pub fn wrap(&self, size: f32, text: &str, max_w: f32) -> Vec<String> {
        let mut lines = vec![];
        for para in text.lines() {
            let mut cur = String::new();
            for word in para.split_whitespace() {
                let cand = if cur.is_empty() { word.to_string() } else { format!("{cur} {word}") };
                if self.width(size, &cand) > max_w && !cur.is_empty() {
                    lines.push(std::mem::take(&mut cur));
                    cur = word.to_string();
                } else {
                    cur = cand;
                }
            }
            lines.push(cur);
        }
        lines
    }

    pub fn draw(&self, pm: &mut Pixmap, size: f32, x: f32, y_top: f32, s: &str, c: (u8, u8, u8)) {
        let sf = self.font.as_scaled(PxScale::from(size));
        let baseline = y_top + sf.ascent();
        let (w, h) = (pm.width() as i32, pm.height() as i32);
        let mut caret = x;
        let mut prev = None;
        for ch in s.chars() {
            let g = sf.glyph_id(ch);
            if let Some(p) = prev {
                caret += sf.kern(p, g);
            }
            let glyph = g.with_scale_and_position(PxScale::from(size), point(caret, baseline));
            caret += sf.h_advance(g);
            prev = Some(g);
            if let Some(og) = self.font.outline_glyph(glyph) {
                let b = og.px_bounds();
                let pixels = pm.pixels_mut();
                og.draw(|gx, gy, cov| {
                    let px = b.min.x as i32 + gx as i32;
                    let py = b.min.y as i32 + gy as i32;
                    if px >= 0 && py >= 0 && px < w && py < h {
                        blend(&mut pixels[(py * w + px) as usize], c, cov);
                    }
                });
            }
        }
    }
}


/// Contour clair autour de ce qui est déjà dessiné dans le rectangle (effet autocollant) :
/// le maki reste lisible sur un fond sombre comme sur un fond clair.
pub fn outline(pm: &mut Pixmap, rect: (f32, f32, f32, f32), radius: i32, c: (u8, u8, u8), strength: f32) {
    let (w, h) = (pm.width() as i32, pm.height() as i32);
    let x0 = (rect.0 as i32 - radius).max(0);
    let y0 = (rect.1 as i32 - radius).max(0);
    let x1 = ((rect.0 + rect.2) as i32 + radius).min(w);
    let y1 = ((rect.1 + rect.3) as i32 + radius).min(h);
    if x1 <= x0 || y1 <= y0 {
        return;
    }
    let bw = (x1 - x0) as usize;
    let src: Vec<u8> = {
        let px = pm.pixels();
        let mut v = Vec::with_capacity(bw * (y1 - y0) as usize);
        for y in y0..y1 {
            for x in x0..x1 {
                v.push(px[(y * w + x) as usize].alpha());
            }
        }
        v
    };
    // Dilatation séparable (max horizontal puis vertical).
    let bh = (y1 - y0) as usize;
    let r = radius as usize;
    let mut tmp = vec![0u8; src.len()];
    for y in 0..bh {
        for x in 0..bw {
            let (a, b) = (x.saturating_sub(r), (x + r).min(bw - 1));
            tmp[y * bw + x] = (a..=b).map(|i| src[y * bw + i]).max().unwrap_or(0);
        }
    }
    let px = pm.pixels_mut();
    for y in 0..bh {
        for x in 0..bw {
            let (a, b) = (y.saturating_sub(r), (y + r).min(bh - 1));
            let d = (a..=b).map(|j| tmp[j * bw + x]).max().unwrap_or(0) as f32 / 255.0 * strength;
            if d <= 0.0 {
                continue;
            }
            let p = &mut px[(y0 as usize + y) * w as usize + x0 as usize + x];
            // Contour placé SOUS le dessin existant (destination-over).
            let da = p.alpha() as f32 / 255.0;
            let k = d * (1.0 - da);
            let na = (p.alpha() as f32 + k * 255.0).min(255.0) as u8;
            let mix = |dst: u8, s: u8| (dst as f32 + s as f32 * k).min(na as f32) as u8;
            if let Some(v) = PremultipliedColorU8::from_rgba(mix(p.red(), c.0), mix(p.green(), c.1), mix(p.blue(), c.2), na) {
                *p = v;
            }
        }
    }
}

/// Couleurs des bulles selon le thème (clair, sombre, ou celui de KDE).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Theme {
    pub bg: (u8, u8, u8),
    pub bg2: (u8, u8, u8),
    pub border: (u8, u8, u8),
    pub text: (u8, u8, u8),
    pub soft: (u8, u8, u8),
    pub faint: (u8, u8, u8),
    pub user: (u8, u8, u8),
    pub field: (u8, u8, u8),
    pub field_border: (u8, u8, u8),
    pub chip: (u8, u8, u8),
    pub chip_text: (u8, u8, u8),
}

pub const CLAIR: Theme = Theme {
    bg: (255, 252, 246),
    bg2: (252, 246, 236),
    border: (205, 194, 176),
    text: (44, 38, 34),
    soft: (150, 140, 126),
    faint: (180, 170, 156),
    user: (36, 84, 160),
    field: (246, 241, 230),
    field_border: (214, 204, 186),
    chip: (246, 238, 224),
    chip_text: (130, 104, 70),
};

pub const SOMBRE: Theme = Theme {
    bg: (44, 42, 48),
    bg2: (36, 34, 40),
    border: (92, 88, 100),
    text: (234, 230, 222),
    soft: (172, 166, 158),
    faint: (152, 146, 138),
    user: (132, 180, 244),
    field: (56, 54, 62),
    field_border: (86, 82, 94),
    chip: (60, 56, 52),
    chip_text: (226, 204, 168),
};

thread_local! {
    static THEME: std::cell::Cell<Theme> = const { std::cell::Cell::new(CLAIR) };
}

/// Le jeu de couleurs de KDE est-il sombre ? (fond des fenêtres dans kdeglobals)
pub fn kde_is_dark() -> bool {
    let p = crate::paths::home_dir().join(".config/kdeglobals");
    let txt = std::fs::read_to_string(p).unwrap_or_default();
    let mut in_window = false;
    for l in txt.lines() {
        if l.starts_with('[') {
            in_window = l == "[Colors:Window]";
        } else if in_window {
            if let Some(v) = l.strip_prefix("BackgroundNormal=") {
                let c: Vec<f32> = v.split(',').filter_map(|x| x.trim().parse().ok()).collect();
                if c.len() >= 3 {
                    return 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2] < 128.0;
                }
            }
        }
    }
    false
}

/// « clair », « sombre » ou « auto » (suit KDE).
pub fn set_theme(name: &str) {
    let t = match name {
        "sombre" => SOMBRE,
        "clair" => CLAIR,
        _ => {
            if kde_is_dark() { SOMBRE } else { CLAIR }
        }
    };
    THEME.with(|x| x.set(t));
}

pub fn th() -> Theme {
    THEME.with(|x| x.get())
}

pub type R = (f32, f32, f32, f32);

pub fn hit(r: R, p: (f32, f32)) -> bool {
    p.0 >= r.0 && p.0 <= r.0 + r.2 && p.1 >= r.1 && p.1 <= r.1 + r.3
}

fn rrect(pm: &mut Pixmap, r: R, rad: f32, fill: &Paint, stroke: Option<((u8, u8, u8), f32)>) {
    let (l, t, w, h) = r;
    let (rr, b) = (l + w, t + h);
    let rad = rad.min(w / 2.0).min(h / 2.0);
    let mut pb = PathBuilder::new();
    pb.move_to(l + rad, t);
    pb.line_to(rr - rad, t);
    pb.quad_to(rr, t, rr, t + rad);
    pb.line_to(rr, b - rad);
    pb.quad_to(rr, b, rr - rad, b);
    pb.line_to(l + rad, b);
    pb.quad_to(l, b, l, b - rad);
    pb.line_to(l, t + rad);
    pb.quad_to(l, t, l + rad, t);
    pb.close();
    if let Some(path) = pb.finish() {
        pm.fill_path(&path, fill, FillRule::Winding, Transform::identity(), None);
        if let Some((c, w)) = stroke {
            pm.stroke_path(&path, &paint(c), &Stroke { width: w, ..Default::default() }, Transform::identity(), None);
        }
    }
}

/// Zones cliquables de la bulle de discussion.
#[derive(Debug, Clone, Default)]
pub struct ChatLayout {
    pub rect: R,
    pub close: R,
    pub yes: Option<R>,
    pub no: Option<R>,
    pub input: R,
    /// Abscisses des positions de curseur visibles dans la saisie, à partir du caractère
    /// `input_off` (pour placer le curseur au clic).
    pub input_x: Vec<f32>,
    pub input_off: usize,
    pub copy: Vec<(R, String)>,
    /// Liens cliquables des réponses (zone, adresse).
    pub links: Vec<(R, String)>,
    /// Pastille « modèle » de l'en-tête (ouvre le sélecteur).
    pub chip: R,
    /// Zones du sélecteur de modèles.
    pub picks: Vec<(R, crate::chat::Pick)>,
    pub picker: Option<R>,
    /// Bouton des réglages (roue dentée).
    pub gear: R,
    pub picks_panel: Vec<(R, crate::chat::Act)>,
    /// Hauteur du contenu du panneau et de sa zone visible (px logiques non mis à l'échelle).
    pub panel_content: f32,
    pub panel_view: f32,
}

/// Hauteur du sélecteur de modèles (avant mise à l'échelle).
const PICKER_H: f32 = 380.0;

/// Hauteur de la zone du sélecteur ou du panneau : le panneau s'ajuste à son contenu.
fn zone_h(chat: &crate::chat::Chat) -> f32 {
    match &chat.panneau {
        Some(p) => {
            let c = p.contenu.get();
            if c <= 0.0 { 360.0 } else { c.clamp(180.0, 470.0) }
        }
        None => PICKER_H,
    }
}
const ROW_H: f32 = 31.0;

/// Petit triangle (la police n'a pas ▲ ▼).
fn tri(pm: &mut Pixmap, cx: f32, cy: f32, r: f32, up: bool, c: (u8, u8, u8)) {
    let k = if up { -1.0 } else { 1.0 };
    poly(pm, Transform::identity(), &[(cx - r, cy - k * r * 0.5), (cx + r, cy - k * r * 0.5), (cx, cy + k * r * 0.6)], c);
}

/// Coche dessinée.
fn check(pm: &mut Pixmap, x: f32, y: f32, s: f32, c: (u8, u8, u8)) {
    let mut pb = PathBuilder::new();
    pb.move_to(x, y + 5.0 * s);
    pb.line_to(x + 3.5 * s, y + 8.5 * s);
    pb.line_to(x + 10.0 * s, y + 1.0 * s);
    if let Some(path) = pb.finish() {
        pm.stroke_path(&path, &paint(c), &Stroke { width: 2.0 * s, line_cap: LineCap::Round, ..Default::default() }, Transform::identity(), None);
    }
}

fn short(s: &str, n: usize) -> String {
    if s.chars().count() <= n { s.to_string() } else { format!("{}…", s.chars().take(n - 1).collect::<String>()) }
}

/// Sélecteur de modèles, dans la zone (x, y, w, h) de la bulle.
fn draw_picker(pm: &mut Pixmap, text: &Text, chat: &crate::chat::Chat, area: R, s: f32, lay: &mut ChatLayout) {
    use crate::chat::{CATS, Pick};
    let Some(p) = &chat.picker else { return };
    let (x, y, w, h) = area;
    lay.picker = Some(area);
    let acc = (46, 126, 192);
    // Onglets des moteurs.
    let mut px = x;
    for (i, name) in chat.engines() {
        let lw = text.width(12.0 * s, &name) + 20.0 * s;
        let r = (px, y, lw, 22.0 * s);
        pill(pm, text, r, &name, acc, i == p.moteur, 12.0 * s);
        lay.picks.push((r, Pick::Engine(i)));
        px += lw + 6.0 * s;
    }
    let mut cy = y + 30.0 * s;
    let rows = chat.picker_rows();
    let has_quality = rows.iter().any(|m| !m.quality.is_empty());
    // Catégories (seulement si le moteur note ses modèles).
    if has_quality {
        let mut cx = x;
        for (i, (_, label)) in CATS.iter().enumerate() {
            let lw = text.width(11.0 * s, label) + 16.0 * s;
            if cx + lw > x + w {
                cx = x;
                cy += 24.0 * s;
            }
            let r = (cx, cy, lw, 20.0 * s);
            pill(pm, text, r, label, th().chip_text, i == p.cat, 11.0 * s);
            lay.picks.push((r, Pick::Cat(i)));
            cx += lw + 5.0 * s;
        }
        cy += 28.0 * s;
    }
    let bottom = y + h;
    let current = chat.current_model(p.moteur);
    let row_h = ROW_H * s;
    let draw_row = |pm: &mut Pixmap, lay: &mut ChatLayout, cy: f32, name: &str, meta: &str, score: Option<f32>, selected: bool, faded: bool, action: Pick| {
        let r = (x - 4.0 * s, cy, w + 8.0 * s, row_h - 2.0 * s);
        if selected {
            rrect(pm, r, 8.0 * s, &paint_a(acc, 0.12), None);
        }
        let fg = if faded { th().soft } else { th().text };
        if selected {
            check(pm, x + 1.0 * s, cy + 5.0 * s, s, acc);
        }
        text.draw(pm, 13.0 * s, x + 16.0 * s, cy + 1.0 * s, &short(name, 34), fg);
        text.draw(pm, 10.5 * s, x + 16.0 * s, cy + 16.0 * s, &short(meta, 52), th().soft);
        if let Some(q) = score {
            let bar = (x + w - 58.0 * s, cy + 10.0 * s, 54.0 * s, 5.0 * s);
            rrect(pm, bar, 2.5 * s, &paint(th().field), None);
            rrect(pm, (bar.0, bar.1, bar.2 * q.clamp(0.04, 1.0), bar.3), 2.5 * s, &paint(acc), None);
        }
        lay.picks.push((r, action));
    };
    match &p.models {
        None => {
            text.draw(pm, 12.5 * s, x, cy, "Je regarde les modèles disponibles…", th().soft);
        }
        Some(Err(e)) => {
            for (i, l) in text.wrap(12.0 * s, e, w).iter().take(4).enumerate() {
                text.draw(pm, 12.0 * s, x, cy + i as f32 * 16.0 * s, l, (176, 48, 40));
            }
            cy += 70.0 * s;
            if chat.has_auto(p.moteur) {
                draw_row(pm, lay, cy, "Automatique", "le meilleur modèle gratuit selon la demande", None, current == "auto", false, Pick::Model(None));
            }
        }
        Some(Ok(_)) => {
            let cat = CATS[p.cat.min(CATS.len() - 1)].0;
            let best = rows.iter().map(|m| m.score(cat)).fold(0.0f32, f32::max).max(1e-3);
            let mut items: Vec<(String, String, Option<f32>, bool, bool, Pick)> = vec![];
            if chat.has_auto(p.moteur) {
                items.push(("Automatique".into(), "le meilleur gratuit selon la demande, et bascule si saturé".into(), None, current == "auto", false, Pick::Model(None)));
            }
            for m in &rows {
                let mut meta = vec![];
                if !m.provider.is_empty() {
                    meta.push(m.provider.clone());
                }
                meta.push(if m.tools { "outils".into() } else { "sans outils : ne peut pas agir".into() });
                if m.vision {
                    meta.push("vision".into());
                }
                if let Some(pz) = &m.pause {
                    meta.push(pz.clone());
                }
                let score = if has_quality { Some(m.score(cat) / best) } else { None };
                items.push((m.name.clone(), meta.join(" · "), score, m.key == current, m.pause.is_some() || !m.tools, Pick::Model(Some(m.key.clone()))));
            }
            let visible = (((bottom - cy) / row_h).floor() as usize).max(1);
            let start = p.scroll.min(items.len().saturating_sub(visible));
            if start > 0 {
                tri(pm, x + w - 8.0 * s, cy - 6.0 * s, 4.0 * s, true, th().faint);
            }
            for (name, meta, score, sel, faded, action) in items.iter().skip(start).take(visible) {
                draw_row(pm, lay, cy, name, meta, *score, *sel, *faded, action.clone());
                cy += row_h;
            }
            if start + visible < items.len() {
                tri(pm, x + w - 8.0 * s, bottom - 4.0 * s, 4.0 * s, false, th().faint);
            }
        }
    }
}

/// Lignes à afficher : (texte, couleur, aligné à droite, taille relative, texte à copier).
/// Police d'un style de texte (gras et code chargés une fois, sinon la police normale).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum St {
    N,
    Gras,
    Code,
}

fn police<'a>(text: &'a Text, st: St) -> &'a Text {
    use std::sync::OnceLock;
    static GRAS: OnceLock<Option<Text>> = OnceLock::new();
    static MONO: OnceLock<Option<Text>> = OnceLock::new();
    let f = match st {
        St::N => None,
        St::Gras => GRAS.get_or_init(load_bold).as_ref(),
        St::Code => MONO.get_or_init(load_mono).as_ref(),
    };
    f.unwrap_or(text)
}

/// Police à chasse fixe (code).
pub fn load_mono() -> Option<Text> {
    let mut c = vec![];
    if let Ok(o) = std::process::Command::new("fc-match").args(["-f", "%{file}", "monospace:style=Regular"]).output() {
        c.push(String::from_utf8_lossy(&o.stdout).to_string());
    }
    c.extend(["/usr/share/fonts/TTF/DejaVuSansMono.ttf", "/usr/share/fonts/noto/NotoSansMono-Regular.ttf"].map(String::from));
    c.into_iter().filter_map(|p| std::fs::read(p.trim()).ok()).find_map(|b| FontVec::try_from_vec(b).ok()).map(|font| Text { font })
}

#[derive(Debug, Clone, PartialEq)]
pub struct Seg {
    pub t: String,
    pub st: St,
    pub lien: Option<String>,
}

/// Une ligne de l'historique de la bulle, déjà coupée à la largeur.
#[derive(Debug, Clone)]
pub struct LigneChat {
    pub segs: Vec<Seg>,
    pub c: (u8, u8, u8),
    pub right: bool,
    pub k: f32,
    pub copy: Option<String>,
    pub indent: f32,
    pub code: bool,
}

impl LigneChat {
    fn largeur(&self, text: &Text, sz: f32) -> f32 {
        self.segs.iter().map(|g| police(text, g.st).width(sz, &g.t)).sum()
    }
}

/// Coupe des morceaux stylés en lignes de largeur `max_w` (première ligne décalée de
/// `first`, les suivantes de `indent`).
fn wrap_spans(text: &Text, sz: f32, spans: &[crate::markdown::Span], max_w: f32, first: Vec<Seg>, indent: f32) -> Vec<(Vec<Seg>, f32)> {
    let mut lines: Vec<(Vec<Seg>, f32)> = vec![];
    let first_w: f32 = first.iter().map(|g| police(text, g.st).width(sz, &g.t)).sum();
    let mut cur: Vec<Seg> = first;
    let mut x = first_w;
    let mut ind = 0.0;
    for sp in spans {
        let st = if sp.code { St::Code } else if sp.gras { St::Gras } else { St::N };
        let f = police(text, st);
        for tok in sp.t.split_inclusive(' ') {
            let w = f.width(sz, tok);
            let blank = tok.trim().is_empty();
            if x + w > max_w - ind && x > 0.0 && !blank {
                lines.push((std::mem::take(&mut cur), ind));
                ind = indent;
                x = 0.0;
            }
            if x == 0.0 && blank && !lines.is_empty() {
                continue;
            }
            match cur.last_mut() {
                Some(g) if g.st == st && g.lien == sp.lien => g.t.push_str(tok),
                _ => cur.push(Seg { t: tok.to_string(), st, lien: sp.lien.clone() }),
            }
            x += w;
        }
    }
    lines.push((cur, ind));
    lines
}

fn plain(t: &str) -> Vec<crate::markdown::Span> {
    vec![crate::markdown::Span { t: t.to_string(), ..Default::default() }]
}

fn chat_lines(text: &Text, chat: &crate::chat::Chat, size: f32, w: f32) -> Vec<LigneChat> {
    use crate::chat::Entry;
    use crate::markdown::{Bloc, blocs};
    let mut out = vec![];
    let push_md = |out: &mut Vec<LigneChat>, t: &str, copy: Option<String>| {
        let c = th().text;
        for b in blocs(t) {
            let mk = |segs, ind, code| LigneChat { segs, c, right: false, k: 1.0, copy: copy.clone(), indent: ind, code };
            match b {
                Bloc::Para(v) => out.extend(wrap_spans(text, size, &v, w, vec![], 0.0).into_iter().map(|(l, i)| mk(l, i, false))),
                Bloc::Titre(v) => {
                    let v: Vec<_> = v.into_iter().map(|mut s| {
                        s.gras = true;
                        s
                    }).collect();
                    out.extend(wrap_spans(text, size, &v, w, vec![], 0.0).into_iter().map(|(l, i)| mk(l, i, false)));
                }
                Bloc::Puce(p, v) => {
                    let puce = Seg { t: format!("{p} "), st: St::N, lien: None };
                    let ind = text.width(size, &puce.t) + 6.0;
                    out.extend(wrap_spans(text, size, &v, w - 6.0, vec![puce], ind).into_iter().map(|(l, i)| {
                        let mut x = mk(l, i, false);
                        x.indent += 6.0;
                        x
                    }));
                }
                Bloc::Code(lines) => {
                    let f = police(text, St::Code);
                    let k = 0.9;
                    for l in lines {
                        // Coupe aux caractères : une commande longue reste entière.
                        let mut cur = String::new();
                        for ch in l.chars() {
                            if f.width(size * k, &format!("{cur}{ch}")) > w - 16.0 && !cur.is_empty() {
                                out.push(LigneChat { k, ..mk(vec![Seg { t: std::mem::take(&mut cur), st: St::Code, lien: None }], 8.0, true) });
                            }
                            cur.push(ch);
                        }
                        out.push(LigneChat { k, ..mk(vec![Seg { t: cur, st: St::Code, lien: None }], 8.0, true) });
                    }
                }
            }
        }
    };
    for e in &chat.entries {
        match e {
            Entry::Maki(t) => push_md(&mut out, t, Some(crate::markdown::brut(t))),
            Entry::User(t) | Entry::Info(t) | Entry::Error(t) => {
                let (c, right, k) = match e {
                    Entry::User(_) => (th().user, true, 1.0),
                    Entry::Info(_) => (th().soft, false, 0.85),
                    _ => ((176, 48, 40), false, 0.85),
                };
                for para in t.lines() {
                    for (l, _) in wrap_spans(text, size * k, &plain(para), if right { w * 0.85 } else { w }, vec![], 0.0) {
                        out.push(LigneChat { segs: l, c, right, k, copy: None, indent: 0.0, code: false });
                    }
                }
            }
        }
        out.push(LigneChat { segs: vec![], c: th().text, right: false, k: 0.45, copy: None, indent: 0.0, code: false });
    }
    // Réponse en train d'arriver.
    let f = crate::flux::visible(&chat.flux);
    if !f.is_empty() {
        push_md(&mut out, &f, None);
    }
    out
}

/// Bulle de discussion ancrée au point (ax, ay) (tête du maki), au-dessus si `above`.
#[allow(clippy::too_many_arguments)]
pub fn chat_bubble(pm: &mut Pixmap, text: &Text, chat: &crate::chat::Chat, ax: f32, ay: f32, above: bool, bx: f32, by: f32, s: f32, caret: bool, focused: bool, time: f32) -> ChatLayout {
    let size = 14.0 * s;
    let pad = 12.0 * s;
    let w = 400.0 * s;
    let inner = w - pad * 2.0;
    let lh = text.line_height(size);
    let mut lay = ChatLayout::default();

    let lines = chat_lines(text, chat, size, inner);
    let max_lines = 13usize;
    let end = lines.len().saturating_sub(chat.scroll.min(lines.len().saturating_sub(1)));
    let start = end.saturating_sub(max_lines);
    let shown = if chat.picker.is_some() || chat.panneau.is_some() { &lines[0..0] } else { &lines[start..end] };
    let hist_h: f32 = shown.iter().map(|l| lh * l.k).sum();

    let confirm_lines = chat.confirm.as_ref().map(|q| text.wrap(size * 0.95, q, inner - 16.0 * s)).unwrap_or_default();
    let confirm_h = if confirm_lines.is_empty() { 0.0 } else { confirm_lines.len() as f32 * lh + 48.0 * s };
    let header_h = 24.0 * s;
    let input_h = 34.0 * s;
    let hist_h = if chat.picker.is_some() || chat.panneau.is_some() { zone_h(chat) * s } else { hist_h };
    let h = pad + header_h + hist_h + confirm_h + 8.0 * s + input_h + pad;
    let x = bx;
    let y = by;
    let _ = (y, h);
    let rect = (x, if above { by - h } else { by }, w, h);
    lay.rect = rect;
    let (x, y) = (rect.0, rect.1);

    // Ombre, fond, pointe.
    rrect(pm, (x + 2.0, y + 4.0, w, h), 16.0 * s, &paint_a((0, 0, 0), 0.18), None);
    let bg = paint(th().bg);
    rrect(pm, rect, 16.0 * s, &bg, Some((th().border, 1.2)));
    let tip_x = ax.clamp(x + 24.0 * s, x + w - 24.0 * s);
    if above {
        poly(pm, Transform::identity(), &[(tip_x - 10.0 * s, y + h - 1.0), (ax, ay - 4.0 * s), (tip_x + 8.0 * s, y + h - 1.0)], th().bg);
    } else {
        poly(pm, Transform::identity(), &[(tip_x - 10.0 * s, y + 1.0), (ax, ay + 4.0 * s), (tip_x + 8.0 * s, y + 1.0)], th().bg);
    }

    // En-tête.
    text.draw(pm, size * 0.9, x + pad, y + pad * 0.6, "L Katta", th().chip_text);
    // Pastille du modèle : un clic ouvre le sélecteur.
    let chip_label = if chat.picker.is_some() {
        "Fermer la liste".to_string()
    } else if chat.panneau.is_some() {
        "Réglages".to_string()
    } else if !chat.engine.is_empty() {
        short(&chat.engine, 34)
    } else {
        let (i, nom) = chat.engines().first().cloned().unwrap_or_default();
        let m = chat.current_model(i);
        format!("{nom} · {}", if m == "auto" { "automatique".into() } else { short(&m, 24) })
    };
    let cw = text.width(size * 0.72, &chip_label) + 28.0 * s;
    lay.chip = (x + w - pad - 26.0 * s - cw, y + pad * 0.45, cw, 20.0 * s);
    rrect(pm, lay.chip, 10.0 * s, &paint(th().chip), Some((th().field_border, 1.0)));
    text.draw(pm, size * 0.72, lay.chip.0 + 8.0 * s, lay.chip.1 + (20.0 * s - text.line_height(size * 0.72)) / 2.0, &chip_label, th().chip_text);
    tri(pm, lay.chip.0 + cw - 11.0 * s, lay.chip.1 + 10.0 * s, 3.5 * s, chat.picker.is_some(), th().chip_text);
    lay.gear = (lay.chip.0 - 26.0 * s, lay.chip.1, 20.0 * s, 20.0 * s);
    rrect(pm, lay.gear, 10.0 * s, &paint(if chat.panneau.is_some() { (46, 126, 192) } else { th().chip }), Some((th().field_border, 1.0)));
    gear(pm, lay.gear.0 + 10.0 * s, lay.gear.1 + 10.0 * s, 6.5 * s, if chat.panneau.is_some() { (255, 255, 255) } else { th().chip_text });
    lay.close = (x + w - pad - 18.0 * s, y + pad * 0.5, 18.0 * s, 18.0 * s);
    text.draw(pm, size * 1.1, lay.close.0 + 3.0 * s, lay.close.1 - 2.0 * s, "×", th().chip_text);

    if chat.picker.is_some() {
        draw_picker(pm, text, chat, (x + pad, y + pad + header_h, inner, PICKER_H * s), s, &mut lay);
    } else if chat.panneau.is_some() {
        draw_panel(pm, text, chat, (x + pad, y + pad + header_h, inner, zone_h(chat) * s), s, &mut lay);
    }
    // Historique.
    let mut cy = y + pad + header_h;
    let mut copy_rect: Option<(R, String)> = None;
    for l in shown {
        let (k, copy) = (l.k, &l.copy);
        let sz = size * k;
        if l.code {
            rrect(pm, (x + pad, cy, inner, lh * k), 3.0 * s, &paint(th().field), None);
        }
        let mut lx = if l.right { x + w - pad - l.largeur(text, sz) } else { x + pad } + l.indent;
        for g in &l.segs {
            let f = police(text, g.st);
            let gw = f.width(sz, &g.t);
            let c = if g.lien.is_some() { th().user } else { l.c };
            f.draw(pm, sz, lx, cy, &g.t, c);
            if let Some(u) = &g.lien {
                let base = cy + f.line_height(sz) - 2.0 * s;
                let tw = f.width(sz, g.t.trim_end());
                line(pm, Transform::identity(), (lx, base), (lx + tw, base), 1.0 * s, c);
                lay.links.push(((lx, cy, tw, lh * k), u.clone()));
            }
            lx += gw;
        }
        if let Some(cp) = copy {
            match &mut copy_rect {
                Some((r, t)) if t == cp => r.3 = cy + lh * k - r.1,
                _ => {
                    if let Some(done) = copy_rect.take() {
                        lay.copy.push(done);
                    }
                    copy_rect = Some(((x + pad, cy, inner, lh * k), cp.clone()));
                }
            }
        } else if let Some(done) = copy_rect.take() {
            lay.copy.push(done);
        }
        cy += lh * k;
    }
    if let Some(done) = copy_rect.take() {
        lay.copy.push(done);
    }
    if start > 0 && chat.picker.is_none() && chat.panneau.is_none() {
        tri(pm, x + w / 2.0, y + pad + header_h - 6.0 * s, 4.5 * s, true, th().faint);
    }
    if chat.picker.is_some() || chat.panneau.is_some() {
        cy = y + pad + header_h + zone_h(chat) * s;
    }

    // Confirmation.
    if !confirm_lines.is_empty() {
        let r = (x + pad, cy, inner, confirm_h - 6.0 * s);
        rrect(pm, r, 10.0 * s, &paint((255, 240, 206)), Some(((230, 196, 120), 1.0)));
        for (i, l) in confirm_lines.iter().enumerate() {
            text.draw(pm, size * 0.95, r.0 + 8.0 * s, r.1 + 6.0 * s + i as f32 * lh, l, (90, 60, 20));
        }
        let by2 = r.1 + r.3 - 32.0 * s;
        let yes = (r.0 + 8.0 * s, by2, 92.0 * s, 26.0 * s);
        let no = (r.0 + 108.0 * s, by2, 76.0 * s, 26.0 * s);
        rrect(pm, yes, 8.0 * s, &paint((70, 150, 90)), None);
        rrect(pm, no, 8.0 * s, &paint((210, 204, 196)), None);
        text.draw(pm, size * 0.95, yes.0 + 12.0 * s, yes.1 + 4.0 * s, "Oui", (255, 255, 255));
        text.draw(pm, size * 0.95, no.0 + 12.0 * s, no.1 + 4.0 * s, "Non", (60, 54, 48));
        lay.yes = Some(yes);
        lay.no = Some(no);
        cy += confirm_h;
    }

    // Saisie.
    let input = (x + pad, cy + 8.0 * s, inner, input_h);
    lay.input = input;
    rrect(pm, input, 10.0 * s, &paint(th().field), Some((th().field_border, 1.0)));
    let ty = input.1 + (input_h - lh) / 2.0;
    if chat.busy && chat.confirm.is_none() {
        let dots = ".".repeat(1 + ((time * 3.0) as usize % 3));
        text.draw(pm, size, input.0 + 10.0 * s, ty, &format!("Je réfléchis{dots}"), th().soft);
    } else if chat.input.is_empty() {
        let hint = if focused { "Écris ici… (Entrée : envoyer, Échap : fermer)" } else { "Clique ici pour écrire…" };
        text.draw(pm, size, input.0 + 10.0 * s, ty, hint, th().faint);
        if caret {
            line(pm, Transform::identity(), (input.0 + 9.0 * s, ty + 2.0), (input.0 + 9.0 * s, ty + lh - 2.0), 1.4, th().text);
        }
    } else {
        // Fenêtre visible autour du curseur, sélection surlignée.
        let maxw = inner - 20.0 * s;
        let cs: Vec<char> = chat.input.texte().chars().collect();
        let cur = chat.input.curseur.min(cs.len());
        let wd = |a: usize, b: usize| text.width(size, &cs[a..b].iter().collect::<String>());
        let mut v0 = 0;
        while v0 < cur && wd(v0, cur) > maxw - 12.0 * s {
            v0 += 1;
        }
        let mut v1 = cur;
        while v1 < cs.len() && wd(v0, v1 + 1) <= maxw {
            v1 += 1;
        }
        let x0 = input.0 + 10.0 * s;
        lay.input_off = v0;
        lay.input_x = (v0..=v1).map(|i| x0 + wd(v0, i)).collect();
        if let Some((a, b)) = chat.input.selection() {
            let (a, b) = (a.clamp(v0, v1), b.clamp(v0, v1));
            if b > a {
                let (xa, xb) = (lay.input_x[a - v0], lay.input_x[b - v0]);
                rrect(pm, (xa, ty, xb - xa, lh), 3.0 * s, &paint_a((46, 126, 192), 0.35), None);
            }
        }
        let shown_in: String = cs[v0..v1].iter().collect();
        text.draw(pm, size, x0, ty, &shown_in, th().text);
        if caret {
            let cx = lay.input_x[cur - v0] + 0.5;
            line(pm, Transform::identity(), (cx, ty + 2.0), (cx, ty + lh - 2.0), 1.4, th().text);
        }
    }
    lay
}

/// Hauteur approximative de la bulle (pour placer la surface avant de dessiner).
pub fn chat_height(text: &Text, chat: &crate::chat::Chat, s: f32) -> f32 {
    let size = 14.0 * s;
    let lh = text.line_height(size);
    let lines = chat_lines(text, chat, size, 400.0 * s - 24.0 * s);
    let n = lines.len().min(13);
    let hist: f32 = if chat.picker.is_some() || chat.panneau.is_some() { zone_h(chat) * s } else { lines[lines.len() - n..].iter().map(|l| lh * l.k).sum() };
    let confirm = chat.confirm.as_ref().map(|q| text.wrap(size * 0.95, q, 360.0 * s).len() as f32 * lh + 48.0 * s).unwrap_or(0.0);
    24.0 * s + 24.0 * s + hist + confirm + 8.0 * s + 34.0 * s
}

/// Zones cliquables d'une bulle de message.
#[derive(Debug, Clone, Default)]
pub struct BubbleLayout {
    pub rect: R,
    pub close: R,
    pub actions: Vec<R>,
}

/// Couleur d'accent de chaque genre de bulle.
pub fn accent(k: crate::conseils::Kind) -> (u8, u8, u8) {
    use crate::conseils::Kind as K;
    match k {
        K::Hello => (226, 140, 30),
        K::Tip => (46, 126, 192),
        K::Reminder => (204, 70, 116),
        K::Alert => (214, 98, 28),
        K::News => (62, 146, 90),
        K::Answer => (120, 88, 184),
        K::Info => (128, 116, 102),
        K::Mail => (196, 92, 58),
    }
}

fn icon(pm: &mut Pixmap, bold: &Text, k: crate::conseils::Kind, cx: f32, cy: f32, r: f32) {
    use crate::conseils::Kind as K;
    let a = accent(k);
    disc(pm, Transform::identity(), cx, cy, r, &paint(a));
    let w = (255, 255, 255);
    let id = Transform::identity();
    let u = r / 14.0;
    match k {
        K::Hello => {
            disc(pm, id, cx, cy, 4.6 * u, &paint(w));
            for i in 0..8 {
                let ang = i as f32 * std::f32::consts::FRAC_PI_4;
                line(pm, id, (cx + ang.cos() * 7.2 * u, cy + ang.sin() * 7.2 * u), (cx + ang.cos() * 9.8 * u, cy + ang.sin() * 9.8 * u), 1.7 * u, w);
            }
        }
        K::Tip => {
            disc(pm, id, cx, cy - 1.8 * u, 5.6 * u, &paint(w));
            poly(pm, id, &[(cx - 3.0 * u, cy + 2.5 * u), (cx + 3.0 * u, cy + 2.5 * u), (cx + 2.4 * u, cy + 6.4 * u), (cx - 2.4 * u, cy + 6.4 * u)], w);
            line(pm, id, (cx - 2.2 * u, cy + 8.2 * u), (cx + 2.2 * u, cy + 8.2 * u), 1.6 * u, w);
        }
        K::Reminder => {
            poly(pm, id, &[(cx, cy - 7.5 * u), (cx + 5.0 * u, cy - 3.0 * u), (cx + 5.6 * u, cy + 3.5 * u), (cx + 7.5 * u, cy + 5.5 * u), (cx - 7.5 * u, cy + 5.5 * u), (cx - 5.6 * u, cy + 3.5 * u), (cx - 5.0 * u, cy - 3.0 * u)], w);
            disc(pm, id, cx, cy + 7.6 * u, 1.9 * u, &paint(w));
        }
        K::News => {
            for (i, l) in [7.5f32, 7.5, 5.0].iter().enumerate() {
                line(pm, id, (cx - 6.5 * u, cy - 4.5 * u + i as f32 * 4.5 * u), (cx - 6.5 * u + l * 1.7 * u, cy - 4.5 * u + i as f32 * 4.5 * u), 1.9 * u, w);
            }
        }
        K::Mail => {
            // Enveloppe.
            let (l, t, r, b) = (cx - 7.5 * u, cy - 5.0 * u, cx + 7.5 * u, cy + 5.5 * u);
            let mut pb = PathBuilder::new();
            pb.move_to(l, t);
            pb.line_to(r, t);
            pb.line_to(r, b);
            pb.line_to(l, b);
            pb.close();
            pb.move_to(l, t);
            pb.line_to(cx, cy + 1.0 * u);
            pb.line_to(r, t);
            if let Some(path) = pb.finish() {
                pm.stroke_path(&path, &paint(w), &Stroke { width: 1.6 * u, line_join: tiny_skia::LineJoin::Round, ..Default::default() }, id, None);
            }
        }
        K::Answer => {
            for i in -1..=1 {
                disc(pm, id, cx + i as f32 * 5.0 * u, cy, 1.9 * u, &paint(w));
            }
        }
        K::Alert | K::Info => {
            let g = if k == K::Alert { "!" } else { "i" };
            let sz = 17.0 * u;
            let gw = bold.width(sz, g);
            bold.draw(pm, sz, cx - gw / 2.0, cy - bold.line_height(sz) / 2.0 + u, g, w);
        }
    }
}

fn pill(pm: &mut Pixmap, text: &Text, r: R, label: &str, c: (u8, u8, u8), filled: bool, size: f32) {
    let fill = if filled { paint(c) } else { paint_a((255, 255, 255), 0.0) };
    rrect(pm, r, r.3 / 2.0, &fill, Some((c, 1.2)));
    let tw = text.width(size, label);
    let fg = if filled { (255, 255, 255) } else { c };
    text.draw(pm, size, r.0 + (r.2 - tw) / 2.0, r.1 + (r.3 - text.line_height(size)) / 2.0, label, fg);
}

/// Bulle de message soignée, au-dessus du point (ax, ay).
#[allow(clippy::too_many_arguments)]
pub fn fancy_bubble(pm: &mut Pixmap, text: &Text, bold: &Text, b: &super::behavior::Bubble, actions: &[&str], ax: f32, ay: f32, s: f32) -> BubbleLayout {
    let msg = &b.msg;
    let acc = accent(msg.kind);
    let pad = 12.0 * s;
    let ir = 14.0 * s;
    let gap = 10.0 * s;
    let (ts, bs, as_) = (14.5 * s, 13.5 * s, 12.5 * s);
    let max_inner = 270.0 * s;
    let title_lines = if msg.title.is_empty() { vec![] } else { bold.wrap(ts, &msg.title, max_inner) };
    let mut body = text.wrap(bs, &msg.text, max_inner);
    body.truncate(7);
    let tlh = bold.line_height(ts);
    let blh = text.line_height(bs);
    let pill_h = 24.0 * s;
    let pill_ws: Vec<f32> = actions.iter().map(|a| text.width(as_, a) + 22.0 * s).collect();
    let acts_w: f32 = pill_ws.iter().sum::<f32>() + 8.0 * s * actions.len().saturating_sub(1) as f32;
    let content_w = title_lines
        .iter()
        .map(|l| bold.width(ts, l))
        .chain(body.iter().map(|l| text.width(bs, l)))
        .fold(acts_w, f32::max)
        .max(120.0 * s);
    let w = pad + ir * 2.0 + gap + content_w + pad + 10.0 * s;
    let content_h = tlh * title_lines.len() as f32 + blh * body.len() as f32 + if actions.is_empty() { 0.0 } else { 10.0 * s + pill_h };
    let h = pad + content_h.max(ir * 2.0) + pad;

    // Apparition : glisse vers le haut.
    let k = (b.age / 0.28).min(1.0);
    let ease = 1.0 - (1.0 - k).powi(3);
    let x = (ax - w * 0.3).clamp(4.0, (pm.width() as f32 - w - 4.0).max(4.0));
    let y = (ay - h - 18.0 * s).max(2.0) + (1.0 - ease) * 10.0 * s;
    let rect = (x, y, w, h);
    let bottom = y + h;

    // Ombre douce.
    rrect(pm, (x + 1.0, y + 5.0 * s, w, h), 16.0 * s, &paint_a((0, 0, 0), 0.10), None);
    rrect(pm, (x, y + 2.0 * s, w, h), 16.0 * s, &paint_a((0, 0, 0), 0.10), None);
    // Pointe courbe vers la tête.
    let tip_x = ax.clamp(x + 26.0 * s, x + w - 26.0 * s);
    let tail = |pm: &mut Pixmap, fill: bool| {
        let mut pb = PathBuilder::new();
        pb.move_to(tip_x - 11.0 * s, bottom - 2.0);
        pb.quad_to(tip_x - 2.0 * s, bottom + 6.0 * s, ax, ay - 2.0 * s);
        pb.quad_to(tip_x + 3.0 * s, bottom + 4.0 * s, tip_x + 9.0 * s, bottom - 2.0);
        if fill {
            pb.close();
        }
        if let Some(path) = pb.finish() {
            if fill {
                pm.fill_path(&path, &paint(th().bg), FillRule::Winding, Transform::identity(), None);
            } else {
                pm.stroke_path(&path, &paint_a(acc, 0.55), &Stroke { width: 1.3, ..Default::default() }, Transform::identity(), None);
            }
        }
    };
    // Fond en léger dégradé, liseré de la couleur du genre.
    let grad = tiny_skia::LinearGradient::new(
        tiny_skia::Point::from_xy(x, y),
        tiny_skia::Point::from_xy(x, y + h),
        vec![
            tiny_skia::GradientStop::new(0.0, Color::from_rgba8(th().bg.0, th().bg.1, th().bg.2, 255)),
            tiny_skia::GradientStop::new(1.0, Color::from_rgba8(th().bg2.0, th().bg2.1, th().bg2.2, 255)),
        ],
        tiny_skia::SpreadMode::Pad,
        Transform::identity(),
    );
    let mut bg = Paint::default();
    bg.anti_alias = true;
    if let Some(g) = grad {
        bg.shader = g;
    } else {
        bg.set_color_rgba8(th().bg.0, th().bg.1, th().bg.2, 255);
    }
    tail(pm, true);
    rrect(pm, rect, 16.0 * s, &bg, None);
    let edge = if th() == SOMBRE { (acc.0 / 2 + 40, acc.1 / 2 + 40, acc.2 / 2 + 40) } else { (acc.0 / 2 + 110, acc.1 / 2 + 110, acc.2 / 2 + 110) };
    rrect(pm, rect, 16.0 * s, &paint_a((0, 0, 0), 0.0), Some((edge, 1.3)));
    tail(pm, true);
    tail(pm, false);
    // Recouvre le liseré à la base de la pointe.
    line(pm, Transform::identity(), (tip_x - 9.0 * s, bottom - 1.2), (tip_x + 7.0 * s, bottom - 1.2), 2.6, th().bg2);

    icon(pm, bold, msg.kind, x + pad + ir, y + pad + ir, ir);
    let close = (x + w - 22.0 * s, y + 6.0 * s, 16.0 * s, 16.0 * s);
    text.draw(pm, 15.0 * s, close.0 + 3.0 * s, close.1 - 3.0 * s, "×", th().faint);

    // Texte qui s'écrit.
    let mut budget = b.revealed();
    let tx = x + pad + ir * 2.0 + gap;
    let mut cy = y + pad;
    let take = |l: &str, budget: &mut usize| -> String {
        let n = l.chars().count().min(*budget);
        *budget -= n;
        l.chars().take(n).collect()
    };
    let dark = if th() == SOMBRE { (acc.0 / 2 + 120, acc.1 / 2 + 120, acc.2 / 2 + 120) } else { (acc.0 / 3 + 30, acc.1 / 3 + 30, acc.2 / 3 + 30) };
    for l in &title_lines {
        let part = take(l, &mut budget);
        bold.draw(pm, ts, tx, cy, &part, dark);
        cy += tlh;
    }
    for l in &body {
        let part = take(l, &mut budget);
        text.draw(pm, bs, tx, cy, &part, th().text);
        cy += blh;
    }
    let mut lay = BubbleLayout { rect, close, actions: vec![] };
    if !actions.is_empty() && b.revealed() >= b.full() {
        let mut px = tx;
        let py = cy + 10.0 * s;
        for (i, (a, pw)) in actions.iter().zip(&pill_ws).enumerate() {
            let r = (px, py, *pw, pill_h);
            pill(pm, text, r, a, acc, i == 0, as_);
            lay.actions.push(r);
            px += pw + 8.0 * s;
        }
    }
    lay
}

// --- panneau de réglages ----------------------------------------------------------------

/// Roue dentée (bouton des réglages).
fn gear(pm: &mut Pixmap, cx: f32, cy: f32, r: f32, c: (u8, u8, u8)) {
    let id = Transform::identity();
    for k in 0..8 {
        let a = k as f32 * std::f32::consts::FRAC_PI_4;
        line(pm, id, (cx + a.cos() * r * 0.55, cy + a.sin() * r * 0.55), (cx + a.cos() * r, cy + a.sin() * r), r * 0.42, c);
    }
    disc(pm, id, cx, cy, r * 0.72, &paint(c));
    disc(pm, id, cx, cy, r * 0.3, &paint(th().chip));
}

struct Ui<'a> {
    pm: &'a mut Pixmap,
    text: &'a Text,
    s: f32,
    x: f32,
    w: f32,
    y: f32,
    top: f32,
    bottom: f32,
    acts: &'a mut Vec<(R, crate::chat::Act)>,
}

const ACC: (u8, u8, u8) = (46, 126, 192);

impl Ui<'_> {
    fn show(&self, h: f32) -> bool {
        self.y >= self.top - 1.0 && self.y + h <= self.bottom + 1.0
    }

    fn section(&mut self, title: &str) {
        let h = 24.0 * self.s;
        if self.show(h) {
            self.text.draw(self.pm, 11.5 * self.s, self.x, self.y + 7.0 * self.s, &title.to_uppercase(), th().chip_text);
        }
        self.y += h;
    }

    fn note(&mut self, t: &str) {
        let sz = 11.0 * self.s;
        let lh = self.text.line_height(sz);
        for l in self.text.wrap(sz, t, self.w) {
            if self.show(lh) {
                self.text.draw(self.pm, sz, self.x, self.y, &l, th().soft);
            }
            self.y += lh;
        }
        self.y += 4.0 * self.s;
    }

    fn label(&mut self, label: &str, h: f32) {
        let sz = 12.5 * self.s;
        self.text.draw(self.pm, sz, self.x, self.y + (h - self.text.line_height(sz)) / 2.0, &short(label, 40), th().text);
    }

    fn toggle(&mut self, label: &str, on: bool, act: crate::chat::Act) {
        let s = self.s;
        let h = 28.0 * s;
        if self.show(h) {
            self.label(label, h);
            let r = (self.x + self.w - 40.0 * s, self.y + 5.0 * s, 38.0 * s, 18.0 * s);
            rrect(self.pm, r, 9.0 * s, &paint(if on { ACC } else { th().field_border }), None);
            let kx = if on { r.0 + r.2 - 9.0 * s } else { r.0 + 9.0 * s };
            disc(self.pm, Transform::identity(), kx, r.1 + 9.0 * s, 7.0 * s, &paint((255, 255, 255)));
            self.acts.push(((self.x, self.y, self.w, h), act));
        }
        self.y += h;
    }

    fn stepper(&mut self, label: &str, value: &str, minus: crate::chat::Act, plus: crate::chat::Act) {
        let s = self.s;
        let h = 28.0 * s;
        if self.show(h) {
            self.label(label, h);
            let bw = 24.0 * s;
            let vw = 62.0 * s;
            let x0 = self.x + self.w - (bw * 2.0 + vw);
            let rm = (x0, self.y + 3.0 * s, bw, 22.0 * s);
            let rp = (x0 + bw + vw, self.y + 3.0 * s, bw, 22.0 * s);
            pill(self.pm, self.text, rm, "−", th().chip_text, false, 13.0 * s);
            pill(self.pm, self.text, rp, "+", th().chip_text, false, 13.0 * s);
            let tw = self.text.width(12.5 * s, value);
            self.text.draw(self.pm, 12.5 * s, x0 + bw + (vw - tw) / 2.0, self.y + (h - self.text.line_height(12.5 * s)) / 2.0, value, th().text);
            self.acts.push((rm, minus));
            self.acts.push((rp, plus));
        }
        self.y += h;
    }

    fn chips(&mut self, label: &str, options: &[(&str, &str)], current: &str, path: &'static str) {
        let s = self.s;
        if !label.is_empty() {
            let h = 20.0 * s;
            if self.show(h) {
                self.text.draw(self.pm, 12.5 * s, self.x, self.y + 2.0 * s, label, th().text);
            }
            self.y += h;
        }
        let mut cx = self.x;
        let ch = 24.0 * s;
        for (value, text) in options {
            let lw = self.text.width(11.5 * s, text) + 18.0 * s;
            if cx + lw > self.x + self.w {
                cx = self.x;
                self.y += ch + 4.0 * s;
            }
            if self.show(ch) {
                let r = (cx, self.y, lw, 21.0 * s);
                pill(self.pm, self.text, r, text, ACC, *value == current, 11.5 * s);
                self.acts.push((r, crate::chat::Act::Choix(path, value.to_string())));
            }
            cx += lw + 5.0 * s;
        }
        self.y += ch + 6.0 * s;
    }

    fn buttons(&mut self, list: &[(&str, crate::chat::Act, bool)]) {
        let s = self.s;
        let h = 28.0 * s;
        let mut bx = self.x;
        for (label, act, filled) in list {
            let lw = self.text.width(11.5 * s, label) + 20.0 * s;
            if bx + lw > self.x + self.w {
                bx = self.x;
                self.y += h;
            }
            if self.show(h) {
                let r = (bx, self.y + 2.0 * s, lw, 22.0 * s);
                pill(self.pm, self.text, r, label, ACC, *filled, 11.5 * s);
                self.acts.push((r, act.clone()));
            }
            bx += lw + 6.0 * s;
        }
        self.y += h;
    }

    #[allow(clippy::too_many_arguments)]
    fn field(&mut self, chat: &crate::chat::Chat, champ: crate::chat::Champ, placeholder: &str, save: Option<&str>) {
        let s = self.s;
        let h = 30.0 * s;
        if self.show(h) {
            let p = chat.panneau.as_ref();
            let value = p.and_then(|p| p.champs.get(&champ).cloned()).unwrap_or_default();
            let focused = p.is_some_and(|p| p.focus.as_ref() == Some(&champ));
            let bw = save.map(|l| self.text.width(11.5 * s, l) + 20.0 * s + 6.0 * s).unwrap_or(0.0);
            let r = (self.x, self.y + 2.0 * s, self.w - bw, 25.0 * s);
            rrect(self.pm, r, 7.0 * s, &paint(th().field), Some((if focused { ACC } else { th().field_border }, if focused { 1.6 } else { 1.0 })));
            let sz = 12.0 * s;
            let ty = r.1 + (r.3 - self.text.line_height(sz)) / 2.0;
            let shown = if champ.secret() { "•".repeat(value.chars().count()) } else { value.clone() };
            let mut vis = shown.clone();
            while self.text.width(sz, &vis) > r.2 - 16.0 * s && !vis.is_empty() {
                vis.remove(0);
            }
            if value.is_empty() && !focused {
                self.text.draw(self.pm, sz, r.0 + 8.0 * s, ty, &short(placeholder, 46), th().faint);
            } else {
                self.text.draw(self.pm, sz, r.0 + 8.0 * s, ty, &vis, th().text);
            }
            if focused {
                let cx = r.0 + 8.0 * s + self.text.width(sz, &vis) + 1.0;
                line(self.pm, Transform::identity(), (cx, ty + 2.0), (cx, ty + self.text.line_height(sz) - 2.0), 1.3, th().text);
            }
            self.acts.push((r, crate::chat::Act::Focus(champ.clone())));
            if let Some(l) = save {
                let rb = (r.0 + r.2 + 6.0 * s, r.1 + 1.5 * s, bw - 6.0 * s, 22.0 * s);
                pill(self.pm, self.text, rb, l, ACC, true, 11.5 * s);
                self.acts.push((rb, crate::chat::Act::Enregistrer(champ)));
            }
        }
        self.y += h + 2.0 * s;
    }
}

/// Panneau de réglages dans la bulle (zone fixe, contenu qui défile).
fn draw_panel(pm: &mut Pixmap, text: &Text, chat: &crate::chat::Chat, area: R, s: f32, lay: &mut ChatLayout) {
    use crate::chat::{Act, Champ, ONGLETS};
    let Some(p) = &chat.panneau else { return };
    let (x, y, w, h) = area;
    lay.picker = Some(area);
    // Onglets (fixes).
    let mut tx = x;
    let mut ty = y;
    for (i, name) in ONGLETS.iter().enumerate() {
        let lw = text.width(11.5 * s, name) + 18.0 * s;
        if tx + lw > x + w {
            tx = x;
            ty += 26.0 * s;
        }
        let r = (tx, ty, lw, 21.0 * s);
        pill(pm, text, r, name, th().chip_text, i == p.onglet, 11.5 * s);
        lay.picks_panel.push((r, Act::Onglet(i)));
        tx += lw + 5.0 * s;
    }
    let top = ty + 30.0 * s;
    let msg_h = if p.message.is_some() { 34.0 * s } else { 0.0 };
    let bottom = y + h - msg_h;
    let cfg = chat.cfg();
    let mut acts = vec![];
    let start = top - p.scroll * s;
    let scrollable = p.contenu.get() > 470.0;
    let cw = if scrollable { w - 12.0 * s } else { w };
    {
        let mut ui = Ui { pm, text, s, x, w: cw, y: start, top, bottom, acts: &mut acts };
        match p.onglet {
            0 => {
                ui.section("Toi");
                ui.field(chat, Champ::Prenom, "Ton prénom", Some("Enregistrer"));
                ui.section("Sa personnalité et ses règles");
                ui.note("Fichiers ouverts dans ton éditeur ; ils valent pour cet ordinateur et ce compte.");
                ui.buttons(&[("Personnalité", Act::OuvrirFichier("prompt.md"), false), ("Règles strictes", Act::OuvrirFichier("regles.md"), false), ("Interdits", Act::OuvrirFichier("regles.toml"), false), ("Ce qu'il sait de toi", Act::OuvrirFichier("profil.md"), false)]);
                ui.section("Cerveau");
                let (i, nom) = chat.engines().first().cloned().unwrap_or_default();
                let m = chat.current_model(i);
                ui.note(&format!("{nom} · modèle : {}", if m == "auto" { "automatique (le meilleur gratuit pour chaque demande)".to_string() } else { m }));
                ui.buttons(&[("Choisir le modèle…", Act::ChoisirModele, true), ("Rafraîchir les modèles", Act::RafraichirCatalogue, false)]);
            }
            1 => {
                let m = &cfg.maki;
                ui.stepper("Durée d'une bulle", &format!("{} s", m.bulle_s), Act::Pas("maki.bulle_s", -1.0, 3.0, 60.0), Act::Pas("maki.bulle_s", 1.0, 3.0, 60.0));
                let c = if m.conseils_h == 0 { "jamais".to_string() } else { format!("{} h", m.conseils_h) };
                ui.stepper("Une astuce toutes les", &c, Act::Pas("maki.conseils_h", -1.0, 0.0, 24.0), Act::Pas("maki.conseils_h", 1.0, 0.0, 24.0));
                ui.stepper("Remarques au plus toutes les", &format!("{} min", m.parole_min), Act::Pas("maki.parole_min", -5.0, 5.0, 240.0), Act::Pas("maki.parole_min", 5.0, 5.0, 240.0));
                ui.toggle("Numéros (arbre, escalade, téléportation)", m.acrobaties, Act::Bascule("maki.acrobaties"));
                ui.stepper("Un numéro au plus toutes les", &format!("{} min", m.numeros_min), Act::Pas("maki.numeros_min", -5.0, 5.0, 240.0), Act::Pas("maki.numeros_min", 5.0, 5.0, 240.0));
                ui.note("Les rappels s'affichent toujours, quels que soient ces réglages. Clic droit sur le maki : mode discret.");
            }
            2 => {
                let m = &cfg.maki;
                ui.stepper("Taille du texte", &format!("× {:.1}", m.texte), Act::Pas("maki.texte", -0.1, 0.8, 2.2), Act::Pas("maki.texte", 0.1, 0.8, 2.2));
                ui.chips("Thème des bulles", &[("auto", "Comme KDE"), ("clair", "Clair"), ("sombre", "Sombre")], &m.theme, "maki.theme");
                let opts: Vec<(&str, &str)> = super::especes::Espece::TOUTES.iter().map(|e| (e.id(), e.nom().split(" (").next().unwrap_or(""))).collect();
                ui.chips("Personnage", &opts, &m.espece, "maki.espece");
                ui.stepper("Taille", &format!("× {:.1}", m.taille), Act::Pas("maki.taille", -0.1, 0.6, 2.5), Act::Pas("maki.taille", 0.1, 0.6, 2.5));
                ui.chips("Où il se promène", &[("bords", "Sur les bords"), ("libre", "Partout")], &m.zone, "maki.zone");
                ui.toggle("Passe d'un écran à l'autre", m.ecrans_multiples, Act::Bascule("maki.ecrans_multiples"));
                ui.toggle("Te suit sur l'écran où tu travailles", m.suivre, Act::Bascule("maki.suivre"));
            }
            3 => {
                ui.toggle("Lancer à l'ouverture de session", cfg.demarrage_auto, Act::Bascule("demarrage_auto"));
                ui.chips("Mises à jour", &[("proposer", "Me demander"), ("auto", "Automatiques"), ("non", "Jamais")], &cfg.maj, "maj");
                ui.buttons(&[("Vérifier maintenant", Act::VerifierMaj, false)]);
                ui.note(&format!("Version installée : {}.", crate::maj::current()));
                let [a, b] = cfg.maki.sommeil;
                ui.stepper("Il s'endort à", &format!("{a} h"), Act::Pas("maki.sommeil.1", -1.0, 0.0, 23.0), Act::Pas("maki.sommeil.1", 1.0, 0.0, 23.0));
                ui.stepper("Il se réveille à", &format!("{b} h"), Act::Pas("maki.sommeil.2", -1.0, 0.0, 23.0), Act::Pas("maki.sommeil.2", 1.0, 0.0, 23.0));
            }
            4 => {
                if !cfg.mail.is_empty() {
                    ui.section("Mails");
                }
                for (i, c) in cfg.mail.iter().enumerate() {
                    ui.note(&format!("{} ({}) — branché", c.adresse, c.nom));
                    ui.buttons(&[("Tester", Act::TesterMail(i), false), ("Retirer", Act::RetirerMail(i), false)]);
                }
                if !cfg.mail.is_empty() {
                    ui.toggle("Me prévenir des nouveaux mails", cfg.alertes.mails, Act::Bascule("alertes.mails"));
                }
                ui.section(if cfg.mail.is_empty() { "Brancher Gmail en 2 étapes" } else { "Ajouter un autre compte" });
                ui.note("1. Crée un « mot de passe d'application » (il faut la validation en deux étapes) :");
                ui.buttons(&[("Ouvrir la page Google", Act::OuvrirLien("https://myaccount.google.com/apppasswords"), false)]);
                ui.note("2. Colle ton adresse et les 16 lettres données par Google :");
                ui.field(chat, Champ::MailAdresse, "ton.adresse@gmail.com", None);
                ui.field(chat, Champ::MailPasse, "Les 16 lettres (les espaces ne comptent pas)", Some("Brancher"));
                ui.note("Autre messagerie : ton adresse et ton mot de passe habituel. Tout va dans le trousseau.");
                ui.section("Agenda");
                if !cfg.agenda.is_empty() {
                    ui.note(&format!("{} agenda(s) branché(s).", cfg.agenda.len()));
                    ui.toggle("Me prévenir avant mes rendez-vous", cfg.alertes.agenda, Act::Bascule("alertes.agenda"));
                    ui.stepper("Combien de temps avant", &format!("{} min", cfg.alertes.avance_min), Act::Pas("alertes.avance_min", -5.0, 5.0, 120.0), Act::Pas("alertes.avance_min", 5.0, 5.0, 120.0));
                }
                ui.note("Google Agenda : ouvre les réglages, choisis ton agenda à gauche, puis « Intégrer l'agenda » › copie l'« adresse secrète au format iCal ».");
                ui.buttons(&[("Ouvrir les réglages de Google Agenda", Act::OuvrirLien("https://calendar.google.com/calendar/r/settings"), false)]);
                ui.field(chat, Champ::Agenda, "Colle l'adresse iCal (ou un fichier .ics)", Some("Brancher"));
                ui.section("Contacts");
                ui.field(chat, Champ::Contacts, "Fichier .vcf (export de Google Contacts…)", Some("Enregistrer"));
            }
            _ => {
                ui.note("Le moteur intégré marche sans aucune clé. Une clé gratuite ajoute les modèles d'un fournisseur ; elle reste dans le trousseau.");
                for f in crate::cles::FOURNISSEURS {
                    let key = crate::cles::cle(f.id);
                    let hdr = 22.0 * s;
                    if ui.show(hdr) {
                        ui.text.draw(ui.pm, 12.5 * s, ui.x, ui.y + 3.0 * s, f.nom, th().text);
                        if let Some(k) = &key {
                            let st = format!("clé enregistrée {}", crate::cles::masked(k));
                            let sw = ui.text.width(10.5 * s, &st);
                            check(ui.pm, ui.x + ui.w - sw - 16.0 * s, ui.y + 5.0 * s, s * 0.9, (70, 150, 90));
                            ui.text.draw(ui.pm, 10.5 * s, ui.x + ui.w - sw, ui.y + 4.0 * s, &st, (70, 150, 90));
                        }
                    }
                    ui.y += hdr;
                    ui.note(f.description);
                    if key.is_some() {
                        ui.buttons(&[("Retirer la clé", Act::RetirerCle(f.id), false)]);
                    } else {
                        ui.buttons(&[("Obtenir une clé", Act::ObtenirCle(f.id), false)]);
                        ui.field(chat, Champ::Cle(f.id), "Colle la clé ici", Some("Enregistrer"));
                    }
                    ui.y += 4.0 * s;
                }
            }
        }
        lay.panel_content = (ui.y - start) / s;
        lay.panel_view = (bottom - top) / s;
        // Hauteur totale voulue : onglets + contenu + message (la bulle s'y ajuste).
        p.contenu.set((top - y) / s + lay.panel_content + msg_h / s + 6.0);
    }
    lay.picks_panel.extend(acts);
    // Barre de défilement visible dès que le contenu dépasse.
    if lay.panel_content > lay.panel_view + 1.0 {
        let track = (x + w - 5.0 * s, top, 5.0 * s, bottom - top);
        rrect(pm, track, 2.5 * s, &paint(th().field), None);
        let ratio = (lay.panel_view / lay.panel_content).clamp(0.08, 1.0);
        let th_h = track.3 * ratio;
        let pos = (p.scroll / (lay.panel_content - lay.panel_view)).clamp(0.0, 1.0);
        rrect(pm, (track.0, track.1 + (track.3 - th_h) * pos, track.2, th_h), 2.5 * s, &paint(th().faint), None);
    }
    if let Some((ok, m)) = &p.message {
        let r = (x, y + h - msg_h + 4.0 * s, w, msg_h - 6.0 * s);
        let (bg, fg) = if *ok { ((226, 242, 230), (40, 110, 60)) } else { ((250, 228, 222), (160, 50, 36)) };
        rrect(pm, r, 8.0 * s, &paint(bg), None);
        for (i, l) in text.wrap(11.0 * s, m, w - 16.0 * s).iter().take(2).enumerate() {
            text.draw(pm, 11.0 * s, r.0 + 8.0 * s, r.1 + 3.0 * s + i as f32 * 13.0 * s, l, fg);
        }
    }
}

/// Pose `src` sur `dst` (même taille, RGBA prémultiplié) dans le rectangle `r` seulement,
/// avec une opacité : bien plus rapide qu'une composition de toute l'image.
pub fn blend_over(dst: &mut Pixmap, src: &Pixmap, r: R, opacity: f32) {
    let (w, h) = (dst.width() as i32, dst.height() as i32);
    let x0 = (r.0.floor() as i32).clamp(0, w);
    let y0 = (r.1.floor() as i32).clamp(0, h);
    let x1 = ((r.0 + r.2).ceil() as i32).clamp(0, w);
    let y1 = ((r.1 + r.3).ceil() as i32).clamp(0, h);
    let op = (opacity.clamp(0.0, 1.0) * 255.0).round() as u32;
    if op == 0 || x1 <= x0 || y1 <= y0 {
        return;
    }
    let sd = src.data();
    let dd = dst.data_mut();
    for y in y0..y1 {
        let row = (y * w) as usize * 4;
        for x in x0..x1 {
            let i = row + x as usize * 4;
            let sa = sd[i + 3] as u32 * op / 255;
            if sa == 0 {
                continue;
            }
            let inv = 255 - sa;
            for c in 0..3 {
                let s = sd[i + c] as u32 * op / 255;
                dd[i + c] = (s + dd[i + c] as u32 * inv / 255).min(255) as u8;
            }
            dd[i + 3] = (sa + dd[i + 3] as u32 * inv / 255).min(255) as u8;
        }
    }
}

/// Efface un rectangle (transparent).
pub fn clear_rect(pm: &mut Pixmap, r: R) {
    let (w, h) = (pm.width() as i32, pm.height() as i32);
    let x0 = (r.0.floor() as i32).clamp(0, w);
    let y0 = (r.1.floor() as i32).clamp(0, h);
    let x1 = ((r.0 + r.2).ceil() as i32).clamp(0, w);
    let y1 = ((r.1 + r.3).ceil() as i32).clamp(0, h);
    if x1 <= x0 {
        return;
    }
    let d = pm.data_mut();
    for y in y0..y1 {
        let a = ((y * w + x0) * 4) as usize;
        let b = ((y * w + x1) * 4) as usize;
        d[a..b].fill(0);
    }
}

/// Liane suspendue du haut (y = top) jusqu'au sol, déroulée à `grow` (0..1), qui ondule.
#[allow(clippy::too_many_arguments)]
pub fn vine(pm: &mut Pixmap, x: f32, top: f32, bottom: f32, grow: f32, s: f32, time: f32) {
    if grow <= 0.01 {
        return;
    }
    let end = top + (bottom - top) * grow.clamp(0.0, 1.0);
    let id = Transform::identity();
    let sway = |y: f32| ((y - top) / (bottom - top).max(1.0)).powf(1.4) * 6.0 * s * (time * 1.3 + y * 0.004).sin();
    let mut pb = PathBuilder::new();
    pb.move_to(x, top);
    let n = 24;
    for i in 1..=n {
        let y = top + (end - top) * i as f32 / n as f32;
        pb.line_to(x + sway(y), y);
    }
    if let Some(path) = pb.finish() {
        let st = Stroke { width: 5.5 * s, line_cap: LineCap::Round, line_join: tiny_skia::LineJoin::Round, ..Default::default() };
        pm.stroke_path(&path, &paint((74, 92, 50)), &st, id, None);
        let hl = Stroke { width: 1.6 * s, line_cap: LineCap::Round, ..Default::default() };
        pm.stroke_path(&path, &paint((120, 140, 80)), &hl, Transform::from_translate(-1.2 * s, 0.0), None);
    }
    // Feuilles alternées.
    let mut y = top + 40.0 * s;
    let mut side = 1.0;
    while y < end - 10.0 * s {
        let cx = x + sway(y);
        let lt = Transform::from_rotate_at(side * 35.0, cx, y);
        oval(pm, lt, cx + side * 8.0 * s, y, 8.0 * s, 3.6 * s, (88, 140, 70));
        y += 46.0 * s;
        side = -side;
    }
}

