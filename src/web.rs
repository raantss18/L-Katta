//! Internet sans clé : recherche DuckDuckGo (version HTML), lecture de page, flux RSS/Atom.

use std::time::Duration;

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(20)))
        .user_agent("Mozilla/5.0 (X11; Linux x86_64) LKatta")
        // Les redirections sont suivies à la main : chaque étape est vérifiée (voir `get_text`).
        .max_redirects(0)
        .max_redirects_will_error(false)
        .build()
        .into()
}

/// Hôte d'une adresse http(s) (sans identifiants ni port ; IPv6 sans crochets).
pub fn hote(url: &str) -> Option<String> {
    let r = url.split_once("://")?.1;
    let autorite = r.split(['/', '?', '#']).next()?;
    let h = autorite.rsplit('@').next()?;
    let h = if let Some(v6) = h.strip_prefix('[') { v6.split(']').next()? } else { h.split(':').next()? };
    (!h.is_empty()).then(|| h.to_lowercase())
}

fn ip_privee(ip: std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(v) => v.is_loopback() || v.is_private() || v.is_link_local() || v.is_unspecified() || v.is_broadcast() || v.octets()[0] == 100 && (v.octets()[1] & 0xC0) == 64,
        std::net::IpAddr::V6(v) => {
            v.is_loopback() || v.is_unspecified() || (v.segments()[0] & 0xfe00) == 0xfc00 || (v.segments()[0] & 0xffc0) == 0xfe80 || v.to_ipv4_mapped().is_some_and(|m| ip_privee(std::net::IpAddr::V4(m)))
        }
    }
}

/// L'adresse vise-t-elle cette machine ou le réseau local (services internes, box, Tailscale) ?
/// Une page web ou un mail ne doit pas pouvoir y envoyer L Katta.
pub fn adresse_locale(url: &str) -> bool {
    use std::net::ToSocketAddrs;
    let Some(h) = hote(url) else { return true };
    if h == "localhost" || h.ends_with(".localhost") || h.ends_with(".local") || h.ends_with(".internal") || h.ends_with(".lan") || h.ends_with(".home.arpa") {
        return true;
    }
    if let Ok(ip) = h.parse::<std::net::IpAddr>() {
        return ip_privee(ip);
    }
    // Un nom public qui pointe vers une adresse privée compte aussi.
    (h.as_str(), 443).to_socket_addrs().map(|mut a| a.any(|s| ip_privee(s.ip()))).unwrap_or(false)
}

/// Adresse qui emporte beaucoup de données (longue requête, long jeton) : une fuite possible.
pub fn url_bavarde(url: &str) -> bool {
    let requete = url.split_once('?').map(|(_, q)| q.len()).unwrap_or(0);
    let jeton = url.split(|c: char| !(c.is_ascii_alphanumeric() || "+=_-%".contains(c))).any(|s| s.len() >= 64);
    url.len() > 400 || requete > 160 || jeton
}

fn get_text(url: &str) -> Result<String, String> {
    let mut url = url.to_string();
    for _ in 0..6 {
        if !(url.starts_with("http://") || url.starts_with("https://")) {
            return Err("adresse http(s) attendue".into());
        }
        if adresse_locale(&url) {
            return Err(t!("adresse refusée : elle vise cette machine ou le réseau local").into());
        }
        let mut r = agent().get(&url).call().map_err(|e| e.to_string())?;
        let code = r.status().as_u16();
        if (300..400).contains(&code) {
            let Some(loc) = r.headers().get("location").and_then(|v| v.to_str().ok()) else { return Err(format!("redirection sans adresse ({code})")) };
            url = if loc.starts_with("http://") || loc.starts_with("https://") {
                loc.to_string()
            } else if let Some(rest) = loc.strip_prefix("//") {
                format!("{}://{rest}", url.split("://").next().unwrap_or("https"))
            } else {
                let base = url.split_once("://").map(|(s, r)| format!("{s}://{}", r.split(['/', '?', '#']).next().unwrap_or(""))).unwrap_or_default();
                if loc.starts_with('/') { format!("{base}{loc}") } else { format!("{base}/{loc}") }
            };
            continue;
        }
        if code >= 400 {
            return Err(format!("http status: {code}"));
        }
        return r.body_mut().with_config().limit(3 * 1024 * 1024).read_to_string().map_err(|e| e.to_string());
    }
    Err("trop de redirections".into())
}

pub struct Resultat {
    pub titre: String,
    pub url: String,
    pub extrait: String,
}

/// Recherche web ; renvoie au plus `n` résultats.
pub fn search(query: &str, n: usize) -> Result<Vec<Resultat>, String> {
    let url = format!("https://html.duckduckgo.com/html/?q={}", encode(query));
    let html = get_text(&url)?;
    Ok(parse_ddg(&html, n))
}

fn parse_ddg(html: &str, n: usize) -> Vec<Resultat> {
    let mut out = vec![];
    for chunk in html.split("class=\"result__a\"").skip(1) {
        if out.len() >= n {
            break;
        }
        let href = between(chunk, "href=\"", "\"").unwrap_or_default();
        let url = match href.split_once("uddg=") {
            Some((_, rest)) => decode(rest.split('&').next().unwrap_or("")),
            None => html_unescape(&href),
        };
        let titre = between(chunk, ">", "</a>").map(|t| strip_tags(&t)).unwrap_or_default();
        let extrait = between(chunk, "class=\"result__snippet\"", "</a>")
            .and_then(|s| s.split_once('>').map(|(_, r)| strip_tags(r)))
            .unwrap_or_default();
        if url.starts_with("http") && !url.contains("duckduckgo.com/y.js") {
            out.push(Resultat { titre, url, extrait });
        }
    }
    out
}

/// Texte lisible d'une page web (tronqué à `max` caractères).
pub fn page_text(url: &str, max: usize) -> Result<String, String> {
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err("adresse http(s) attendue".into());
    }
    let html = get_text(url)?;
    let mut s = remove_blocks(&html, "script");
    s = remove_blocks(&s, "style");
    s = remove_blocks(&s, "nav");
    let t = strip_tags(&s);
    Ok(t.chars().take(max).collect())
}

pub struct Nouvelle {
    pub titre: String,
    pub lien: String,
    pub date: String,
    pub source: String,
}

/// Lit un flux RSS ou Atom.
pub fn feed(url: &str, n: usize) -> Result<Vec<Nouvelle>, String> {
    let xml = get_text(url)?;
    parse_feed(&xml, n)
}

fn parse_feed(xml: &str, n: usize) -> Result<Vec<Nouvelle>, String> {
    let doc = roxmltree::Document::parse(xml).map_err(|e| e.to_string())?;
    let child_text = |node: roxmltree::Node, name: &str| {
        node.children()
            .find(|c| c.tag_name().name() == name)
            .and_then(|c| c.text())
            .map(|t| t.trim().to_string())
            .unwrap_or_default()
    };
    let source = doc
        .descendants()
        .find(|d| d.tag_name().name() == "title")
        .and_then(|t| t.text())
        .unwrap_or("")
        .trim()
        .to_string();
    let mut out = vec![];
    for node in doc.descendants().filter(|d| matches!(d.tag_name().name(), "item" | "entry")) {
        if out.len() >= n {
            break;
        }
        let lien = {
            let l = child_text(node, "link");
            if l.is_empty() {
                node.children()
                    .find(|c| c.tag_name().name() == "link")
                    .and_then(|c| c.attribute("href"))
                    .unwrap_or("")
                    .to_string()
            } else {
                l
            }
        };
        let mut date = child_text(node, "pubDate");
        if date.is_empty() {
            date = child_text(node, "updated");
        }
        if date.is_empty() {
            date = child_text(node, "published");
        }
        out.push(Nouvelle { titre: strip_tags(&child_text(node, "title")), lien, date, source: source.clone() });
    }
    Ok(out)
}

// --- petites aides texte ---------------------------------------------------

fn between(s: &str, a: &str, b: &str) -> Option<String> {
    let start = s.find(a)? + a.len();
    let end = s[start..].find(b)? + start;
    Some(s[start..end].to_string())
}

fn remove_blocks(s: &str, tag: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    loop {
        let lower = rest.to_ascii_lowercase();
        match lower.find(&open) {
            Some(a) => {
                out.push_str(&rest[..a]);
                match lower[a..].find(&close) {
                    Some(b) => rest = &rest[a + b + close.len()..],
                    None => break,
                }
            }
            None => {
                out.push_str(rest);
                break;
            }
        }
    }
    out
}

pub fn strip_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => {
                in_tag = true;
                out.push(' ');
            }
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    let un = html_unescape(&out);
    // Lignes non vides, espaces compactés.
    un.lines()
        .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn html_unescape(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#x27;", "'")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
}

pub fn encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

pub fn decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(v) = u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""), 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(if bytes[i] == b'+' { b' ' } else { bytes[i] });
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_local_addresses_and_chatty_urls() {
        for u in ["http://127.0.0.1:11434/api/tags", "http://localhost:8765/v1", "http://[::1]/", "http://192.168.1.1/", "http://10.0.0.5:8420", "http://100.101.102.103/", "http://169.254.169.254/latest/meta-data", "http://box.local/", "http://user:pw@127.0.0.1/"] {
            assert!(adresse_locale(u), "{u}");
        }
        assert!(!adresse_locale("https://93.184.216.34/page"));
        assert_eq!(hote("https://User@Example.org:8443/a?b").as_deref(), Some("example.org"));
        assert!(!url_bavarde("https://fr.wikipedia.org/wiki/Lemur_catta?uselang=fr"));
        assert!(url_bavarde(&format!("https://x.example/?k={}", "A".repeat(200))));
        assert!(url_bavarde(&format!("https://x.example/{}", "b3BlbnNzaC1rZXk".repeat(6))));
    }

    #[test]
    fn url_codec() {
        assert_eq!(encode("maki catta"), "maki+catta");
        assert_eq!(decode("https%3A%2F%2Fexample.org%2Fa%20b"), "https://example.org/a b");
        assert_eq!(decode("50%"), "50%");
    }

    #[test]
    fn ddg_parsing() {
        let html = r#"<div><a rel="nofollow" class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fkde.org%2F&amp;rut=x">KDE <b>Plasma</b></a>
        <a class="result__snippet" href="x">Le bureau <b>libre</b></a></div>"#;
        let r = parse_ddg(html, 5);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].url, "https://kde.org/");
        assert_eq!(r[0].titre, "KDE Plasma");
        assert_eq!(r[0].extrait, "Le bureau libre");
    }

    #[test]
    fn feeds() {
        let rss = r#"<rss><channel><title>Arch</title><item><title>Intervention</title><link>https://a/1</link><pubDate>Sun</pubDate></item></channel></rss>"#;
        let n = parse_feed(rss, 5).unwrap();
        assert_eq!((n[0].titre.as_str(), n[0].lien.as_str(), n[0].source.as_str()), ("Intervention", "https://a/1", "Arch"));
        let atom = r#"<feed xmlns="http://www.w3.org/2005/Atom"><title>KDE</title><entry><title>Plasma 6.9</title><link href="https://k/2"/><updated>2026</updated></entry></feed>"#;
        let n = parse_feed(atom, 5).unwrap();
        assert_eq!(n[0].lien, "https://k/2");
    }

    #[test]
    fn strips() {
        assert_eq!(strip_tags(&remove_blocks("<p>Salut <script>x()</script>toi</p>", "script")), "Salut toi");
    }
}
