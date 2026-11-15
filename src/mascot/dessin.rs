//! Le maki dessiné dans le style de l'identité visuelle de L Katta : trait de contour sombre
//! et épais, grosse tête de trois quarts aux deux yeux ronds orangés dans leurs taches
//! sombres, masque clair, ventre clair, queue épaisse (annelée pour le catta).
//!
//! Repère : origine entre les pieds au sol, y vers le bas, regard vers la droite (la
//! direction et l'échelle viennent de la transformation `base`).

use std::f32::consts::{FRAC_PI_2, PI};

use tiny_skia::{FillRule, LineCap, Paint, PathBuilder, Pixmap, Rect, Stroke, Transform};

pub use super::behavior::Attitude;
use super::behavior::{Expr, Pose, PoseKind};
use super::especes::{Oreilles, Pal, Queue};

/// Encre des traits fins (sourcils, bouche, doigt de l'aye-aye).
const INK: (u8, u8, u8) = (52, 40, 46);
/// Demi-épaisseur du contour (unités du personnage).
const OW: f32 = 1.3;

type C = (u8, u8, u8);

fn mix(a: C, b: C, k: f32) -> C {
    let f = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * k).round().clamp(0.0, 255.0) as u8;
    (f(a.0, b.0), f(a.1, b.1), f(a.2, b.2))
}

/// Plus sombre (vers l'encre).
fn ombre(c: C, k: f32) -> C {
    mix(c, (24, 16, 22), k)
}

/// Plus clair (vers un blanc chaud).
fn lumiere(c: C, k: f32) -> C {
    mix(c, (255, 250, 244), k)
}

/// Contour d'une partie : une teinte sombre de sa propre couleur, plus doux qu'un trait noir.
fn bord(c: C) -> C {
    ombre(c, 0.62)
}

fn paint(c: C) -> Paint<'static> {
    paint_a(c, 1.0)
}

fn paint_a(c: C, a: f32) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color_rgba8(c.0, c.1, c.2, (a.clamp(0.0, 1.0) * 255.0) as u8);
    p.anti_alias = true;
    p
}

fn ovale(cx: f32, cy: f32, rx: f32, ry: f32) -> Option<tiny_skia::Path> {
    Rect::from_xywh(cx - rx, cy - ry, rx * 2.0, ry * 2.0).and_then(PathBuilder::from_oval)
}

fn fill_oval(pm: &mut Pixmap, t: Transform, cx: f32, cy: f32, rx: f32, ry: f32, c: C) {
    fill_oval_a(pm, t, cx, cy, rx, ry, c, 1.0);
}

#[allow(clippy::too_many_arguments)]
fn fill_oval_a(pm: &mut Pixmap, t: Transform, cx: f32, cy: f32, rx: f32, ry: f32, c: C, a: f32) {
    if let Some(path) = ovale(cx, cy, rx, ry) {
        pm.fill_path(&path, &paint_a(c, a), FillRule::Winding, t, None);
    }
}

/// Ovale en volume, sans dégradé (les dégradés coûtent cher à dessiner) : ombre en bas à
/// droite, couleur, puis un reflet clair et transparent vers la lumière (en haut à gauche).
#[allow(clippy::too_many_arguments)]
fn volume(pm: &mut Pixmap, t: Transform, cx: f32, cy: f32, rx: f32, ry: f32, c: C, fort: f32) {
    fill_oval(pm, t, cx, cy, rx, ry, ombre(c, 0.16 * fort));
    fill_oval(pm, t, cx - rx * 0.06, cy - ry * 0.08, rx * 0.93, ry * 0.91, c);
    fill_oval_a(pm, t, cx - rx * 0.22, cy - ry * 0.28, rx * 0.6, ry * 0.55, lumiere(c, 0.5), 0.2 * fort);
}

/// Ovale en volume, sans contour (ventre, masque, calotte).
fn douce(pm: &mut Pixmap, t: Transform, cx: f32, cy: f32, rx: f32, ry: f32, c: C) {
    volume(pm, t, cx, cy, rx, ry, c, 0.7);
}

/// Ovale cerné de sa propre teinte sombre, en volume (les petits restent unis : moins de calcul).
fn ov(pm: &mut Pixmap, t: Transform, cx: f32, cy: f32, rx: f32, ry: f32, c: C) {
    fill_oval(pm, t, cx, cy, rx + OW, ry + OW, bord(c));
    if rx.min(ry) < 5.0 {
        fill_oval(pm, t, cx, cy, rx, ry, c);
    } else {
        volume(pm, t, cx, cy, rx, ry, c, 1.0);
    }
}

fn chemin(pts: &[(f32, f32)]) -> Option<tiny_skia::Path> {
    let mut pb = PathBuilder::new();
    pb.move_to(pts[0].0, pts[0].1);
    match pts.len() {
        2 => pb.line_to(pts[1].0, pts[1].1),
        3 => pb.quad_to(pts[1].0, pts[1].1, pts[2].0, pts[2].1),
        _ => pb.cubic_to(pts[1].0, pts[1].1, pts[2].0, pts[2].1, pts[3].0, pts[3].1),
    }
    pb.finish()
}

fn stroke(pm: &mut Pixmap, t: Transform, pts: &[(f32, f32)], w: f32, c: C, cap: LineCap) {
    stroke_a(pm, t, pts, w, c, cap, 1.0);
}

#[allow(clippy::too_many_arguments)]
fn stroke_a(pm: &mut Pixmap, t: Transform, pts: &[(f32, f32)], w: f32, c: C, cap: LineCap, a: f32) {
    if let Some(path) = chemin(pts) {
        pm.stroke_path(&path, &paint_a(c, a), &Stroke { width: w, line_cap: cap, ..Default::default() }, t, None);
    }
}

/// Membre cerné de sa teinte sombre : segment, courbe quadratique ou cubique.
fn lim(pm: &mut Pixmap, t: Transform, pts: &[(f32, f32)], w: f32, c: C) {
    let Some(path) = chemin(pts) else { return };
    let trait_ = |w: f32| Stroke { width: w, line_cap: LineCap::Round, line_join: tiny_skia::LineJoin::Round, ..Default::default() };
    pm.stroke_path(&path, &paint(bord(c)), &trait_(w + 2.0 * OW), t, None);
    pm.stroke_path(&path, &paint(c), &trait_(w), t, None);
}

/// Trait fin d'encre (sourcils, bouche, doigts).
fn ink(pm: &mut Pixmap, t: Transform, pts: &[(f32, f32)], w: f32) {
    stroke(pm, t, pts, w, INK, LineCap::Round);
}

fn poly(pm: &mut Pixmap, t: Transform, pts: &[(f32, f32)], c: C, cerne: bool) {
    let mut pb = PathBuilder::new();
    pb.move_to(pts[0].0, pts[0].1);
    for p in &pts[1..] {
        pb.line_to(p.0, p.1);
    }
    pb.close();
    if let Some(path) = pb.finish() {
        if cerne {
            pm.stroke_path(&path, &paint(bord(c)), &Stroke { width: 2.0 * OW, line_join: tiny_skia::LineJoin::Round, ..Default::default() }, t, None);
        }
        pm.fill_path(&path, &paint(c), FillRule::Winding, t, None);
    }
}

/// Duvet : petits ronds cernés qui se chevauchent (joues, collerette), sans dégradé.
fn duvet(pm: &mut Pixmap, t: Transform, ronds: &[(f32, f32, f32)], c: C) {
    for &(x, y, r) in ronds {
        fill_oval(pm, t, x, y, r + OW, r + OW, bord(c));
    }
    for &(x, y, r) in ronds {
        fill_oval(pm, t, x, y, r, r, c);
    }
}

/// Patte (main ou pied) : petit ovale cerné, deux doigts esquissés et un coussinet.
fn patte(pm: &mut Pixmap, t: Transform, x: f32, y: f32, rx: f32, ry: f32, c: C) {
    ov(pm, t, x, y, rx, ry, c);
    let d = ombre(c, 0.45);
    stroke(pm, t, &[(x + rx * 0.05, y + ry * 0.35), (x + rx * 0.08, y + ry * 0.95)], 0.7, d, LineCap::Round);
    stroke(pm, t, &[(x + rx * 0.5, y + ry * 0.25), (x + rx * 0.55, y + ry * 0.85)], 0.7, d, LineCap::Round);
    fill_oval_a(pm, t, x - rx * 0.35, y - ry * 0.3, rx * 0.35, ry * 0.3, lumiere(c, 0.5), 0.45);
}

/// Main de lémurien au bout d'un bras : paume et quatre longs doigts écartés en éventail,
/// orientés dans l'axe `dir` (radians). Un seul contour pour toute la main.
fn main(pm: &mut Pixmap, t: Transform, x: f32, y: f32, dir: f32, taille: f32, c: C) {
    let ht = t.pre_concat(Transform::from_rotate_at(dir.to_degrees(), x, y));
    let k = taille;
    // Paume (un peu en avant du poignet), puis doigts : pouce écarté, trois doigts serrés.
    let paume = (x + k * 0.45, y, k * 0.85, k * 0.72);
    let doigts = [(-62.0f32, 0.8f32), (-20.0, 1.0), (8.0, 1.05), (34.0, 0.95)];
    let doigt = |a: f32, l: f32| {
        let r = a.to_radians();
        (paume.0 + r.cos() * k * 0.95 * l, paume.1 + r.sin() * k * 0.8, k * 0.48 * l, k * 0.27, a)
    };
    let tous: Vec<(f32, f32, f32, f32, f32)> = doigts.iter().map(|&(a, l)| doigt(a, l)).collect();
    let rond = |pm: &mut Pixmap, (cx, cy, rx, ry, a): (f32, f32, f32, f32, f32), plus: f32, col: C| {
        fill_oval(pm, ht.pre_concat(Transform::from_rotate_at(a, cx, cy)), cx, cy, rx + plus, ry + plus, col);
    };
    let contour = bord(c);
    fill_oval(pm, ht, paume.0, paume.1, paume.2 + OW, paume.3 + OW, contour);
    for d in &tous {
        rond(pm, *d, OW, contour);
    }
    for d in &tous {
        rond(pm, *d, 0.0, c);
    }
    fill_oval(pm, ht, paume.0, paume.1, paume.2, paume.3, c);
    // Coussinet clair de la paume.
    fill_oval_a(pm, ht, paume.0 + k * 0.1, paume.1 + k * 0.1, k * 0.38, k * 0.3, lumiere(c, 0.45), 0.5);
}

/// Bras cerné, coude marqué, terminé par une main dans l'axe de l'avant-bras.
fn bras(pm: &mut Pixmap, t: Transform, pts: &[(f32, f32)], w: f32, c: C) {
    lim(pm, t, pts, w * 0.92, c);
    let (a, b) = (pts[pts.len() - 2], pts[pts.len() - 1]);
    let dir = (b.1 - a.1).atan2(b.0 - a.0);
    main(pm, t, b.0, b.1, dir, w * 0.62, c);
}

fn bez(p: [(f32, f32); 4], u: f32) -> (f32, f32) {
    let v = 1.0 - u;
    let (a, b, c, d) = (v * v * v, 3.0 * v * v * u, 3.0 * v * u * u, u * u * u);
    (a * p[0].0 + b * p[1].0 + c * p[2].0 + d * p[3].0, a * p[0].1 + b * p[1].1 + c * p[2].1 + d * p[3].1)
}

/// Ruban le long des points `pts[a..=b]`, demi-largeur `hw(i)` de chaque côté de la normale.
fn ruban(pts: &[(f32, f32)], nor: &[(f32, f32)], a: usize, b: usize, hw: &dyn Fn(usize) -> f32) -> Option<tiny_skia::Path> {
    let mut pb = PathBuilder::new();
    for i in a..=b {
        let (p, n, h) = (pts[i], nor[i], hw(i));
        if i == a {
            pb.move_to(p.0 + n.0 * h, p.1 + n.1 * h);
        } else {
            pb.line_to(p.0 + n.0 * h, p.1 + n.1 * h);
        }
    }
    for i in (a..=b).rev() {
        let (p, n, h) = (pts[i], nor[i], hw(i));
        pb.line_to(p.0 - n.0 * h, p.1 - n.1 * h);
    }
    pb.close();
    pb.finish()
}

/// Queue épaisse : un ruban rempli (quelques remplissages au lieu de dizaines de traits),
/// anneaux ou bout coloré, bouts arrondis.
fn queue(pm: &mut Pixmap, t: Transform, pal: &Pal, ctrl: [(f32, f32); 4]) {
    let k = pal.tail_len;
    let o = ctrl[0];
    let ctrl = ctrl.map(|p| (o.0 + (p.0 - o.0) * k, o.1 + (p.1 - o.1) * k));
    let n = if k < 0.3 { 6 } else { 28 };
    let w0 = pal.tail_w * 1.25;
    let largeur = |u: f32| if pal.tail_w > 11.0 { w0 * (0.75 + 0.45 * (u * PI).sin()) } else { w0 * (1.0 - 0.18 * u) };
    let pts: Vec<(f32, f32)> = (0..=n).map(|i| bez(ctrl, i as f32 / n as f32)).collect();
    let nor: Vec<(f32, f32)> = (0..=n)
        .map(|i: usize| {
            let (a, b) = (pts[i.saturating_sub(1)], pts[(i + 1).min(n)]);
            let (dx, dy) = (b.0 - a.0, b.1 - a.1);
            let l = (dx * dx + dy * dy).sqrt().max(1e-3);
            (-dy / l, dx / l)
        })
        .collect();
    let hw = |i: usize| largeur(i as f32 / n as f32) / 2.0;
    let (base, autre) = match pal.queue {
        Queue::Annelee(a, b) => (a, b),
        Queue::Unie(a, tip) => (a, tip),
    };
    let fonce = if base.0 as u32 + base.1 as u32 + base.2 as u32 > autre.0 as u32 + autre.1 as u32 + autre.2 as u32 { autre } else { base };
    let disque = |pm: &mut Pixmap, i: usize, r: f32, c: C| fill_oval(pm, t, pts[i].0, pts[i].1, r, r, c);
    // Contour d'un seul tenant, bouts arrondis.
    if let Some(p) = ruban(&pts, &nor, 0, n, &|i| hw(i) + OW) {
        pm.fill_path(&p, &paint(bord(fonce)), FillRule::Winding, t, None);
    }
    disque(pm, 0, hw(0) + OW, bord(fonce));
    disque(pm, n, hw(n) + OW, bord(fonce));
    if let Some(p) = ruban(&pts, &nor, 0, n, &hw) {
        pm.fill_path(&p, &paint(base), FillRule::Winding, t, None);
    }
    disque(pm, 0, hw(0), base);
    // Anneaux (ou bout) : les tronçons de même couleur forment un seul morceau.
    let teinte = |i: usize| match pal.queue {
        Queue::Annelee(..) => i > n - 2 || ((i - 1) / 2) % 2 == 1,
        Queue::Unie(..) => i as f32 / n as f32 > 0.85,
    };
    let mut i = 1;
    while i <= n {
        if teinte(i) {
            let debut = i - 1;
            while i < n && teinte(i + 1) {
                i += 1;
            }
            if let Some(p) = ruban(&pts, &nor, debut, i, &|j| hw(j) + 0.25) {
                pm.fill_path(&p, &paint(autre), FillRule::Winding, t, None);
            }
        }
        i += 1;
    }
    disque(pm, n, hw(n), if teinte(n) { autre } else { base });
    let _ = w0;
}

/// Un œil de trois quarts : tache sombre, iris lumineux en dégradé, grande pupille, deux reflets.
#[allow(clippy::too_many_arguments)]
fn oeil(pm: &mut Pixmap, t: Transform, pal: &Pal, x: f32, y: f32, r: f32, tache: bool, blink: bool, expr: Expr, sleepy: bool, regard: (f32, f32)) {
    if tache && !pal.face_dark {
        // Tache sombre en goutte, pointe vers le museau, bord adouci.
        let tt = t.pre_concat(Transform::from_rotate_at(-25.0, x, y));
        fill_oval_a(pm, tt, x, y + r * 0.15, r * 1.6, r * 1.45, pal.black, 0.35);
        fill_oval(pm, tt, x, y + r * 0.15, r * 1.45, r * 1.3, pal.black);
    }
    let lid = if pal.face_dark { (90, 84, 80) } else { pal.cream };
    // Trait d'œil fermé : clair sur la tache sombre (sinon invisible).
    let ferme = if pal.face_dark { (200, 192, 184) } else { pal.cream };
    if blink {
        stroke(pm, t, &[(x - r * 0.9, y), (x, y + r * 0.5), (x + r * 0.9, y)], 1.6, ferme, LineCap::Round);
        return;
    }
    if matches!(expr, Expr::Happy) {
        // Yeux plissés de contentement : arcs.
        stroke(pm, t, &[(x - r * 0.9, y + r * 0.35), (x, y - r * 0.7), (x + r * 0.9, y + r * 0.35)], 1.9, ferme, LineCap::Round);
        return;
    }
    let ri = if matches!(expr, Expr::Surprised) { r * 1.12 } else { r };
    fill_oval(pm, t, x, y, ri + 0.75, ri + 0.75, ombre(pal.eye, 0.8));
    // Iris : bord sombre, couleur, lueur claire en bas (la lumière traverse l'œil).
    fill_oval(pm, t, x, y, ri, ri, ombre(pal.eye, 0.4));
    fill_oval(pm, t, x, y + ri * 0.1, ri * 0.82, ri * 0.82, pal.eye);
    fill_oval_a(pm, t, x, y + ri * 0.45, ri * 0.55, ri * 0.3, lumiere(pal.eye, 0.5), 0.75);
    let pr = if matches!(expr, Expr::Surprised) { ri * 0.34 } else { ri * 0.5 };
    let (px, py) = (x + regard.0 * ri * 0.3, y + regard.1 * ri * 0.28);
    fill_oval(pm, t, px, py, pr, pr * 1.05, (14, 10, 12));
    // Deux reflets : le grand (fenêtre) et un petit en bas, qui rendent l'œil « vivant ».
    fill_oval(pm, t, px + pr * 0.45, py - pr * 0.55, pr * 0.5, pr * 0.46, (255, 255, 255));
    fill_oval_a(pm, t, x - ri * 0.42, y + ri * 0.45, ri * 0.16, ri * 0.14, (255, 255, 255), 0.85);
    if sleepy && !matches!(expr, Expr::Surprised) {
        // Espèce nocturne en plein jour : paupière lourde.
        if let Some(rc) = Rect::from_xywh(x - ri - 1.0, y - ri - 1.0, ri * 2.0 + 2.0, ri * 0.95) {
            pm.fill_path(&PathBuilder::from_rect(rc), &paint(pal.black), FillRule::Winding, t, None);
        }
        stroke(pm, t, &[(x - ri, y - 0.05 * ri), (x + ri, y - 0.05 * ri)], 1.2, lid, LineCap::Round);
    }
}

/// Tête de trois quarts, regard vers la droite. `(x, y)` : centre du crâne.
#[allow(clippy::too_many_arguments)]
fn tete(pm: &mut Pixmap, t: Transform, pal: &Pal, x: f32, y: f32, blink: bool, expr: Expr, att: Attitude, sleepy: bool, time: f32, vu: Option<(f32, f32)>, baille: f32) {
    let blink = blink || baille > 0.25;
    // Tête un peu plus grosse : proportions de bébé, plus attendrissantes.
    let t = t.pre_translate(x, y).pre_scale(1.08, 1.08).pre_translate(-x, -y);
    let r = 16.5;
    // Collerette (indri, vari).
    if pal.ruff {
        let ruff = if pal.fur_dark.0 < 60 && pal.fur.0 > 200 { pal.cream } else { pal.fur_dark };
        let ronds: Vec<(f32, f32, f32)> = (0..10)
            .map(|k| {
                let a = k as f32 * PI / 5.0;
                (x - 1.0 + a.cos() * (r - 0.5), y + 3.0 + a.sin() * (r - 2.5), 6.8)
            })
            .collect();
        duvet(pm, t, &ronds, ruff);
    }
    // Oreilles (couchées en arrière quand il s'inquiète ou boude).
    let couche = matches!(expr, Expr::Worried) || att == Attitude::Boude;
    let inner = pal.fur_dark;
    let bord_o = if pal.ears == Oreilles::Pointues && !pal.face_dark { pal.cream } else { pal.ear_color };
    for (ex, ang, echelle) in [(-11.0f32, -28.0f32, 0.9f32), (8.0, 22.0, 1.0)] {
        let a = if couche { ang - 35.0 * ex.signum() } else { ang };
        let et = t.pre_concat(Transform::from_rotate_at(a, x + ex, y - r * 0.55));
        match pal.ears {
            Oreilles::Pointues => {
                let (h, l) = (13.0 * echelle, 7.5 * echelle);
                poly(pm, et, &[(x + ex - l, y - r * 0.4), (x + ex - l * 0.35, y - r * 0.55 - h), (x + ex + l * 0.35, y - r * 0.55 - h), (x + ex + l, y - r * 0.4)], bord_o, true);
                poly(pm, et, &[(x + ex - l * 0.55, y - r * 0.45), (x + ex - l * 0.2, y - r * 0.5 - h * 0.75), (x + ex + l * 0.2, y - r * 0.5 - h * 0.75), (x + ex + l * 0.55, y - r * 0.45)], inner, false);
            }
            Oreilles::Rondes => {
                ov(pm, et, x + ex, y - r * 0.75, 6.5 * echelle, 6.0 * echelle, pal.ear_color);
                fill_oval(pm, et, x + ex, y - r * 0.72, 3.6 * echelle, 3.2 * echelle, pal.fur_dark);
            }
            Oreilles::Grandes => {
                ov(pm, et, x + ex * 1.15, y - r * 1.0, 10.0 * echelle, 13.5 * echelle, pal.ear_color);
                // Intérieur rosé, un peu translucide, comme une vraie oreille de petit lémurien.
                let clair = mix(lumiere(pal.ear_color, 0.25), (226, 150, 150), 0.35);
                douce(pm, et, x + ex * 1.15, y - r * 0.95, 6.0 * echelle, 9.5 * echelle, clair);
            }
        }
    }
    // Favoris : duvet de joue qui dépasse à l'arrière (clair si le masque l'est).
    let face = if pal.face_dark { pal.black } else { pal.cream };
    if !pal.ruff {
        let joue = if pal.face_dark { pal.fur } else { pal.cream };
        duvet(pm, t, &[(x - 14.0, y + 4.0, 4.0), (x - 12.0, y + 9.5, 4.3)], joue);
    }
    // Crâne.
    ov(pm, t, x, y, r, r * 0.92, pal.fur);
    if let Some(cap) = pal.cap {
        douce(pm, t, x - 1.0, y - r * 0.55, r * 0.8, r * 0.42, cap);
    } else if !pal.face_dark {
        // Calotte plus sombre (catta : sommet gris foncé).
        douce(pm, t, x - 1.0, y - r * 0.55, r * 0.72, r * 0.4, pal.fur_dark);
    }
    // Masque du visage.
    douce(pm, t, x + 3.0, y + 3.0, r * 0.86, r * 0.72, face);
    // Joues roses, très légères.
    let rose = (238, 132, 146);
    let a_rose = if pal.face_dark { 0.22 } else { 0.32 };
    fill_oval_a(pm, t, x + 14.0, y + 7.5, 3.4, 2.2, rose, a_rose);
    fill_oval_a(pm, t, x - 4.5, y + 7.0, 2.4, 1.8, rose, a_rose * 0.7);
    // Yeux : le lointain (plus petit) puis le proche, plus grands qu'avant.
    let er = 4.4 * pal.eye_r * 1.12;
    let regard = if att == Attitude::Boude { (-0.6, 0.2) } else { vu.map(|(a, b)| (a.clamp(-1.0, 1.0), b.clamp(-1.0, 1.0))).unwrap_or((0.5, 0.0)) };
    oeil(pm, t, pal, x - 2.5, y + 0.5, er * 0.88, true, blink, expr, sleepy, regard);
    oeil(pm, t, pal, x + 9.5, y + 0.5, er, true, blink, expr, sleepy, regard);
    // Sourcils : froncés (boude), inquiets.
    let brow = if pal.face_dark { (210, 204, 196) } else { INK };
    if att == Attitude::Boude {
        stroke(pm, t, &[(x - 7.0, y - 7.5), (x + 1.0, y - 4.8)], 1.8, brow, LineCap::Round);
        stroke(pm, t, &[(x + 5.0, y - 4.8), (x + 14.0, y - 7.8)], 1.8, brow, LineCap::Round);
    } else if matches!(expr, Expr::Worried) {
        stroke(pm, t, &[(x - 7.0, y - 5.0), (x + 0.5, y - 7.5)], 1.6, brow, LineCap::Round);
        stroke(pm, t, &[(x + 6.0, y - 7.5), (x + 13.5, y - 5.0)], 1.6, brow, LineCap::Round);
    }
    // Museau et truffe arrondie, avec un reflet humide.
    let m = pal.muzzle;
    let (nx, ny) = (x + 4.0 + 3.5 * m, y + 7.5 + 2.0 * m);
    if !pal.face_dark {
        poly(pm, t, &[(x + 2.0, y + 2.0), (x + 6.0, y + 2.0), (nx + 2.5, ny - 1.0), (nx - 2.5, ny - 1.0)], mix(pal.fur_dark, pal.cream, 0.15), false);
    }
    let (w, h) = (3.3, 2.6);
    let mut pb = PathBuilder::new();
    pb.move_to(nx - w, ny - h * 0.35);
    pb.quad_to(nx, ny - h * 1.15, nx + w, ny - h * 0.35);
    pb.quad_to(nx + w * 0.7, ny + h * 0.55, nx, ny + h * 0.7);
    pb.quad_to(nx - w * 0.7, ny + h * 0.55, nx - w, ny - h * 0.35);
    pb.close();
    if let Some(path) = pb.finish() {
        pm.fill_path(&path, &paint((30, 22, 26)), FillRule::Winding, t, None);
    }
    fill_oval_a(pm, t, nx - 0.6, ny - h * 0.45, 1.3, 0.6, (200, 190, 196), 0.8);
    // Bouche.
    let ouverte = matches!(att, Attitude::Parle | Attitude::Danse | Attitude::Fier) || matches!(expr, Expr::Happy | Expr::Surprised);
    let mouth = if pal.face_dark { (200, 190, 182) } else { INK };
    if baille > 0.05 {
        // Bâillement : grande bouche ronde, langue, petites dents.
        let (mx, my) = (nx - 0.5, ny + 3.0);
        let (rx, ry) = (2.4 + 2.0 * baille, 1.0 + 3.4 * baille);
        fill_oval(pm, t, mx, my + ry, rx + 0.9, ry + 0.9, mouth);
        fill_oval(pm, t, mx, my + ry, rx, ry, (110, 36, 46));
        fill_oval(pm, t, mx, my + ry * 1.55, rx * 0.65, ry * 0.4, (236, 120, 130));
        for dx in [-0.55f32, 0.55] {
            poly(pm, t, &[(mx + dx * rx - 0.7, my + 0.3), (mx + dx * rx + 0.7, my + 0.3), (mx + dx * rx, my + 1.8)], (250, 248, 244), false);
        }
    } else if att == Attitude::Boude {
        // Dents serrées.
        let (mx, my) = (nx - 1.0, ny + 4.0);
        if let Some(rc) = Rect::from_xywh(mx - 3.5, my - 1.4, 7.0, 2.8) {
            pm.fill_path(&PathBuilder::from_rect(rc), &paint((250, 248, 244)), FillRule::Winding, t, None);
        }
        stroke(pm, t, &[(mx - 3.5, my - 1.4), (mx + 3.5, my - 1.4)], 1.0, mouth, LineCap::Round);
        stroke(pm, t, &[(mx - 3.5, my + 1.4), (mx + 3.5, my + 1.4)], 1.0, mouth, LineCap::Round);
        stroke(pm, t, &[(mx, my - 1.4), (mx, my + 1.4)], 0.8, mouth, LineCap::Round);
    } else if ouverte {
        let parle = if att == Attitude::Parle { 0.6 + 0.4 * (time * 9.0).sin().abs() } else { 1.0 };
        let (mx, my) = (nx - 0.5, ny + 3.2);
        let hh = 4.2 * parle;
        // Bouche ronde (« o » ou sourire ouvert) avec la langue.
        let mut pb = PathBuilder::new();
        pb.move_to(mx - 3.8, my);
        pb.quad_to(mx, my - 0.8, mx + 3.8, my);
        pb.quad_to(mx + 2.6, my + hh * 1.15, mx, my + hh);
        pb.quad_to(mx - 2.6, my + hh * 1.15, mx - 3.8, my);
        pb.close();
        if let Some(path) = pb.finish() {
            pm.stroke_path(&path, &paint(mouth), &Stroke { width: 1.0, line_join: tiny_skia::LineJoin::Round, ..Default::default() }, t, None);
            pm.fill_path(&path, &paint((110, 36, 46)), FillRule::Winding, t, None);
        }
        fill_oval(pm, t, mx, my + hh * 0.72, 2.2, 1.2 * parle.max(0.5), (236, 120, 130));
    } else {
        // Petite bouche en ω sous la truffe.
        stroke(pm, t, &[(nx, ny + 1.6), (nx, ny + 3.2)], 0.9, mouth, LineCap::Round);
        stroke(pm, t, &[(nx - 2.8, ny + 2.7), (nx - 1.4, ny + 4.4), (nx, ny + 3.2)], 0.9, mouth, LineCap::Round);
        stroke(pm, t, &[(nx, ny + 3.2), (nx + 1.4, ny + 4.4), (nx + 2.8, ny + 2.7)], 0.9, mouth, LineCap::Round);
    }
}

/// Bras croisés devant la poitrine.
fn bras_croises(pm: &mut Pixmap, t: Transform, pal: &Pal, cx: f32, cy: f32) {
    bras(pm, t, &[(cx - 10.0, cy - 2.0), (cx + 2.0, cy + 6.0), (cx + 12.0, cy + 1.0)], 6.0, pal.fur_dark);
    bras(pm, t, &[(cx + 11.0, cy - 3.0), (cx - 1.0, cy + 3.0), (cx - 10.0, cy + 1.0)], 6.0, pal.fur);
}

/// Dessine le maki. `base` place l'origine (entre les pieds) avec l'échelle et la direction.
pub fn maki(pm: &mut Pixmap, base: Transform, p: &Pose, pal: &Pal, att: Attitude) {
    if p.pop <= 0.01 {
        return;
    }
    let sq = p.squash;
    let s0 = 0.92 * pal.size.clamp(0.7, 1.3);
    let mut t = base
        .pre_concat(Transform::from_scale(s0, s0))
        .pre_concat(Transform::from_translate(0.0, 0.0).pre_scale(1.0 + sq * 0.6, 1.0 - sq))
        .pre_concat(Transform::from_rotate(p.lean.to_degrees()))
        .pre_concat(Transform::from_rotate_at(p.spin.to_degrees(), 0.0, -45.0))
        .pre_concat(Transform::from_translate(0.0, -45.0).pre_scale(p.pop, p.pop).pre_translate(0.0, 45.0));
    let (tx, ty) = p.tail;
    let (fur, dark, cream) = (pal.fur, pal.fur_dark, pal.cream);
    // Debout seulement pour les espèces qui se tiennent debout ; les autres vivent leurs
    // émotions assises (bouche, oreilles, sourcils).
    let debout = matches!(p.kind, PoseKind::Sit) && att != Attitude::Aucune && p.bipede;
    match p.kind {
        _ if debout => {
            // Debout sur deux pattes (il parle, danse, boude ou fait le fier).
            let danse = att == Attitude::Danse;
            let k = if danse { (p.time * 6.0).sin() } else { 0.0 };
            let h = (p.time * 2.0).sin() * 0.8;
            queue(pm, t, pal, [(-6.0, -26.0), (-26.0 + tx * 0.3, -24.0), (-36.0 + tx * 0.6, -52.0 + ty * 0.5), (-22.0 + tx, -66.0 + ty)]);
            // Jambes : la danse en lève une.
            let lever = if danse { 10.0 + 6.0 * k } else { 0.0 };
            lim(pm, t, &[(-5.0, -28.0), (-8.0, -14.0), (-7.0, -3.0 - lever * 0.2)], 6.5, dark);
            patte(pm, t, -5.0, -2.5 - lever * 0.2, 5.0, 2.8, dark);
            lim(pm, t, &[(6.0, -28.0), (9.0 + lever * 0.6, -14.0 - lever * 0.4), (8.0 + lever * 0.2, -3.0 - lever)], 6.5, fur);
            patte(pm, t, 10.0 + lever * 0.2, -2.5 - lever, 5.0, 2.8, fur);
            // Corps et ventre.
            let bt = t.pre_concat(Transform::from_rotate_at(if danse { k * 6.0 } else { 0.0 }, 0.0, -30.0));
            ov(pm, bt, 0.0, -45.0 + h, 13.5, 21.0, fur);
            douce(pm, bt, 4.0, -43.0 + h, 7.5, 16.0, cream);
            let (sx, sy) = (2.0, -58.0 + h);
            match att {
                Attitude::Boude => bras_croises(pm, bt, pal, 2.0, -48.0 + h),
                Attitude::Fier => {
                    // Mains sur les hanches.
                    lim(pm, bt, &[(sx - 9.0, sy), (sx - 20.0, sy + 9.0), (sx - 11.0, sy + 18.0)], 5.5, dark);
                    bras(pm, bt, &[(sx + 9.0, sy), (sx + 21.0, sy + 9.0), (sx + 12.0, sy + 18.0)], 5.5, fur);
                }
                Attitude::Danse => {
                    let a = 0.5 + 0.5 * (p.time * 6.0).sin();
                    bras(pm, bt, &[(sx - 8.0, sy), (sx - 16.0, sy + 10.0), (sx - 8.0, sy + 16.0)], 5.5, dark);
                    bras(pm, bt, &[(sx + 9.0, sy), (sx + 20.0, sy - 6.0 - 6.0 * a), (sx + 26.0, sy - 18.0 - 6.0 * a)], 5.5, fur);
                }
                _ => {
                    // Il parle : une main accompagne ce qu'il dit.
                    let g = (p.time * 2.4).sin();
                    bras(pm, bt, &[(sx - 8.0, sy), (sx - 12.0, sy + 14.0), (sx - 9.0, sy + 24.0)], 5.5, dark);
                    bras(pm, bt, &[(sx + 9.0, sy), (sx + 20.0, sy + 6.0), (sx + 25.0, sy - 2.0 + g * 4.0)], 5.5, fur);
                }
            }
            tete(pm, bt.pre_concat(Transform::from_rotate_at(-p.look_up * 20.0, 2.0, -80.0)), pal, 2.0, -80.0 + h, p.blink, p.expr, att, p.sleepy, p.time, p.regard, p.baille);
        }
        PoseKind::Sleep => {
            // Boule endormie, queue enroulée autour.
            ov(pm, t, 0.0, -15.0, 27.0, 15.0, fur);
            douce(pm, t, 6.0, -9.0, 17.0, 6.0, cream);
            queue(pm, t, pal, [(-24.0, -10.0), (-40.0, 4.0), (10.0, 6.0), (34.0, -2.0)]);
            tete(pm, t.pre_concat(Transform::from_rotate_at(16.0, 22.0, -20.0)), pal, 22.0, -20.0, true, Expr::Normal, Attitude::Aucune, false, p.time, None, 0.0);
        }
        PoseKind::Sit => {
            // Assis bien droit, pieds devant, queue qui revient s'enrouler au sol.
            if pal.tail_up {
                queue(pm, t, pal, [(-12.0, -10.0), (-36.0, -2.0), (-30.0 + tx * 0.4, 6.0), (22.0 + tx * 0.5, 2.0 + ty * 0.2)]);
            } else {
                queue(pm, t, pal, [(-12.0, -10.0), (-40.0, -2.0), (-62.0 + tx * 0.5, 0.0), (-80.0 + tx, -6.0 + ty * 0.3)]);
            }
            // Cuisses, corps en poire, ventre.
            ov(pm, t, -6.0, -14.0, 15.0, 12.0, dark);
            ov(pm, t, 0.0, -36.0, 16.5, 25.0, fur);
            douce(pm, t, 5.0, -33.0, 9.0, 19.0, cream);
            ov(pm, t, 6.0, -12.0, 12.0, 9.5, fur);
            patte(pm, t, 12.0, -2.5, 6.0, 3.0, dark);
            patte(pm, t, 20.0, -2.5, 6.0, 3.0, fur);
            let occupe = p.wave != 0.0 || p.scratch != 0.0 || p.point || p.tap != 0.0;
            // Bras : l'arrière repose, l'avant est libre ou occupé.
            bras(pm, t, &[(-2.0, -48.0), (-5.0, -30.0), (9.0, -22.0)], 5.5, dark);
            let (sx, sy) = (8.0, -48.0);
            if p.wave != 0.0 {
                let ang = (-70.0 + p.wave * 25.0f32).to_radians();
                let coude = (sx + 12.0, sy - 2.0);
                let main = (coude.0 + ang.cos() * 17.0, coude.1 + ang.sin() * 17.0);
                bras(pm, t, &[(sx, sy), coude, main], 5.5, fur);
            } else if p.point {
                bras(pm, t, &[(sx, sy), (sx + 12.0, sy - 12.0), (sx + 14.0, sy - 32.0)], 5.5, fur);
                let k = (p.time * 6.0).sin() * 0.3 + 1.0;
                for a in 0..4 {
                    let ang = a as f32 * FRAC_PI_2 + p.time;
                    stroke(pm, t, &[(sx + 20.0 + ang.cos() * 3.0, sy - 44.0 + ang.sin() * 3.0), (sx + 20.0 + ang.cos() * 7.0 * k, sy - 44.0 + ang.sin() * 7.0 * k)], 1.6, (250, 200, 60), LineCap::Round);
                }
            } else if p.tap != 0.0 {
                // Aye-aye : le long doigt fin tapote le sol.
                lim(pm, t, &[(sx, sy), (sx + 12.0, sy + 20.0), (sx + 22.0, -14.0)], 5.0, fur);
                let tip = (sx + 36.0, -1.0 - p.tap.max(0.0) * 5.0);
                ink(pm, t, &[(sx + 22.0, -14.0), tip], 1.6);
            } else if p.scratch != 0.0 {
                bras(pm, t, &[(sx, sy), (sx + 10.0, sy - 18.0), (sx - 2.0 + p.scratch * 3.0, sy - 36.0)], 5.5, fur);
            } else if !occupe {
                bras(pm, t, &[(sx, sy), (sx + 15.0, sy + 11.0), (sx + 13.0, sy + 27.0)], 5.5, fur);
            }
            tete(pm, t.pre_concat(Transform::from_rotate_at(-p.look_up * 25.0, 4.0, -70.0)), pal, 4.0, -70.0, p.blink, p.expr, att, p.sleepy, p.time, p.regard, p.baille);
        }
        PoseKind::Dangle { arms_up } => {
            // Suspendu par les bras (derrière la tête), corps groupé, queue qui pend.
            let g = -super::behavior::DANGLE;
            t = t.pre_concat(Transform::from_rotate_at(p.swing.to_degrees(), 0.0, g));
            let kick = (p.time * 5.0).sin() * 4.0;
            queue(pm, t, pal, [(-2.0, -26.0), (-12.0 + tx * 0.4, 4.0), (14.0 + tx, 24.0 + ty * 0.3), (0.0 + tx * 1.4, 48.0)]);
            if arms_up {
                bras(pm, t, &[(-10.0, -58.0), (-17.0, g + 14.0), (-12.0, g + 3.0)], 6.0, dark);
                bras(pm, t, &[(12.0, -58.0), (19.0, g + 14.0), (14.0, g + 3.0)], 6.0, fur);
            }
            lim(pm, t, &[(-6.0, -28.0), (-10.0 - kick, -14.0), (-8.0 - kick, -4.0)], 7.0, dark);
            patte(pm, t, -8.0 - kick, -3.0, 4.6, 2.9, dark);
            ov(pm, t, 0.0, -44.0, 15.0, 22.0, fur);
            douce(pm, t, 4.0, -42.0, 7.5, 15.0, cream);
            lim(pm, t, &[(6.0, -28.0), (12.0 + kick, -14.0), (10.0 + kick, -4.0)], 7.0, fur);
            patte(pm, t, 10.0 + kick, -3.0, 4.6, 2.9, fur);
            if !arms_up {
                bras(pm, t, &[(-10.0, -56.0), (-18.0 + kick * 0.5, -47.0), (-15.0 + kick, -36.0)], 5.5, dark);
                bras(pm, t, &[(12.0, -56.0), (20.0 - kick * 0.5, -47.0), (17.0 - kick, -36.0)], 5.5, fur);
            }
            tete(pm, t, pal, 3.0, -72.0, p.blink, p.expr, att, p.sleepy, p.time, p.regard, p.baille);
        }
        PoseKind::Grimpe => {
            // Agrippé à la liane devant lui, une main après l'autre.
            let a = p.walk_phase.sin();
            queue(pm, t, pal, [(-6.0, -30.0), (-16.0 + tx * 0.3, -6.0), (-22.0 + tx * 0.6, 14.0), (-12.0 + tx, 34.0)]);
            bras(pm, t, &[(-2.0, -26.0), (-6.0, -16.0 + 3.0 * a), (10.0, -10.0 + 4.0 * a)], 6.0, dark);
            bras(pm, t, &[(4.0, -62.0), (10.0, -80.0), (12.0, -94.0 - 8.0 * a)], 5.5, dark);
            let bt = t.pre_concat(Transform::from_rotate_at(-8.0, 0.0, -48.0));
            ov(pm, bt, 0.0, -48.0, 14.5, 24.0, fur);
            douce(pm, bt, 6.0, -46.0, 6.5, 16.0, cream);
            bras(pm, t, &[(4.0, -28.0), (-2.0, -20.0 - 3.0 * a), (10.0, -22.0 - 4.0 * a)], 6.0, fur);
            tete(pm, t.pre_concat(Transform::from_rotate_at(-22.0, 4.0, -82.0)), pal, 4.0, -82.0, p.blink, p.expr, att, p.sleepy, p.time, p.regard, p.baille);
            bras(pm, t, &[(7.0, -62.0), (12.0, -70.0), (13.0, -78.0 + 8.0 * a)], 5.5, fur);
        }
        PoseKind::Upright => {
            // Debout en équilibre (propithèque, indri) : bras levés, jambes fléchies.
            let k = p.walk_phase.sin() * 0.5 + 0.5;
            let bend = 6.0 + k * 6.0;
            queue(pm, t, pal, [(-4.0, -22.0), (-16.0 + tx * 0.3, -8.0), (-32.0 + tx * 0.6, 2.0), (-56.0 + tx, -2.0 + ty * 0.3)]);
            lim(pm, t, &[(-6.0, -26.0), (-2.0 + bend, -14.0), (-8.0, -3.0)], 6.5, dark);
            patte(pm, t, -6.0, -2.0, 5.0, 2.6, dark);
            ov(pm, t, 0.0, -46.0, 15.5, 23.0, fur);
            douce(pm, t, 5.0, -44.0, 8.0, 16.0, cream);
            lim(pm, t, &[(6.0, -26.0), (10.0 + bend, -14.0), (6.0, -3.0)], 6.5, fur);
            patte(pm, t, 8.0, -2.0, 5.0, 2.6, fur);
            let up = 4.0 * (p.time * 3.0).sin();
            bras(pm, t, &[(-8.0, -58.0), (-18.0, -70.0), (-24.0, -84.0 + up)], 5.5, dark);
            tete(pm, t.pre_concat(Transform::from_rotate_at(-p.look_up * 25.0, 3.0, -78.0)), pal, 3.0, -78.0, p.blink, p.expr, att, p.sleepy, p.time, p.regard, p.baille);
            bras(pm, t, &[(10.0, -58.0), (20.0, -68.0), (28.0, -82.0 - up)], 5.5, fur);
        }
        PoseKind::Stretch => {
            let k = (p.walk_phase * PI).sin();
            queue(pm, t, pal, [(-24.0, -44.0 - k * 6.0), (-50.0, -60.0), (-56.0 + tx, -104.0), (-40.0 + tx, -122.0)]);
            lim(pm, t, &[(-15.0, -36.0 - k * 4.0), (-17.0, -2.0)], 7.0, dark);
            patte(pm, t, -15.0, -2.0, 5.0, 2.6, dark);
            let bt = t.pre_concat(Transform::from_rotate_at(k * 16.0, 0.0, -38.0));
            ov(pm, bt, 0.0, -40.0, 27.0 + k * 4.0, 14.0, fur);
            douce(pm, bt, 2.0, -31.0, 18.0, 5.0, cream);
            lim(pm, t, &[(18.0, -36.0 + k * 10.0), (24.0 + k * 22.0, -2.0)], 6.5, fur);
            patte(pm, t, 26.0 + k * 22.0, -2.0, 5.0, 2.6, fur);
            lim(pm, t, &[(-12.0, -34.0 - k * 4.0), (-12.0, -2.0)], 7.0, fur);
            patte(pm, t, -10.0, -2.0, 5.0, 2.6, fur);
            tete(pm, t.pre_concat(Transform::from_rotate_at(k * 14.0, 31.0, -50.0)), pal, 33.0, -54.0 + k * 16.0, k > 0.6, p.expr, att, p.sleepy, p.time, p.regard, p.baille);
        }
        PoseKind::Walk | PoseKind::Fly | PoseKind::Climb => {
            // À quatre pattes, queue dressée en S (ou portée à l'horizontale).
            if p.kind == PoseKind::Climb {
                t = t.pre_concat(Transform::from_rotate(-90.0));
            }
            let ph = p.walk_phase;
            let (sw, f1, f2) = match p.kind {
                PoseKind::Fly => (11.0, PI / 2.0, -PI / 2.0),
                PoseKind::Climb => (10.0, ph, ph + PI),
                _ => (7.0, ph, ph + PI),
            };
            if pal.tail_up {
                queue(pm, t, pal, [(-24.0, -42.0), (-48.0 + tx * 0.3, -52.0 + ty * 0.4), (-30.0 + tx * 0.7, -100.0 + ty * 0.7), (-58.0 + tx, -118.0 + ty)]);
            } else {
                queue(pm, t, pal, [(-24.0, -42.0), (-48.0 + tx * 0.3, -46.0 + ty * 0.4), (-72.0 + tx * 0.7, -44.0 + ty * 0.7), (-96.0 + tx, -34.0 + ty)]);
            }
            let jambe = |pm: &mut Pixmap, hip: (f32, f32), phase: f32, c: C| {
                let fx = hip.0 + phase.sin() * sw;
                let lift = phase.cos().max(0.0) * sw * 0.5;
                let genou = ((hip.0 + fx) / 2.0 + 2.0, hip.1 / 2.0 - 2.0);
                lim(pm, t, &[hip, genou, (fx, -lift - 3.0)], 8.0, c);
                patte(pm, t, fx + 2.0, -lift - 2.0, 5.4, 3.0, c);
            };
            jambe(pm, (-15.0, -34.0), f2, dark);
            jambe(pm, (17.0, -34.0), f1, dark);
            let bob = (ph * 2.0).sin().abs() * 1.5;
            ov(pm, t, 0.0, -42.0 - bob, 27.0, 15.5, fur);
            douce(pm, t, 4.0, -32.0 - bob, 18.0, 6.0, cream);
            jambe(pm, (-12.0, -32.0), f1, fur);
            jambe(pm, (20.0, -32.0), f2, fur);
            tete(pm, t, pal, 30.0, -58.0 - bob, p.blink, p.expr, att, p.sleepy, p.time, p.regard, p.baille);
        }
    }
    // Notes de musique (danse, chant de l'indri) ; cri (vari).
    if p.sing || att == Attitude::Danse {
        let couleurs = [(240, 80, 70), (240, 190, 40), (70, 170, 90)];
        for n in 0..3 {
            let ph = (p.time * 0.8 + n as f32 * 0.33) % 1.0;
            let (nx, ny) = (-16.0 + n as f32 * 18.0 + (ph * 6.0).sin() * 4.0, -98.0 - ph * 30.0);
            if ph < 0.95 {
                let col = couleurs[n];
                fill_oval(pm, t, nx, ny, 3.2, 2.5, col);
                stroke(pm, t, &[(nx + 2.8, ny), (nx + 2.8, ny - 10.0)], 1.4, col, LineCap::Round);
                stroke(pm, t, &[(nx + 2.8, ny - 10.0), (nx + 7.5, ny - 7.0)], 1.4, col, LineCap::Round);
            }
        }
    }
    if p.call {
        let k = (p.time * 10.0).sin() * 2.0;
        for (dx, dy) in [(1.0, -0.5), (1.0, 0.0), (1.0, 0.5)] {
            let (bx, by) = (36.0, -66.0);
            stroke(pm, t, &[(bx + dx * 6.0, by + dy * 6.0), (bx + dx * (16.0 + k), by + dy * (16.0 + k))], 2.0, (240, 120, 60), LineCap::Round);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mascot::especes::Espece;

    /// Coût du dessin (pas un vrai test) : `cargo test --release cout_dessin -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn cout_dessin() {
        let mut pm = Pixmap::new(260, 260).unwrap();
        let poses = [PoseKind::Sit, PoseKind::Walk, PoseKind::Upright, PoseKind::Sleep];
        let n = 120;
        let mut meilleur = f64::MAX;
        for _ in 0..7 {
            let t0 = std::time::Instant::now();
            for e in Espece::TOUTES {
                let pal = e.pal();
                for i in 0..n {
                    let pose = Pose { kind: poses[i % 4], pop: 1.0, time: i as f32 * 0.05, walk_phase: i as f32 * 0.2, wave: if i % 8 == 0 { 0.5 } else { 0.0 }, ..Default::default() };
                    pm.fill(tiny_skia::Color::TRANSPARENT);
                    maki(&mut pm, Transform::from_translate(130.0, 240.0).pre_scale(1.5, 1.5), &pose, &pal, Attitude::Aucune);
                }
            }
            meilleur = meilleur.min(t0.elapsed().as_secs_f64() * 1e6 / (n * Espece::TOUTES.len()) as f64);
        }
        println!("COUT_DESSIN {meilleur:.0} µs par image (260 px, échelle 1,5 ; meilleur de 7 passes)");
    }

    /// Planche de contrôle (pas un vrai test) :
    /// `cargo test --release planche -- --ignored` puis voir $LKATTA_PLANCHE (PNG).
    #[test]
    #[ignore]
    fn planche() {
        let out = std::env::var("LKATTA_PLANCHE").unwrap_or_else(|_| "/tmp/planche.png".into());
        let cases: Vec<(PoseKind, Attitude, Expr, bool)> = vec![
            (PoseKind::Walk, Attitude::Aucune, Expr::Normal, false),
            (PoseKind::Sit, Attitude::Aucune, Expr::Normal, false),
            (PoseKind::Dangle { arms_up: true }, Attitude::Aucune, Expr::Normal, false),
            (PoseKind::Sit, Attitude::Danse, Expr::Happy, false),
            (PoseKind::Sit, Attitude::Parle, Expr::Normal, false),
            (PoseKind::Sit, Attitude::Boude, Expr::Normal, false),
            (PoseKind::Sit, Attitude::Fier, Expr::Normal, false),
            (PoseKind::Sleep, Attitude::Aucune, Expr::Normal, true),
            (PoseKind::Upright, Attitude::Aucune, Expr::Surprised, false),
            // Bâillement, puis regard vers un pointeur en haut à droite.
            (PoseKind::Sit, Attitude::Aucune, Expr::Normal, false),
            (PoseKind::Sit, Attitude::Aucune, Expr::Normal, false),
        ];
        let filtre = std::env::var("LKATTA_PLANCHE_ESPECE").ok().and_then(|e| Espece::parse(&e));
        let especes: Vec<Espece> = Espece::TOUTES.into_iter().filter(|e| filtre.is_none_or(|f| f == *e)).collect();
        let z: f32 = std::env::var("LKATTA_PLANCHE_ZOOM").ok().and_then(|z| z.parse().ok()).unwrap_or(1.6);
        let (cw, ch) = (140.0 * z, 145.0 * z);
        let mut pm = Pixmap::new((cw * cases.len() as f32) as u32, (ch * especes.len() as f32) as u32).unwrap();
        pm.fill(tiny_skia::Color::from_rgba8(255, 255, 255, 255));
        for (j, e) in especes.iter().enumerate() {
            let pal = e.pal();
            for (i, (kind, att, expr, blink)) in cases.iter().enumerate() {
                let mut pose = Pose { kind: *kind, expr: *expr, blink: *blink, pop: 1.0, time: 0.6 + i as f32, wave: 0.0, bipede: e.style().bipede, ..Default::default() };
                if i == cases.len() - 2 {
                    pose.baille = 1.0;
                    pose.look_up = 0.35;
                } else if i == cases.len() - 1 {
                    pose.regard = Some((0.6, -0.8));
                }
                let base = Transform::from_translate(i as f32 * cw + cw * 0.5, j as f32 * ch + ch - 12.0 * z).pre_scale(z, z);
                maki(&mut pm, base, &pose, &pal, *att);
            }
        }
        pm.save_png(out).unwrap();
    }
}
