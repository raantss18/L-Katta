//! Petit protocole texte sur socket Unix entre la ligne de commande / le panneau et le démon.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};

use crate::paths;

#[derive(Debug, Clone, PartialEq)]
pub enum Msg {
    /// Parole demandée (toujours affichée).
    Say(String),
    /// Remarque spontanée (soumise à la discrétion).
    Hint(String),
    /// Rappel échu (toujours affiché).
    Reminder(String),
    Flip,
    Wave,
    Tree,
    Teleport,
    Climb,
    /// Ouvre ou ferme la bulle de discussion.
    Chat,
    /// Bulle typée ; `true` = toujours affichée, `false` = remarque spontanée.
    Bubble(crate::conseils::Message, bool),
    /// Écran où l'on travaille (nom KWin, ex. « HDMI-A-1 »).
    ActiveOutput(String),
    /// Relit config.toml.
    Reload,
    /// Va sur un autre écran.
    Screen,
    /// Le bureau virtuel courant a changé (identifiant KWin).
    Desktop(String),
    /// Discrétion : Some(v) fixe, None bascule.
    Quiet(Option<bool>),
    /// Masquer le maki : Some(v) fixe, None bascule.
    Hide(Option<bool>),
    Quit,
}

pub fn parse(line: &str) -> Option<Msg> {
    let (cmd, rest) = line.trim().split_once(' ').unwrap_or((line.trim(), ""));
    let rest = rest.trim().to_string();
    let flag = |r: &str| match r {
        "on" | "oui" | "1" => Some(true),
        "off" | "non" | "0" => Some(false),
        _ => None,
    };
    Some(match cmd {
        "dire" if !rest.is_empty() => Msg::Say(rest),
        "remarque" if !rest.is_empty() => Msg::Hint(rest),
        "rappel" if !rest.is_empty() => Msg::Reminder(rest),
        "acrobatie" => Msg::Flip,
        "salut" => Msg::Wave,
        "arbre" => Msg::Tree,
        "teleporter" => Msg::Teleport,
        "grimper" => Msg::Climb,
        "discuter" | "panneau" => Msg::Chat,
        "ecran" => Msg::Screen,
        "recharger" => Msg::Reload,
        // bulle <genre> <titre> | <texte>   (genres : bonjour, astuce, rappel, alerte, nouvelle, reponse, info)
        "bulle" => {
            let (kind, rest) = rest.split_once(' ').unwrap_or((rest.as_str(), ""));
            let kind = crate::conseils::Kind::parse(kind)?;
            let (title, text) = rest.split_once('|').unwrap_or(("", rest));
            Msg::Bubble(crate::conseils::Message::new(kind, title.trim(), text.trim()), true)
        }
        "astuce" => Msg::Bubble(crate::conseils::astuce(), true),
        "discret" => Msg::Quiet(flag(&rest)),
        "cacher" => Msg::Hide(flag(&rest)),
        "quitter" => Msg::Quit,
        _ => return None,
    })
}

/// Écoute le socket (bloquant, à lancer dans un fil).
pub fn listen(on_msg: impl Fn(Msg) + Send + 'static) -> std::io::Result<()> {
    let path = paths::socket_file();
    std::fs::create_dir_all(path.parent().unwrap())?;
    let _ = std::fs::remove_file(&path);
    let listener = UnixListener::bind(&path)?;
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut w = match stream.try_clone() {
                Ok(w) => w,
                Err(_) => continue,
            };
            let mut line = String::new();
            if BufReader::new(stream).read_line(&mut line).is_ok() {
                match parse(&line) {
                    Some(m) => {
                        on_msg(m);
                        let _ = w.write_all(b"ok\n");
                    }
                    None => {
                        let _ = w.write_all(b"commande inconnue\n");
                    }
                }
            }
        }
    });
    Ok(())
}

/// Envoie une commande au démon. Erreur si le démon ne tourne pas.
pub fn send(line: &str) -> std::io::Result<String> {
    let mut s = UnixStream::connect(paths::socket_file())?;
    s.write_all(line.replace('\n', " ").as_bytes())?;
    s.write_all(b"\n")?;
    let mut out = String::new();
    BufReader::new(s).read_line(&mut out)?;
    Ok(out.trim().to_string())
}

pub fn daemon_running() -> bool {
    UnixStream::connect(paths::socket_file()).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parsing() {
        assert_eq!(parse("dire Bonjour toi"), Some(Msg::Say("Bonjour toi".into())));
        assert_eq!(parse("discret on"), Some(Msg::Quiet(Some(true))));
        assert_eq!(parse("cacher"), Some(Msg::Hide(None)));
        assert_eq!(parse("dire"), None);
        assert_eq!(parse("n'importe"), None);
    }
}
