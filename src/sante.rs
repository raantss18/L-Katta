//! Santé de la machine et garde-fous des commandes.
//! Porté de mi-saina (services/health_monitor.py, diagnostics.py, shell_stream.py).

use std::process::Command;

use crate::config::which;

#[derive(Debug, Clone, PartialEq)]
pub struct Constat {
    pub id: String,
    /// « critique », « attention » ou « info ».
    pub gravite: &'static str,
    pub titre: String,
    pub detail: String,
    pub conseil: String,
    pub commande: String,
}

fn run(cmd: &str, args: &[&str]) -> Option<String> {
    let o = Command::new(cmd).args(args).output().ok()?;
    o.status.success().then(|| String::from_utf8_lossy(&o.stdout).into_owned())
}

fn constat(id: &str, g: &'static str, titre: &str, detail: String, conseil: &str, commande: &str) -> Constat {
    Constat { id: id.into(), gravite: g, titre: titre.into(), detail, conseil: conseil.into(), commande: commande.into() }
}

fn check_disk() -> Vec<Constat> {
    let home = crate::paths::home_dir();
    let Some(out) = run("df", &["-P", "/", &home.to_string_lossy()]) else { return vec![] };
    let mut v = vec![];
    let mut seen = std::collections::HashSet::new();
    for l in out.lines().skip(1) {
        let f: Vec<&str> = l.split_whitespace().collect();
        if f.len() < 6 || !seen.insert(f[0].to_string()) {
            continue;
        }
        let Ok(pc) = f[4].trim_end_matches('%').parse::<u32>() else { continue };
        let free_gb = f[3].parse::<f64>().unwrap_or(0.0) / 1024.0 / 1024.0;
        let mount = f[5];
        let cmd = format!("du -h -d1 {} 2>/dev/null | sort -hr | head -15", if mount == "/" { "~" } else { mount });
        if pc >= 95 {
            v.push(constat(&format!("disque:{mount}"), "critique", "Disque presque plein",
                format!("{mount} est plein à {pc} % (reste {free_gb:.1} Go)."), "Libère de l'espace : gros fichiers, caches, corbeille.", &cmd));
        } else if pc >= 90 {
            v.push(constat(&format!("disque:{mount}"), "attention", "Espace disque faible",
                format!("{mount} est utilisé à {pc} % (reste {free_gb:.1} Go)."), "Repère les plus gros dossiers.", &cmd));
        }
    }
    v
}

fn check_failed_services() -> Vec<Constat> {
    let mut v = vec![];
    for (scope, flag) in [("utilisateur", "--user"), ("système", "--system")] {
        let Some(out) = run("systemctl", &[flag, "--failed", "--no-legend", "--plain"]) else { continue };
        let units: Vec<&str> = out.lines().filter_map(|l| l.split_whitespace().next()).collect();
        if units.is_empty() {
            continue;
        }
        let mut names = units.iter().take(5).copied().collect::<Vec<_>>().join(", ");
        if units.len() > 5 {
            names.push_str(" …");
        }
        let user = if flag == "--user" { "--user " } else { "" };
        v.push(constat(
            &format!("services:{scope}:{}", units.join(",")),
            "attention",
            &format!("Service(s) {scope} en échec"),
            format!("{} unité(s) en échec : {names}.", units.len()),
            "Regarde le journal de l'unité concernée.",
            &format!("systemctl {user}status {}", units[0]),
        ));
    }
    v
}

fn check_updates() -> Vec<Constat> {
    let (count, cmd) = if which("checkupdates") {
        let n = run("checkupdates", &[]).map(|o| o.lines().filter(|l| !l.trim().is_empty()).count()).unwrap_or(0);
        (n, if which("paru") { "paru -Syu" } else { "sudo pacman -Syu" })
    } else if which("apt") {
        let n = run("apt", &["list", "--upgradable"]).map(|o| o.lines().filter(|l| l.contains('/')).count()).unwrap_or(0);
        (n, "sudo apt update && sudo apt upgrade")
    } else if which("dnf") {
        // dnf check-update renvoie 100 quand il y a des mises à jour : pas de `run`.
        let n = Command::new("dnf").args(["-q", "check-update"]).output().ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).lines().filter(|l| l.chars().next().is_some_and(|c| c.is_alphanumeric())).count())
            .unwrap_or(0);
        (n, "sudo dnf upgrade")
    } else {
        return vec![];
    };
    if count == 0 {
        return vec![];
    }
    let g = if count >= 150 { "attention" } else { "info" };
    vec![constat("maj", g, "Mises à jour disponibles", format!("{count} paquet(s) peuvent être mis à jour."),
        "Mets le système à jour (tu valideras dans le terminal).", cmd)]
}

fn check_journal() -> Vec<Constat> {
    let Some(out) = run("journalctl", &["-p", "err", "-b", "--since", "-1h", "--no-pager", "-q", "-k"]) else { return vec![] };
    let n = out.lines().filter(|l| !l.trim().is_empty()).count();
    if n < 20 {
        return vec![];
    }
    vec![constat("journal", "attention", "Erreurs noyau récentes", format!("{n} erreur(s) noyau dans la dernière heure."),
        "Regarde les erreurs récentes.", "journalctl -p err -b --since -1h --no-pager -k | tail -40")]
}

fn check_memory() -> Vec<Constat> {
    let info = std::fs::read_to_string("/proc/meminfo").unwrap_or_default();
    let kb = |k: &str| info.lines().find_map(|l| l.strip_prefix(k)).and_then(|v| v.split_whitespace().next()?.parse::<f64>().ok());
    let (Some(total), Some(avail)) = (kb("MemTotal:"), kb("MemAvailable:")) else { return vec![] };
    let pc = avail / total * 100.0;
    if pc >= 8.0 {
        return vec![];
    }
    vec![constat("memoire", "attention", "Mémoire presque saturée",
        format!("Il ne reste que {:.1} Go de mémoire vive ({pc:.0} %).", avail / 1024.0 / 1024.0),
        "Ferme les applications les plus gourmandes.", "ps -eo pid,comm,%mem --sort=-%mem | head -8")]
}

/// Lance toutes les vérifications (lecture seule, sans sudo).
pub fn bilan() -> Vec<Constat> {
    let mut v = vec![];
    v.extend(check_disk());
    v.extend(check_memory());
    v.extend(check_failed_services());
    v.extend(check_updates());
    v.extend(check_journal());
    v
}

/// Vérifications instantanées (sans réseau ni journal) : pour l'accueil.
pub fn bilan_rapide() -> Vec<Constat> {
    let mut v = check_disk();
    v.extend(check_memory());
    v.extend(check_failed_services());
    v
}

pub fn bilan_texte() -> String {
    let b = bilan();
    if b.is_empty() {
        return "Aucun problème détecté : disques, mémoire, services, mises à jour et journal sont sains.".into();
    }
    b.iter()
        .map(|c| format!("- [{}] {} : {}\n  conseil : {}\n  commande : {}", c.gravite, c.titre, c.detail, c.conseil, c.commande))
        .collect::<Vec<_>>()
        .join("\n")
}

// --- garde-fous des commandes ---------------------------------------------------

/// Commandes refusées quoi qu'il arrive.
pub fn dangereuse(cmd: &str) -> bool {
    let c = cmd.to_lowercase().split_whitespace().collect::<Vec<_>>().join(" ");
    let roots = ["rm -rf /", "rm -rf /*", "rm -rf ~", "rm -rf $home", "rm -fr /", "rm -rf --no-preserve-root"];
    roots.iter().any(|r| c == *r || c.starts_with(&format!("{r} ")) || c.contains(&format!("{r};")))
        || c.contains(":(){")
        || c.contains(":() {")
        || (c.contains("mkfs") && c.contains("/dev/"))
        || (c.starts_with("dd ") && c.contains("of=/dev/"))
        || c.contains("> /dev/sd")
        || c.contains("> /dev/nvme")
        || c.contains("chmod -r 777 /")
        || c.contains("chown -r") && c.contains(" /")  && c.trim_end().ends_with(" /")
}

/// Commandes autorisées mais irréversibles : on le signale dans la confirmation.
pub fn irreversible(cmd: &str) -> bool {
    let c = format!(" {} ", cmd.to_lowercase());
    [" rm ", " rmdir ", " shred ", " unlink ", " truncate ", " mkswap ", " wipefs ", " fdisk ", " parted ",
     " kill ", " pkill ", " killall ", "reset --hard", "clean -f", "clean -df", "push --force", "push -f ", " chmod -r ", " chown -r "]
        .iter()
        .any(|p| c.contains(p))
}

struct Regle {
    motifs: &'static [&'static str],
    libelle: &'static str,
    message: &'static str,
    correctif: &'static str,
}

const REGLES: &[Regle] = &[
    Regle { motifs: &["could not lock database", "unable to lock database", "db.lck"], libelle: "Verrou pacman",
        message: "pacman ne peut pas verrouiller sa base ; si aucun pacman ne tourne, le verrou est périmé.",
        correctif: "pgrep -x pacman || sudo rm -f /var/lib/pacman/db.lck" },
    Regle { motifs: &["signature is unknown trust", "invalid or corrupted package", "gpgme error"], libelle: "Clés de signature",
        message: "Trousseau de clés obsolète.", correctif: "sudo pacman -Sy --needed archlinux-keyring" },
    Regle { motifs: &["conflicting files", "exists in filesystem"], libelle: "Conflit de fichiers",
        message: "Des fichiers existent déjà : vérifier le paquet propriétaire avant de forcer.", correctif: "" },
    Regle { motifs: &["could not satisfy dependencies", "unable to satisfy dependency", "unmet dependencies"], libelle: "Dépendances",
        message: "Dépendances non satisfaites : mettre le système à jour d'abord.", correctif: "" },
    Regle { motifs: &["target not found", "unable to locate package", "no match for argument"], libelle: "Paquet introuvable",
        message: "Nom de paquet inexact, ou paquet AUR / Flatpak.", correctif: "" },
    Regle { motifs: &["could not resolve host", "temporary failure in name resolution", "name or service not known"], libelle: "Réseau / DNS",
        message: "Pas de résolution de noms : vérifier la connexion.", correctif: "ping -c2 1.1.1.1" },
    Regle { motifs: &["connection refused"], libelle: "Connexion refusée", message: "Le service visé n'écoute pas.", correctif: "" },
    Regle { motifs: &["permission denied", "operation not permitted", "you need to be root"], libelle: "Droits insuffisants",
        message: "Il faut des droits supplémentaires (sudo, à lancer soi-même).", correctif: "" },
    Regle { motifs: &["no space left on device"], libelle: "Disque plein", message: "Plus de place sur le disque.",
        correctif: "du -h -d1 ~ 2>/dev/null | sort -hr | head" },
    Regle { motifs: &["read-only file system"], libelle: "Lecture seule", message: "Système de fichiers monté en lecture seule.", correctif: "" },
    Regle { motifs: &["device or resource busy", "text file busy"], libelle: "Ressource occupée", message: "Fichier ou périphérique utilisé par un autre programme.", correctif: "" },
    Regle { motifs: &["out of memory", "cannot allocate memory", "killed process"], libelle: "Mémoire insuffisante", message: "Le système a manqué de mémoire.", correctif: "" },
    Regle { motifs: &["segmentation fault", "core dumped"], libelle: "Plantage", message: "Le programme a planté.", correctif: "" },
    Regle { motifs: &["not a git repository"], libelle: "Pas un dépôt git", message: "Le dossier n'est pas un dépôt git.", correctif: "" },
    Regle { motifs: &["modulenotfounderror", "no module named"], libelle: "Module Python manquant", message: "Installer le module (de préférence dans un venv).", correctif: "" },
    Regle { motifs: &["externally-managed-environment"], libelle: "Python géré (PEP 668)", message: "Utiliser un venv, pipx ou uv plutôt que pip global.", correctif: "" },
    Regle { motifs: &["address already in use"], libelle: "Port déjà utilisé", message: "Un autre programme utilise ce port.", correctif: "ss -ltnp" },
    Regle { motifs: &["command not found", "commande introuvable"], libelle: "Commande introuvable", message: "Programme non installé ou mal orthographié.", correctif: "" },
    Regle { motifs: &["unit not found", "could not be found."], libelle: "Service systemd", message: "Nom d'unité inexact.", correctif: "systemctl list-units --type=service" },
    Regle { motifs: &["interactive authentication required"], libelle: "Authentification requise", message: "Action système : à lancer soi-même avec sudo.", correctif: "" },
];

/// Diagnostics reconnus dans une sortie de commande (sans doublon).
pub fn diagnostiquer(sortie: &str) -> Vec<String> {
    let s = sortie.to_lowercase();
    REGLES
        .iter()
        .filter(|r| r.motifs.iter().any(|m| s.contains(m)))
        .map(|r| {
            if r.correctif.is_empty() {
                format!("{} : {}", r.libelle, r.message)
            } else {
                format!("{} : {} Correctif proposé : `{}`", r.libelle, r.message, r.correctif)
            }
        })
        .collect()
}

/// Échec « logique » malgré un code retour nul (tests échoués, traceback…).
pub fn echec_logique(sortie: &str) -> Option<String> {
    let s = sortie.to_lowercase();
    ["traceback (most recent call last)", "panic:", "segmentation fault", "build failed", "compilation failed", " failed\n", "error[e", ": error:", "erreur :"]
        .iter()
        .find(|m| s.contains(*m))
        .map(|m| format!("sortie suspecte malgré le code 0 : « {} »", m.trim()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dangerous() {
        assert!(dangereuse("rm -rf /"));
        assert!(dangereuse("sudo rm -rf / --no-preserve-root") || dangereuse("rm -rf --no-preserve-root /"));
        assert!(dangereuse(":(){ :|:& };:"));
        assert!(dangereuse("dd if=/dev/zero of=/dev/sda"));
        assert!(!dangereuse("rm -rf ~/build/target"));
        assert!(irreversible("rm -rf ~/build/target"));
        assert!(!irreversible("ls -la"));
    }

    #[test]
    fn diagnostics() {
        let d = diagnostiquer("error: could not lock database: File exists");
        assert_eq!(d.len(), 1);
        assert!(d[0].starts_with("Verrou pacman"));
        assert!(diagnostiquer("tout va bien").is_empty());
        assert!(echec_logique("Traceback (most recent call last):\n x").is_some());
        assert!(echec_logique("ok").is_none());
    }

    #[test]
    fn bilan_runs() {
        let _ = bilan_texte();
    }
}
