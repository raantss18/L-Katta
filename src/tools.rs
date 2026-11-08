//! Outils que le moteur peut appeler pour agir sur la machine.
//!
//! Lecture seule : exécutés directement. Tout ce qui modifie : confirmation demandée.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::brain::{Event, Host};
use crate::config::{Config, which};
use crate::{clipboard, paths, reminders, web};

const MAX_OUT: usize = 8000;

fn tool(name: &str, desc: &str, props: Value, required: &[&str]) -> Value {
    json!({
        "type": "function",
        "function": {
            "name": name,
            "description": desc,
            "parameters": {"type": "object", "properties": props, "required": required},
        }
    })
}

fn s(desc: &str) -> Value {
    json!({"type": "string", "description": desc})
}

/// Schémas OpenAI des outils.
pub fn schemas() -> Value {
    json!([
        tool("etat_systeme", "État de la machine : mémoire, disques, charge, processus gourmands, services en échec, mises à jour en attente.", json!({}), &[]),
        tool("chercher_fichiers", "Cherche des fichiers ou dossiers par nom (motif insensible à la casse, * accepté).",
             json!({"motif": s("ex. « facture*2026 » ou « rapport »"), "dossier": s("dossier de départ, défaut ~")}), &["motif"]),
        tool("chercher_texte", "Cherche les fichiers texte qui contiennent une phrase.",
             json!({"texte": s("texte recherché"), "dossier": s("défaut ~/Documents")}), &["texte"]),
        tool("lister_dossier", "Liste le contenu d'un dossier (taille, date).", json!({"chemin": s("dossier")}), &["chemin"]),
        tool("lire_fichier", "Lit un fichier texte (début du fichier).", json!({"chemin": s("chemin du fichier")}), &["chemin"]),
        tool("creer_fichier", "Crée (ou remplace si ecraser) un fichier texte : notes, scripts, Markdown, CSV, LaTeX… Demande confirmation.",
             json!({"chemin": s("chemin complet"), "contenu": s("contenu"), "ecraser": {"type": "boolean"}}), &["chemin", "contenu"]),
        tool("commande", "Exécute une commande shell. Lecture seule : directe ; sinon l'utilisateur confirme. Jamais de sudo.",
             json!({"commande": s("commande bash")}), &["commande"]),
        tool("ouvrir", "Ouvre un fichier, un dossier ou une adresse web avec l'application par défaut.", json!({"cible": s("chemin ou URL")}), &["cible"]),
        tool("recherche_web", "Recherche sur Internet (titres, liens, extraits).", json!({"requete": s("requête")}), &["requete"]),
        tool("lire_page_web", "Lit le texte d'une page web.", json!({"url": s("adresse http(s)")}), &["url"]),
        tool("actualites", "Dernières nouvelles des flux suivis (Arch, KDE, LinuxFr…) ou d'un flux RSS donné.", json!({"flux": s("URL d'un flux (optionnel)")}), &[]),
        tool("rappel_ajouter", "Programme un rappel. quand : « 10m », « 2h », « 18h30 », « demain 9h » ou « AAAA-MM-JJ HH:MM ».",
             json!({"quand": s("moment"), "texte": s("ce qu'il faut rappeler")}), &["quand", "texte"]),
        tool("rappels_lister", "Liste les rappels à venir.", json!({}), &[]),
        tool("rappel_supprimer", "Supprime un rappel par son numéro.", json!({"id": {"type": "integer"}}), &["id"]),
        tool("presse_papiers", "Historique récent du presse-papiers (texte).", json!({"nombre": {"type": "integer"}}), &[]),
        tool("copier", "Place un texte dans le presse-papiers.", json!({"texte": s("texte")}), &["texte"]),
        tool("sante_machine", "Bilan de santé : disques, mémoire, services en échec, mises à jour, erreurs du journal ; avec conseils et commandes.", json!({}), &[]),
        tool("lancer_application", "Ouvre une application installée par son nom, même approximatif (« navigateur », « dolphin », « gestionnaire de fichiers »).", json!({"nom": s("nom ou description")}), &["nom"]),
        tool("applications", "Liste les applications installées.", json!({}), &[]),
        tool("memoriser", "Retient durablement une préférence ou un fait sur l'utilisateur.", json!({"fait": s("phrase courte")}), &["fait"]),
        tool("oublier", "Oublie les préférences retenues qui contiennent ce texte.", json!({"texte": s("texte")}), &["texte"]),
        tool("memoire", "Cherche dans nos échanges passés.", json!({"requete": s("mots-clés")}), &["requete"]),
        tool("bureaux", "Liste les bureaux virtuels KDE (→ = bureau courant) et les écrans.", json!({}), &[]),
        tool("aller_bureau", "Passe sur un bureau virtuel (numéro à partir de 1, ou nom).", json!({"bureau": s("numéro ou nom")}), &["bureau"]),
        tool("notifier", "Affiche une notification du bureau.", json!({"titre": s("titre"), "texte": s("texte")}), &["titre"]),
    ])
}

fn arg<'a>(a: &'a Value, k: &str) -> &'a str {
    a.get(k).and_then(Value::as_str).unwrap_or("")
}

fn truncate(mut s: String) -> String {
    if s.len() > MAX_OUT {
        let mut cut = MAX_OUT;
        while !s.is_char_boundary(cut) {
            cut -= 1;
        }
        s.truncate(cut);
        s.push_str("\n…(tronqué)");
    }
    s
}

/// Exécute un outil ; renvoie toujours un texte (erreurs comprises) pour le moteur.
pub fn execute(name: &str, a: &Value, cfg: &Config, host: &mut dyn Host) -> String {
    let summary = match name {
        "chercher_fichiers" => format!("je cherche « {} »", arg(a, "motif")),
        "chercher_texte" => format!("je cherche le texte « {} »", arg(a, "texte")),
        "lire_fichier" | "lister_dossier" => format!("je lis {}", arg(a, "chemin")),
        "commande" => format!("$ {}", arg(a, "commande")),
        "recherche_web" => format!("je cherche sur le web « {} »", arg(a, "requete")),
        "lire_page_web" => format!("je lis {}", arg(a, "url")),
        "etat_systeme" => "j'examine la machine".into(),
        other => other.replace('_', " "),
    };
    host.event(Event::Tool(name.to_string(), summary));
    let r = match name {
        "etat_systeme" => Ok(system_status()),
        "chercher_fichiers" => find_files(arg(a, "motif"), arg(a, "dossier")),
        "chercher_texte" => grep_text(arg(a, "texte"), arg(a, "dossier")),
        "lister_dossier" => list_dir(arg(a, "chemin")),
        "lire_fichier" => read_file(arg(a, "chemin")),
        "creer_fichier" => create_file(arg(a, "chemin"), arg(a, "contenu"), a.get("ecraser").and_then(Value::as_bool).unwrap_or(false), host),
        "commande" => shell(arg(a, "commande"), host),
        "ouvrir" => open(arg(a, "cible")),
        "recherche_web" => web::search(arg(a, "requete"), 6).map(|rs| {
            rs.iter().enumerate().map(|(i, r)| format!("{}. {}\n   {}\n   {}", i + 1, r.titre, r.url, r.extrait)).collect::<Vec<_>>().join("\n")
        }),
        "lire_page_web" => web::page_text(arg(a, "url"), 6000),
        "actualites" => Ok(news(arg(a, "flux"), cfg)),
        "rappel_ajouter" => add_reminder(arg(a, "quand"), arg(a, "texte")),
        "rappels_lister" => Ok(list_reminders()),
        "rappel_supprimer" => {
            let id = a.get("id").and_then(Value::as_u64).unwrap_or(0);
            reminders::remove(id).map(|ok| if ok { format!("Rappel {id} supprimé.") } else { format!("Pas de rappel {id}.") }).map_err(|e| e.to_string())
        }
        "presse_papiers" => {
            let n = a.get("nombre").and_then(Value::as_u64).unwrap_or(10) as usize;
            let l = clipboard::load();
            if l.is_empty() {
                Ok("Historique vide.".into())
            } else {
                Ok(l.iter().take(n).enumerate().map(|(i, e)| format!("{}. {}", i + 1, e.texte.chars().take(300).collect::<String>())).collect::<Vec<_>>().join("\n"))
            }
        }
        "copier" => clipboard::copy(arg(a, "texte")).map(|_| "Copié.".into()).map_err(|e| e.to_string()),
        "notifier" => notify(arg(a, "titre"), arg(a, "texte")).map(|_| "Notification affichée.".into()),
        "bureaux" => Ok(format!("## Bureaux virtuels\n{}\n\n## Écrans\n{}", crate::kde::desktops_text(), crate::kde::screens_text())),
        "aller_bureau" => crate::kde::switch(arg(a, "bureau")),
        "sante_machine" => Ok(crate::sante::bilan_texte()),
        "lancer_application" => crate::apps::launch(arg(a, "nom")),
        "applications" => Ok(crate::apps::list_text()),
        "memoriser" => crate::memoire::retenir(arg(a, "fait")),
        "oublier" => crate::memoire::oublier(arg(a, "texte")),
        "memoire" => Ok(crate::memoire::chercher(arg(a, "requete"))),
        other => Err(format!("outil inconnu : {other}")),
    };
    truncate(match r {
        Ok(s) if s.trim().is_empty() => "(rien)".into(),
        Ok(s) => s,
        Err(e) => format!("ERREUR : {e}"),
    })
}

// --- implémentations -------------------------------------------------------

fn run_capture(cmd: &str, timeout: Duration) -> Result<(i32, String), String> {
    let mut child = Command::new("bash")
        .args(["-c", cmd])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .current_dir(paths::home_dir())
        .spawn()
        .map_err(|e| e.to_string())?;
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let t_out = std::thread::spawn(move || {
        let mut v = vec![];
        let _ = std::io::Read::read_to_end(&mut stdout, &mut v);
        v
    });
    let t_err = std::thread::spawn(move || {
        let mut v = vec![];
        let _ = std::io::Read::read_to_end(&mut stderr, &mut v);
        v
    });
    let t0 = Instant::now();
    let status = loop {
        if let Some(st) = child.try_wait().map_err(|e| e.to_string())? {
            break st.code().unwrap_or(-1);
        }
        if t0.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("délai dépassé ({} s)", timeout.as_secs()));
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let mut out = String::from_utf8_lossy(&t_out.join().unwrap_or_default()).into_owned();
    let err = String::from_utf8_lossy(&t_err.join().unwrap_or_default()).into_owned();
    if !err.trim().is_empty() {
        out.push_str("\n[stderr]\n");
        out.push_str(&err);
    }
    Ok((status, out))
}

fn quick(cmd: &str) -> String {
    run_capture(cmd, Duration::from_secs(8)).map(|(_, o)| o.trim().to_string()).unwrap_or_default()
}

pub fn system_status() -> String {
    let mut parts = vec![
        format!("## Mémoire\n{}", quick("free -h")),
        format!("## Disques\n{}", quick("df -h -x tmpfs -x devtmpfs -x efivarfs -x overlay 2>/dev/null")),
        format!("## Charge\n{}", quick("uptime")),
        format!("## Processus les plus gourmands (mémoire)\n{}", quick("ps -eo pid,comm,%mem,%cpu --sort=-%mem | head -8")),
    ];
    let failed = quick("systemctl --failed --no-legend --plain 2>/dev/null; systemctl --user --failed --no-legend --plain 2>/dev/null");
    parts.push(format!("## Services en échec\n{}", if failed.is_empty() { "aucun".into() } else { failed }));
    if which("checkupdates") {
        let n = quick("checkupdates 2>/dev/null | wc -l");
        parts.push(format!("## Mises à jour en attente (pacman)\n{n}"));
    }
    let temp = quick("sensors 2>/dev/null | grep -E '^(Package|Tctl|Composite|edge)' | head -4");
    if !temp.is_empty() {
        parts.push(format!("## Températures\n{temp}"));
    }
    let cache = quick("du -sh ~/.cache 2>/dev/null | cut -f1");
    if !cache.is_empty() {
        parts.push(format!("## Cache utilisateur (~/.cache)\n{cache}"));
    }
    parts.join("\n\n")
}

const SKIP_DIRS: &[&str] = &["node_modules", ".git", "target", ".cache", "__pycache__", ".venv", "venv", ".gradle", "build", ".local/share/Trash"];

/// Motif non ancré : les morceaux séparés par `*` doivent apparaître dans l'ordre.
fn wildcard_match(pattern: &str, name: &str) -> bool {
    let n = name.to_lowercase();
    let mut pos = 0;
    for part in pattern.to_lowercase().split('*').filter(|p| !p.is_empty()) {
        match n[pos..].find(part) {
            Some(f) => pos += f + part.len(),
            None => return false,
        }
    }
    true
}

fn find_files(motif: &str, dossier: &str) -> Result<String, String> {
    if motif.trim().is_empty() {
        return Err("motif vide".into());
    }
    let root = if dossier.is_empty() { paths::home_dir() } else { paths::expand(dossier) };
    let t0 = Instant::now();
    let mut hits = vec![];
    let walker = walkdir::WalkDir::new(&root).max_depth(12).into_iter().filter_entry(|e| {
        let n = e.file_name().to_string_lossy();
        e.depth() == 0 || !(SKIP_DIRS.contains(&n.as_ref()) || (n.starts_with('.') && e.file_type().is_dir() && n != ".config"))
    });
    for e in walker.flatten() {
        if t0.elapsed() > Duration::from_secs(8) || hits.len() >= 40 {
            hits.push("…(recherche arrêtée : trop de résultats ou trop long)".into());
            break;
        }
        if e.depth() > 0 && wildcard_match(motif, &e.file_name().to_string_lossy()) {
            let kind = if e.file_type().is_dir() { "📁" } else { "" };
            hits.push(format!("{kind}{}", e.path().display()));
        }
    }
    if hits.is_empty() { Ok(format!("Rien trouvé pour « {motif} » sous {}.", root.display())) } else { Ok(hits.join("\n")) }
}

fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

fn grep_text(texte: &str, dossier: &str) -> Result<String, String> {
    let root = if dossier.is_empty() { paths::home_dir().join("Documents") } else { paths::expand(dossier) };
    let cmd = if which("rg") {
        format!("rg -l -i -F --max-filesize 5M -- {} {} | head -30", sh_quote(texte), sh_quote(&root.to_string_lossy()))
    } else {
        format!("grep -rIl -i -F --exclude-dir=.git --exclude-dir=node_modules -- {} {} | head -30", sh_quote(texte), sh_quote(&root.to_string_lossy()))
    };
    let (_, out) = run_capture(&cmd, Duration::from_secs(20))?;
    Ok(if out.trim().is_empty() { format!("Aucun fichier ne contient « {texte} ».") } else { out })
}

fn list_dir(chemin: &str) -> Result<String, String> {
    let p = paths::expand(if chemin.is_empty() { "~" } else { chemin });
    let mut rows = vec![];
    for e in std::fs::read_dir(&p).map_err(|e| format!("{} : {e}", p.display()))?.flatten() {
        let md = e.metadata().ok();
        let size = md.as_ref().map(|m| m.len()).unwrap_or(0);
        let date = md
            .and_then(|m| m.modified().ok())
            .map(|t| chrono::DateTime::<chrono::Local>::from(t).format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_default();
        let dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
        rows.push((dir, e.file_name().to_string_lossy().into_owned(), size, date));
    }
    rows.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.to_lowercase().cmp(&b.1.to_lowercase())));
    Ok(rows
        .iter()
        .take(200)
        .map(|(d, n, s, t)| if *d { format!("📁 {n}/  {t}") } else { format!("   {n}  {}  {t}", human(*s)) })
        .collect::<Vec<_>>()
        .join("\n"))
}

fn human(b: u64) -> String {
    match b {
        0..1024 => format!("{b} o"),
        1024..1_048_576 => format!("{:.0} Ko", b as f64 / 1024.0),
        1_048_576..1_073_741_824 => format!("{:.1} Mo", b as f64 / 1_048_576.0),
        _ => format!("{:.1} Go", b as f64 / 1_073_741_824.0),
    }
}

fn read_file(chemin: &str) -> Result<String, String> {
    let p = paths::expand(chemin);
    let bytes = std::fs::read(&p).map_err(|e| format!("{} : {e}", p.display()))?;
    if bytes.iter().take(4096).any(|b| *b == 0) {
        return Err("fichier binaire".into());
    }
    let s = String::from_utf8_lossy(&bytes);
    Ok(s.chars().take(MAX_OUT).collect())
}

fn create_file(chemin: &str, contenu: &str, ecraser: bool, host: &mut dyn Host) -> Result<String, String> {
    let p: PathBuf = paths::expand(chemin);
    if !p.is_absolute() {
        return Err("chemin absolu ou commençant par ~ attendu".into());
    }
    let exists = p.exists();
    if exists && !ecraser {
        return Err(format!("{} existe déjà (ecraser=true pour remplacer)", p.display()));
    }
    let verb = if exists { "Remplacer" } else { "Créer" };
    let q = format!("{verb} le fichier {} ({} lignes) ?", p.display(), contenu.lines().count());
    if !host.confirm(&q) {
        return Ok("Refusé par l'utilisateur.".into());
    }
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    std::fs::write(&p, contenu).map_err(|e| e.to_string())?;
    Ok(format!("Fichier écrit : {}", p.display()))
}

const READ_ONLY: &[&str] = &[
    "ls", "ll", "df", "du", "free", "uptime", "uname", "cat", "head", "tail", "grep", "rg", "fd", "find", "ps",
    "pgrep", "wc", "sort", "uniq", "cut", "awk", "stat", "file", "which", "whereis", "whoami", "id", "date", "cal",
    "pwd", "echo", "printf", "lsblk", "lscpu", "lspci", "lsusb", "lsmod", "sensors", "ip", "ss", "hostnamectl",
    "timedatectl", "journalctl", "checkupdates", "neofetch", "fastfetch", "inxi", "env", "printenv", "locale",
    "realpath", "basename", "dirname", "tree", "less", "more", "nproc", "vmstat", "iostat", "dmesg", "xdg-user-dir",
    "flatpak", "pactl", "wpctl", "nmcli", "bluetoothctl", "kreadconfig6", "git", "column", "tr", "jq", "md5sum", "sha256sum",
];

/// Une commande est « lecture seule » si chaque maillon du tube commence par un
/// programme sûr, sans redirection, enchaînement ni sous-commande.
pub fn is_read_only(cmd: &str) -> bool {
    let c = cmd.trim();
    if c.is_empty() || ["sudo", ">", "<", ";", "&&", "||", "$(", "`", "&", "\n"].iter().any(|t| c.contains(t)) {
        return false;
    }
    let risky_args = ["-delete", "-exec", "-execdir", "-ok", "--vacuum", "--rotate", "-fprint"];
    c.split('|').all(|seg| {
        let words: Vec<&str> = seg.split_whitespace().collect();
        let Some(first) = words.first() else { return false };
        if !READ_ONLY.contains(first) || words.iter().any(|w| risky_args.contains(w)) {
            return false;
        }
        match *first {
            "git" => words.get(1).is_some_and(|w| ["status", "log", "diff", "show", "branch", "remote"].contains(w)),
            "flatpak" => words.get(1).is_some_and(|w| ["list", "info", "remotes"].contains(w)),
            "ip" | "nmcli" | "bluetoothctl" | "pactl" | "wpctl" => {
                !words.iter().any(|w| ["set", "add", "del", "delete", "connect", "disconnect", "up", "down", "remove", "modify", "power", "load-module", "unload-module"].contains(w))
            }
            _ => true,
        }
    })
}

fn shell(cmd: &str, host: &mut dyn Host) -> Result<String, String> {
    if cmd.trim().is_empty() {
        return Err("commande vide".into());
    }
    if cmd.split_whitespace().any(|w| w == "sudo" || w == "pkexec" || w == "su") {
        let _ = clipboard::copy(cmd);
        return Ok(format!(
            "Commande avec droits administrateur non lancée. Je l'ai copiée dans le presse-papiers pour que l'utilisateur la colle dans un terminal :\n{cmd}"
        ));
    }
    if crate::sante::dangereuse(cmd) {
        return Ok("Commande refusée : elle peut détruire le système. Je ne la lancerai pas.".into());
    }
    if !is_read_only(cmd) {
        let warn = if crate::sante::irreversible(cmd) { " ⚠ irréversible" } else { "" };
        if !host.confirm(&format!("Exécuter cette commande ?{warn}\n$ {cmd}")) {
            return Ok("Refusé par l'utilisateur.".into());
        }
    }
    let (code, out) = run_capture(cmd, Duration::from_secs(120))?;
    let mut res = format!("[code {code}]\n{out}");
    if code == 0 {
        if let Some(e) = crate::sante::echec_logique(&out) {
            res.push_str(&format!("\n[statut] échec probable : {e}"));
        }
    }
    let diags = crate::sante::diagnostiquer(&out);
    if !diags.is_empty() {
        res.push_str("\n[diagnostic]\n- ");
        res.push_str(&diags.join("\n- "));
    }
    Ok(res)
}

fn open(cible: &str) -> Result<String, String> {
    let target = if cible.starts_with("http") { cible.to_string() } else { paths::expand(cible).to_string_lossy().into_owned() };
    if !target.starts_with("http") && !Path::new(&target).exists() {
        return Err(format!("{target} n'existe pas"));
    }
    Command::new("xdg-open")
        .arg(&target)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(format!("Ouvert : {target}"))
}

pub fn news(flux: &str, cfg: &Config) -> String {
    let feeds: Vec<String> = if flux.is_empty() { cfg.actualites.flux.clone() } else { vec![flux.to_string()] };
    let mut out = vec![];
    for f in feeds {
        match web::feed(&f, 5) {
            Ok(items) => {
                let src = items.first().map(|i| i.source.clone()).unwrap_or_else(|| f.clone());
                out.push(format!("## {src}"));
                for i in items {
                    out.push(format!("- {} ({}) {}", i.titre, i.date, i.lien));
                }
            }
            Err(e) => out.push(format!("## {f}\n(illisible : {e})")),
        }
    }
    out.join("\n")
}

fn add_reminder(quand: &str, texte: &str) -> Result<String, String> {
    let now = chrono::Local::now();
    let when = reminders::parse_when(quand, now).ok_or_else(|| format!("moment incompris : « {quand} »"))?;
    let r = reminders::add(when, texte).map_err(|e| e.to_string())?;
    Ok(format!("Rappel n°{} programmé pour {} : {}", r.id, crate::fr::date_longue(&when), r.texte))
}

pub fn list_reminders() -> String {
    let l: Vec<_> = reminders::load().into_iter().filter(|r| !r.fait).collect();
    if l.is_empty() {
        return "Aucun rappel à venir.".into();
    }
    l.iter()
        .map(|r| format!("n°{} — {} : {}", r.id, r.date().map(|d| crate::fr::date_longue(&d)).unwrap_or_default(), r.texte))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn notify(titre: &str, texte: &str) -> Result<(), String> {
    notify_rust::Notification::new()
        .appname("L Katta")
        .summary(titre)
        .body(texte)
        .icon("face-smile")
        .show()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_only_detection() {
        assert!(is_read_only("df -h"));
        assert!(is_read_only("ps aux | grep firefox | head -5"));
        assert!(is_read_only("git status"));
        assert!(!is_read_only("git push"));
        assert!(!is_read_only("rm -rf ~/x"));
        assert!(!is_read_only("ls > a.txt"));
        assert!(!is_read_only("find . -delete"));
        assert!(!is_read_only("ls; rm a"));
        assert!(!is_read_only("echo $(rm a)"));
        assert!(!is_read_only("nmcli radio wifi off; ls"));
        assert!(!is_read_only("sudo pacman -Syu"));
    }

    #[test]
    fn wildcard() {
        assert!(wildcard_match("rapport", "Rapport_final.pdf"));
        assert!(wildcard_match("*.pdf", "a.pdf"));
        assert!(wildcard_match("facture*2026", "Facture_mars_2026"));
        assert!(!wildcard_match("facture*2026", "Facture_mars_2025"));
        assert!(!wildcard_match("*.pdf", "a.txt"));
    }

    #[test]
    fn schemas_are_valid() {
        let s = schemas();
        let arr = s.as_array().unwrap();
        assert!(arr.len() >= 22);
        for t in arr {
            assert_eq!(t["type"], "function");
            assert!(t["function"]["name"].is_string());
        }
    }
}
