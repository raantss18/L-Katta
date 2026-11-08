//! Markdown simple pour la bulle : titres, listes, blocs de code, **gras**, `code`,
//! [liens](https://…) et adresses nues. Le reste passe tel quel.

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Span {
    pub t: String,
    pub gras: bool,
    pub code: bool,
    pub lien: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Bloc {
    Para(Vec<Span>),
    /// Élément de liste, avec sa puce (« • » ou « 1. »).
    Puce(String, Vec<Span>),
    Titre(Vec<Span>),
    Code(Vec<String>),
}

pub fn blocs(texte: &str) -> Vec<Bloc> {
    let mut out = vec![];
    let mut code: Option<Vec<String>> = None;
    for l in texte.lines() {
        let t = l.trim_start();
        if t.starts_with("```") {
            match code.take() {
                Some(c) => out.push(Bloc::Code(c)),
                None => code = Some(vec![]),
            }
            continue;
        }
        if let Some(c) = &mut code {
            c.push(l.trim_end().to_string());
            continue;
        }
        if t.is_empty() {
            continue;
        }
        if let Some(r) = t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")).or_else(|| t.strip_prefix("• ")) {
            out.push(Bloc::Puce("•".into(), spans(r)));
        } else if let Some((n, r)) = t.split_once(". ").filter(|(n, _)| !n.is_empty() && n.len() <= 3 && n.chars().all(|c| c.is_ascii_digit())) {
            out.push(Bloc::Puce(format!("{n}."), spans(r)));
        } else if t.starts_with('#') {
            out.push(Bloc::Titre(spans(t.trim_start_matches('#').trim())));
        } else {
            out.push(Bloc::Para(spans(t)));
        }
    }
    if let Some(c) = code {
        out.push(Bloc::Code(c));
    }
    out
}

fn fin_url(s: &str) -> usize {
    let n = s.find(|c: char| c.is_whitespace() || matches!(c, '<' | '>' | '"' | '`')).unwrap_or(s.len());
    // Ponctuation finale : pas dans le lien.
    s[..n].trim_end_matches(['.', ',', ';', ':', '!', '?', ')', '»']).len()
}

/// Découpe une ligne en morceaux stylés.
pub fn spans(s: &str) -> Vec<Span> {
    let mut out: Vec<Span> = vec![];
    let mut cur = String::new();
    let mut gras = false;
    let flush = |cur: &mut String, out: &mut Vec<Span>, gras: bool| {
        if !cur.is_empty() {
            out.push(Span { t: std::mem::take(cur), gras, ..Default::default() });
        }
    };
    let mut i = 0;
    while i < s.len() {
        let r = &s[i..];
        if r.starts_with("**") {
            flush(&mut cur, &mut out, gras);
            gras = !gras;
            i += 2;
        } else if let Some(n) = italique(r) {
            // *italique* : pas d'italique dans la bulle, on garde le texte.
            cur.push_str(&r[1..n - 1]);
            i += n;
        } else if let Some(rest) = r.strip_prefix('`').filter(|x| x.contains('`')) {
            let n = rest.find('`').unwrap();
            flush(&mut cur, &mut out, gras);
            out.push(Span { t: rest[..n].to_string(), code: true, gras, ..Default::default() });
            i += n + 2;
        } else if let Some((txt, url, n)) = lien_md(r) {
            flush(&mut cur, &mut out, gras);
            out.push(Span { t: txt, lien: Some(url), gras, ..Default::default() });
            i += n;
        } else if r.starts_with("https://") || r.starts_with("http://") {
            let n = fin_url(r).max(1);
            flush(&mut cur, &mut out, gras);
            out.push(Span { t: r[..n].to_string(), lien: Some(r[..n].to_string()), gras, ..Default::default() });
            i += n;
        } else {
            let c = r.chars().next().unwrap();
            cur.push(c);
            i += c.len_utf8();
        }
    }
    flush(&mut cur, &mut out, gras);
    out
}

/// `*mot*` au début de `r` (pas `**`, pas « 2 * 3 ») : longueur consommée.
fn italique(r: &str) -> Option<usize> {
    let m = r.chars().next().filter(|c| *c == '*')?;
    let rest = &r[1..];
    if rest.starts_with(m) || rest.starts_with(char::is_whitespace) {
        return None;
    }
    let n = rest.find(m)?;
    let inner = &rest[..n];
    (!inner.is_empty() && !inner.ends_with(char::is_whitespace) && !inner.contains('\n')).then_some(n + 2)
}

/// `[texte](https://…)` au début de `r` : (texte, adresse, longueur consommée).
fn lien_md(r: &str) -> Option<(String, String, usize)> {
    let rest = r.strip_prefix('[')?;
    let fin_t = rest.find("](")?;
    let txt = &rest[..fin_t];
    if txt.contains('\n') {
        return None;
    }
    let apres = &rest[fin_t + 2..];
    let fin_u = apres.find(')')?;
    let url = apres[..fin_u].trim();
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return None;
    }
    Some((txt.to_string(), url.to_string(), 1 + fin_t + 2 + fin_u + 1))
}

/// Texte brut (pour copier une réponse sans les marques).
pub fn brut(texte: &str) -> String {
    blocs(texte)
        .into_iter()
        .map(|b| {
            let j = |v: Vec<Span>| v.into_iter().map(|s| s.t).collect::<String>();
            match b {
                Bloc::Para(v) | Bloc::Titre(v) => j(v),
                Bloc::Puce(p, v) => format!("{p} {}", j(v)),
                Bloc::Code(l) => l.join("\n"),
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_blocks_and_inline() {
        let b = blocs("## Bilan\nLe disque est **presque plein** : lance `df -h`.\n- un\n2. deux\n```bash\nls -la\n```\nVoir [la doc](https://wiki.archlinux.org/title/Pacman).");
        assert_eq!(b.len(), 6);
        assert!(matches!(&b[0], Bloc::Titre(v) if v[0].t == "Bilan"));
        let Bloc::Para(p) = &b[1] else { panic!() };
        assert!(p.iter().any(|s| s.gras && s.t == "presque plein"));
        assert!(p.iter().any(|s| s.code && s.t == "df -h"));
        assert!(matches!(&b[2], Bloc::Puce(p, _) if p == "•"));
        assert!(matches!(&b[3], Bloc::Puce(p, _) if p == "2."));
        assert_eq!(b[4], Bloc::Code(vec!["ls -la".into()]));
        let Bloc::Para(l) = &b[5] else { panic!() };
        assert_eq!(l[1].lien.as_deref(), Some("https://wiki.archlinux.org/title/Pacman"));
        assert_eq!(l[1].t, "la doc");
    }

    #[test]
    fn bare_urls_and_plain_copy() {
        let s = spans("Lis https://archlinux.org/news/. Merci");
        assert_eq!(s[1].lien.as_deref(), Some("https://archlinux.org/news/"));
        assert_eq!(s[2].t, ". Merci");
        assert_eq!(brut("**Oui** : `ls`\n- a"), "Oui : ls\n• a");
        assert_eq!(brut("prend *47 Go* et 2 * 3"), "prend 47 Go et 2 * 3");
        assert_eq!(brut("mon_fichier_test.txt"), "mon_fichier_test.txt");
        // Un `[` ordinaire ne casse rien.
        assert_eq!(spans("tableau[0] (ok)")[0].t, "tableau[0] (ok)");
    }
}
