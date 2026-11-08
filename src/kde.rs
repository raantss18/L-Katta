//! Bureaux virtuels et écrans de KDE Plasma (D-Bus de KWin via `gdbus`, `kscreen-doctor`).

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

use crate::config::which;

#[derive(Debug, Clone, PartialEq)]
pub struct Bureau {
    pub numero: u32,
    pub id: String,
    pub nom: String,
}

fn gdbus(args: &[&str]) -> Option<String> {
    let o = Command::new("gdbus").args(["call", "--session", "--dest", "org.kde.KWin", "--object-path", "/VirtualDesktopManager"]).args(args).output().ok()?;
    o.status.success().then(|| String::from_utf8_lossy(&o.stdout).into_owned())
}

/// Chaînes entre apostrophes d'une sortie GVariant.
fn quoted(s: &str) -> Vec<String> {
    s.split('\'').skip(1).step_by(2).map(String::from).collect()
}

pub fn parse_desktops(txt: &str) -> Vec<Bureau> {
    let Some(start) = txt.find("'desktops'") else { return vec![] };
    let rest = &txt[start..];
    let end = rest.find("]>").unwrap_or(rest.len());
    rest[..end]
        .split('(')
        .skip(1)
        .filter_map(|item| {
            let num = item.split(',').next()?.trim().trim_start_matches("uint32").trim().parse().ok()?;
            let q = quoted(item);
            Some(Bureau { numero: num, id: q.first()?.clone(), nom: q.get(1).cloned().unwrap_or_default() })
        })
        .collect()
}

pub fn desktops() -> Vec<Bureau> {
    gdbus(&["--method", "org.freedesktop.DBus.Properties.GetAll", "org.kde.KWin.VirtualDesktopManager"])
        .map(|t| parse_desktops(&t))
        .unwrap_or_default()
}

pub fn current() -> Option<String> {
    let t = gdbus(&["--method", "org.freedesktop.DBus.Properties.Get", "org.kde.KWin.VirtualDesktopManager", "current"])?;
    quoted(&t).into_iter().next()
}

pub fn desktops_text() -> String {
    let list = desktops();
    if list.is_empty() {
        return "Bureaux virtuels indisponibles (KWin injoignable).".into();
    }
    let cur = current().unwrap_or_default();
    list.iter()
        .map(|b| format!("{} {} : {}", if b.id == cur { "→" } else { " " }, b.numero + 1, b.nom))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Va sur un bureau par numéro (1, 2…) ou par nom.
pub fn switch(which_one: &str) -> Result<String, String> {
    let list = desktops();
    let w = which_one.trim().to_lowercase();
    let b = list
        .iter()
        .find(|b| w.parse::<u32>().is_ok_and(|n| n == b.numero + 1) || b.nom.to_lowercase() == w)
        .ok_or_else(|| format!("pas de bureau « {which_one} »"))?;
    let v = format!("<'{}'>", b.id);
    gdbus(&["--method", "org.freedesktop.DBus.Properties.Set", "org.kde.KWin.VirtualDesktopManager", "current", &v])
        .ok_or("KWin a refusé le changement")?;
    Ok(format!("Bureau {} : {}", b.numero + 1, b.nom))
}

/// Écrans (sortie de kscreen-doctor, sans couleurs).
pub fn screens_text() -> String {
    if !which("kscreen-doctor") {
        return String::new();
    }
    let out = Command::new("kscreen-doctor").arg("-o").output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
    let mut clean = String::new();
    let mut in_esc = false;
    for c in out.chars() {
        match c {
            '\u{1b}' => in_esc = true,
            'm' if in_esc => in_esc = false,
            _ if in_esc => {}
            _ => clean.push(c),
        }
    }
    let mut rows = vec![];
    let mut cur = String::new();
    for l in clean.lines() {
        let l = l.trim();
        if let Some(r) = l.strip_prefix("Output:") {
            if !cur.is_empty() {
                rows.push(std::mem::take(&mut cur));
            }
            cur = r.split_whitespace().nth(1).unwrap_or("").to_string();
        } else if l.starts_with("Geometry:") || l.starts_with("Scale:") || l == "disabled" {
            cur.push_str(&format!(" · {l}"));
        }
    }
    if !cur.is_empty() {
        rows.push(cur);
    }
    rows.join("\n")
}

/// Écran actif selon KWin (celui de la souris ou de la fenêtre active).
pub fn active_output() -> Option<String> {
    let bin = if which("qdbus6") { "qdbus6" } else { "qdbus" };
    let o = Command::new(bin).args(["org.kde.KWin", "/KWin", "org.kde.KWin.activeOutputName"]).stderr(Stdio::null()).output().ok()?;
    let n = String::from_utf8_lossy(&o.stdout).trim().to_string();
    (o.status.success() && !n.is_empty()).then_some(n)
}

/// Suit les changements de bureau (bloquant, à lancer dans un fil).
pub fn watch(mut on_change: impl FnMut(String)) {
    if !which("gdbus") {
        return;
    }
    let Ok(mut child) = Command::new("gdbus")
        .args(["monitor", "--session", "--dest", "org.kde.KWin", "--object-path", "/VirtualDesktopManager"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    else {
        return;
    };
    let out = child.stdout.take().expect("stdout");
    for line in BufReader::new(out).lines().map_while(Result::ok) {
        if line.contains("currentChanged") {
            if let Some(id) = quoted(&line).into_iter().next() {
                on_change(id);
            }
        }
    }
    let _ = child.wait();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_kwin_desktops() {
        let t = "({'count': <uint32 2>, 'current': <'b'>, 'desktops': <[(uint32 0, 'a', 'Desktop 1'), (1, 'b', 'Travail')]>, 'rows': <uint32 2>},)";
        let d = parse_desktops(t);
        assert_eq!(d.len(), 2);
        assert_eq!((d[1].numero, d[1].id.as_str(), d[1].nom.as_str()), (1, "b", "Travail"));
        assert_eq!(quoted("/VirtualDesktopManager: org.kde.KWin.VirtualDesktopManager.currentChanged ('7bc3',)"), vec!["7bc3"]);
    }
}
