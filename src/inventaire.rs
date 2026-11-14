//! Inventaire de la machine : matériel, système, bureau, logiciels, index. Pensé pour toute
//! machine Linux (n'importe quelle distribution, carte graphique ou bureau) : chaque rubrique
//! se tait si l'information manque, aucun outil n'est supposé installé.

use std::process::{Command, Stdio};

fn lire(p: &str) -> String {
    std::fs::read_to_string(p).unwrap_or_default()
}

/// Sortie d'une commande si elle existe et réussit (jamais d'erreur affichée).
fn cmd(prog: &str, args: &[&str]) -> Option<String> {
    let o = Command::new(prog).args(args).stdin(Stdio::null()).stderr(Stdio::null()).output().ok()?;
    o.status.success().then(|| String::from_utf8_lossy(&o.stdout).trim().to_string()).filter(|s| !s.is_empty())
}

fn os_release(txt: &str) -> String {
    let get = |k: &str| txt.lines().find_map(|l| l.strip_prefix(&format!("{k}="))).map(|v| v.trim_matches('"').to_string());
    get("PRETTY_NAME").or_else(|| get("NAME")).unwrap_or_else(|| "Linux".into())
}

/// Modèle du processeur et nombre de cœurs logiques.
fn processeur(cpuinfo: &str) -> Option<String> {
    let modele = cpuinfo.lines().find_map(|l| l.strip_prefix("model name").and_then(|r| r.split_once(':')).map(|(_, v)| v.trim().to_string()))
        // ARM : pas de « model name ».
        .or_else(|| cpuinfo.lines().find_map(|l| l.strip_prefix("Hardware").and_then(|r| r.split_once(':')).map(|(_, v)| v.trim().to_string())));
    let coeurs = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1);
    Some(format!("{} ({} cœurs logiques)", modele.unwrap_or_else(|| std::env::consts::ARCH.into()), coeurs))
}

/// Cartes graphiques vues par le noyau (tout fabricant), avec la mémoire si on la connaît.
fn cartes_graphiques() -> Vec<String> {
    let mut v = vec![];
    if let Some(l) = cmd("lspci", &[]) {
        for x in l.lines().filter(|x| x.contains("VGA") || x.contains("3D controller") || x.contains("Display controller")) {
            v.push(x.split_once(": ").map(|(_, n)| n).unwrap_or(x).to_string());
        }
    }
    v
}

/// Disques réels, une ligne par système de fichiers : les sous-volumes (btrfs) et les montages
/// d'AppImage ou de Snap, qui répètent le même disque, sont écartés.
fn disques(df: &str) -> String {
    let mut vus = std::collections::HashSet::new();
    let mut out = vec![];
    for (i, l) in df.lines().enumerate() {
        let c: Vec<&str> = l.split_whitespace().collect();
        if i == 0 {
            out.push(format!("{:<12} {}", "", c.get(2..).map(|x| x.join("  ")).unwrap_or_default()));
            continue;
        }
        if c.len() < 6 || c[1].starts_with("/tmp/.mount_") || c[1].starts_with("/snap/") || c[1].starts_with("/run/") {
            continue;
        }
        if vus.insert(c[0]) {
            out.push(format!("{:<12} {}", c[1], c[2..].join("  ")));
        }
    }
    out.join("\n")
}

/// Batterie : niveau et état, si l'ordinateur en a une.
fn batterie() -> Option<String> {
    let d = std::fs::read_dir("/sys/class/power_supply").ok()?;
    for e in d.flatten() {
        let p = e.path();
        if lire(&format!("{}/type", p.display())).trim() == "Battery" {
            let cap = lire(&format!("{}/capacity", p.display()));
            let st = lire(&format!("{}/status", p.display()));
            let usure = match (lire(&format!("{}/energy_full", p.display())).trim().parse::<f64>(), lire(&format!("{}/energy_full_design", p.display())).trim().parse::<f64>()) {
                (Ok(a), Ok(b)) if b > 0.0 => format!(", santé {:.0} %", a / b * 100.0),
                _ => String::new(),
            };
            return Some(format!("{} % ({}){usure}", cap.trim(), st.trim()));
        }
    }
    None
}

/// Nombre de paquets selon le gestionnaire présent (une distribution peut en avoir plusieurs).
fn paquets() -> Vec<String> {
    let compte = |s: String| s.lines().filter(|l| !l.trim().is_empty()).count();
    let mut v = vec![];
    for (nom, prog, args) in [
        ("pacman", "pacman", &["-Qq"][..]),
        ("dpkg", "dpkg-query", &["-f", "${binary:Package}\n", "-W"][..]),
        ("rpm", "rpm", &["-qa"][..]),
        ("apk", "apk", &["info"][..]),
        ("xbps", "xbps-query", &["-l"][..]),
        ("flatpak", "flatpak", &["list", "--app", "--columns=application"][..]),
        ("snap", "snap", &["list"][..]),
        ("nix", "nix-env", &["-q"][..]),
    ] {
        if let Some(o) = cmd(prog, args) {
            let n = compte(o).saturating_sub(if nom == "snap" { 1 } else { 0 });
            if n > 0 {
                v.push(format!("{nom} : {n}"));
            }
        }
    }
    v
}

fn bureau() -> String {
    let de = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    let session = std::env::var("XDG_SESSION_TYPE").unwrap_or_default();
    let version = if de.to_uppercase().contains("KDE") {
        cmd("plasmashell", &["--version"]).unwrap_or_default()
    } else if de.to_uppercase().contains("GNOME") {
        cmd("gnome-shell", &["--version"]).unwrap_or_default()
    } else {
        String::new()
    };
    let de = if de.is_empty() { "inconnu".into() } else { de.replace(':', " / ") };
    [de, version, session].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · ")
}

/// Inventaire lisible (Markdown court).
pub fn texte() -> String {
    let mut parts = vec![];
    let noyau = cmd("uname", &["-r"]).unwrap_or_default();
    let hote = lire("/etc/hostname");
    let hote = if hote.trim().is_empty() { cmd("uname", &["-n"]).unwrap_or_default() } else { hote.trim().to_string() };
    let modele = [lire("/sys/class/dmi/id/sys_vendor"), lire("/sys/class/dmi/id/product_name")].iter().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" ");
    parts.push(format!(
        "## Système\n- {} (noyau {noyau})\n- Machine : {}{}\n- Bureau : {}",
        os_release(&lire("/etc/os-release")),
        if hote.is_empty() { "—".into() } else { hote },
        if modele.is_empty() { String::new() } else { format!(" — {modele}") },
        bureau()
    ));
    let m = crate::local::machine();
    let mut mat = vec![];
    if let Some(p) = processeur(&lire("/proc/cpuinfo")) {
        mat.push(format!("- Processeur : {p}"));
    }
    mat.push(format!("- Mémoire : {:.1} Go", m.ram_go));
    for g in cartes_graphiques() {
        mat.push(format!("- Graphique : {g}"));
    }
    if let Some((nom, go)) = &m.gpu {
        mat.push(format!("- Mémoire graphique dédiée : {go:.0} Go ({nom})"));
    }
    if let Some(b) = batterie() {
        mat.push(format!("- Batterie : {b}"));
    }
    parts.push(format!("## Matériel\n{}", mat.join("\n")));
    if let Some(df) = cmd("df", &["-h", "--output=source,target,size,used,avail,pcent", "-x", "tmpfs", "-x", "devtmpfs", "-x", "efivarfs", "-x", "overlay", "-x", "squashfs"]) {
        parts.push(format!("## Disques\n{}", disques(&df)));
    }
    // KDE d'abord, sinon wlroots (Sway, Hyprland…) ou X11.
    let ecrans = Some(crate::kde::screens_text())
        .filter(|e| !e.trim().is_empty())
        .or_else(|| cmd("wlr-randr", &[]))
        .or_else(|| cmd("xrandr", &["--listactivemonitors"]))
        .unwrap_or_default();
    if !ecrans.trim().is_empty() {
        parts.push(format!("## Écrans\n{ecrans}"));
    }
    let p = paquets();
    if !p.is_empty() {
        parts.push(format!("## Logiciels installés\n- {}", p.join("\n- ")));
    }
    let conseil = crate::local::conseil(&m);
    parts.push(format!("## Modèle d'IA local conseillé\n- {} ({})", conseil.tag, crate::local::resume(&m)));
    match crate::index::etat() {
        Some((n, quand)) => parts.push(format!("## Index des fichiers\n- {n} fichiers, dernier passage {quand}")),
        None => parts.push("## Index des fichiers\n- pas encore construit (premier passage quelques minutes après le démarrage)".into()),
    }
    parts.join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_os_and_cpu() {
        assert_eq!(os_release("NAME=\"Arch Linux\"\nPRETTY_NAME=\"Arch Linux\"\n"), "Arch Linux");
        assert_eq!(os_release("NAME=Debian\n"), "Debian");
        assert_eq!(os_release(""), "Linux");
        assert!(processeur("processor\t: 0\nmodel name\t: AMD Ryzen 7 7735HS\n").unwrap().starts_with("AMD Ryzen 7 7735HS ("));
        assert!(processeur("Hardware\t: BCM2835\n").unwrap().starts_with("BCM2835"));
        let df = "Filesystem Mounted on Size Used Avail Use%\n/dev/nvme0n1p2 / 445G 261G 178G 60%\n/dev/nvme0n1p2 /var/log 445G 261G 178G 60%\n/dev/nvme1n1 /home 1,8T 534G 1,2T 31%\nlkatta /tmp/.mount_lkattaX 4,3M 4,3M 0 100%\n";
        let d = disques(df);
        assert_eq!(d.lines().count(), 3, "{d}");
        assert!(d.contains("/home") && !d.contains("/var/log") && !d.contains(".mount_"));
    }
}
