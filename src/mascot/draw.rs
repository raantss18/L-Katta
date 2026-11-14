//! Dessin vectoriel du maki catta et des bulles (tiny-skia + ab_glyph).
//! Repère du personnage : origine entre les pattes au sol, y vers le bas, regard à droite.

use ab_glyph::{Font, FontVec, GlyphId, PxScale, ScaleFont, VariableFont, point};
use tiny_skia::{
    Color, FillRule, LineCap, Paint, PathBuilder, Pixmap, PremultipliedColorU8, Rect, Stroke, Transform,
};

use super::behavior::Pose;

use super::especes::{Espece, Pal};

thread_local! {
    static PAL: std::cell::Cell<Pal> = std::cell::Cell::new(Espece::Catta.pal());
}

/// Choisit l'espèce dessinée par les appels suivants.
pub fn set_species(e: Espece) {
    PAL.with(|p| p.set(e.pal()));
}

fn pal() -> Pal {
    PAL.with(|p| p.get())
}

/// Mélange de deux couleurs (k = part de `b`).
fn mix(a: (u8, u8, u8), b: (u8, u8, u8), k: f32) -> (u8, u8, u8) {
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * k).round() as u8;
    (m(a.0, b.0), m(a.1, b.1), m(a.2, b.2))
}

pub(crate) fn paint(c: (u8, u8, u8)) -> Paint<'static> {
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

fn rot(deg: f32, x: f32, y: f32) -> Transform {
    Transform::from_rotate_at(deg, x, y)
}

fn scale_at(sx: f32, sy: f32, x: f32, y: f32) -> Transform {
    Transform::from_translate(x, y).pre_scale(sx, sy).pre_translate(-x, -y)
}

/// Dessine le maki (style de l'identité visuelle : voir `dessin.rs`). `base` place
/// l'origine (entre les pattes) avec l'échelle et la direction.
pub fn maki(pm: &mut Pixmap, base: Transform, p: &Pose) {
    super::dessin::maki(pm, base, p, &pal(), p.attitude);
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
    /// Police du système pour les signes absents de `font` (✓, ↳…).
    repli: Option<FontVec>,
}

/// Polices intégrées (OFL) : le même rendu sur toutes les machines.
const ATKINSON: &[u8] = include_bytes!("../../assets/fonts/AtkinsonHyperlegible-Regular.ttf");
const ATKINSON_GRAS: &[u8] = include_bytes!("../../assets/fonts/AtkinsonHyperlegible-Bold.ttf");
const ALEGREYA: &[u8] = include_bytes!("../../assets/fonts/Alegreya-wght.ttf");
/// Émojis en noir et blanc (dessinés à l'encre comme le reste du carnet) : les émojis en
/// couleur du système sont des images, que le rendu du texte ne sait pas lire.
const NOTO_EMOJI: &[u8] = include_bytes!("../../assets/fonts/NotoEmoji-wght.ttf");

fn emoji() -> Option<&'static FontVec> {
    use std::sync::OnceLock;
    static E: OnceLock<Option<FontVec>> = OnceLock::new();
    E.get_or_init(|| {
        let mut f = FontVec::try_from_vec(NOTO_EMOJI.to_vec()).ok()?;
        f.set_variation(b"wght", 500.0);
        Some(f)
    })
    .as_ref()
}

/// Signes sans dessin propre : sélecteurs de variante, liaison d'émojis, tons de peau.
fn invisible(c: char) -> bool {
    matches!(c as u32, 0xFE0E | 0xFE0F | 0x200D | 0x200B | 0x1F3FB..=0x1F3FF | 0xE0020..=0xE007F)
}

/// Police sans empattement du système (repli des signes rares).
fn systeme_sans(style: &str) -> Option<FontVec> {
    let mut c = vec![];
    // Une police qui a vraiment les signes dont la bulle se sert (coche, flèches, puces) :
    // celle du système par défaut ne les a pas toujours.
    if let Ok(o) = std::process::Command::new("fc-match").args(["-f", "%{file}", &format!("sans:style={style}:charset=2713 21b3 25cf 25b8 2192 2022")]).output() {
        c.push(String::from_utf8_lossy(&o.stdout).to_string());
    }
    c.extend(["/usr/share/fonts/TTF/DejaVuSans.ttf", "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf", "/usr/share/fonts/noto/NotoSans-Regular.ttf"].map(String::from));
    c.into_iter().filter_map(|p| std::fs::read(p.trim()).ok()).find_map(|b| FontVec::try_from_vec(b).ok())
}

/// Police des titres (Alegreya, demi-gras).
pub fn titres() -> Option<&'static Text> {
    use std::sync::OnceLock;
    static T: OnceLock<Option<Text>> = OnceLock::new();
    T.get_or_init(|| {
        let mut font = FontVec::try_from_vec(ALEGREYA.to_vec()).ok()?;
        font.set_variation(b"wght", 600.0);
        Some(Text { font, repli: systeme_sans("Regular") })
    })
    .as_ref()
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
    let font = FontVec::try_from_vec(ATKINSON_GRAS.to_vec()).ok()?;
    Some(Text { font, repli: systeme_sans("Bold") })
}

impl Text {

    /// Police du texte : Atkinson Hyperlegible (intégrée), signes rares pris au système.
    pub fn load() -> Option<Text> {
        let font = FontVec::try_from_vec(ATKINSON.to_vec()).ok()?;
        Some(Text { font, repli: systeme_sans("Regular") })
    }

    /// Police qui a ce signe (la principale, puis les émojis, puis le repli du système) et
    /// son glyphe. `None` : rien à dessiner (signe invisible, ou introuvable : pas de carré).
    fn glyphe(&self, c: char) -> Option<(&FontVec, GlyphId)> {
        if invisible(c) {
            return None;
        }
        let g = self.font.glyph_id(c);
        if g.0 != 0 || c.is_whitespace() {
            return Some((&self.font, g));
        }
        if let Some(e) = emoji() {
            let h = e.glyph_id(c);
            if h.0 != 0 {
                return Some((e, h));
            }
        }
        if let Some(r) = &self.repli {
            let h = r.glyph_id(c);
            if h.0 != 0 {
                return Some((r, h));
            }
        }
        None
    }

    pub fn width(&self, size: f32, s: &str) -> f32 {
        let sf = self.font.as_scaled(PxScale::from(size));
        let mut w = 0.0;
        let mut prev = None;
        for c in s.chars() {
            let Some((f, g)) = self.glyphe(c) else { continue };
            if !std::ptr::eq(f, &self.font) {
                w += f.as_scaled(PxScale::from(size)).h_advance(g);
                prev = None;
                continue;
            }
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
            let Some((f, g)) = self.glyphe(ch) else { continue };
            let propre = std::ptr::eq(f, &self.font);
            if let (Some(p), true) = (prev, propre) {
                caret += sf.kern(p, g);
            }
            let glyph = g.with_scale_and_position(PxScale::from(size), point(caret, baseline));
            caret += f.as_scaled(PxScale::from(size)).h_advance(g);
            prev = if propre { Some(g) } else { None };
            if let Some(og) = f.outline_glyph(glyph) {
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

/// Couleurs des bulles selon le thème (clair, sombre, ou celui de KDE). Les deux thèmes
/// sont des pages de carnet de naturaliste : papier vieilli et encre ferrogallique le jour,
/// papier sombre à la lueur d'une lampe le soir. Un seul accent : le vert du ravinala, pour
/// ce qui agit ; le rouge latérite ne sert qu'aux erreurs.
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
    /// Accent (boutons principaux, interrupteurs, choix actif) et texte posé dessus.
    pub acc: (u8, u8, u8),
    pub acc_text: (u8, u8, u8),
    pub err: (u8, u8, u8),
    /// Rousseurs du papier (bords assombris, grain).
    pub edge: (u8, u8, u8),
    pub sombre: bool,
}

pub const CLAIR: Theme = Theme {
    bg: (238, 225, 193),
    bg2: (231, 215, 178),
    border: (176, 148, 104),
    text: (43, 33, 24),
    soft: (98, 80, 58),
    faint: (140, 118, 88),
    user: (36, 62, 98),
    field: (244, 235, 211),
    field_border: (178, 152, 110),
    chip: (229, 213, 175),
    chip_text: (92, 70, 46),
    acc: (41, 92, 66),
    acc_text: (247, 240, 224),
    err: (146, 48, 28),
    edge: (140, 98, 48),
    sombre: false,
};

pub const SOMBRE: Theme = Theme {
    bg: (37, 31, 26),
    bg2: (31, 26, 22),
    border: (96, 80, 62),
    text: (236, 224, 198),
    soft: (184, 166, 138),
    faint: (140, 124, 102),
    user: (150, 186, 226),
    field: (46, 39, 33),
    field_border: (104, 88, 68),
    chip: (52, 44, 36),
    chip_text: (218, 196, 156),
    acc: (126, 184, 140),
    acc_text: (24, 32, 26),
    err: (228, 132, 104),
    edge: (8, 6, 4),
    sombre: true,
};

/// Une feuille de papier : bords légèrement déchirés, rousseurs vers les bords, grain.
/// Dessinée une fois par taille (dégradé et grain coûtent cher), puis recopiée.
pub fn papier(pm: &mut Pixmap, r: R, rad: f32, s: f32) {
    thread_local! {
        static CACHE: std::cell::RefCell<Vec<((u32, u32, u32, u32, bool), Pixmap)>> = const { std::cell::RefCell::new(Vec::new()) };
    }
    let (x, y, w, h) = r;
    // Marge pour l'ombre portée.
    let m = (6.0 * s).ceil();
    let cle = (w.round() as u32, h.round() as u32, (rad * 10.0) as u32, (s * 100.0) as u32, th().sombre);
    let fx = x.floor();
    let fy = y.floor();
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        if !c.iter().any(|(k, _)| *k == cle) {
            let Some(mut feuille) = Pixmap::new((w + 2.0 * m).ceil() as u32 + 1, (h + 2.0 * m).ceil() as u32 + 1) else { return };
            papier_direct(&mut feuille, (m + (x - fx), m + (y - fy), w, h), rad, s);
            // Deux tailles gardées (la bulle et la petite bulle, ou l'avant/après d'un agrandissement).
            if c.len() >= 3 {
                c.remove(0);
            }
            c.push((cle, feuille));
        }
        if let Some((_, f)) = c.iter().find(|(k, _)| *k == cle) {
            poser(pm, f, (fx - m) as i32, (fy - m) as i32);
        }
    });
}

fn papier_direct(pm: &mut Pixmap, r: R, rad: f32, s: f32) {
    let (x, y, w, h) = r;
    let t = th();
    // Ombre portée douce.
    rrect(pm, (x + 1.5 * s, y + 4.0 * s, w, h), rad, &paint_a((0, 0, 0), 0.16), None);
    let Some(path) = bord_dechire(r, rad, s) else { return };
    let Some(mut masque) = tiny_skia::Mask::new(pm.width(), pm.height()) else { return };
    masque.fill_path(&path, FillRule::Winding, true, Transform::identity());
    // Fond, rousseurs (les bords foncent doucement) et grain, calculés pixel par pixel :
    // le même rendu qu'un dégradé radial et un motif, dix fois moins cher.
    let fort = if t.sombre { 0.35 } else { 0.24 };
    let (cx, cy, rx, ry) = (x + w / 2.0, y + h / 2.0, w * 0.62, h * 0.66);
    let pw = pm.width() as usize;
    let (x0, x1) = ((x.floor().max(0.0)) as usize, ((x + w).ceil() as usize).min(pw));
    let (y0, y1) = ((y.floor().max(0.0)) as usize, ((y + h).ceil() as usize).min(pm.height() as usize));
    grain_tile(|tuile| {
        let td = tuile.data();
        let tw = tuile.width() as usize;
        let th_ = tuile.height() as usize;
        let md = masque.data();
        let dd = pm.data_mut();
        for py in y0..y1 {
            let dy = (py as f32 + 0.5 - cy) / ry;
            for px in x0..x1 {
                let m = md[py * pw + px] as u32;
                if m == 0 {
                    continue;
                }
                let dx = (px as f32 + 0.5 - cx) / rx;
                let d = (dx * dx + dy * dy).sqrt();
                let a = ((d - 0.62) / 0.38).clamp(0.0, 1.0) * fort;
                let mut c = [
                    t.bg.0 as f32 + (t.edge.0 as f32 - t.bg.0 as f32) * a,
                    t.bg.1 as f32 + (t.edge.1 as f32 - t.bg.1 as f32) * a,
                    t.bg.2 as f32 + (t.edge.2 as f32 - t.bg.2 as f32) * a,
                ];
                // Grain (motif prémultiplié) posé par-dessus.
                let ti = ((py % th_) * tw + (px % tw)) * 4;
                let ga = td[ti + 3] as f32 / 255.0;
                if ga > 0.0 {
                    for k in 0..3 {
                        c[k] = td[ti + k] as f32 + c[k] * (1.0 - ga);
                    }
                }
                // Bord adouci (masque), puis « par-dessus » l'ombre déjà posée.
                let al = m as f32 / 255.0;
                let i = (py * pw + px) * 4;
                let inv = 1.0 - al;
                for k in 0..3 {
                    dd[i + k] = (c[k] * al + dd[i + k] as f32 * inv).min(255.0) as u8;
                }
                dd[i + 3] = (255.0 * al + dd[i + 3] as f32 * inv).min(255.0) as u8;
            }
        }
    });
    pm.stroke_path(&path, &paint_a(t.border, 0.7), &Stroke { width: 1.0, ..Default::default() }, Transform::identity(), None);
}

/// Contour d'une feuille aux bords légèrement irréguliers (toujours le même pour une taille).
fn bord_dechire(r: R, rad: f32, s: f32) -> Option<tiny_skia::Path> {
    let (x, y, w, h) = r;
    let rad = rad.min(w / 2.0).min(h / 2.0);
    // Bruit déterministe, doux : somme de deux sinus.
    let n = |u: f32| (u * 0.37).sin() * 0.6 + (u * 1.13 + 1.7).sin() * 0.4;
    let amp = 1.1 * s;
    let mut pts: Vec<(f32, f32)> = vec![];
    let pas = 5.0 * s;
    let mut cote = |a: (f32, f32), b: (f32, f32), nx: f32, ny: f32, graine: f32| {
        let len = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
        let k = (len / pas).max(1.0) as usize;
        for i in 0..k {
            let f = i as f32 / k as f32;
            let d = (n(graine + i as f32) * amp).max(-amp) - amp * 0.5;
            pts.push((a.0 + (b.0 - a.0) * f + nx * d, a.1 + (b.1 - a.1) * f + ny * d));
        }
    };
    cote((x + rad, y), (x + w - rad, y), 0.0, 1.0, 0.0);
    cote((x + w, y + rad), (x + w, y + h - rad), -1.0, 0.0, 31.0);
    cote((x + w - rad, y + h), (x + rad, y + h), 0.0, -1.0, 67.0);
    cote((x, y + h - rad), (x, y + rad), 1.0, 0.0, 113.0);
    let mut pb = PathBuilder::new();
    let coin = |pb: &mut PathBuilder, cx: f32, cy: f32, a0: f32| {
        for i in 0..=4 {
            let a = a0 + i as f32 * std::f32::consts::FRAC_PI_8;
            pb.line_to(cx + rad * a.cos(), cy + rad * a.sin());
        }
    };
    // Parcours : haut, coin haut-droit, droite, coin bas-droit, bas, coin bas-gauche, gauche, coin haut-gauche.
    let q = |i: usize| pts[i];
    let total = pts.len();
    if total == 0 {
        return None;
    }
    // Indices de début de chaque côté (dans l'ordre d'ajout).
    let l_h = (((w - 2.0 * rad) / pas).max(1.0)) as usize;
    let l_v = (((h - 2.0 * rad) / pas).max(1.0)) as usize;
    let starts = [0, l_h, l_h + l_v, 2 * l_h + l_v, 2 * l_h + 2 * l_v];
    pb.move_to(q(0).0, q(0).1);
    let pi = std::f32::consts::PI;
    let corners = [(x + w - rad, y + rad, -pi / 2.0), (x + w - rad, y + h - rad, 0.0), (x + rad, y + h - rad, pi / 2.0), (x + rad, y + rad, pi)];
    for side in 0..4 {
        for i in starts[side]..starts[side + 1].min(total) {
            pb.line_to(q(i).0, q(i).1);
        }
        let (cx, cy, a0) = corners[side];
        coin(&mut pb, cx, cy, a0);
    }
    pb.close();
    pb.finish()
}

/// Motif de grain du papier (96 × 96), calculé une fois par thème.
fn grain_tile(f: impl FnOnce(&Pixmap)) {
    thread_local! {
        static G: std::cell::RefCell<Option<(bool, Pixmap)>> = const { std::cell::RefCell::new(None) };
    }
    let t = th();
    G.with(|g| {
        let mut g = g.borrow_mut();
        if g.as_ref().is_none_or(|(sombre, _)| *sombre != t.sombre) {
            let Some(mut pm) = Pixmap::new(96, 96) else { return };
            let mut rng = fastrand::Rng::with_seed(7);
            let c = if t.sombre { (255, 240, 210) } else { t.edge };
            for _ in 0..360 {
                let (x, y) = (rng.f32() * 96.0, rng.f32() * 96.0);
                let a = 0.025 + rng.f32() * if t.sombre { 0.03 } else { 0.05 };
                let r = 0.35 + rng.f32() * 0.7;
                disc(&mut pm, Transform::identity(), x, y, r, &paint_a(c, a));
            }
            // Quelques fibres, à peine visibles (elles ne doivent pas se lire comme des signes).
            for _ in 0..5 {
                let (x, y) = (rng.f32() * 96.0, rng.f32() * 96.0);
                let (dx, dy) = (rng.f32() * 6.0 - 3.0, rng.f32() * 3.0 - 1.5);
                let mut p = paint_a(c, if t.sombre { 0.05 } else { 0.08 });
                p.anti_alias = true;
                if let Some(path) = { let mut pb = PathBuilder::new(); pb.move_to(x, y); pb.line_to(x + dx, y + dy); pb.finish() } {
                    pm.stroke_path(&path, &p, &Stroke { width: 0.4, ..Default::default() }, Transform::identity(), None);
                }
            }
            *g = Some((t.sombre, pm));
        }
        if let Some((_, pm)) = g.as_ref() {
            f(pm);
        }
    });
}

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

pub(crate) fn rrect(pm: &mut Pixmap, r: R, rad: f32, fill: &Paint, stroke: Option<((u8, u8, u8), f32)>) {
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
    /// Loupe de la recherche approfondie (côté Discussion) : un clic l'allume ou l'éteint.
    pub mode: R,
    /// Interrupteur Discussion · Code.
    pub cotes: Vec<(R, crate::fils::Cote)>,
    /// Bouton de l'historique des fils, et ligne du fil ouvert (un clic ouvre l'historique).
    pub histo_btn: R,
    pub fil: R,
    /// Bouton « Arrêter » pendant qu'il travaille.
    pub stop: Option<R>,
    /// Zones de l'historique, et son champ (recherche ou chemin).
    pub histo: Vec<(R, crate::chat::HistoAct)>,
    pub histo_champ: Option<R>,
    /// Actions sur le texte copié (zone, n° d'action).
    pub presse: Vec<(R, usize)>,
    pub picks_panel: Vec<(R, crate::chat::Act)>,
    /// Hauteur du contenu du panneau et de sa zone visible (px logiques non mis à l'échelle).
    pub panel_content: f32,
    pub panel_view: f32,
}

/// Hauteur du sélecteur de modèles (avant mise à l'échelle).
const PICKER_H: f32 = 380.0;

/// Le sélecteur, le panneau de réglages ou l'historique occupe la bulle.
fn zone(chat: &crate::chat::Chat) -> bool {
    chat.picker.is_some() || chat.panneau.is_some() || chat.histo.is_some()
}

/// Hauteur de l'historique (avant mise à l'échelle).
const HISTO_H: f32 = 400.0;

/// Hauteur de la zone du sélecteur ou du panneau : le panneau s'ajuste à son contenu.
fn zone_h(chat: &crate::chat::Chat) -> f32 {
    if chat.histo.is_some() {
        // À la taille du contenu : recherche, projets, fils et titres de groupes, message.
        let f = chat.histo_fils();
        let groupes = if chat.cote == crate::fils::Cote::Code {
            chat.projets().len()
        } else {
            let mut g: Vec<&str> = f.iter().map(|r| crate::fils::periode(&r.maj)).collect();
            g.dedup();
            g.len()
        };
        let code = if chat.cote == crate::fils::Cote::Code { 30.0 } else { 0.0 };
        let h = 34.0 + code + (f.len().max(1) as f32) * 25.0 + groupes as f32 * 22.5 + 40.0;
        return h.clamp(200.0, HISTO_H);
    }
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
    let acc = th().acc;
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
        for (i, label) in CATS.iter().map(|(_, l)| crate::lang::trd(l)).enumerate() {
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
            text.draw(pm, 12.5 * s, x, cy, t!("Je regarde les modèles disponibles…"), th().soft);
        }
        Some(Err(e)) => {
            for (i, l) in text.wrap(12.0 * s, e, w).iter().take(4).enumerate() {
                text.draw(pm, 12.0 * s, x, cy + i as f32 * 16.0 * s, l, (176, 48, 40));
            }
            cy += 70.0 * s;
            if chat.has_auto(p.moteur) {
                draw_row(pm, lay, cy, t!("Automatique"), t!("le meilleur modèle gratuit selon la demande"), None, current == "auto", false, Pick::Model(None));
            }
        }
        Some(Ok(_)) => {
            let cat = CATS[p.cat.min(CATS.len() - 1)].0;
            let best = rows.iter().map(|m| m.score(cat)).fold(0.0f32, f32::max).max(1e-3);
            let mut items: Vec<(String, String, Option<f32>, bool, bool, Pick)> = vec![];
            if chat.has_auto(p.moteur) {
                items.push((t!("Automatique").into(), t!("le meilleur gratuit selon la demande, et bascule si saturé").into(), None, current == "auto", false, Pick::Model(None)));
            }
            for m in &rows {
                let mut meta = vec![];
                if !m.provider.is_empty() {
                    meta.push(m.provider.clone());
                }
                meta.push(if m.tools { t!("outils").into() } else { t!("sans outils : ne peut pas agir").into() });
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
    c.into_iter().filter_map(|p| std::fs::read(p.trim()).ok()).find_map(|b| FontVec::try_from_vec(b).ok()).map(|font| Text { font, repli: systeme_sans("Regular") })
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
    let shown = if zone(chat) { &lines[0..0] } else { &lines[start..end] };
    let hist_h: f32 = shown.iter().map(|l| lh * l.k).sum();

    let mut confirm_lines = chat.confirm.as_ref().map(|q| text.wrap(size * 0.95, q, inner - 16.0 * s)).unwrap_or_default();
    // Une demande très longue (contenu d'un fichier, mail entier) : le début et la fin, et le
    // nombre de lignes passées, pour que la bulle tienne à l'écran sans rien cacher en douce.
    if confirm_lines.len() > 18 {
        let n = confirm_lines.len();
        let fin = confirm_lines.split_off(n - 5);
        confirm_lines.truncate(11);
        confirm_lines.push(t!("[… {} lignes non montrées …]", n - 16).to_string());
        confirm_lines.extend(fin);
    }
    let confirm_h = if confirm_lines.is_empty() { 0.0 } else { confirm_lines.len() as f32 * lh + 48.0 * s };
    // En-tête : titre, interrupteur, boutons ; puis la ligne du fil ouvert.
    let header_h = 24.0 * s + FIL_H * s;
    let input_h = 34.0 * s;
    let hist_h = if zone(chat) { zone_h(chat) * s } else { hist_h };
    let presse = presse_visible(chat);
    let presse_h = if presse { 30.0 * s } else { 0.0 };
    let h = pad + header_h + hist_h + confirm_h + presse_h + 8.0 * s + input_h + pad;
    let x = bx;
    let y = by;
    let _ = (y, h);
    let rect = (x, if above { by - h } else { by }, w, h);
    lay.rect = rect;
    let (x, y) = (rect.0, rect.1);

    // Ombre, fond, pointe.
    // La pointe d'abord (sous la feuille), puis la feuille de papier.
    let tip_x = ax.clamp(x + 24.0 * s, x + w - 24.0 * s);
    let pointe = mix(th().bg, th().edge, 0.12);
    if above {
        poly(pm, Transform::identity(), &[(tip_x - 10.0 * s, y + h - 3.0), (ax, ay - 4.0 * s), (tip_x + 8.0 * s, y + h - 3.0)], pointe);
    } else {
        poly(pm, Transform::identity(), &[(tip_x - 10.0 * s, y + 3.0), (ax, ay + 4.0 * s), (tip_x + 8.0 * s, y + 3.0)], pointe);
    }
    papier(pm, rect, 10.0 * s, s);

    // En-tête.
    let serif = titres().unwrap_or(text);
    serif.draw(pm, size * 1.12, x + pad, y + pad * 0.42, "L Katta", th().text);
    // À droite : fermer, modèle, réglages, historique ; à gauche : titre, interrupteur, loupe.
    lay.close = (x + w - pad - 18.0 * s, y + pad * 0.5, 18.0 * s, 18.0 * s);
    text.draw(pm, size * 1.1, lay.close.0 + 3.0 * s, lay.close.1 - 2.0 * s, "×", th().chip_text);
    let ty0 = y + pad * 0.45;
    let titre_w = serif.width(size * 1.12, "L Katta");
    // Interrupteur Discussion · Code : deux segments d'une même étiquette, le côté actif à l'encre.
    let fs = size * 0.72;
    let (l1, l2) = (t!("Discussion"), t!("Code"));
    let (w1, w2) = (text.width(fs, l1) + 16.0 * s, text.width(fs, l2) + 16.0 * s);
    let sx = x + pad + titre_w + 10.0 * s;
    let sw_r = (sx, ty0, w1 + w2, 20.0 * s);
    rrect(pm, sw_r, 5.0 * s, &paint_a(th().field, 0.55), Some((th().field_border, 1.0)));
    for (i, (lab, lw, c)) in [(l1, w1, crate::fils::Cote::Discussion), (l2, w2, crate::fils::Cote::Code)].into_iter().enumerate() {
        let r = (sx + if i == 0 { 0.0 } else { w1 }, ty0, lw, 20.0 * s);
        let actif = chat.cote == c;
        if actif {
            rrect(pm, (r.0 + 2.0 * s, r.1 + 2.0 * s, r.2 - 4.0 * s, r.3 - 4.0 * s), 4.0 * s, &paint(th().acc), None);
        }
        let tw = text.width(fs, lab);
        text.draw(pm, fs, r.0 + (lw - tw) / 2.0, r.1 + (r.3 - text.line_height(fs)) / 2.0, lab, if actif { th().acc_text } else { th().soft });
        lay.cotes.push((r, c));
    }
    let mut gauche = sx + w1 + w2 + 6.0 * s;
    // Loupe (recherche approfondie), côté Discussion seulement.
    if chat.cote == crate::fils::Cote::Discussion && chat.picker.is_none() && chat.panneau.is_none() {
        let actif = chat.mode == crate::brain::Mode::Recherche;
        lay.mode = (gauche, ty0, 20.0 * s, 20.0 * s);
        rrect(pm, lay.mode, 10.0 * s, &paint(if actif { th().acc } else { th().chip }), Some((th().field_border, 1.0)));
        loupe(pm, lay.mode.0 + 9.0 * s, lay.mode.1 + 9.0 * s, 4.2 * s, if actif { th().acc_text } else { th().chip_text }, s);
        gauche += 26.0 * s;
    }
    // Boutons historique et réglages, puis la pastille du modèle dans la place qui reste.
    let droite = lay.close.0 - 6.0 * s;
    let place = droite - gauche - 2.0 * (20.0 * s + 6.0 * s);
    let chip_label = if chat.picker.is_some() {
        t!("Fermer la liste").to_string()
    } else if chat.panneau.is_some() {
        t!("Réglages").to_string()
    } else if chat.histo.is_some() {
        t!("Historique").to_string()
    } else if !chat.engine.is_empty() {
        chat.engine.clone()
    } else {
        let (i, nom) = chat.engines().first().cloned().unwrap_or_default();
        let m = chat.current_model(i);
        format!("{nom} · {}", if m == "auto" { t!("automatique").into() } else { m })
    };
    let mut n = chip_label.chars().count().max(4);
    let mut label = chip_label.clone();
    while n > 4 && text.width(fs, &label) + 28.0 * s > place {
        n -= 1;
        label = short(&chip_label, n);
    }
    let cw = (text.width(fs, &label) + 28.0 * s).min(place.max(40.0 * s));
    lay.chip = (droite - cw, ty0, cw, 20.0 * s);
    rrect(pm, lay.chip, 10.0 * s, &paint(th().chip), Some((th().field_border, 1.0)));
    text.draw(pm, fs, lay.chip.0 + 8.0 * s, lay.chip.1 + (20.0 * s - text.line_height(fs)) / 2.0, &label, th().chip_text);
    tri(pm, lay.chip.0 + cw - 11.0 * s, lay.chip.1 + 10.0 * s, 3.5 * s, chat.picker.is_some(), th().chip_text);
    lay.gear = (lay.chip.0 - 26.0 * s, ty0, 20.0 * s, 20.0 * s);
    rrect(pm, lay.gear, 10.0 * s, &paint(if chat.panneau.is_some() { th().acc } else { th().chip }), Some((th().field_border, 1.0)));
    gear(pm, lay.gear.0 + 10.0 * s, lay.gear.1 + 10.0 * s, 6.5 * s, if chat.panneau.is_some() { th().acc_text } else { th().chip_text });
    lay.histo_btn = (lay.gear.0 - 26.0 * s, ty0, 20.0 * s, 20.0 * s);
    rrect(pm, lay.histo_btn, 10.0 * s, &paint(if chat.histo.is_some() { th().acc } else { th().chip }), Some((th().field_border, 1.0)));
    horloge(pm, lay.histo_btn.0 + 10.0 * s, lay.histo_btn.1 + 10.0 * s, 6.0 * s, if chat.histo.is_some() { th().acc_text } else { th().chip_text }, s);
    // Ligne du fil ouvert : projet (côté Code) et titre ; un clic ouvre l'historique.
    {
        let fy = ty0 + 24.0 * s;
        let fz = size * 0.78;
        let mut fx = x + pad;
        lay.fil = (x + pad, fy - 2.0 * s, inner, FIL_H * s);
        if chat.cote == crate::fils::Cote::Code {
            let projet = chat.fil.dossier.as_deref().map(crate::fils::nom_projet).unwrap_or_else(|| "~".into());
            dossier_ico(pm, fx + 6.0 * s, fy + text.line_height(fz) / 2.0, 5.5 * s, th().acc, s);
            fx += 15.0 * s;
            let g = police(text, St::Gras);
            g.draw(pm, fz, fx, fy, &projet, th().acc);
            fx += g.width(fz, &projet) + 8.0 * s;
        }
        let titre = if chat.fil.titre.trim().is_empty() {
            if chat.entries.iter().any(|e| matches!(e, crate::chat::Entry::User(_))) { t!("Fil en cours").to_string() } else { t!("Nouveau fil").to_string() }
        } else {
            chat.fil.titre.clone()
        };
        let maxw = x + pad + inner - fx;
        let mut t = titre.clone();
        let mut k = titre.chars().count();
        while k > 3 && text.width(fz, &t) > maxw {
            k -= 1;
            t = short(&titre, k);
        }
        text.draw(pm, fz, fx, fy, &t, th().faint);
        let base = fy + FIL_H * s - 3.0 * s;
        line(pm, Transform::identity(), (x + pad, base), (x + pad + inner, base), 0.6 * s, mix(th().bg, th().edge, 0.25));
    }

    if chat.picker.is_some() {
        draw_picker(pm, text, chat, (x + pad, y + pad + header_h, inner, PICKER_H * s), s, &mut lay);
    } else if chat.panneau.is_some() {
        draw_panel(pm, text, chat, (x + pad, y + pad + header_h, inner, zone_h(chat) * s), s, &mut lay);
    } else if chat.histo.is_some() {
        draw_histo(pm, text, chat, (x + pad, y + pad + header_h, inner, zone_h(chat) * s), s, &mut lay);
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
    if start > 0 && !zone(chat) {
        tri(pm, x + w / 2.0, y + pad + header_h - 6.0 * s, 4.5 * s, true, th().faint);
    }
    if zone(chat) {
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
        rrect(pm, yes, 6.0 * s, &paint(th().acc), None);
        rrect(pm, no, 6.0 * s, &paint(th().field), Some((th().field_border, 1.0)));
        text.draw(pm, size * 0.95, yes.0 + 12.0 * s, yes.1 + 4.0 * s, t!("Oui"), th().acc_text);
        text.draw(pm, size * 0.95, no.0 + 12.0 * s, no.1 + 4.0 * s, t!("Non"), th().text);
        lay.yes = Some(yes);
        lay.no = Some(no);
        cy += confirm_h;
    }

    // Texte qui vient d'être copié : quatre actions d'un clic.
    if presse {
        let sz = 12.0 * s;
        let lab = t!("Texte copié :");
        text.draw(pm, sz, x + pad, cy + 10.0 * s, lab, th().soft);
        let mut px = x + pad + text.width(sz, lab) + 8.0 * s;
        for (k, nom) in [t!("Traduire"), t!("Résumer"), t!("Corriger"), t!("Expliquer")].into_iter().enumerate() {
            let pw = text.width(sz, nom) + 16.0 * s;
            let r = (px, cy + 6.0 * s, pw, 23.0 * s);
            pill(pm, text, r, nom, th().acc, false, sz);
            lay.presse.push((r, k));
            px += pw + 5.0 * s;
        }
        cy += presse_h;
    }

    // Saisie.
    let input = (x + pad, cy + 8.0 * s, inner, input_h);
    lay.input = input;
    rrect(pm, input, 10.0 * s, &paint(th().field), Some((th().field_border, 1.0)));
    let ty = input.1 + (input_h - lh) / 2.0;
    if chat.busy && chat.confirm.is_none() {
        let dots = ".".repeat(1 + ((time * 3.0) as usize % 3));
        let sec = chat.debut.map(|d| d.elapsed().as_secs()).unwrap_or(0);
        let temps = if sec >= 60 { format!(" {} min {:02} s", sec / 60, sec % 60) } else if sec >= 3 { format!(" {sec} s") } else { String::new() };
        text.draw(pm, size, input.0 + 10.0 * s, ty, &format!("{}{dots}{temps}", t!("Je réfléchis")), th().soft);
        // Bouton Arrêter, dans la saisie : tâche trop longue ou demande mal partie.
        let lab = t!("Arrêter");
        let sz = size * 0.82;
        let bw = text.width(sz, lab) + 30.0 * s;
        let r = (input.0 + input.2 - bw - 5.0 * s, input.1 + 5.0 * s, bw, input_h - 10.0 * s);
        rrect(pm, r, 6.0 * s, &paint(th().err), None);
        rrect(pm, (r.0 + 9.0 * s, r.1 + r.3 / 2.0 - 3.5 * s, 7.0 * s, 7.0 * s), 1.2 * s, &paint(th().bg), None);
        text.draw(pm, sz, r.0 + 21.0 * s, r.1 + (r.3 - text.line_height(sz)) / 2.0, lab, th().bg);
        lay.stop = Some(r);
    } else if chat.input.is_empty() {
        let hint = if chat.histo.is_some() {
            t!("Tape pour chercher un fil · Entrée : ouvrir · Échap : fermer")
        } else if focused {
            t!("Écris ici… (Entrée : envoyer, Échap : fermer)")
        } else {
            t!("Clique ici pour écrire…")
        };
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
                rrect(pm, (xa, ty, xb - xa, lh), 3.0 * s, &paint_a(th().acc, 0.28), None);
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
    let hist: f32 = if zone(chat) { zone_h(chat) * s } else { lines[lines.len() - n..].iter().map(|l| lh * l.k).sum() };
    let confirm = chat.confirm.as_ref().map(|q| text.wrap(size * 0.95, q, 360.0 * s).len() as f32 * lh + 48.0 * s).unwrap_or(0.0);
    let presse = if presse_visible(chat) { 30.0 * s } else { 0.0 };
    24.0 * s + 24.0 * s + FIL_H * s + hist + confirm + presse + 8.0 * s + 34.0 * s
}

/// Hauteur de la ligne du fil ouvert, sous l'en-tête (avant mise à l'échelle).
const FIL_H: f32 = 20.0;

/// La rangée « Texte copié » s'affiche-t-elle ? (pas pendant un panneau, une question, une réponse)
fn presse_visible(chat: &crate::chat::Chat) -> bool {
    !zone(chat) && chat.confirm.is_none() && !chat.busy && chat.presse_propose().is_some()
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
    // Étiquette de papier : coins à peine arrondis ; pleine = encre de la couleur.
    let fill = if filled { paint(c) } else { paint_a(th().field, 0.55) };
    rrect(pm, r, (r.3 * 0.28).max(3.0), &fill, Some((c, 1.1)));
    let tw = text.width(size, label);
    let fg = if filled { if c == th().acc { th().acc_text } else { th().bg } } else { c };
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
    let actions: Vec<&str> = actions.iter().map(|a| crate::lang::trd(a)).collect();
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
                pm.fill_path(&path, &paint(mix(th().bg, th().edge, 0.1)), FillRule::Winding, Transform::identity(), None);
            } else {
                pm.stroke_path(&path, &paint_a(th().border, 0.7), &Stroke { width: 1.0, ..Default::default() }, Transform::identity(), None);
            }
        }
    };
    // Feuille de papier, puis la pointe qui s'y raccorde.
    papier(pm, rect, 10.0 * s, s);
    tail(pm, true);
    tail(pm, false);
    // Recouvre le liseré à la base de la pointe.
    line(pm, Transform::identity(), (tip_x - 9.0 * s, bottom - 1.6), (tip_x + 7.0 * s, bottom - 1.6), 2.8, mix(th().bg, th().edge, 0.1));

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
/// Loupe dessinée (recherche approfondie).
fn loupe(pm: &mut Pixmap, cx: f32, cy: f32, r: f32, c: (u8, u8, u8), s: f32) {
    let mut p = paint(c);
    p.anti_alias = true;
    if let Some(path) = PathBuilder::from_circle(cx, cy, r) {
        pm.stroke_path(&path, &p, &Stroke { width: 1.5 * s, ..Default::default() }, Transform::identity(), None);
    }
    line(pm, Transform::identity(), (cx + r * 0.72, cy + r * 0.72), (cx + r * 1.75, cy + r * 1.75), 1.8 * s, c);
}

/// Horloge dessinée (historique).
fn horloge(pm: &mut Pixmap, cx: f32, cy: f32, r: f32, c: (u8, u8, u8), s: f32) {
    let p = paint(c);
    if let Some(path) = PathBuilder::from_circle(cx, cy, r) {
        pm.stroke_path(&path, &p, &Stroke { width: 1.4 * s, ..Default::default() }, Transform::identity(), None);
    }
    line(pm, Transform::identity(), (cx, cy), (cx, cy - r * 0.62), 1.4 * s, c);
    line(pm, Transform::identity(), (cx, cy), (cx + r * 0.48, cy + r * 0.2), 1.4 * s, c);
}

/// Dossier dessiné (projet du côté Code).
fn dossier_ico(pm: &mut Pixmap, cx: f32, cy: f32, r: f32, c: (u8, u8, u8), s: f32) {
    let (x, y, w, h) = (cx - r, cy - r * 0.7, r * 2.0, r * 1.5);
    rrect(pm, (x, y + 1.6 * s, w, h - 1.6 * s), 1.4 * s, &paint(c), None);
    rrect(pm, (x, y, w * 0.45, 3.0 * s), 1.0 * s, &paint(c), None);
}

/// Historique des fils du côté affiché : recherche par le sens, nouveau fil, projets (côté
/// Code), liste rangée par projet ou par date, suppression confirmée.
fn draw_histo(pm: &mut Pixmap, text: &Text, chat: &crate::chat::Chat, area: R, s: f32, lay: &mut ChatLayout) {
    use crate::chat::{HistoAct as A, HistoSaisie};
    use crate::fils::Cote;
    let Some(h) = &chat.histo else { return };
    let (x, y, w, hh) = area;
    lay.picker = Some(area);
    let sz = 13.0 * s;
    let petit = 11.5 * s;
    let lhp = text.line_height(sz);
    // Champ de recherche + « Nouveau fil ».
    let nl = t!("+ Nouveau fil");
    let nw = text.width(petit, nl) + 18.0 * s;
    let champ = (x, y, w - nw - 6.0 * s, 26.0 * s);
    let recherche = h.saisie == HistoSaisie::Recherche;
    rrect(pm, champ, 6.0 * s, &paint(th().field), Some((if recherche { th().acc } else { th().field_border }, if recherche { 1.4 } else { 1.0 })));
    loupe(pm, champ.0 + 12.0 * s, champ.1 + 12.0 * s, 4.0 * s, th().faint, s);
    let ty = champ.1 + (champ.3 - lhp) / 2.0;
    if h.requete.is_empty() {
        text.draw(pm, sz, champ.0 + 24.0 * s, ty, t!("Chercher un fil (par le sens)…"), th().faint);
    } else {
        let q = short(&h.requete, 40);
        text.draw(pm, sz, champ.0 + 24.0 * s, ty, &q, th().text);
        if recherche {
            let cx = champ.0 + 24.0 * s + text.width(sz, &q) + 1.0;
            line(pm, Transform::identity(), (cx, ty + 2.0), (cx, ty + lhp - 2.0), 1.3, th().text);
        }
    }
    lay.histo.push((champ, A::SaisirRecherche));
    if recherche {
        lay.histo_champ = Some(champ);
    }
    let nb = (x + w - nw, y, nw, 26.0 * s);
    pill(pm, text, nb, nl, th().acc, true, petit);
    lay.histo.push((nb, A::Nouveau));
    let mut cy = y + 34.0 * s;

    // Côté Code : projets connus (nouveau fil dans ce projet) et « autre dossier ».
    if chat.cote == Cote::Code {
        text.draw(pm, petit, x, cy + 3.0 * s, t!("Projets :"), th().soft);
        let mut px = x + text.width(petit, t!("Projets :")) + 8.0 * s;
        let mut rang = 0;
        if h.saisie == HistoSaisie::Chemin {
            let ok = t!("Ouvrir");
            let ow = text.width(petit, ok) + 18.0 * s;
            let f = (px, cy, x + w - px - ow - 6.0 * s, 22.0 * s);
            rrect(pm, f, 5.0 * s, &paint(th().field), Some((th().acc, 1.4)));
            let c = short(&h.chemin, 44);
            let fy = f.1 + (f.3 - text.line_height(petit)) / 2.0;
            text.draw(pm, petit, f.0 + 7.0 * s, fy, &c, th().text);
            let cx = f.0 + 7.0 * s + text.width(petit, &c) + 1.0;
            line(pm, Transform::identity(), (cx, fy + 2.0), (cx, fy + text.line_height(petit) - 2.0), 1.3, th().text);
            lay.histo_champ = Some(f);
            let b = (x + w - ow, cy, ow, 22.0 * s);
            pill(pm, text, b, ok, th().acc, true, petit);
            lay.histo.push((b, A::ValiderChemin));
            cy += 28.0 * s;
        } else {
            for d in chat.projets().into_iter().take(6) {
                let nom = crate::fils::nom_projet(&d);
                let pw = text.width(petit, &nom) + 16.0 * s;
                if px + pw > x + w {
                    if rang == 1 {
                        break;
                    }
                    rang += 1;
                    px = x;
                    cy += 26.0 * s;
                }
                let r = (px, cy, pw, 22.0 * s);
                let actuel = chat.fil.dossier.as_deref() == Some(d.as_str());
                pill(pm, text, r, &nom, th().acc, actuel, petit);
                lay.histo.push((r, A::Projet(d)));
                px += pw + 5.0 * s;
            }
            let autre = t!("Autre dossier…");
            let aw = text.width(petit, autre) + 16.0 * s;
            if px + aw > x + w {
                px = x;
                cy += 26.0 * s;
            }
            let r = (px, cy, aw, 22.0 * s);
            pill(pm, text, r, autre, th().soft, false, petit);
            lay.histo.push((r, A::SaisirChemin));
            cy += 30.0 * s;
        }
    }

    // Message (erreur de chemin, fil supprimé…) ou confirmation de suppression, en bas.
    let bas_h = if h.supprimer.is_some() || h.message.is_some() { 34.0 * s } else { 0.0 };
    let bottom = y + hh - bas_h;

    // Liste : par projet (Code) ou par date (Discussion) ; par pertinence pendant une recherche.
    let fils = chat.histo_fils();
    let cherche = !h.requete.trim().is_empty();
    let mut lignes: Vec<(Option<String>, Option<crate::fils::Resume>)> = vec![];
    let mut groupe = String::new();
    let mut liste = fils.clone();
    if chat.cote == Cote::Code && !cherche {
        // Projets dans l'ordre de leur fil le plus récent, fils par date dans chaque projet.
        let ordre: Vec<String> = {
            let mut o: Vec<String> = vec![];
            for r in &fils {
                let d = r.dossier.clone().unwrap_or_else(|| "~".into());
                if !o.contains(&d) {
                    o.push(d);
                }
            }
            o
        };
        liste.sort_by_key(|r| ordre.iter().position(|d| *d == r.dossier.clone().unwrap_or_else(|| "~".into())).unwrap_or(0));
    }
    for r in liste {
        let g = if cherche {
            String::new()
        } else if chat.cote == Cote::Code {
            crate::fils::nom_projet(r.dossier.as_deref().unwrap_or("~"))
        } else {
            crate::lang::trd(crate::fils::periode(&r.maj)).to_string()
        };
        if !g.is_empty() && g != groupe {
            lignes.push((Some(g.clone()), None));
            groupe = g;
        }
        lignes.push((None, Some(r)));
    }
    if lignes.is_empty() {
        let vide = if cherche { t!("Aucun fil ne parle de ça.") } else { t!("Aucun fil pour l'instant : pose une question, il apparaîtra ici.") };
        text.draw(pm, sz, x, cy + 6.0 * s, vide, th().faint);
    }
    let row_h = 25.0 * s;
    let skip = h.scroll.min(lignes.len().saturating_sub(1));
    if skip > 0 {
        tri(pm, x + w / 2.0, cy - 2.0 * s, 4.0 * s, true, th().faint);
    }
    let mut cachees = 0;
    for (g, r) in lignes.into_iter().skip(skip) {
        if cy + row_h > bottom {
            cachees += 1;
            continue;
        }
        if let Some(g) = g {
            let gg = police(text, St::Gras);
            gg.draw(pm, petit, x, cy + 6.0 * s, &g, th().soft);
            cy += row_h * 0.9;
            continue;
        }
        let Some(r) = r else { continue };
        let actuel = r.id == chat.fil.id;
        let rr = (x, cy, w, row_h - 3.0 * s);
        if actuel {
            rrect(pm, rr, 5.0 * s, &paint_a(th().acc, 0.16), None);
        }
        let date = crate::fils::date_courte(&r.maj);
        let dw = text.width(petit, &date);
        let xw = 18.0 * s;
        let maxw = w - dw - xw - 22.0 * s;
        let mut t = if r.titre.is_empty() { t!("Sans titre").to_string() } else { r.titre.clone() };
        if cherche && chat.cote == Cote::Code {
            t = format!("{} · {t}", crate::fils::nom_projet(r.dossier.as_deref().unwrap_or("~")));
        }
        let complet = t.clone();
        let mut k = complet.chars().count();
        while k > 3 && text.width(sz, &t) > maxw {
            k -= 1;
            t = short(&complet, k);
        }
        let f = if actuel { police(text, St::Gras) } else { text };
        f.draw(pm, sz, x + 8.0 * s, cy + (rr.3 - lhp) / 2.0, &t, th().text);
        text.draw(pm, petit, x + w - xw - 6.0 * s - dw, cy + (rr.3 - text.line_height(petit)) / 2.0, &date, th().faint);
        let xr = (x + w - xw, cy + (rr.3 - xw) / 2.0, xw, xw);
        text.draw(pm, sz, xr.0 + 4.0 * s, xr.1 - 1.0 * s, "×", th().faint);
        lay.histo.push((xr, A::Supprimer(r.id.clone())));
        lay.histo.push(((x, cy, w - xw - 4.0 * s, rr.3), A::Ouvrir(r.id.clone())));
        cy += row_h;
    }
    if cachees > 0 {
        tri(pm, x + w / 2.0, bottom - 4.0 * s, 4.0 * s, false, th().faint);
    }
    // Confirmation ou message.
    if let Some(id) = &h.supprimer {
        let titre = fils.iter().find(|r| &r.id == id).map(|r| r.titre.clone()).unwrap_or_default();
        let q = t!("Mettre « {} » à la corbeille ?", short(&titre, 26));
        let by = bottom + 6.0 * s;
        text.draw(pm, petit, x, by + 4.0 * s, &q, th().text);
        let (oui, non) = (t!("Oui"), t!("Non"));
        let (ow, nw2) = (text.width(petit, oui) + 18.0 * s, text.width(petit, non) + 18.0 * s);
        let rn = (x + w - nw2, by, nw2, 22.0 * s);
        let ro = (rn.0 - ow - 6.0 * s, by, ow, 22.0 * s);
        pill(pm, text, ro, oui, th().err, true, petit);
        pill(pm, text, rn, non, th().soft, false, petit);
        lay.histo.push((ro, A::Confirmer(true)));
        lay.histo.push((rn, A::Confirmer(false)));
    } else if let Some((ok, m)) = &h.message {
        text.draw(pm, petit, x, bottom + 10.0 * s, &short(m, 70), if *ok { th().soft } else { th().err });
    }
}

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


impl Ui<'_> {
    fn show(&self, h: f32) -> bool {
        self.y >= self.top - 1.0 && self.y + h <= self.bottom + 1.0
    }

    /// Titre de section : Alegreya, en minuscules, à l'encre.
    fn section(&mut self, title: &str) {
        let h = 31.0 * self.s;
        if self.show(h) {
            let f = titres().unwrap_or(self.text);
            f.draw(self.pm, 16.0 * self.s, self.x, self.y + 8.0 * self.s, title, th().text);
        }
        self.y += h;
    }

    fn note(&mut self, t: &str) {
        let sz = 12.0 * self.s;
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
        let sz = 13.5 * self.s;
        self.text.draw(self.pm, sz, self.x, self.y + (h - self.text.line_height(sz)) / 2.0, &short(label, 40), th().text);
    }

    fn toggle(&mut self, label: &str, on: bool, act: crate::chat::Act) {
        let s = self.s;
        let h = 32.0 * s;
        if self.show(h) {
            self.label(label, h);
            let r = (self.x + self.w - 40.0 * s, self.y + 7.0 * s, 38.0 * s, 18.0 * s);
            rrect(self.pm, r, 9.0 * s, &paint(if on { th().acc } else { th().field_border }), None);
            let kx = if on { r.0 + r.2 - 9.0 * s } else { r.0 + 9.0 * s };
            disc(self.pm, Transform::identity(), kx, r.1 + 9.0 * s, 7.0 * s, &paint(th().field));
            self.acts.push(((self.x, self.y, self.w, h), act));
        }
        self.y += h;
    }

    fn stepper(&mut self, label: &str, value: &str, minus: crate::chat::Act, plus: crate::chat::Act) {
        let s = self.s;
        let h = 32.0 * s;
        if self.show(h) {
            self.label(label, h);
            let bw = 26.0 * s;
            let vw = 64.0 * s;
            let x0 = self.x + self.w - (bw * 2.0 + vw);
            let rm = (x0, self.y + 4.0 * s, bw, 24.0 * s);
            let rp = (x0 + bw + vw, self.y + 4.0 * s, bw, 24.0 * s);
            pill(self.pm, self.text, rm, "−", th().chip_text, false, 14.0 * s);
            pill(self.pm, self.text, rp, "+", th().chip_text, false, 14.0 * s);
            let tw = self.text.width(13.5 * s, value);
            self.text.draw(self.pm, 13.5 * s, x0 + bw + (vw - tw) / 2.0, self.y + (h - self.text.line_height(13.5 * s)) / 2.0, value, th().text);
            self.acts.push((rm, minus));
            self.acts.push((rp, plus));
        }
        self.y += h;
    }

    fn chips(&mut self, label: &str, options: &[(&str, &str)], current: &str, path: &'static str) {
        let s = self.s;
        if !label.is_empty() {
            let h = 23.0 * s;
            if self.show(h) {
                self.text.draw(self.pm, 13.5 * s, self.x, self.y + 2.0 * s, label, th().text);
            }
            self.y += h;
        }
        let mut cx = self.x;
        let ch = 27.0 * s;
        for (value, text) in options {
            let lw = self.text.width(12.5 * s, text) + 20.0 * s;
            if cx + lw > self.x + self.w {
                cx = self.x;
                self.y += ch + 4.0 * s;
            }
            if self.show(ch) {
                let r = (cx, self.y, lw, 24.0 * s);
                pill(self.pm, self.text, r, text, th().acc, *value == current, 12.5 * s);
                self.acts.push((r, crate::chat::Act::Choix(path, value.to_string())));
            }
            cx += lw + 5.0 * s;
        }
        self.y += ch + 6.0 * s;
    }

    fn buttons(&mut self, list: &[(&str, crate::chat::Act, bool)]) {
        let s = self.s;
        let h = 32.0 * s;
        let mut bx = self.x;
        for (label, act, filled) in list {
            let lw = self.text.width(12.5 * s, label) + 24.0 * s;
            if bx + lw > self.x + self.w {
                bx = self.x;
                self.y += h;
            }
            if self.show(h) {
                let r = (bx, self.y + 3.0 * s, lw, 25.0 * s);
                pill(self.pm, self.text, r, label, th().acc, *filled, 12.5 * s);
                self.acts.push((r, act.clone()));
            }
            bx += lw + 6.0 * s;
        }
        self.y += h;
    }

    #[allow(clippy::too_many_arguments)]
    fn field(&mut self, chat: &crate::chat::Chat, champ: crate::chat::Champ, placeholder: &str, save: Option<&str>) {
        let s = self.s;
        let h = 34.0 * s;
        if self.show(h) {
            let p = chat.panneau.as_ref();
            let value = p.and_then(|p| p.champs.get(&champ).cloned()).unwrap_or_default();
            let focused = p.is_some_and(|p| p.focus.as_ref() == Some(&champ));
            let bw = save.map(|l| self.text.width(12.5 * s, l) + 24.0 * s + 8.0 * s).unwrap_or(0.0);
            let r = (self.x, self.y + 2.0 * s, self.w - bw, 28.0 * s);
            // On écrit sur une ligne, comme dans un carnet ; la ligne verdit quand on y écrit.
            rrect(self.pm, r, 3.0 * s, &paint_a(th().field, if focused { 0.9 } else { 0.45 }), None);
            let (lc, lw) = if focused { (th().acc, 2.0 * s) } else { (th().field_border, 1.0 * s) };
            line(self.pm, Transform::identity(), (r.0, r.1 + r.3 - lw / 2.0), (r.0 + r.2, r.1 + r.3 - lw / 2.0), lw, lc);
            let sz = 13.0 * s;
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
                let rb = (r.0 + r.2 + 8.0 * s, r.1 + 2.0 * s, bw - 8.0 * s, 25.0 * s);
                pill(self.pm, self.text, rb, l, th().acc, true, 12.5 * s);
                self.acts.push((rb, crate::chat::Act::Enregistrer(champ)));
            }
        }
        self.y += h + 3.0 * s;
    }
}

/// Le modèle local conseillé pour la machine, et de quoi l'installer.
fn section_local(ui: &mut Ui) {
    use crate::chat::Act;
    ui.section(t!("Modèle local (sans Internet)"));
    match crate::local::etat() {
        None => ui.note(t!("Je regarde ta machine…")),
        Some(e) => {
            let c = e.conseil;
            ui.note(&t!("Ta machine : {}. Conseillé : {} ({} Go à télécharger).", crate::local::resume(&e.machine), c.tag, format!("{:.1}", c.poids_go)));
            if e.pret() {
                ui.note(t!("Installé : je m'en sers quand les modèles en ligne ne répondent pas, ou sans réseau."));
            } else if e.ollama {
                if let Some(m) = e.un_modele() {
                    ui.note(&t!("Déjà là : {} (je m'en sers en secours).", m));
                }
                ui.buttons(&[(&t!("Télécharger {}", c.tag), Act::TelechargerLocal(c.tag), true)]);
            } else {
                ui.note(t!("Il faut d'abord installer Ollama (gratuit), puis revenir ici pour télécharger le modèle."));
                ui.buttons(&[(t!("Installer Ollama…"), Act::Ouvrir("https://ollama.com/download/linux"), true)]);
            }
        }
    }
}

/// Contenu d'un onglet du panneau de réglages.
fn onglet(ui: &mut Ui, chat: &crate::chat::Chat, n: usize) {
    use crate::chat::{Act, Champ};
    let cfg = chat.cfg();
    let s = ui.s;
    match n {
        0 => {
            ui.section(t!("Toi"));
            ui.field(chat, Champ::Prenom, t!("Ton prénom"), Some(t!("Enregistrer")));
            ui.chips(t!("Langue"), &[("auto", t!("Comme le système")), ("fr", "Français"), ("en", "English")], &cfg.langue, "langue");
            ui.section(t!("Sa personnalité et ses règles"));
            ui.note(t!("Fichiers ouverts dans ton éditeur ; ils valent pour cet ordinateur et ce compte."));
            ui.buttons(&[(t!("Personnalité"), Act::OuvrirFichier("prompt.md"), false), (t!("Règles strictes"), Act::OuvrirFichier("regles.md"), false), (t!("Interdits"), Act::OuvrirFichier("regles.toml"), false), (t!("Ce qu'il sait de toi"), Act::OuvrirFichier("profil.md"), false)]);
            ui.section(t!("Contrôle de l'ordinateur"));
            ui.chips("", &[("demander", t!("Me demander")), ("confiance", t!("Confiance")), ("non", t!("Jamais"))], &cfg.controle, "controle");
            ui.note(match cfg.controle.as_str() {
                "non" => t!("Il ne touche à rien : ni fenêtres, ni clavier, ni son."),
                "confiance" => t!("Fenêtres, son, musique et luminosité sans demander. Fermer une fenêtre, taper et les raccourcis sont toujours demandés. Échap l'arrête."),
                _ => t!("Il demande avant chaque action : fenêtres, clavier, son, musique, luminosité. Jamais dans les gestionnaires de mots de passe, les banques ni les pages de connexion. Échap l'arrête."),
            });
            ui.section(t!("Cerveau"));
            let (i, nom) = chat.engines().first().cloned().unwrap_or_default();
            let m = chat.current_model(i);
            ui.note(&t!("{} · modèle : {}", nom, if m == "auto" { t!("automatique (le meilleur gratuit pour chaque demande)").to_string() } else { m }));
            ui.buttons(&[(t!("Choisir le modèle…"), Act::ChoisirModele, true), (t!("Rafraîchir les modèles"), Act::RafraichirCatalogue, false)]);
        }
        1 => {
            let m = &cfg.maki;
            ui.stepper(t!("Durée d'une bulle"), &format!("{} s", m.bulle_s), Act::Pas("maki.bulle_s", -1.0, 3.0, 60.0), Act::Pas("maki.bulle_s", 1.0, 3.0, 60.0));
            let c = if m.conseils_h == 0 { t!("jamais").to_string() } else { format!("{} h", m.conseils_h) };
            ui.stepper(t!("Une astuce toutes les"), &c, Act::Pas("maki.conseils_h", -1.0, 0.0, 24.0), Act::Pas("maki.conseils_h", 1.0, 0.0, 24.0));
            ui.stepper(t!("Remarques au plus toutes les"), &format!("{} min", m.parole_min), Act::Pas("maki.parole_min", -5.0, 5.0, 240.0), Act::Pas("maki.parole_min", 5.0, 5.0, 240.0));
            ui.toggle(t!("Numéros (arbre, escalade, téléportation)"), m.acrobaties, Act::Bascule("maki.acrobaties"));
            ui.stepper(t!("Un numéro au plus toutes les"), &format!("{} min", m.numeros_min), Act::Pas("maki.numeros_min", -5.0, 5.0, 240.0), Act::Pas("maki.numeros_min", 5.0, 5.0, 240.0));
            ui.note(t!("Les rappels s'affichent toujours, quels que soient ces réglages. Clic droit sur le maki : mode discret."));
        }
        2 => {
            let m = &cfg.maki;
            ui.stepper(t!("Taille du texte"), &format!("× {:.1}", m.texte), Act::Pas("maki.texte", -0.1, 0.8, 2.2), Act::Pas("maki.texte", 0.1, 0.8, 2.2));
            ui.chips(t!("Thème des bulles"), &[("auto", t!("Comme KDE")), ("clair", t!("Clair")), ("sombre", t!("Sombre"))], &m.theme, "maki.theme");
            let opts: Vec<(&str, &str)> = super::especes::Espece::TOUTES.iter().map(|e| (e.id(), crate::lang::trd(e.nom()).split(" (").next().unwrap_or(""))).collect();
            ui.chips(t!("Personnage"), &opts, &m.espece, "maki.espece");
            ui.stepper(t!("Taille"), &format!("× {:.1}", m.taille), Act::Pas("maki.taille", -0.1, 0.6, 2.5), Act::Pas("maki.taille", 0.1, 0.6, 2.5));
            ui.chips(t!("Où il se promène"), &[("bords", t!("Sur les bords")), ("libre", t!("Partout"))], &m.zone, "maki.zone");
            ui.toggle(t!("Passe d'un écran à l'autre"), m.ecrans_multiples, Act::Bascule("maki.ecrans_multiples"));
            ui.toggle(t!("Te suit sur l'écran où tu travailles"), m.suivre, Act::Bascule("maki.suivre"));
        }
        3 => {
            ui.toggle(t!("Lancer à l'ouverture de session"), cfg.demarrage_auto, Act::Bascule("demarrage_auto"));
            ui.toggle(t!("Index local de mes fichiers"), cfg.index, Act::Bascule("index"));
            ui.chips(t!("Mises à jour"), &[("proposer", t!("Me demander")), ("auto", t!("Automatiques")), ("non", t!("Jamais"))], &cfg.maj, "maj");
            ui.buttons(&[(t!("Vérifier maintenant"), Act::VerifierMaj, false)]);
            ui.note(&t!("Version installée : {}.", crate::maj::current()));
            let [a, b] = cfg.maki.sommeil;
            ui.stepper(t!("Il s'endort à"), &format!("{a} h"), Act::Pas("maki.sommeil.1", -1.0, 0.0, 23.0), Act::Pas("maki.sommeil.1", 1.0, 0.0, 23.0));
            ui.stepper(t!("Il se réveille à"), &format!("{b} h"), Act::Pas("maki.sommeil.2", -1.0, 0.0, 23.0), Act::Pas("maki.sommeil.2", 1.0, 0.0, 23.0));
        }
        4 => {
            if !cfg.mail.is_empty() {
                ui.section(t!("Mails"));
            }
            for (i, c) in cfg.mail.iter().enumerate() {
                ui.note(&t!("{} ({}) — branché", c.adresse, c.nom));
                ui.buttons(&[(t!("Tester"), Act::TesterMail(i), false), (t!("Retirer"), Act::RetirerMail(i), false)]);
            }
            if !cfg.mail.is_empty() {
                ui.toggle(t!("Me prévenir des nouveaux mails"), cfg.alertes.mails, Act::Bascule("alertes.mails"));
            }
            ui.section(if cfg.mail.is_empty() { t!("Brancher Gmail en 2 étapes") } else { t!("Ajouter un autre compte") });
            ui.note(t!("1. Crée un « mot de passe d'application » (il faut la validation en deux étapes) :"));
            ui.buttons(&[(t!("Ouvrir la page Google"), Act::OuvrirLien("https://myaccount.google.com/apppasswords"), false)]);
            ui.note(t!("2. Colle ton adresse et les 16 lettres données par Google :"));
            ui.field(chat, Champ::MailAdresse, t!("ton.adresse@gmail.com"), None);
            ui.field(chat, Champ::MailPasse, t!("Les 16 lettres (les espaces ne comptent pas)"), Some(t!("Brancher")));
            ui.note(t!("Autre messagerie : ton adresse et ton mot de passe habituel. Tout va dans le trousseau."));
            ui.section(t!("Agenda"));
            if crate::agenda::a_un_agenda(&cfg.agenda) {
                if !cfg.agenda.is_empty() {
                    ui.note(&t!("{} agenda(s) branché(s).", cfg.agenda.len()));
                }
                ui.toggle(t!("Me prévenir avant mes rendez-vous"), cfg.alertes.agenda, Act::Bascule("alertes.agenda"));
                ui.stepper(t!("Combien de temps avant"), &format!("{} min", cfg.alertes.avance_min), Act::Pas("alertes.avance_min", -5.0, 5.0, 120.0), Act::Pas("alertes.avance_min", 5.0, 5.0, 120.0));
            }
            ui.note(t!("Google Agenda : ouvre les réglages, choisis ton agenda à gauche, puis « Intégrer l'agenda » › copie l'« adresse secrète au format iCal »."));
            ui.buttons(&[(t!("Ouvrir les réglages de Google Agenda"), Act::OuvrirLien("https://calendar.google.com/calendar/r/settings"), false)]);
            ui.field(chat, Champ::Agenda, t!("Colle l'adresse iCal (ou un fichier .ics)"), Some(t!("Brancher")));
            ui.section(t!("Contacts"));
            ui.field(chat, Champ::Contacts, t!("Fichier .vcf (export de Google Contacts…)"), Some(t!("Enregistrer")));
        }
        5 => {
            section_local(ui);
            ui.section(t!("Fournisseurs gratuits"));
            ui.note(t!("Le moteur intégré marche sans aucune clé. Une clé gratuite ajoute les modèles d'un fournisseur ; elle reste dans le trousseau."));
            for payants in [false, true] {
                if payants {
                    ui.section(t!("Fournisseurs payants"));
                    ui.note(t!("Facturés à l'usage par le fournisseur. Leurs modèles ne servent que si tu les choisis dans la liste des modèles, ou en premier si tu l'actives ici."));
                    ui.toggle(t!("Utiliser mes modèles payants en premier"), cfg.payants_en_premier, Act::Bascule("payants_en_premier"));
                }
                for f in crate::cles::FOURNISSEURS.iter().filter(|f| f.payant == payants) {
                    let key = crate::cles::cle(f.id);
                    let hdr = 22.0 * s;
                    if ui.show(hdr) {
                        ui.text.draw(ui.pm, 12.5 * s, ui.x, ui.y + 3.0 * s, f.nom, th().text);
                        if let Some(k) = &key {
                            let st = t!("clé enregistrée {}", crate::cles::masked(k));
                            let sw = ui.text.width(10.5 * s, &st);
                            check(ui.pm, ui.x + ui.w - sw - 16.0 * s, ui.y + 5.0 * s, s * 0.9, th().acc);
                            ui.text.draw(ui.pm, 10.5 * s, ui.x + ui.w - sw, ui.y + 4.0 * s, &st, th().acc);
                        }
                    }
                    ui.y += hdr;
                    ui.note(crate::lang::trd(f.description));
                    if key.is_some() {
                        ui.buttons(&[(t!("Retirer la clé"), Act::RetirerCle(f.id), false)]);
                    } else {
                        ui.buttons(&[(t!("Obtenir une clé"), Act::ObtenirCle(f.id), false)]);
                        ui.field(chat, Champ::Cle(f.id), t!("Colle la clé ici"), Some(t!("Enregistrer")));
                    }
                    ui.y += 4.0 * s;
                }
            }
        }
        _ => {
            use crate::apropos as a;
            ui.section("L Katta (Lemur Kata)");
            ui.note(&t!("Version {} · licence Apache-2.0. Un petit lémurien assistant pour ton bureau Linux.", env!("CARGO_PKG_VERSION")));
            ui.section(t!("Conçu par"));
            ui.note(a::AUTEUR);
            ui.note(&format!("{} {}", t!("E-mail :"), a::MAIL));
            ui.note(&format!("{} {}", t!("Téléphone et WhatsApp :"), a::TELEPHONE));
            ui.buttons(&[(t!("Écrire un e-mail"), Act::Ouvrir("mailto:antsa.raniriamanjaka@univ-antananarivo.mg?subject=L%20Katta"), true), ("WhatsApp", Act::Ouvrir(a::WHATSAPP), false)]);
            ui.section(t!("Liens"));
            ui.buttons(&[(t!("Site"), Act::Ouvrir(a::SITE), false), (t!("Code source"), Act::Ouvrir(a::DEPOT), false), (t!("Signaler un problème"), Act::Ouvrir("https://github.com/raantss18/L-Katta/issues"), false)]);
            ui.note(t!("Moteur d'IA dérivé de Tontolo, outils repris de mi-saina."));
        }
    }
}

/// Une étape de l'assistant de premier lancement.
fn etape_accueil(ui: &mut Ui, chat: &crate::chat::Chat, e: usize) {
    use crate::chat::{Act, Champ};
    use super::especes::Espece;
    let cfg = chat.cfg();
    let s = ui.s;
    match e {
        0 => {
            ui.note(t!("Bienvenue ! En quelques petites étapes, je m'adapte à toi. Tout se change plus tard dans les réglages (roue dentée)."));
            ui.chips(t!("Dans quelle langue parlons-nous ?"), &[("auto", t!("Comme le système")), ("fr", "Français"), ("en", "English")], &cfg.langue, "langue");
        }
        1 => {
            ui.section(t!("Comment t'appelles-tu ?"));
            ui.field(chat, Champ::Prenom, t!("Ton prénom"), None);
            ui.note(t!("Je t'appellerai ainsi dans mes bulles."));
        }
        2 => {
            ui.section(t!("Quel lémurien veux-tu ?"));
            let gap = 6.0 * s;
            let cw = (ui.w - 2.0 * gap) / 3.0;
            let ch = 96.0 * s;
            for (i, sp) in Espece::TOUTES.iter().enumerate() {
                let (col, row) = (i % 3, i / 3);
                let r = (ui.x + col as f32 * (cw + gap), ui.y + row as f32 * (ch + gap), cw, ch);
                if r.1 >= ui.top - 1.0 && r.1 + r.3 <= ui.bottom + 1.0 {
                    let on = sp.id() == cfg.maki.espece;
                    // Fond de carte assez clair pour que les espèces noires (indri, aye-aye) se voient.
                    let fond = if th().sombre { mix(th().field, th().text, 0.42) } else { th().field };
                    rrect(ui.pm, r, 6.0 * s, &paint(fond), Some((if on { th().acc } else { th().field_border }, if on { 2.0 } else { 1.0 })));
                    let pose = super::behavior::Pose { kind: super::behavior::PoseKind::Sit, pop: 1.0, time: 0.6, bipede: sp.style().bipede, ..Default::default() };
                    let k = 0.55 * s / sp.pal().size.clamp(0.8, 1.3);
                    let base = Transform::from_translate(r.0 + r.2 / 2.0, r.1 + 70.0 * s).pre_scale(k, k);
                    super::dessin::maki(ui.pm, base, &pose, &sp.pal(), super::behavior::Attitude::Aucune);
                    let nom = crate::lang::trd(sp.nom()).split(" (").next().unwrap_or("").to_string();
                    let sz = 11.0 * s;
                    let nw = ui.text.width(sz, &nom);
                    let nc = if th().sombre { if on { (24, 60, 36) } else { th().bg } } else if on { th().acc } else { th().text };
                    ui.text.draw(ui.pm, sz, r.0 + (r.2 - nw).max(0.0) / 2.0, r.1 + 76.0 * s, &short(&nom, 18), nc);
                    ui.acts.push((r, Act::Choix("maki.espece", sp.id().to_string())));
                }
            }
            ui.y += 2.0 * (ch + gap) + 4.0 * s;
            if let Some(sp) = Espece::parse(&cfg.maki.espece) {
                ui.note(&format!("{} — {}", crate::lang::trd(sp.nom()), crate::lang::trd(sp.description())));
            }
            let m = &cfg.maki;
            ui.stepper(t!("Taille"), &format!("× {:.1}", m.taille), Act::Pas("maki.taille", -0.1, 0.6, 2.5), Act::Pas("maki.taille", 0.1, 0.6, 2.5));
        }
        3 => {
            let m = &cfg.maki;
            ui.note(t!("Je me tais pendant tes visios, tes présentations, en plein écran et en « Ne pas déranger ». Clic droit sur moi : mode discret."));
            ui.chips(t!("Où il se promène"), &[("bords", t!("Sur les bords")), ("libre", t!("Partout"))], &m.zone, "maki.zone");
            ui.toggle(t!("Numéros (arbre, escalade, téléportation)"), m.acrobaties, Act::Bascule("maki.acrobaties"));
            let c = if m.conseils_h == 0 { t!("jamais").to_string() } else { format!("{} h", m.conseils_h) };
            ui.stepper(t!("Une astuce toutes les"), &c, Act::Pas("maki.conseils_h", -1.0, 0.0, 24.0), Act::Pas("maki.conseils_h", 1.0, 0.0, 24.0));
            ui.toggle(t!("Lancer à l'ouverture de session"), cfg.demarrage_auto, Act::Bascule("demarrage_auto"));
        }
        4 => {
            ui.note(t!("Je marche sans compte ni clé, avec des modèles d'IA gratuits en ligne. Un modèle installé sur ta machine prend le relais sans Internet."));
            section_local(ui);
            ui.note(t!("Tu as une clé (gratuite, ou payante comme Anthropic ou OpenAI) ? Ajoute-la plus tard dans les réglages › Modèles d'IA."));
        }
        _ => {
            ui.note(t!("Facultatif : je peux te prévenir des nouveaux mails et de tes rendez-vous. Tu peux aussi le faire plus tard (/mails, /agenda)."));
            onglet(ui, chat, 4);
        }
    }
}

/// Panneau de réglages dans la bulle (zone fixe, contenu qui défile).
fn draw_panel(pm: &mut Pixmap, text: &Text, chat: &crate::chat::Chat, area: R, s: f32, lay: &mut ChatLayout) {
    use crate::chat::{Act, ETAPES, ONGLETS};
    let Some(p) = &chat.panneau else { return };
    let (x, y, w, h) = area;
    lay.picker = Some(area);
    // Onglets (fixes).
    let mut tx = x;
    let mut ty = y;
    let onglets: &[&str] = if p.accueil.is_some() { &[] } else { &ONGLETS };
    if let Some(e) = p.accueil {
        // Assistant : titre de l'étape et barre de progression.
        // Le nom de l'étape en titre ; « 2 sur 6 » discret à droite.
        let serif = titres().unwrap_or(text);
        serif.draw(pm, 17.0 * s, x, y - 2.0 * s, crate::lang::trd(ETAPES[e]), th().text);
        let rang = t!("{} sur {}", e + 1, ETAPES.len());
        text.draw(pm, 12.0 * s, x + w - text.width(12.0 * s, &rang), y + 2.0 * s, &rang, th().soft);
        let n = ETAPES.len() as f32;
        let sw = (w - (n - 1.0) * 4.0 * s) / n;
        for i in 0..ETAPES.len() {
            let r = (x + i as f32 * (sw + 4.0 * s), y + 24.0 * s, sw, 3.0 * s);
            rrect(pm, r, 2.0 * s, &paint(if i <= e { th().acc } else { th().field_border }), None);
        }
        ty += 4.0 * s;
    }
    // Onglets : des mots, l'actif souligné à l'encre verte (comme un signet).
    for (i, name) in onglets.iter().map(|n| crate::lang::trd(n)).enumerate() {
        let sz = 12.5 * s;
        let lw = text.width(sz, name);
        if tx + lw > x + w {
            tx = x;
            ty += 27.0 * s;
        }
        let actif = i == p.onglet;
        text.draw(pm, sz, tx, ty + 2.0 * s, name, if actif { th().acc } else { th().soft });
        if actif {
            line(pm, Transform::identity(), (tx, ty + 22.0 * s), (tx + lw, ty + 22.0 * s), 2.2 * s, th().acc);
        }
        lay.picks_panel.push(((tx - 5.0 * s, ty - 2.0 * s, lw + 10.0 * s, 27.0 * s), Act::Onglet(i)));
        tx += lw + 11.0 * s;
    }
    if !onglets.is_empty() {
        line(pm, Transform::identity(), (x, ty + 23.5 * s), (x + w, ty + 23.5 * s), 1.0, mix(th().bg, th().border, 0.55));
    }
    let top = ty + 30.0 * s;
    let msg_h = if p.message.is_some() { 34.0 * s } else { 0.0 };
    let nav_h = if p.accueil.is_some() { 32.0 * s } else { 0.0 };
    let bottom = y + h - msg_h - nav_h;
    let mut acts = vec![];
    let start = top - p.scroll * s;
    let scrollable = p.contenu.get() > 470.0;
    let cw = if scrollable { w - 12.0 * s } else { w };
    {
        let mut ui = Ui { pm, text, s, x, w: cw, y: start, top, bottom, acts: &mut acts };
        match p.accueil {
            Some(e) => etape_accueil(&mut ui, chat, e),
            None => onglet(&mut ui, chat, p.onglet),
        }
        lay.panel_content = (ui.y - start) / s;
        lay.panel_view = (bottom - top) / s;
        // Hauteur totale voulue : onglets + contenu + message (la bulle s'y ajuste).
        p.contenu.set((top - y) / s + lay.panel_content + (msg_h + nav_h) / s + 6.0);
    }
    lay.picks_panel.extend(acts);
    // Assistant : boutons de navigation fixes en bas.
    if let Some(e) = p.accueil {
        let by = y + h - msg_h - nav_h + 6.0 * s;
        let sz = 12.0 * s;
        let bh = 24.0 * s;
        let derniere = e + 1 == ETAPES.len();
        let suivant = if derniere { t!("Terminer") } else { t!("Suivant ›") };
        let lw = text.width(sz, suivant) + 26.0 * s;
        let r = (x + w - lw, by, lw, bh);
        pill(pm, text, r, suivant, th().acc, true, sz);
        lay.picks_panel.push((r, if derniere { Act::FinAccueil } else { Act::Etape(e + 1) }));
        let mut bx = x;
        if e > 0 {
            let l = t!("‹ Retour");
            let lw = text.width(sz, l) + 22.0 * s;
            let r = (bx, by, lw, bh);
            pill(pm, text, r, l, th().acc, false, sz);
            lay.picks_panel.push((r, Act::Etape(e - 1)));
            bx += lw + 6.0 * s;
        }
        if !derniere {
            let l = t!("Passer");
            let lw = text.width(sz, l) + 22.0 * s;
            let r = (bx, by, lw, bh);
            pill(pm, text, r, l, th().chip_text, false, sz);
            lay.picks_panel.push((r, Act::FinAccueil));
        }
    }
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
        let fg = if *ok { th().acc } else { th().err };
        let bg = mix(th().field, fg, 0.12);
        rrect(pm, r, 8.0 * s, &paint(bg), None);
        for (i, l) in text.wrap(11.0 * s, m, w - 16.0 * s).iter().take(2).enumerate() {
            text.draw(pm, 11.0 * s, r.0 + 8.0 * s, r.1 + 3.0 * s + i as f32 * 13.0 * s, l, fg);
        }
    }
}

/// Pose `src` sur `dst` (même taille, RGBA prémultiplié) dans le rectangle `r` seulement,
/// avec une opacité : bien plus rapide qu'une composition de toute l'image.
/// Pose `src` sur `dst` à (dx, dy) (RGBA prémultiplié, « par-dessus ») : copie directe des
/// pixels opaques, mélange entier ailleurs. Bien plus rapide que `draw_pixmap`.
pub fn poser(dst: &mut Pixmap, src: &Pixmap, dx: i32, dy: i32) {
    let (dw, dh) = (dst.width() as i32, dst.height() as i32);
    let (sw, sh) = (src.width() as i32, src.height() as i32);
    let x0 = dx.max(0);
    let x1 = (dx + sw).min(dw);
    if x1 <= x0 {
        return;
    }
    let sd = src.data();
    let dd = dst.data_mut();
    for y in dy.max(0)..(dy + sh).min(dh) {
        let so = ((y - dy) * sw + (x0 - dx)) as usize * 4;
        let doff = (y * dw + x0) as usize * 4;
        let n = (x1 - x0) as usize * 4;
        let (s, d) = (&sd[so..so + n], &mut dd[doff..doff + n]);
        for (sp, dp) in s.chunks_exact(4).zip(d.chunks_exact_mut(4)) {
            let sa = sp[3] as u32;
            if sa == 255 {
                dp.copy_from_slice(sp);
            } else if sa != 0 {
                let inv = 255 - sa;
                for c in 0..4 {
                    dp[c] = (sp[c] as u32 + (dp[c] as u32 * inv + 127) / 255).min(255) as u8;
                }
            }
        }
    }
}

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


#[cfg(test)]
mod tests_texte {
    use super::*;

    #[test]
    fn emojis_are_drawn_and_invisibles_take_no_room() {
        let t = Text::load().unwrap();
        let sans = t.width(16.0, "KCalc est ouverte.");
        let avec = t.width(16.0, "KCalc est ouverte. 🧮");
        assert!(avec - sans > 8.0, "{sans} {avec}");
        // Sélecteur de variante : aucune place.
        assert_eq!(t.width(16.0, "❤\u{FE0F}"), t.width(16.0, "❤"));
        // Aperçu : `LKATTA_APERCU_EMOJI=fichier.png cargo test emojis`.
        if let Ok(f) = std::env::var("LKATTA_APERCU_EMOJI") {
            let mut pm = Pixmap::new(560, 70).unwrap();
            pm.fill(Color::from_rgba8(238, 225, 193, 255));
            t.draw(&mut pm, 20.0, 12.0, 8.0, "KCalc est ouverte. 🧮 ✅ 👍 🎉 ❤️ 💡 ⚠️ 🐒", (43, 33, 24));
            t.draw(&mut pm, 20.0, 12.0, 38.0, "Coche ✓, flèche ↳, puce ● · drapeau 🇲🇬 · peau 👍🏾", (43, 33, 24));
            pm.save_png(f).unwrap();
        }
    }
}
