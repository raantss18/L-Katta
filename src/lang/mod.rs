//! Langue de l'interface : français (texte source) ou anglais.
//!
//! Les textes s'écrivent en français dans le code, entourés de `t!` :
//! `t!("Réglages")` ou `t!("{} mail(s) non lu(s)", n)` (`{}` remplis dans l'ordre).
//! En anglais, la traduction vient des tables `en_*.rs` (une par groupe de modules) ;
//! un test vérifie que chaque `t!` du code a sa traduction, avec autant de `{}`.

use std::collections::HashMap;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

mod en_bulles;
mod en_cli;
mod en_ui;

static EN: AtomicBool = AtomicBool::new(false);

/// Toutes les tables anglaises.
fn tables() -> [&'static [(&'static str, &'static str)]; 3] {
    [en_ui::T, en_bulles::T, en_cli::T]
}

fn map() -> &'static HashMap<&'static str, &'static str> {
    static M: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    M.get_or_init(|| tables().iter().flat_map(|t| t.iter().copied()).collect())
}

/// Langue d'après l'environnement (`LANGUAGE`, `LC_ALL`, `LC_MESSAGES`, `LANG`) :
/// français si la première langue trouvée est le français, anglais sinon.
pub fn systeme() -> &'static str {
    for v in ["LANGUAGE", "LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(x) = std::env::var(v) {
            let x = x.trim().to_lowercase();
            if !x.is_empty() && x != "c" && x != "posix" {
                return if x.starts_with("fr") { "fr" } else { "en" };
            }
        }
    }
    "en"
}

/// Applique le réglage : « fr », « en » ou « auto » (langue du système).
pub fn regler(langue: &str) {
    // Les tests tournent en parallèle : la langue n'y change que par `tester_en`.
    if cfg!(test) {
        return;
    }
    let l = match langue.trim() {
        "fr" | "français" | "francais" => "fr",
        "en" | "english" | "anglais" => "en",
        _ => systeme(),
    };
    EN.store(l == "en", Ordering::Relaxed);
}

pub fn en() -> bool {
    EN.load(Ordering::Relaxed)
}

/// « fr » ou « en ».
pub fn code() -> &'static str {
    if en() { "en" } else { "fr" }
}

/// Traduction d'un texte source français (lui-même s'il n'y en a pas).
pub fn tr(fr: &'static str) -> &'static str {
    if !en() {
        return fr;
    }
    map().get(fr).copied().unwrap_or(fr)
}

/// Remplit les `{}` dans l'ordre.
pub fn fill(tpl: &str, args: &[String]) -> String {
    let mut out = String::with_capacity(tpl.len() + 16);
    let mut it = args.iter();
    let mut rest = tpl;
    while let Some(i) = rest.find("{}") {
        out.push_str(&rest[..i]);
        out.push_str(it.next().map(String::as_str).unwrap_or(""));
        rest = &rest[i + 2..];
    }
    out.push_str(rest);
    out
}

/// `t!("texte")` → &'static str ; `t!("texte {} {}", a, b)` → String.
#[macro_export]
macro_rules! t {
    ($s:literal) => {
        $crate::lang::tr($s)
    };
    ($s:literal, $($a:expr),+ $(,)?) => {
        $crate::lang::fill($crate::lang::tr($s), &[$(($a).to_string()),+])
    };
}

#[cfg(test)]
pub(crate) fn tester_en(on: bool) {
    EN.store(on, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Littéraux de `t!("…")` dans le code (échappements \n, \", \\ compris).
    fn litteraux() -> Vec<(String, String)> {
        let re = regex_lite::Regex::new(r#"(?:^|[^A-Za-z0-9_])t!\(\s*"((?:[^"\\]|\\.)*)""#).unwrap();
        let mut out = vec![];
        let mut pile = vec![std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/src"))];
        while let Some(d) = pile.pop() {
            for e in std::fs::read_dir(&d).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    pile.push(p);
                } else if p.extension().is_some_and(|x| x == "rs") && !p.starts_with(concat!(env!("CARGO_MANIFEST_DIR"), "/src/lang")) {
                    let src = std::fs::read_to_string(&p).unwrap();
                    for c in re.captures_iter(&src) {
                        let s = c[1].replace("\\n", "\n").replace("\\\"", "\"").replace("\\\\", "\\");
                        out.push((p.file_name().unwrap().to_string_lossy().into_owned(), s));
                    }
                }
            }
        }
        out
    }

    #[test]
    fn every_text_has_an_english_translation() {
        let m = map();
        let mut manque = vec![];
        for (f, s) in litteraux() {
            match m.get(s.as_str()) {
                None => manque.push(format!("{f} : {s:?}")),
                Some(en) if en.matches("{}").count() != s.matches("{}").count() => manque.push(format!("{f} : nombre de {{}} différent : {s:?} → {en:?}")),
                _ => {}
            }
        }
        manque.sort();
        manque.dedup();
        assert!(manque.is_empty(), "{} texte(s) sans traduction :\n{}", manque.len(), manque.join("\n"));
    }

    #[test]
    fn no_duplicate_or_empty_entries() {
        let mut vu = std::collections::HashSet::new();
        for t in tables() {
            for (fr, en) in t {
                assert!(!en.trim().is_empty(), "vide : {fr:?}");
                assert!(vu.insert(*fr), "en double : {fr:?}");
            }
        }
    }

    #[test]
    fn fills_and_switches() {
        assert_eq!(fill("{} sur {}", &["1".into(), "2".into()]), "1 sur 2");
        assert_eq!(fill("rien", &[]), "rien");
        assert!(["fr", "en"].contains(&systeme()));
    }
}
