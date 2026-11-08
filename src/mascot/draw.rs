//! Dessin vectoriel du maki catta et des bulles (tiny-skia + ab_glyph).
//! Repère du personnage : origine entre les pattes au sol, y vers le bas, regard à droite.

use ab_glyph::{Font, FontVec, PxScale, ScaleFont, point};
use tiny_skia::{
    Color, FillRule, LineCap, Paint, PathBuilder, Pixmap, PremultipliedColorU8, Rect, Stroke, Transform,
};

use super::behavior::Pose;

const FUR: (u8, u8, u8) = (150, 148, 146);
const FUR_DARK: (u8, u8, u8) = (104, 102, 102);
const CREAM: (u8, u8, u8) = (244, 241, 233);
const BLACK: (u8, u8, u8) = (34, 32, 32);
const EYE: (u8, u8, u8) = (246, 166, 35);

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

/// La fameuse queue annelée noir et blanc.
fn tail(pm: &mut Pixmap, t: Transform, ctrl: [(f32, f32); 4]) {
    let n = 26;
    let mut prev = bezier(ctrl, 0.0);
    for i in 1..=n {
        let u = i as f32 / n as f32;
        let cur = bezier(ctrl, u);
        let ring = (i - 1) / 2;
        let col = if i > n - 2 || ring % 2 == 1 { BLACK } else { CREAM };
        let w = 10.0 - 3.5 * u;
        line(pm, t, prev, cur, w, col);
        prev = cur;
    }
}

fn leg(pm: &mut Pixmap, t: Transform, hip: (f32, f32), phase: f32, swing: f32, c: (u8, u8, u8)) {
    let fx = hip.0 + phase.sin() * swing;
    let lift = (phase.cos()).max(0.0) * swing * 0.5;
    let knee = ((hip.0 + fx) / 2.0 + 2.0, hip.1 / 2.0 - 2.0);
    line(pm, t, hip, knee, 7.0, c);
    line(pm, t, knee, (fx, -lift - 2.0), 6.0, c);
    oval(pm, t, fx + 2.0, -lift - 1.5, 4.5, 2.6, BLACK);
}

fn head(pm: &mut Pixmap, t: Transform, hc: (f32, f32), blink: bool, expr: super::behavior::Expr) {
    use super::behavior::Expr;
    let (x, y) = hc;
    // Oreilles : dressées, ou couchées en arrière quand il s'inquiète.
    let (ex, ey) = if expr == Expr::Worried { (-6.0, 7.0) } else { (0.0, 0.0) };
    poly(pm, t, &[(x - 11.0, y - 6.0), (x - 8.0 + ex, y - 23.0 + ey), (x + 1.0, y - 11.0)], FUR_DARK);
    poly(pm, t, &[(x - 8.5, y - 9.0), (x - 7.5 + ex * 0.8, y - 18.5 + ey * 0.8), (x - 2.0, y - 11.0)], CREAM);
    oval(pm, t, x, y, 14.0, 13.5, FUR);
    // Visage clair et museau sombre.
    oval(pm, t, x + 4.0, y + 3.0, 11.5, 10.0, CREAM);
    poly(pm, t, &[(x + 6.0, y - 1.0), (x + 22.0, y + 5.0), (x + 20.0, y + 8.5), (x + 6.0, y + 10.0)], BLACK);
    oval(pm, t, x + 21.5, y + 5.5, 2.4, 2.2, (12, 12, 12));
    if expr == Expr::Happy {
        // Petit sourire au coin du museau.
        let mut pb = PathBuilder::new();
        pb.move_to(x + 11.0, y + 9.0);
        pb.quad_to(x + 14.0, y + 11.5, x + 17.0, y + 9.5);
        if let Some(path) = pb.finish() {
            pm.stroke_path(&path, &paint(CREAM), &Stroke { width: 1.2, line_cap: LineCap::Round, ..Default::default() }, t, None);
        }
    }
    // Masque noir autour de l'œil orange.
    oval(pm, t, x + 5.5, y - 1.0, 6.0, 5.4, BLACK);
    let stroke = |pm: &mut Pixmap, pts: &[(f32, f32)], c: (u8, u8, u8), w: f32| {
        let mut pb = PathBuilder::new();
        pb.move_to(pts[0].0, pts[0].1);
        pb.quad_to(pts[1].0, pts[1].1, pts[2].0, pts[2].1);
        if let Some(path) = pb.finish() {
            pm.stroke_path(&path, &paint(c), &Stroke { width: w, line_cap: LineCap::Round, ..Default::default() }, t, None);
        }
    };
    if blink {
        line(pm, t, (x + 2.5, y - 0.5), (x + 8.5, y - 0.5), 1.6, CREAM);
    } else if expr == Expr::Happy {
        // Œil plissé « ^ ».
        stroke(pm, &[(x + 2.6, y + 0.6), (x + 5.8, y - 4.6), (x + 9.0, y + 0.6)], EYE, 1.9);
    } else {
        let r = if expr == Expr::Surprised { 4.3 } else { 3.6 };
        let pr = if expr == Expr::Surprised { 1.3 } else { 1.7 };
        oval(pm, t, x + 6.0, y - 1.0, r, r, EYE);
        oval(pm, t, x + 6.6, y - 1.0, pr, pr + 0.2, (10, 10, 10));
        oval(pm, t, x + 7.4, y - 2.2, 0.8, 0.8, (255, 255, 255));
    }
    if expr == Expr::Worried {
        // Sourcil relevé vers le milieu.
        line(pm, t, (x + 1.0, y - 8.6), (x + 9.5, y - 6.2), 1.8, (40, 36, 36));
    }
}

fn rot(deg: f32, x: f32, y: f32) -> Transform {
    Transform::from_rotate_at(deg, x, y)
}

fn scale_at(sx: f32, sy: f32, x: f32, y: f32) -> Transform {
    Transform::from_translate(x, y).pre_scale(sx, sy).pre_translate(-x, -y)
}

fn arm(pm: &mut Pixmap, t: Transform, a: (f32, f32), b: (f32, f32), c: (u8, u8, u8)) {
    line(pm, t, a, b, 6.2, c);
    oval(pm, t, b.0, b.1, 4.2, 4.0, BLACK);
}

/// Dessine le maki. `base` place l'origine (entre les pattes) avec l'échelle et la direction.
pub fn maki(pm: &mut Pixmap, base: Transform, p: &Pose) {
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
    match p.kind {
        PoseKind::Sleep => {
            tail(pm, t, [(-26.0, -14.0), (-56.0, 6.0), (24.0, 10.0), (40.0, -2.0)]);
            oval(pm, t, 0.0, -16.0, 30.0, 16.0, FUR);
            oval(pm, t, 4.0, -9.0, 20.0, 7.0, CREAM);
            head(pm, t.pre_concat(rot(18.0, 24.0, -18.0)), (24.0, -18.0), true, super::behavior::Expr::Normal);
        }
        PoseKind::Sit => {
            tail(pm, t, [(-16.0, -10.0), (-52.0, -2.0), (-70.0 + tx * 0.5, -34.0 + ty * 0.5), (-56.0 + tx, -78.0 + ty)]);
            oval(pm, t, -8.0, -15.0, 15.0, 12.0, FUR_DARK);
            oval(pm, t.pre_concat(rot(-58.0, 0.0, -38.0)), 0.0, -38.0, 25.0, 15.0, FUR);
            oval(pm, t, 7.0, -36.0, 7.5, 15.0, CREAM);
            oval(pm, t, 2.0, -2.0, 9.0, 3.2, BLACK);
            line(pm, t, (10.0, -40.0), (13.0, -3.0), 6.5, FUR_DARK);
            oval(pm, t, 15.0, -2.0, 4.5, 2.6, BLACK);
            let busy_arm = p.wave != 0.0 || p.scratch != 0.0 || p.point;
            if !busy_arm {
                line(pm, t, (14.0, -42.0), (19.0, -3.0), 6.5, FUR);
                oval(pm, t, 21.0, -2.0, 4.5, 2.6, BLACK);
            }
            head(pm, t.pre_concat(rot(-p.look_up * 25.0, 12.0, -70.0)), (12.0, -70.0), p.blink, p.expr);
            if p.wave != 0.0 {
                let ang = (-62.0 + p.wave * 24.0f32).to_radians();
                let sh = (14.0, -44.0);
                let elbow = (sh.0 + 14.0, sh.1 - 4.0);
                line(pm, t, sh, elbow, 6.5, FUR);
                arm(pm, t, elbow, (elbow.0 + ang.cos() * 18.0, elbow.1 + ang.sin() * 18.0), FUR);
            } else if p.point {
                // Patte levée bien haut, avec une petite étincelle d'idée.
                let sh = (14.0, -44.0);
                let elbow = (24.0, -58.0);
                line(pm, t, sh, elbow, 6.5, FUR);
                arm(pm, t, elbow, (27.0, -80.0), FUR);
                let k = (p.time * 6.0).sin() * 0.3 + 1.0;
                for a in 0..4 {
                    let ang = a as f32 * std::f32::consts::FRAC_PI_2 + p.time;
                    line(pm, t, (33.0 + ang.cos() * 3.0, -92.0 + ang.sin() * 3.0), (33.0 + ang.cos() * 7.0 * k, -92.0 + ang.sin() * 7.0 * k), 1.6, (250, 200, 60));
                }
            } else if p.scratch != 0.0 {
                // Se gratte la tête.
                let sh = (14.0, -44.0);
                let elbow = (24.0, -64.0);
                line(pm, t, sh, elbow, 6.5, FUR);
                arm(pm, t, elbow, (10.0 + p.scratch * 3.0, -84.0 + p.scratch.abs() * 2.0), FUR);
            }
        }
        PoseKind::Dangle { arms_up } => {
            // Suspendu par les mains (ou la peau du cou) : corps vertical qui se balance.
            let g = -super::behavior::DANGLE;
            t = t.pre_concat(rot(p.swing.to_degrees(), 0.0, g));
            let kick = (p.time * 5.0).sin() * 4.0;
            tail(pm, t, [(-2.0, -24.0), (-14.0 + tx * 0.4, 6.0), (16.0 + tx, 26.0 + ty * 0.3), (2.0 + tx * 1.4, 52.0)]);
            line(pm, t, (-7.0, -26.0), (-10.0 - kick, -3.0), 6.5, FUR_DARK);
            oval(pm, t, -10.0 - kick, -2.0, 4.0, 3.0, BLACK);
            oval(pm, t, 0.0, -44.0, 16.0, 25.0, FUR);
            oval(pm, t, 4.0, -42.0, 8.0, 17.0, CREAM);
            line(pm, t, (7.0, -26.0), (10.0 + kick, -3.0), 6.5, FUR);
            oval(pm, t, 10.0 + kick, -2.0, 4.0, 3.0, BLACK);
            head(pm, t, (3.0, -74.0), p.blink, p.expr);
            if arms_up {
                arm(pm, t, (-9.0, -60.0), (-7.0, g + 2.0), FUR_DARK);
                arm(pm, t, (11.0, -60.0), (9.0, g + 2.0), FUR);
            } else {
                arm(pm, t, (-10.0, -58.0), (-15.0 + kick, -36.0), FUR_DARK);
                arm(pm, t, (12.0, -58.0), (17.0 - kick, -36.0), FUR);
            }
        }
        PoseKind::Stretch => {
            // Étirement : avant bas, arrière haut, pattes avant tendues.
            let k = (p.walk_phase * std::f32::consts::PI).sin();
            tail(pm, t, [(-24.0, -44.0 - k * 6.0), (-50.0, -60.0), (-56.0 + tx, -104.0), (-40.0 + tx, -122.0)]);
            line(pm, t, (-15.0, -36.0 - k * 4.0), (-17.0, -2.0), 7.0, FUR_DARK);
            let bt = t.pre_concat(rot(k * 16.0, 0.0, -38.0));
            oval(pm, bt, 0.0, -40.0, 28.0 + k * 4.0, 14.0, FUR);
            oval(pm, bt, 2.0, -31.0, 19.0, 5.0, CREAM);
            line(pm, t, (18.0, -36.0 + k * 10.0), (24.0 + k * 22.0, -2.0), 6.5, FUR);
            oval(pm, t, 26.0 + k * 22.0, -2.0, 4.5, 2.6, BLACK);
            line(pm, t, (-12.0, -34.0 - k * 4.0), (-12.0, -2.0), 7.0, FUR);
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
            tail(pm, t, [(-24.0, -42.0), (-52.0 + tx * 0.3, -48.0 + ty * 0.4), (-64.0 + tx * 0.7, -92.0 + ty * 0.7), (-46.0 + tx, -114.0 + ty)]);
            leg(pm, t, (-15.0, -34.0), f2, sw, FUR_DARK);
            leg(pm, t, (17.0, -34.0), f1, sw, FUR_DARK);
            let bob = if moving { (ph * 2.0).sin().abs() * 1.5 } else { 0.0 };
            oval(pm, t, 0.0, -40.0 - bob, 27.0, 15.0, FUR);
            oval(pm, t, 2.0, -31.0 - bob, 19.0, 6.0, CREAM);
            leg(pm, t, (-12.0, -32.0), f1, sw, FUR);
            leg(pm, t, (20.0, -32.0), f2, sw, FUR);
            head(pm, t, (31.0, -55.0 - bob), p.blink, p.expr);
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
pub fn puffs(pm: &mut Pixmap, origin: (f32, f32), list: &[super::behavior::Puff]) {
    for p in list {
        let k = (p.life / p.max).clamp(0.0, 1.0);
        let t = Transform::from_translate(-origin.0, -origin.1);
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

/// Bulle de dialogue au-dessus du point (ax, ay). Renvoie son rectangle (x, y, w, h).
pub fn bubble(pm: &mut Pixmap, text: &Text, msg: &str, ax: f32, ay: f32, scale: f32) -> (f32, f32, f32, f32) {
    let size = 14.5 * scale;
    let pad = 10.0 * scale;
    let max_w = 280.0 * scale;
    let mut lines = text.wrap(size, msg, max_w);
    lines.truncate(5);
    let lh = text.line_height(size);
    let w = lines.iter().map(|l| text.width(size, l)).fold(0.0, f32::max) + pad * 2.0;
    let h = lh * lines.len() as f32 + pad * 2.0;
    let x = (ax - w * 0.35).clamp(4.0, pm.width() as f32 - w - 4.0);
    let y = (ay - h - 12.0 * scale).max(2.0);
    let shadow = Rect::from_xywh(x + 2.0, y + 3.0, w, h).unwrap();
    let rect = Rect::from_xywh(x, y, w, h).unwrap();
    let mut sp = Paint::default();
    sp.set_color(Color::from_rgba8(0, 0, 0, 40));
    sp.anti_alias = true;
    let rr = |r: Rect| {
        let rad = 12.0 * scale;
        let mut pb = PathBuilder::new();
        let (l, t, rr, b) = (r.left(), r.top(), r.right(), r.bottom());
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
        pb.finish().unwrap()
    };
    pm.fill_path(&rr(shadow), &sp, FillRule::Winding, Transform::identity(), None);
    let bg = paint((255, 253, 247));
    pm.fill_path(&rr(rect), &bg, FillRule::Winding, Transform::identity(), None);
    let mut border = paint((205, 196, 180));
    border.anti_alias = true;
    pm.stroke_path(&rr(rect), &border, &Stroke { width: 1.2, ..Default::default() }, Transform::identity(), None);
    // Pointe vers la tête.
    let tip_x = ax.clamp(x + 16.0, x + w - 16.0);
    poly(pm, Transform::identity(), &[(tip_x - 8.0, y + h - 1.0), (ax, ay - 2.0), (tip_x + 6.0, y + h - 1.0)], (255, 253, 247));
    for (i, l) in lines.iter().enumerate() {
        text.draw(pm, size, x + pad, y + pad + i as f32 * lh, l, (45, 40, 36));
    }
    (x, y, w, h)
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
    pub copy: Vec<(R, String)>,
}

/// Lignes à afficher : (texte, couleur, aligné à droite, taille relative, texte à copier).
fn chat_lines(text: &Text, chat: &crate::chat::Chat, size: f32, w: f32) -> Vec<(String, (u8, u8, u8), bool, f32, Option<String>)> {
    use crate::chat::Entry;
    let mut out = vec![];
    for e in &chat.entries {
        let (t, c, right, k, copy) = match e {
            Entry::User(t) => (t.as_str(), (36, 84, 160), true, 1.0, None),
            Entry::Maki(t) => (t.as_str(), (44, 38, 34), false, 1.0, Some(t.clone())),
            Entry::Info(t) => (t.as_str(), (130, 124, 116), false, 0.85, None),
            Entry::Error(t) => (t.as_str(), (176, 48, 40), false, 0.85, None),
        };
        let clean: String = t.replace("**", "").replace('`', "");
        for l in text.wrap(size * k, &clean, if right { w * 0.85 } else { w }) {
            out.push((l, c, right, k, copy.clone()));
        }
        out.push((String::new(), c, false, 0.45, None));
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
    let shown = &lines[start..end];
    let hist_h: f32 = shown.iter().map(|l| lh * l.3).sum();

    let confirm_lines = chat.confirm.as_ref().map(|q| text.wrap(size * 0.95, q, inner - 16.0 * s)).unwrap_or_default();
    let confirm_h = if confirm_lines.is_empty() { 0.0 } else { confirm_lines.len() as f32 * lh + 48.0 * s };
    let header_h = 24.0 * s;
    let input_h = 34.0 * s;
    let h = pad + header_h + hist_h + confirm_h + 8.0 * s + input_h + pad;
    let x = bx;
    let y = by;
    let _ = (y, h);
    let rect = (x, if above { by - h } else { by }, w, h);
    lay.rect = rect;
    let (x, y) = (rect.0, rect.1);

    // Ombre, fond, pointe.
    rrect(pm, (x + 2.0, y + 4.0, w, h), 16.0 * s, &paint_a((0, 0, 0), 0.18), None);
    let bg = paint((255, 252, 246));
    rrect(pm, rect, 16.0 * s, &bg, Some(((205, 194, 176), 1.2)));
    let tip_x = ax.clamp(x + 24.0 * s, x + w - 24.0 * s);
    if above {
        poly(pm, Transform::identity(), &[(tip_x - 10.0 * s, y + h - 1.0), (ax, ay - 4.0 * s), (tip_x + 8.0 * s, y + h - 1.0)], (255, 252, 246));
    } else {
        poly(pm, Transform::identity(), &[(tip_x - 10.0 * s, y + 1.0), (ax, ay + 4.0 * s), (tip_x + 8.0 * s, y + 1.0)], (255, 252, 246));
    }

    // En-tête.
    text.draw(pm, size * 0.9, x + pad, y + pad * 0.6, "L Katta", (120, 96, 60));
    if !chat.engine.is_empty() {
        let e: String = chat.engine.chars().take(40).collect();
        let ew = text.width(size * 0.7, &e);
        text.draw(pm, size * 0.7, x + w - pad - 26.0 * s - ew, y + pad * 0.75, &e, (160, 150, 136));
    }
    lay.close = (x + w - pad - 18.0 * s, y + pad * 0.5, 18.0 * s, 18.0 * s);
    text.draw(pm, size * 1.1, lay.close.0 + 3.0 * s, lay.close.1 - 2.0 * s, "×", (150, 120, 100));

    // Historique.
    let mut cy = y + pad + header_h;
    let mut copy_rect: Option<(R, String)> = None;
    for (l, c, right, k, copy) in shown {
        let sz = size * k;
        let lx = if *right { x + w - pad - text.width(sz, l) } else { x + pad };
        text.draw(pm, sz, lx, cy, l, *c);
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
    if start > 0 {
        text.draw(pm, size * 0.75, x + w / 2.0 - 8.0 * s, y + pad + header_h - 14.0 * s, "▲", (190, 180, 165));
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
    rrect(pm, input, 10.0 * s, &paint((246, 241, 230)), Some(((214, 204, 186), 1.0)));
    let ty = input.1 + (input_h - lh) / 2.0;
    if chat.busy && chat.confirm.is_none() {
        let dots = ".".repeat(1 + ((time * 3.0) as usize % 3));
        text.draw(pm, size, input.0 + 10.0 * s, ty, &format!("Je réfléchis{dots}"), (150, 140, 126));
    } else if chat.input.is_empty() {
        let hint = if focused { "Écris ici… (Entrée : envoyer, Échap : fermer)" } else { "Clique ici pour écrire…" };
        text.draw(pm, size, input.0 + 10.0 * s, ty, hint, (172, 162, 148));
        if caret {
            line(pm, Transform::identity(), (input.0 + 9.0 * s, ty + 2.0), (input.0 + 9.0 * s, ty + lh - 2.0), 1.4, (60, 50, 40));
        }
    } else {
        // Fait défiler la fin du texte si trop long.
        let maxw = inner - 20.0 * s;
        let mut shown_in: String = chat.input.clone();
        while text.width(size, &shown_in) > maxw && !shown_in.is_empty() {
            shown_in.remove(0);
        }
        text.draw(pm, size, input.0 + 10.0 * s, ty, &shown_in, (40, 34, 30));
        if caret {
            let cx = input.0 + 10.0 * s + text.width(size, &shown_in) + 1.0;
            line(pm, Transform::identity(), (cx, ty + 2.0), (cx, ty + lh - 2.0), 1.4, (60, 50, 40));
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
    let hist: f32 = lines[lines.len() - n..].iter().map(|l| lh * l.3).sum();
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
                pm.fill_path(&path, &paint((255, 251, 244)), FillRule::Winding, Transform::identity(), None);
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
            tiny_skia::GradientStop::new(0.0, Color::from_rgba8(255, 254, 250, 255)),
            tiny_skia::GradientStop::new(1.0, Color::from_rgba8(252, 246, 236, 255)),
        ],
        tiny_skia::SpreadMode::Pad,
        Transform::identity(),
    );
    let mut bg = Paint::default();
    bg.anti_alias = true;
    if let Some(g) = grad {
        bg.shader = g;
    } else {
        bg.set_color_rgba8(255, 251, 244, 255);
    }
    tail(pm, true);
    rrect(pm, rect, 16.0 * s, &bg, None);
    rrect(pm, rect, 16.0 * s, &paint_a((0, 0, 0), 0.0), Some(((acc.0 / 2 + 110, acc.1 / 2 + 110, acc.2 / 2 + 110), 1.3)));
    tail(pm, true);
    tail(pm, false);
    // Recouvre le liseré à la base de la pointe.
    line(pm, Transform::identity(), (tip_x - 9.0 * s, bottom - 1.2), (tip_x + 7.0 * s, bottom - 1.2), 2.6, (252, 246, 236));

    icon(pm, bold, msg.kind, x + pad + ir, y + pad + ir, ir);
    let close = (x + w - 22.0 * s, y + 6.0 * s, 16.0 * s, 16.0 * s);
    text.draw(pm, 15.0 * s, close.0 + 3.0 * s, close.1 - 3.0 * s, "×", (180, 168, 152));

    // Texte qui s'écrit.
    let mut budget = b.revealed();
    let tx = x + pad + ir * 2.0 + gap;
    let mut cy = y + pad;
    let take = |l: &str, budget: &mut usize| -> String {
        let n = l.chars().count().min(*budget);
        *budget -= n;
        l.chars().take(n).collect()
    };
    let dark = (acc.0 / 3 + 30, acc.1 / 3 + 30, acc.2 / 3 + 30);
    for l in &title_lines {
        let part = take(l, &mut budget);
        bold.draw(pm, ts, tx, cy, &part, dark);
        cy += tlh;
    }
    for l in &body {
        let part = take(l, &mut budget);
        text.draw(pm, bs, tx, cy, &part, (58, 52, 46));
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
