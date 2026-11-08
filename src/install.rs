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
    // Lancé depuis l'AppImage : on installe l'AppImage entière (un seul fichier).
    let exe = match std::env::var_os("APPIMAGE") {
        Some(a) => std::path::PathBuf::from(a),
        None => crate::paths::self_exe(),
    };
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
        desktop(&format!("env APPIMAGELAUNCHER_DISABLE=1 {b} panneau"), "L Katta", "Discuter avec le maki assistant", "Keywords=assistant;ia;maki;\n"),
    )
    .map_err(|e| e.to_string())?;
    let auto_msg = sync_autostart(crate::config::Config::load().demarrage_auto)?;
    icon(256).save_png(icons.join("lkatta.png")).map_err(|e| e.to_string())?;
    println!(
        "Installé : {b}\n{auto_msg}\nLanceur : « L Katta » dans le menu.\n\
         Astuce KDE : Configuration › Raccourcis › ajouter « L Katta » pour ouvrir la bulle au clavier.\n\
         Lancer maintenant : lkatta &"
    );
    let _ = &auto;
    Ok(())
}

/// Démarrage automatique à l'ouverture de session (fichier ~/.config/autostart/lkatta.desktop).
pub fn sync_autostart(on: bool) -> Result<String, String> {
    let file = paths::home_dir().join(".config/autostart/lkatta.desktop");
    if !on {
        let _ = std::fs::remove_file(&file);
        return Ok("Démarrage automatique : désactivé.".into());
    }
    let bin = paths::home_dir().join(".local/bin/lkatta");
    let exe = if bin.exists() { bin } else { crate::paths::self_exe() };
    std::fs::create_dir_all(file.parent().unwrap()).map_err(|e| e.to_string())?;
    // Une AppImage se lance sans passer par AppImageLauncher (pas de fenêtre à chaque session).
    let exec = format!("env APPIMAGELAUNCHER_DISABLE=1 {}", exe.display());
    std::fs::write(
        &file,
        desktop(&exec, "L Katta (maki)", "Le petit maki assistant du bureau", "X-KDE-autostart-phase=2\nX-GNOME-Autostart-enabled=true\n"),
    )
    .map_err(|e| e.to_string())?;
    Ok(format!("Démarrage automatique : activé ({}).", file.display()))
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

/// Planche des espèces et de leurs poses, utile pour la documentation et les essais.
pub fn preview(out: &str) -> Result<(), String> {
    use crate::mascot::behavior::{Expr, PoseKind as K};
    use crate::mascot::especes::{Espece, Numero};
    let (cw, ch) = (190.0, 200.0);
    let cols = 5usize;
    let rows = Espece::TOUTES.len();
    let mut pm = Pixmap::new((cw * cols as f32) as u32 + 160, (ch * rows as f32) as u32).unwrap();
    pm.fill(tiny_skia::Color::from_rgba8(36, 38, 44, 255));
    let text = draw::Text::load();
    let base = Pose { pop: 1.0, ..Default::default() };
    for (r, e) in Espece::TOUTES.iter().enumerate() {
        draw::set_species(*e);
        let pal = e.pal();
        let st = e.style();
        let upright = st.demarche == crate::mascot::especes::Demarche::Bonds;
        let special = match st.numero {
            Numero::Chant => Pose { kind: K::Upright, sing: true, look_up: 0.6, time: 0.4, ..base },
            Numero::Cri => Pose { kind: K::Sit, call: true, expr: Expr::Surprised, ..base },
            Numero::Tapote => Pose { kind: K::Sit, tap: 0.9, ..base },
            Numero::Guette => Pose { kind: K::Sit, expr: Expr::Surprised, ..base },
            Numero::Danse => Pose { kind: K::Upright, walk_phase: 1.2, time: 0.3, expr: Expr::Happy, ..base },
            Numero::Soleil => Pose { kind: K::Stretch, walk_phase: 0.5, ..base },
        };
        let poses = [
            Pose { kind: K::Sit, sleepy: st.nocturne, ..base },
            if upright { Pose { kind: K::Upright, walk_phase: 0.3, ..base } } else { Pose { kind: K::Walk, walk_phase: 0.6, ..base } },
            special,
            Pose { kind: K::Sit, wave: 0.7, expr: Expr::Happy, ..base },
            Pose { kind: K::Sleep, blink: true, ..base },
        ];
        let y = ch * r as f32 + ch - 22.0;
        if let Some(t) = &text {
            t.draw(&mut pm, 14.0, 12.0, y - 70.0, e.nom().split(" (").next().unwrap_or(""), (230, 226, 218));
            if let Some(mg) = e.nom().split(" (").nth(1) {
                t.draw(&mut pm, 12.0, 12.0, y - 52.0, &format!("({mg}"), (170, 166, 158));
            }
        }
        for (c, p) in poses.iter().enumerate() {
            let x = 160.0 + cw * c as f32 + cw / 2.0;
            let k = pal.size.min(1.2);
            let mut sprite = Pixmap::new(pm.width(), pm.height()).unwrap();
            draw::maki(&mut sprite, Transform::from_translate(x, y).pre_scale(k, k), p);
            draw::outline(&mut sprite, (x - 95.0, y - 170.0, 190.0, 180.0), 2, (236, 232, 222), 0.85);
            pm.draw_pixmap(0, 0, sprite.as_ref(), &tiny_skia::PixmapPaint::default(), Transform::identity(), None);
        }
    }
    draw::set_species(Espece::Catta);
    pm.save_png(out).map_err(|e| e.to_string())?;
    println!("Aperçu enregistré : {out}");
    Ok(())
}

/// Planche des bulles et des expressions (documentation, vérification du dessin).
pub fn preview_bubbles(out: &str) -> Result<(), String> {
    if std::env::var("LKATTA_THEME").is_ok_and(|t| t == "sombre") {
        draw::set_theme("sombre");
    } else {
        draw::set_theme("clair");
    }
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

/// Image du sélecteur de modèles (catalogue réel du premier moteur).
pub fn preview_models(out: &str) -> Result<(), String> {
    draw::set_theme(&std::env::var("LKATTA_THEME").unwrap_or_else(|_| "clair".into()));
    let text = draw::Text::load().ok_or("pas de police")?;
    let mut chat = crate::chat::Chat::new(crate::config::Config::load());
    chat.toggle();
    chat.pick(crate::chat::Pick::Open);
    let t0 = std::time::Instant::now();
    while chat.picker.as_ref().is_some_and(|p| p.models.is_none()) && t0.elapsed().as_secs() < 20 {
        chat.poll();
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    let shots = [(0usize, "Discussion"), (1, "Code")];
    let mut pm = Pixmap::new(900, 560).unwrap();
    pm.fill(tiny_skia::Color::from_rgba8(36, 38, 44, 255));
    for (i, (cat, _)) in shots.iter().enumerate() {
        chat.pick(crate::chat::Pick::Cat(*cat));
        draw::chat_bubble(&mut pm, &text, &chat, 40.0 + i as f32 * 440.0 + 200.0, 545.0, true, 20.0 + i as f32 * 440.0, 535.0, 1.0, false, false, 0.0);
    }
    pm.save_png(out).map_err(|e| e.to_string())?;
    println!("Sélecteur : {out}");
    Ok(())
}

/// Image des six onglets du panneau de réglages.
/// Aperçu de la bulle de discussion : Markdown, liens, code, saisie avec sélection,
/// thèmes clair et sombre côte à côte.
pub fn preview_chat(out: &str) -> Result<(), String> {
    use crate::chat::Entry;
    let text = draw::Text::load().ok_or("pas de police")?;
    let u = 1.2;
    let mut pm = Pixmap::new(1080, 820).unwrap();
    pm.fill(tiny_skia::Color::from_rgba8(36, 38, 44, 255));
    for (i, theme) in ["clair", "sombre"].iter().enumerate() {
        draw::set_theme(theme);
        let mut chat = crate::chat::Chat::new(crate::config::Config::load());
        chat.toggle();
        chat.entries = vec![
            Entry::User("Pourquoi mon disque est plein ?".into()),
            Entry::Info("› $ du -h -d1 ~ | sort -hr | head".into()),
            Entry::Maki("## Bilan\nTon dossier **~/.cache** prend *47 Go*. Les plus gros :\n- `~/.cache/yay` : 18 Go\n- `~/.cache/pip` : 6 Go\n```bash\nyay -Sc && rm -rf ~/.cache/pip\n```\nDétails : [wiki Arch](https://wiki.archlinux.org/title/Pacman) ou https://archlinux.org/news/.".into()),
            Entry::User("Et les journaux ?".into()),
        ];
        chat.flux = "Les journaux prennent **2,1 Go** : `journalctl --vacuum".into();
        chat.input.set("nettoie le cache de yay maintenant");
        chat.input.gauche(true, true);
        let cx = 20.0 + i as f32 * 530.0;
        draw::chat_bubble(&mut pm, &text, &chat, cx + 240.0, 810.0, true, cx, 800.0, u, true, true, 0.0);
    }
    pm.save_png(out).map_err(|e| e.to_string())?;
    println!("Discussion : {out}");
    Ok(())
}

pub fn preview_settings(out: &str) -> Result<(), String> {
    draw::set_theme(&std::env::var("LKATTA_THEME").unwrap_or_else(|_| "clair".into()));
    let text = draw::Text::load().ok_or("pas de police")?;
    let mut chat = crate::chat::Chat::new(crate::config::Config::load());
    chat.toggle();
    chat.open_settings(0);
    let u = 1.2;
    let mut pm = Pixmap::new(1560, 1360).unwrap();
    pm.fill(tiny_skia::Color::from_rgba8(36, 38, 44, 255));
    for i in 0..6 {
        chat.settings_act(crate::chat::Act::Onglet(i));
        // Deux passes : la première mesure le contenu, la seconde ajuste la bulle.
        let mut scratch = Pixmap::new(10, 10).unwrap();
        draw::chat_bubble(&mut scratch, &text, &chat, 0.0, 0.0, true, 0.0, 0.0, u, false, false, 0.0);
        let (cx, cy) = (20.0 + (i % 3) as f32 * 515.0, 660.0 + (i / 3) as f32 * 680.0);
        draw::chat_bubble(&mut pm, &text, &chat, cx + 240.0, cy + 10.0, true, cx, cy, u, false, false, 0.0);
    }
    pm.save_png(out).map_err(|e| e.to_string())?;
    println!("Réglages : {out}");
    Ok(())
}


/// Planches d'animation des lémuriens pour le site (une image par animation et par espèce,
/// vignettes côte à côte) et leur description `lemuriens.json`. Le site anime ces images :
/// c'est donc bien le même personnage que dans l'app.
pub fn site_images(dir: &str) -> Result<(), String> {
    use crate::mascot::behavior::{Expr, PoseKind as K};
    use crate::mascot::especes::{Demarche, Espece, Numero};
    let out = std::path::Path::new(dir);
    std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
    let (fw, fh) = (240u32, 240u32);
    let scale = 1.35;
    let base = Pose { pop: 1.0, ..Default::default() };
    let mut manifest = vec![];
    for e in Espece::TOUTES {
        draw::set_species(e);
        let st = e.style();
        let k = scale * e.pal().size.min(1.25);
        let cycle = |n: usize, f: &dyn Fn(f32) -> Pose| -> Vec<Pose> { (0..n).map(|i| f(i as f32 / n as f32)).collect() };
        let tau = std::f32::consts::TAU;
        let upright = st.demarche == Demarche::Bonds;
        let marche: Vec<Pose> = if upright {
            cycle(8, &|t| Pose { kind: K::Upright, walk_phase: t * tau, time: t * 2.0, ..base })
        } else {
            cycle(8, &|t| Pose { kind: K::Walk, walk_phase: t * tau, tail: ((t * tau).sin() * 4.0, 0.0), ..base })
        };
        let numero: Vec<Pose> = match st.numero {
            Numero::Chant => cycle(8, &|t| Pose { kind: K::Upright, sing: true, look_up: 0.6, time: t * 1.25, ..base }),
            Numero::Cri => cycle(6, &|t| Pose { kind: K::Sit, call: true, expr: Expr::Surprised, time: t, ..base }),
            Numero::Tapote => cycle(6, &|t| Pose { kind: K::Sit, tap: (t * tau * 2.0).sin(), ..base }),
            Numero::Guette => cycle(6, &|t| Pose { kind: K::Sit, expr: Expr::Surprised, look_up: (t * tau).sin() * 0.3, ..base }),
            Numero::Danse => cycle(8, &|t| Pose { kind: K::Upright, walk_phase: t * tau, expr: Expr::Happy, time: t, ..base }),
            Numero::Soleil => cycle(6, &|t| Pose { kind: K::Stretch, walk_phase: (t * std::f32::consts::PI).sin() * 0.9, ..base }),
        };
        let anims: Vec<(&str, Vec<Pose>, u32)> = vec![
            ("assis", vec![base, Pose { blink: true, ..base }], 2),
            ("marche", marche, if e == Espece::AyeAye { 6 } else { 10 }),
            ("salut", cycle(6, &|t| Pose { kind: K::Sit, wave: (t * tau).sin(), expr: Expr::Happy, ..base }), 10),
            ("dodo", vec![Pose { kind: K::Sleep, blink: true, ..base }], 1),
            ("grimpe", cycle(8, &|t| Pose { kind: K::Grimpe, walk_phase: t * tau, ..base }), 8),
            ("vol", vec![Pose { kind: K::Fly, spin: -0.4, ..base }], 1),
            ("numero", numero, 8),
        ];
        let mut a_json = vec![];
        for (name, frames, fps) in anims {
            let mut pm = Pixmap::new(fw * frames.len() as u32, fh).unwrap();
            for (i, p) in frames.iter().enumerate() {
                let cx = (i as u32 * fw) as f32 + fw as f32 / 2.0;
                let y = if p.kind == K::Grimpe { fh as f32 - 30.0 } else { fh as f32 - 12.0 };
                let mut sprite = Pixmap::new(pm.width(), fh).unwrap();
                draw::maki(&mut sprite, Transform::from_translate(cx, y).pre_scale(k, k), p);
                draw::outline(&mut sprite, ((i as u32 * fw) as f32, 0.0, fw as f32, fh as f32), 2, (236, 232, 222), 0.85);
                pm.draw_pixmap(0, 0, sprite.as_ref(), &tiny_skia::PixmapPaint::default(), Transform::identity(), None);
            }
            let file = format!("{}-{}.png", e.id(), name);
            pm.save_png(out.join(&file)).map_err(|er| er.to_string())?;
            a_json.push(format!("\"{name}\":{{\"fichier\":\"{file}\",\"images\":{},\"ips\":{fps}}}", frames.len()));
        }
        manifest.push(format!(
            "{{\"id\":\"{}\",\"nom\":{},\"description\":{},\"presentation\":{},\"vitesse\":{},\"debout\":{},\"animations\":{{{}}}}}",
            e.id(),
            serde_json::to_string(e.nom()).unwrap(),
            serde_json::to_string(e.description()).unwrap(),
            serde_json::to_string(e.presentation()).unwrap(),
            st.vitesse,
            upright,
            a_json.join(",")
        ));
    }
    draw::set_species(Espece::Catta);
    let json = format!("{{\"version\":\"{}\",\"cadre\":{{\"largeur\":{fw},\"hauteur\":{fh}}},\"lemuriens\":[{}]}}", env!("CARGO_PKG_VERSION"), manifest.join(","));
    std::fs::write(out.join("lemuriens.json"), json).map_err(|e| e.to_string())?;
    icon(512).save_png(out.join("icone.png")).map_err(|e| e.to_string())?;
    println!("Images du site : {dir}");
    Ok(())
}
