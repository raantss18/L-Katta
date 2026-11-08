//! Installation pour l'utilisateur courant (sans droits administrateur) et aperçu.

use tiny_skia::{Pixmap, Transform};

use crate::mascot::{behavior::Pose, draw};
use crate::paths;

fn desktop(exec: &str, name: &str, comment: &str, extra: &str) -> String {
    format!(
        "[Desktop Entry]\nType=Application\nName={name}\nComment={comment}\nExec={exec}\nIcon=lkatta\n\
         Terminal=false\nCategories=Utility;\nStartupWMClass=lkatta\n{extra}"
    )
}

pub fn install() -> Result<(), String> {
    let home = paths::home_dir();
    let bin = home.join(".local/bin/lkatta");
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(bin.parent().unwrap()).map_err(|e| e.to_string())?;
    if exe != bin {
        // Copie via un fichier temporaire : remplace même un binaire en cours d'exécution.
        let tmp = bin.with_extension("new");
        std::fs::copy(&exe, &tmp).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &bin).map_err(|e| e.to_string())?;
    }
    let b = bin.display();
    let apps = home.join(".local/share/applications");
    let auto = paths::home_dir().join(".config/autostart");
    let icons = home.join(".local/share/icons/hicolor/256x256/apps");
    for d in [&apps, &auto, &icons] {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    std::fs::write(
        apps.join("lkatta.desktop"),
        desktop(&format!("{b} panneau"), "L Katta", "Discuter avec le maki assistant", "Keywords=assistant;ia;maki;\n"),
    )
    .map_err(|e| e.to_string())?;
    std::fs::write(
        auto.join("lkatta.desktop"),
        desktop(&b.to_string(), "L Katta (maki)", "Le petit maki assistant du bureau", "X-KDE-autostart-phase=2\n"),
    )
    .map_err(|e| e.to_string())?;
    icon(256).save_png(icons.join("lkatta.png")).map_err(|e| e.to_string())?;
    println!(
        "Installé : {b}\nDémarrage automatique : {}\nLanceur : « L Katta » dans le menu.\n\
         Astuce KDE : Configuration › Raccourcis › ajouter « L Katta » pour ouvrir le panneau au clavier.\n\
         Lancer maintenant : lkatta &",
        auto.join("lkatta.desktop").display()
    );
    Ok(())
}

pub fn uninstall() -> Result<(), String> {
    let home = paths::home_dir();
    for p in [
        home.join(".config/autostart/lkatta.desktop"),
        home.join(".local/share/applications/lkatta.desktop"),
        home.join(".local/share/icons/hicolor/256x256/apps/lkatta.png"),
    ] {
        let _ = std::fs::remove_file(p);
    }
    let _ = crate::ipc::send("quitter");
    println!(
        "Démarrage automatique et lanceur retirés. Le binaire ~/.local/bin/lkatta et vos données\n({} , {}) sont conservés.",
        paths::config_dir().display(),
        paths::data_dir().display()
    );
    Ok(())
}

/// Icône : la tête du maki.
pub fn icon(size: u32) -> Pixmap {
    let mut pm = Pixmap::new(size, size).unwrap();
    let s = size as f32 / 120.0;
    let pose = Pose { kind: crate::mascot::behavior::PoseKind::Sit, pop: 1.0, ..Default::default() };
    draw::maki(&mut pm, Transform::from_translate(size as f32 * 0.55, size as f32 * 0.97).pre_scale(s, s), &pose);
    pm
}

/// Planche des poses, utile pour la documentation et les essais.
pub fn preview(out: &str) -> Result<(), String> {
    let cell = 170.0;
    use crate::mascot::behavior::PoseKind as K;
    let base = Pose { pop: 1.0, ..Default::default() };
    let poses = [
        ("marche", Pose { kind: K::Walk, walk_phase: 0.6, ..base }),
        ("assis", Pose { kind: K::Sit, ..base }),
        ("salut", Pose { kind: K::Sit, wave: 0.8, ..base }),
        ("se gratte", Pose { kind: K::Sit, scratch: 0.7, ..base }),
        ("s'étire", Pose { kind: K::Stretch, walk_phase: 0.5, ..base }),
        ("vol", Pose { kind: K::Fly, spin: -0.5, squash: -0.08, ..base }),
        ("escalade", Pose { kind: K::Climb, walk_phase: 1.0, ..base }),
        ("suspendu", Pose { kind: K::Dangle { arms_up: true }, swing: 0.15, ..base }),
        ("dodo", Pose { kind: K::Sleep, blink: true, ..base }),
    ];
    let mut pm = Pixmap::new((cell * poses.len() as f32) as u32, 330).unwrap();
    let cell = cell;
    pm.fill(tiny_skia::Color::from_rgba8(40, 42, 48, 255));
    let text = draw::Text::load();
    for (i, (name, p)) in poses.iter().enumerate() {
        let x = cell * i as f32 + cell / 2.0;
        let y = if matches!(p.kind, K::Climb) { 220.0 } else { 290.0 };
        draw::maki(&mut pm, Transform::from_translate(x, y).pre_scale(1.1, 1.1), p);
        draw::outline(&mut pm, (x - 85.0, 20.0, 170.0, 300.0), 2, (236, 232, 222), 0.85);
        if let Some(t) = &text {
            let w = t.width(15.0, name);
            t.draw(&mut pm, 14.0, x - w / 2.0, 308.0, name, (220, 220, 228));
        }
    }
    if let Some(t) = &text {
        draw::bubble(&mut pm, t, "Bonjour Antsa ! Il est 9h. Un rappel aujourd'hui : réunion à 14h.", cell * 1.5 + 14.0, 186.0, 1.0);
    }
    pm.save_png(out).map_err(|e| e.to_string())?;
    println!("Aperçu enregistré : {out}");
    Ok(())
}

/// Planche des bulles et des expressions (documentation, vérification du dessin).
pub fn preview_bubbles(out: &str) -> Result<(), String> {
    use crate::conseils::{Kind, Message};
    use crate::mascot::behavior::{Bubble, Expr, PoseKind as K};
    let text = draw::Text::load().ok_or("pas de police")?;
    let bold = draw::load_bold().unwrap_or_else(|| draw::Text::load().unwrap());
    let cell = 420.0;
    let mut pm = Pixmap::new((cell * 3.0) as u32, 620).unwrap();
    pm.fill(tiny_skia::Color::from_rgba8(36, 38, 44, 255));
    let samples = [
        (Message::new(Kind::Hello, "Bonjour Antsa !", "Lundi 9 novembre — 9h05\nAujourd'hui : réunion du labo à 14h."), vec!["Discuter", "Une astuce"], Expr::Happy, true, false),
        (crate::conseils::astuce(), vec!["Autre astuce", "Merci"], Expr::Surprised, false, true),
        (Message::new(Kind::Reminder, "Rappel", "Appeler Hery pour le dossier COMIMa."), vec!["C'est noté", "Dans 10 min"], Expr::Surprised, true, false),
        (Message::new(Kind::Alert, "Espace disque faible", "/home est utilisé à 91 %. Repère les plus gros dossiers."), vec!["Qu'en faire ?", "Plus tard"], Expr::Worried, false, false),
        (Message::new(Kind::News, "Nouvelle Arch Linux", "« Intervention manuelle pour glibc » — à lire avant la prochaine mise à jour."), vec!["Lire", "OK"], Expr::Surprised, false, true),
        (Message::new(Kind::Answer, "Voilà !", "Le dossier le plus lourd est ~/Videos (38 Go), puis ~/.cache (6 Go)."), vec!["Lire en entier", "OK"], Expr::Happy, true, false),
    ];
    for (i, (m, acts, expr, wave, point)) in samples.into_iter().enumerate() {
        let (cx, base) = (cell * (i % 3) as f32 + 150.0, 290.0 + 310.0 * (i / 3) as f32);
        let p = Pose { kind: K::Sit, pop: 1.0, expr, wave: if wave { 0.7 } else { 0.0 }, point, ..Default::default() };
        let mut sprite = Pixmap::new(pm.width(), pm.height()).unwrap();
        draw::maki(&mut sprite, Transform::from_translate(cx, base).pre_scale(1.0, 1.0), &p);
        draw::outline(&mut sprite, (cx - 80.0, base - 140.0, 160.0, 150.0), 2, (236, 232, 222), 0.85);
        pm.draw_pixmap(0, 0, sprite.as_ref(), &tiny_skia::PixmapPaint::default(), Transform::identity(), None);
        let b = Bubble { msg: m, ttl: 10.0, age: 60.0 };
        draw::fancy_bubble(&mut pm, &text, &bold, &b, &acts, cx + 14.0, base - 88.0, 1.0);
    }
    pm.save_png(out).map_err(|e| e.to_string())?;
    println!("Planche des bulles : {out}");
    Ok(())
}

