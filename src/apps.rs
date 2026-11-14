//! Applications installées (fichiers .desktop, Flatpak compris) et lancement par un nom
//! approximatif. Porté de mi-saina (services/apps.py).

use std::path::PathBuf;

use crate::paths;

#[derive(Debug, Clone)]
pub struct App {
    pub id: String,
    pub nom: String,
    pub exec: String,
    pub bin: String,
    pub termes: String,
    pub terminal: bool,
    pub cachee: bool,
}

fn dirs() -> Vec<PathBuf> {
    let h = paths::home_dir();
    // Ordre XDG : ceux de l'utilisateur d'abord (ils remplacent ceux du système de même nom).
    vec![
        h.join(".local/share/applications"),
        h.join(".local/share/flatpak/exports/share/applications"),
        "/var/lib/flatpak/exports/share/applications".into(),
        "/usr/local/share/applications".into(),
        "/usr/share/applications".into(),
    ]
}

/// Retire les codes de champ (%f, %U…) d'une ligne Exec.
fn clean_exec(e: &str) -> String {
    let mut out = String::new();
    let mut it = e.chars().peekable();
    while let Some(c) = it.next() {
        if c == '%' {
            if it.peek() == Some(&'%') {
                out.push('%');
            }
            it.next();
        } else {
            out.push(c);
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn parse_desktop(id: &str, txt: &str) -> Option<App> {
    let mut in_main = false;
    let (mut nom, mut nom_fr, mut exec, mut termes) = (String::new(), String::new(), String::new(), String::new());
    let (mut terminal, mut cachee, mut is_app) = (false, false, false);
    for l in txt.lines() {
        let l = l.trim();
        if l.starts_with('[') {
            in_main = l == "[Desktop Entry]";
            continue;
        }
        if !in_main {
            continue;
        }
        let Some((k, v)) = l.split_once('=') else { continue };
        match k.trim() {
            "Name" => nom = v.into(),
            "Name[fr]" => nom_fr = v.into(),
            "Exec" => exec = clean_exec(v),
            "Type" => is_app = v.trim() == "Application",
            "Terminal" => terminal = v.trim() == "true",
            "NoDisplay" | "Hidden" => cachee |= v.trim() == "true",
            "GenericName" | "GenericName[fr]" | "Keywords" | "Keywords[fr]" | "Comment[fr]" | "Categories" => {
                termes.push(' ');
                termes.push_str(v);
            }
            _ => {}
        }
    }
    if !is_app || exec.is_empty() {
        return None;
    }
    let first = exec.split_whitespace().find(|t| !t.contains('=') && *t != "env").unwrap_or("");
    let bin = std::path::Path::new(first).file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    Some(App {
        id: id.trim_end_matches(".desktop").into(),
        nom: if nom_fr.is_empty() { nom } else { nom_fr },
        exec,
        bin,
        termes,
        terminal,
        cachee,
    })
}

pub fn index() -> Vec<App> {
    let mut seen = std::collections::HashSet::new();
    let mut out = vec![];
    for d in dirs() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if !name.ends_with(".desktop") || !seen.insert(name.clone()) {
                continue;
            }
            if let Ok(txt) = std::fs::read_to_string(e.path()) {
                if let Some(a) = parse_desktop(&name, &txt) {
                    out.push(a);
                }
            }
        }
    }
    out
}

fn norm(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| match c {
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'à' | 'â' => 'a',
            'î' | 'ï' => 'i',
            'ô' => 'o',
            'ù' | 'û' => 'u',
            'ç' => 'c',
            c => c,
        })
        .filter(|c| c.is_alphanumeric())
        .collect()
}

/// Similarité de Dice sur les bigrammes (0..1).
fn similar(a: &str, b: &str) -> f32 {
    if a.len() < 2 || b.len() < 2 {
        return if a == b { 1.0 } else { 0.0 };
    }
    let bg = |s: &str| s.chars().collect::<Vec<_>>().windows(2).map(|w| (w[0], w[1])).collect::<Vec<_>>();
    let (x, mut y) = (bg(a), bg(b));
    let total = (x.len() + y.len()) as f32;
    let mut inter = 0.0;
    for p in x {
        if let Some(i) = y.iter().position(|q| *q == p) {
            y.remove(i);
            inter += 1.0;
        }
    }
    2.0 * inter / total
}

fn score(q: &str, raw: &str, a: &App) -> f32 {
    let mut best: f32 = 0.0;
    for f in [&a.nom, &a.id, &a.bin] {
        let f = norm(f);
        if f.is_empty() {
            continue;
        }
        if f == q {
            return 1.0;
        }
        if q.len() >= 3 && f.contains(q) {
            best = best.max(0.92);
        }
        if f.len() >= 4 && q.contains(&f) {
            best = best.max(0.85);
        }
        best = best.max(similar(q, &f));
    }
    let t = norm(&a.termes);
    let words: Vec<String> = raw.split(|c: char| !c.is_alphanumeric()).map(norm).filter(|w| w.len() >= 4).collect();
    if !words.is_empty() && words.iter().all(|w| t.contains(w.as_str())) {
        best = best.max(0.8);
    }
    best
}

/// Trouve l'application qui correspond le mieux à un nom approximatif.
pub fn resolve<'a>(apps: &'a [App], query: &str) -> Option<&'a App> {
    let q = norm(query);
    if q.is_empty() {
        return None;
    }
    apps.iter()
        .filter(|a| !a.cachee)
        .map(|a| (a, score(&q, query, a) - if a.terminal { 0.05 } else { 0.0 }))
        .filter(|(_, s)| *s >= 0.7)
        .max_by(|x, y| x.1.total_cmp(&y.1))
        .map(|(a, _)| a)
}

/// Lance une application par son nom (« dolphin », « gestionnaire de fichiers », « mission center »).
pub fn launch(query: &str) -> Result<String, String> {
    let apps = index();
    let a = resolve(&apps, query).ok_or_else(|| format!("aucune application ne correspond à « {query} »"))?;
    let cmd = if crate::config::which("gtk-launch") || crate::config::which("kioclient") {
        // Laisse le bureau lancer l'entrée .desktop (variables, Flatpak, activation D-Bus).
        if crate::config::which("gtk-launch") { format!("gtk-launch {}", a.id) } else { format!("kioclient exec {}.desktop", a.id) }
    } else {
        a.exec.clone()
    };
    std::process::Command::new("sh")
        .args(["-c", &format!("setsid {cmd} >/dev/null 2>&1 &")])
        .status()
        .map_err(|e| e.to_string())?;
    Ok(format!("Lancé : {} ({})", a.nom, a.id))
}

pub fn list_text() -> String {
    let mut v: Vec<String> = index().into_iter().filter(|a| !a.cachee).map(|a| a.nom).collect();
    v.sort_by_key(|s| s.to_lowercase());
    v.dedup();
    v.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOLPHIN: &str = "[Desktop Entry]\nType=Application\nName=Dolphin\nGenericName=File Manager\nGenericName[fr]=Gestionnaire de fichiers\nExec=dolphin %u\n";

    #[test]
    fn parse_and_resolve() {
        let a = parse_desktop("org.kde.dolphin.desktop", DOLPHIN).unwrap();
        assert_eq!((a.exec.as_str(), a.bin.as_str(), a.id.as_str()), ("dolphin", "dolphin", "org.kde.dolphin"));
        let mc = parse_desktop("io.missioncenter.MissionCenter.desktop", "[Desktop Entry]\nType=Application\nName=Mission Center\nExec=missioncenter\n").unwrap();
        let apps = vec![a, mc];
        assert_eq!(resolve(&apps, "mission-center").unwrap().nom, "Mission Center");
        assert_eq!(resolve(&apps, "gestionnaire de fichiers").unwrap().nom, "Dolphin");
        assert!(resolve(&apps, "photoshop").is_none());
    }
}
