//! Lire l'écran : capture (Spectacle, l'outil de KDE) de l'écran où l'on travaille, le
//! maki rendu invisible le temps de la capture, puis description par un modèle de vision.
//! Une capture ne part vers un modèle en ligne qu'avec l'accord de l'utilisateur : demande
//! explicite (`/ecran`, `lkatta ecran`) ou confirmation quand le cerveau en a l'idée.

use std::time::Duration;

use serde_json::{Value, json};
use tiny_skia::{Pixmap, PixmapPaint, Transform};

/// Largeur maximale envoyée au modèle (assez pour lire du texte, assez léger pour le réseau).
const LARGEUR_MAX: u32 = 1600;

/// Capture l'écran sous le pointeur (PNG réduit).
pub fn capturer() -> Result<Vec<u8>, String> {
    if !crate::config::which("spectacle") {
        return Err(t!("Spectacle (l'outil de capture de KDE) n'est pas installé.").into());
    }
    let dir = crate::paths::data_dir().join("captures");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let fichier = dir.join(format!("ecran-{}.png", std::process::id()));
    // Le maki se rend invisible (sans bouger) le temps de la capture.
    let _ = crate::ipc::send("capture on");
    std::thread::sleep(Duration::from_millis(400));
    let r = std::process::Command::new("spectacle")
        .args(["--background", "--nonotify", "--current", "--output"])
        .arg(&fichier)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    let _ = crate::ipc::send("capture off");
    if !r.map(|s| s.success()).unwrap_or(false) {
        return Err(t!("La capture d'écran a échoué.").into());
    }
    let png = std::fs::read(&fichier).map_err(|e| e.to_string());
    let _ = std::fs::remove_file(&fichier);
    reduire(&png?)
}

/// Réduit une image PNG à `LARGEUR_MAX` de large au plus.
pub fn reduire(png: &[u8]) -> Result<Vec<u8>, String> {
    let img = Pixmap::decode_png(png).map_err(|e| e.to_string())?;
    if img.width() <= LARGEUR_MAX {
        return Ok(png.to_vec());
    }
    let k = LARGEUR_MAX as f32 / img.width() as f32;
    let (w, h) = (LARGEUR_MAX, ((img.height() as f32 * k).round() as u32).max(1));
    let mut out = Pixmap::new(w, h).ok_or("image vide")?;
    let paint = PixmapPaint { quality: tiny_skia::FilterQuality::Bicubic, ..Default::default() };
    out.draw_pixmap(0, 0, img.as_ref(), &paint, Transform::from_scale(k, k), None);
    out.encode_png().map_err(|e| e.to_string())
}

/// Ce que montre l'écran, en réponse à une question (vide : description générale).
pub fn regarder(question: &str) -> Result<String, String> {
    let png = capturer()?;
    decrire(&png, question)
}

/// Fait décrire une image par un modèle de vision (moteur intégré).
pub fn decrire(png: &[u8], question: &str) -> Result<String, String> {
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(png);
    let q = if question.trim().is_empty() { t!("Décris ce qu'il y a à l'écran : l'application, ce que je suis en train de faire, et ce qui mérite mon attention (erreur, avertissement).").to_string() } else { question.trim().to_string() };
    let lang = if crate::lang::en() { "Answer in English." } else { "Réponds en français." };
    let messages = vec![
        json!({"role": "system", "content": format!(
            "Tu regardes une capture de l'écran de l'utilisateur (bureau Linux KDE). Réponds précisément à sa question en t'appuyant sur ce qui est visible : \
             recopie exactement les messages d'erreur importants, nomme les applications. Ignore le petit lémurien et sa bulle s'ils apparaissent. \
             Si un texte est illisible, dis-le plutôt que de deviner. {lang}")}),
        json!({"role": "user", "content": [
            {"type": "text", "text": q},
            {"type": "image_url", "image_url": {"url": format!("data:image/png;base64,{b64}")}}
        ]}),
    ];
    let opts = crate::moteur::Options { vision: true, max_tokens: 1500, ..Default::default() };
    let (msg, _) = crate::moteur::complete(&messages, &json!([]), "auto", &opts, &mut |_| {})?;
    let t = crate::brain::clean(msg.get("content").and_then(Value::as_str).unwrap_or(""));
    if t.trim().is_empty() { Err(t!("(réponse vide)").into()) } else { Ok(t) }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Capture locale seulement (rien n'est envoyé) : `cargo test capture_locale -- --ignored`.
    #[test]
    #[ignore]
    fn capture_locale() {
        let png = capturer().unwrap();
        let img = Pixmap::decode_png(&png).unwrap();
        println!("capture : {} × {}, {} Ko", img.width(), img.height(), png.len() / 1024);
        assert!(img.width() <= LARGEUR_MAX && img.height() > 100);
    }

    /// Essai réel (réseau) : `cargo test lit_une_erreur -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn lit_une_erreur() {
        let text = crate::mascot::draw::Text::load().unwrap();
        let mut pm = Pixmap::new(900, 220).unwrap();
        pm.fill(tiny_skia::Color::from_rgba8(30, 30, 36, 255));
        text.draw(&mut pm, 22.0, 20.0, 30.0, "$ cargo build", (220, 220, 220));
        text.draw(&mut pm, 22.0, 20.0, 80.0, "error[E0425]: cannot find value `taille` in this scope", (240, 90, 80));
        text.draw(&mut pm, 22.0, 20.0, 125.0, "  --> src/main.rs:12:9", (150, 170, 240));
        let r = decrire(&pm.encode_png().unwrap(), "Quelle erreur s'affiche et comment la corriger ?").unwrap();
        println!("{r}");
        assert!(r.contains("taille") || r.contains("E0425"), "{r}");
    }

    #[test]
    fn shrinks_wide_images_only() {
        let mut big = Pixmap::new(3200, 1800).unwrap();
        big.fill(tiny_skia::Color::from_rgba8(200, 100, 50, 255));
        let png = big.encode_png().unwrap();
        let out = Pixmap::decode_png(&reduire(&png).unwrap()).unwrap();
        assert_eq!((out.width(), out.height()), (1600, 900));
        let small = Pixmap::new(800, 600).unwrap().encode_png().unwrap();
        assert_eq!(reduire(&small).unwrap(), small);
    }
}
