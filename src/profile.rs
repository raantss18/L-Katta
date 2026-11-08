//! Profil de la machine et prompt système propre à chaque ordinateur.
//!
//! `machine.md` (faits mesurés, régénérable) + `prompt.md` (personnalité et règles,
//! éditable à la main et jamais écrasé) = le prompt système envoyé au moteur.

use std::process::Command;

use crate::{config::which, paths};

fn run(cmd: &str, args: &[&str]) -> String {
    Command::new(cmd)
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

fn read(path: &str) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

fn os_release() -> String {
    let txt = read("/etc/os-release");
    txt.lines()
        .find_map(|l| l.strip_prefix("PRETTY_NAME="))
        .map(|v| v.trim_matches('"').to_string())
        .unwrap_or_else(|| "Linux".into())
}

fn cpu() -> String {
    let info = read("/proc/cpuinfo");
    let name = info
        .lines()
        .find_map(|l| l.strip_prefix("model name").and_then(|r| r.split_once(':')).map(|(_, v)| v.trim().to_string()))
        .unwrap_or_else(|| "inconnu".into());
    let n = info.lines().filter(|l| l.starts_with("processor")).count();
    format!("{name} ({n} fils)")
}

/// Mémoire totale en Go (lue dans /proc/meminfo).
pub fn mem_total_gb() -> f64 {
    read("/proc/meminfo")
        .lines()
        .find_map(|l| l.strip_prefix("MemTotal:"))
        .and_then(|v| v.split_whitespace().next()?.parse::<f64>().ok())
        .map(|kb| kb / 1024.0 / 1024.0)
        .unwrap_or(0.0)
}

fn gpu() -> String {
    let out = run("lspci", &[]);
    let gpus: Vec<String> = out
        .lines()
        .filter(|l| l.contains("VGA") || l.contains("3D controller"))
        .map(|l| l.splitn(2, ": ").nth(1).unwrap_or(l).to_string())
        .collect();
    if gpus.is_empty() { "inconnu".into() } else { gpus.join(" ; ") }
}

fn package_managers() -> Vec<&'static str> {
    ["pacman", "paru", "yay", "apt", "dnf", "zypper", "flatpak", "snap", "nix", "brew"]
        .into_iter()
        .filter(|c| which(c))
        .collect()
}

fn notable_apps() -> Vec<&'static str> {
    [
        "dolphin", "konsole", "kate", "okular", "firefox", "chromium", "google-chrome-stable", "thunderbird",
        "kmail", "korganizer", "libreoffice", "code", "codium", "nvim", "vim", "emacs", "gimp", "inkscape",
        "krita", "kdenlive", "obs", "vlc", "mpv", "spectacle", "docker", "podman", "git", "gh", "python3",
        "cargo", "flutter", "latexmk", "tesseract", "rg", "fd", "plocate", "ollama", "tontolo", "uv",
    ]
    .into_iter()
    .filter(|c| which(c))
    .collect()
}

fn disks() -> String {
    run("df", &["-h", "--output=target,size,avail,pcent", "/", &paths::home_dir().to_string_lossy()])
}

/// Dossiers de premier niveau de ~ (noms seulement, sans fichiers : vie privée).
fn home_dirs() -> String {
    let mut v: Vec<String> = std::fs::read_dir(paths::home_dir())
        .map(|rd| {
            rd.flatten()
                .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|n| !n.starts_with('.'))
                .collect()
        })
        .unwrap_or_default();
    v.sort_by_key(|s| s.to_lowercase());
    v.join(", ")
}

fn user_dirs() -> String {
    let mut out = vec![];
    for key in ["DESKTOP", "DOCUMENTS", "DOWNLOAD", "PICTURES", "MUSIC", "VIDEOS"] {
        let d = run("xdg-user-dir", &[key]);
        if !d.is_empty() {
            out.push(format!("{key}={d}"));
        }
    }
    out.join(", ")
}

/// Mesure la machine et renvoie le texte de `machine.md`.
pub fn measure() -> String {
    let host = read("/etc/hostname").trim().to_string();
    let host = if host.is_empty() { run("uname", &["-n"]) } else { host };
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_else(|_| "inconnu".into());
    let session = std::env::var("XDG_SESSION_TYPE").unwrap_or_else(|_| "inconnue".into());
    let plasma = run("plasmashell", &["--version"]);
    format!(
        "# Machine « {host} »\n\n\
         - Système : {os} ; noyau {kernel}\n\
         - Bureau : {desktop} ({session}){plasma}\n\
         - Processeur : {cpu}\n\
         - Mémoire vive : {mem:.1} Go — rester léger\n\
         - Carte graphique : {gpu}\n\
         - Utilisateur : {user} ; shell {shell} ; dossier personnel {home}\n\
         - Dossiers XDG réels (utiliser ces chemins exacts) : {dirs}\n\
         - Sous-dossiers de ~ : {homedirs}\n\
         - Gestionnaires de paquets : {pm}\n\
         - Logiciels repérés : {apps}\n\n\
         ## Écrans\n\n{screens}\n\n## Bureaux virtuels\n\n{desks}\n\n## Disques\n\n```\n{disks}\n```\n",
        os = os_release(),
        kernel = run("uname", &["-r"]),
        plasma = if plasma.is_empty() { String::new() } else { format!(", {plasma}") },
        cpu = cpu(),
        mem = mem_total_gb(),
        gpu = gpu(),
        user = std::env::var("USER").unwrap_or_default(),
        shell = std::env::var("SHELL").unwrap_or_default(),
        home = paths::home_dir().display(),
        dirs = user_dirs(),
        homedirs = home_dirs(),
        pm = package_managers().join(", "),
        apps = notable_apps().join(", "),
        disks = disks(),
        screens = crate::kde::screens_text(),
        desks = crate::kde::desktops().iter().map(|b| format!("{} : {}", b.numero + 1, b.nom)).collect::<Vec<_>>().join(", "),
    )
}

pub const DEFAULT_PROMPT: &str = "\
Tu es **L Katta** (Lemur Kata), un petit lémurien de Madagascar qui vit sur le bureau {de_prenom}.
Tu es l'assistant de CET ordinateur précis : tu le connais (voir la fiche machine) et tu
aides à l'entretenir, à retrouver des fichiers, à créer des documents, à suivre les
nouveautés, à lire les mails et l'agenda, à gérer rappels et presse-papiers.

Caractère : discret, chaleureux, précis, un brin espiègle. Phrases courtes. Tu tutoies.
Tu réponds en français sauf si on t'écrit dans une autre langue (malgache compris).

Façon de travailler :
- Utilise les outils plutôt que de deviner (état du système, fichiers, mails, agenda…).
- Avant toute modification (écrire, supprimer, installer, commande non lecture seule),
  dis en une phrase ce que tu vas faire : l'utilisateur confirmera.
- Commandes administrateur (sudo) : lance-les dans un terminal visible (terminal_executer),
  jamais en arrière-plan.
- Mails : résume (expéditeur, objet, l'essentiel en une ligne) ; n'envoie rien sans
  montrer le texte complet.
- Réponds court ; propose d'approfondir plutôt que de tout déverser.
- Ne dérange pas : pas de questions inutiles, pas d'emojis en rafale.
";

/// Crée `machine.md` et `prompt.md` s'ils manquent. `force` remesure la machine.
pub fn ensure(force: bool) -> std::io::Result<()> {
    let m = paths::machine_file();
    if force || !m.exists() {
        paths::write_private(&m, measure().as_bytes())?;
    }
    let p = paths::prompt_file();
    if !p.exists() {
        paths::write_private(&p, DEFAULT_PROMPT.as_bytes())?;
    }
    Ok(())
}

/// Prompt système complet : personnalité + fiche machine + date et heure courantes.
pub fn system_prompt(cfg: &crate::config::Config) -> String {
    let prenom = cfg.prenom.as_str();
    let _ = ensure(false);
    let persona = std::fs::read_to_string(paths::prompt_file()).unwrap_or_else(|_| DEFAULT_PROMPT.into());
    let machine = std::fs::read_to_string(paths::machine_file()).unwrap_or_default();
    let now = chrono::Local::now();
    let prefs = crate::memoire::profil();
    let prefs = if prefs.trim().is_empty() { String::new() } else { format!("{prefs}\n---\n") };
    format!(
        "{}\n\n---\n{}\n\n---\n{}\n---\n{}Nous sommes le {} (heure locale). Dossier courant : {}.{}",
        crate::regles::prompt_block(prenom),
        persona.replace("{de_prenom}", &crate::fr::de(prenom)).replace("{prenom}", prenom),
        machine,
        prefs,
        crate::fr::date_longue(&now),
        paths::home_dir().display(),
        if crate::lang::en() {
            "\n\nLANGUE : l'utilisateur a choisi l'anglais. Réponds TOUJOURS en anglais (English), même si ces consignes sont en français ; garde les noms de commandes tels quels."
        } else {
            ""
        }
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn measure_mentions_machine() {
        let m = super::measure();
        assert!(m.contains("Mémoire vive"));
        assert!(m.starts_with("# Machine"));
    }
}
