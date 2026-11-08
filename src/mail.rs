//! Mails par IMAP/SMTP via `curl` (aucune bibliothèque lourde).
//!
//! Les mots de passe ne sont jamais dans la configuration : ils sont rangés dans le
//! trousseau du bureau (Secret Service, KWallet ou GNOME) par `lkatta compte-mail`, où
//! l'utilisateur les tape lui-même. La lecture ne marque pas les messages comme lus.

use std::io::Write;
use std::process::{Command, Stdio};

use base64::Engine;
use serde::{Deserialize, Serialize};

use crate::paths;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Compte {
    pub nom: String,
    pub adresse: String,
    /// Identifiant de connexion (souvent l'adresse elle-même).
    pub identifiant: String,
    /// ex. `imaps://imap.gmail.com`
    pub imap: String,
    /// ex. `smtps://smtp.gmail.com:465`
    pub smtp: String,
}

impl Default for Compte {
    fn default() -> Self {
        Self { nom: String::new(), adresse: String::new(), identifiant: String::new(), imap: String::new(), smtp: String::new() }
    }
}

impl Compte {
    /// Réglages connus selon le domaine de l'adresse.
    pub fn guess(adresse: &str) -> Compte {
        let dom = adresse.rsplit('@').next().unwrap_or("").to_lowercase();
        let (imap, smtp, nom) = match dom.as_str() {
            "gmail.com" | "googlemail.com" => ("imaps://imap.gmail.com", "smtps://smtp.gmail.com:465", "Gmail"),
            "outlook.com" | "hotmail.com" | "hotmail.fr" | "live.com" | "live.fr" | "msn.com" => {
                ("imaps://outlook.office365.com", "smtp://smtp-mail.outlook.com:587", "Outlook")
            }
            "yahoo.com" | "yahoo.fr" => ("imaps://imap.mail.yahoo.com", "smtps://smtp.mail.yahoo.com:465", "Yahoo"),
            "orange.fr" | "wanadoo.fr" => ("imaps://imap.orange.fr", "smtps://smtp.orange.fr:465", "Orange"),
            "free.fr" => ("imaps://imap.free.fr", "smtps://smtp.free.fr:465", "Free"),
            "laposte.net" => ("imaps://imap.laposte.net", "smtps://smtp.laposte.net:465", "La Poste"),
            _ => ("", "", ""),
        };
        let (imap, smtp) = if imap.is_empty() { (format!("imaps://imap.{dom}"), format!("smtps://smtp.{dom}:465")) } else { (imap.into(), smtp.into()) };
        Compte {
            nom: if nom.is_empty() { dom.clone() } else { nom.into() },
            adresse: adresse.into(),
            identifiant: adresse.into(),
            imap,
            smtp,
        }
    }

    fn login(&self) -> &str {
        if self.identifiant.is_empty() { &self.adresse } else { &self.identifiant }
    }
}

fn secret(c: &Compte) -> Result<String, String> {
    let o = Command::new("secret-tool")
        .args(["lookup", "service", "lkatta", "compte", &c.adresse])
        .output()
        .map_err(|_| "secret-tool absent (paquet libsecret)".to_string())?;
    let p = String::from_utf8_lossy(&o.stdout).trim_end_matches('\n').to_string();
    if p.is_empty() {
        return Err(format!("pas de mot de passe pour {} : lancer « lkatta compte-mail {} »", c.adresse, c.adresse));
    }
    Ok(p)
}

/// Enregistre le mot de passe dans le trousseau (saisi par l'utilisateur dans le terminal).
pub fn store_secret(adresse: &str) -> Result<(), String> {
    let st = Command::new("secret-tool")
        .args(["store", &format!("--label=L Katta : {adresse}"), "service", "lkatta", "compte", adresse])
        .status()
        .map_err(|_| "secret-tool absent (paquet libsecret)".to_string())?;
    if st.success() { Ok(()) } else { Err("mot de passe non enregistré".into()) }
}

fn host_of(url: &str) -> String {
    url.split("://").nth(1).unwrap_or(url).split(['/', ':']).next().unwrap_or("").to_string()
}

/// Lance curl avec les identifiants dans un fichier netrc temporaire (jamais en argument).
fn curl(c: &Compte, url: &str, args: &[&str], stdin: Option<&[u8]>) -> Result<String, String> {
    let pass = secret(c)?;
    let dir = paths::runtime_dir();
    let _ = std::fs::create_dir_all(&dir);
    let netrc = dir.join(format!("netrc-{}", std::process::id()));
    let esc = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
    paths::write_private(&netrc, format!("machine {} login \"{}\" password \"{}\"\n", host_of(url), esc(c.login()), esc(&pass)).as_bytes())
        .map_err(|e| e.to_string())?;
    let mut cmd = Command::new("curl");
    cmd.args(["-sS", "--max-time", "40", "--netrc-file"]).arg(&netrc).args(["--url", url]).args(args);
    if url.starts_with("smtp://") || url.starts_with("imap://") {
        cmd.arg("--ssl-reqd");
    }
    cmd.stdin(if stdin.is_some() { Stdio::piped() } else { Stdio::null() }).stdout(Stdio::piped()).stderr(Stdio::piped());
    let child = cmd.spawn();
    let out = child.and_then(|mut ch| {
        if let (Some(data), Some(mut si)) = (stdin, ch.stdin.take()) {
            si.write_all(data)?;
        }
        ch.wait_with_output()
    });
    let _ = std::fs::remove_file(&netrc);
    let out = out.map_err(|e| e.to_string())?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        let hint = if err.contains("Login denied") || err.contains("(67)") {
            " — identifiants refusés (Gmail : il faut un « mot de passe d'application »)"
        } else {
            ""
        };
        return Err(format!("{err}{hint}"));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

// --- décodage MIME minimal --------------------------------------------------------

fn latin1(b: &[u8]) -> String {
    b.iter().map(|&c| c as char).collect()
}

fn to_text(bytes: &[u8], charset: &str) -> String {
    let cs = charset.to_lowercase();
    if cs.contains("8859") || cs.contains("1252") || cs.contains("latin") {
        latin1(bytes)
    } else {
        String::from_utf8_lossy(bytes).into_owned()
    }
}

fn qp_decode(s: &str, header: bool) -> Vec<u8> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'=' if i + 2 < b.len() + 1 && b.get(i + 1) == Some(&b'\r') && b.get(i + 2) == Some(&b'\n') => i += 3,
            b'=' if b.get(i + 1) == Some(&b'\n') => i += 2,
            b'=' if i + 2 < b.len() => match u8::from_str_radix(std::str::from_utf8(&b[i + 1..i + 3]).unwrap_or("zz"), 16) {
                Ok(v) => {
                    out.push(v);
                    i += 3;
                }
                Err(_) => {
                    out.push(b'=');
                    i += 1;
                }
            },
            b'_' if header => {
                out.push(b' ');
                i += 1;
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    out
}

/// Décode les en-têtes encodés (RFC 2047 : `=?UTF-8?B?…?=`, `=?ISO-8859-1?Q?…?=`).
pub fn decode_header(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    let mut last_was_word = false;
    while let Some(start) = rest.find("=?") {
        let before = &rest[..start];
        if !(last_was_word && before.trim().is_empty()) {
            out.push_str(before);
        }
        let after = &rest[start + 2..];
        let parts: Vec<&str> = after.splitn(3, '?').collect();
        if parts.len() < 3 {
            out.push_str(&rest[start..]);
            return out;
        }
        let (cs, enc, tail) = (parts[0], parts[1], parts[2]);
        let Some(end) = tail.find("?=") else {
            out.push_str(&rest[start..]);
            return out;
        };
        let data = &tail[..end];
        let bytes = if enc.eq_ignore_ascii_case("B") {
            base64::engine::general_purpose::STANDARD.decode(data.trim()).unwrap_or_default()
        } else {
            qp_decode(data, true)
        };
        out.push_str(&to_text(&bytes, cs));
        rest = &tail[end + 2..];
        last_was_word = true;
    }
    out.push_str(rest);
    out
}

fn header_value(headers: &str, name: &str) -> String {
    let mut val = String::new();
    let mut on = false;
    for l in headers.lines() {
        if l.starts_with(' ') || l.starts_with('\t') {
            if on {
                val.push(' ');
                val.push_str(l.trim());
            }
            continue;
        }
        on = false;
        if let Some((k, v)) = l.split_once(':') {
            if k.trim().eq_ignore_ascii_case(name) && val.is_empty() {
                val = v.trim().to_string();
                on = true;
            }
        }
    }
    val
}

fn param(v: &str, p: &str) -> String {
    v.split(';')
        .filter_map(|x| x.trim().split_once('='))
        .find(|(k, _)| k.trim().eq_ignore_ascii_case(p))
        .map(|(_, v)| v.trim().trim_matches('"').to_string())
        .unwrap_or_default()
}

/// Texte lisible d'un message brut (préfère text/plain ; sinon HTML nettoyé).
pub fn body_text(raw: &str) -> String {
    let raw = raw.replace("\r\n", "\n");
    let (headers, body) = raw.split_once("\n\n").unwrap_or((&raw, ""));
    let ctype = header_value(headers, "Content-Type");
    let ctype_l = ctype.to_lowercase();
    if ctype_l.starts_with("multipart/") {
        let boundary = param(&ctype, "boundary");
        if boundary.is_empty() {
            return body.to_string();
        }
        let sep = format!("--{boundary}");
        let parts: Vec<&str> = body.split(sep.as_str()).skip(1).filter(|p| !p.starts_with("--")).collect();
        let pick = |want: &str| parts.iter().find(|p| header_value(p.trim_start_matches('\n'), "Content-Type").to_lowercase().starts_with(want));
        for want in ["text/plain", "multipart/", "text/html"] {
            if let Some(p) = pick(want) {
                return body_text(p.trim_start_matches('\n'));
            }
        }
        return parts.first().map(|p| body_text(p.trim_start_matches('\n'))).unwrap_or_default();
    }
    let cte = header_value(headers, "Content-Transfer-Encoding").to_lowercase();
    let bytes = match cte.as_str() {
        "base64" => base64::engine::general_purpose::STANDARD.decode(body.split_whitespace().collect::<String>()).unwrap_or_default(),
        "quoted-printable" => qp_decode(body, false),
        _ => body.as_bytes().to_vec(),
    };
    let text = to_text(&bytes, &param(&ctype, "charset"));
    if ctype_l.starts_with("text/html") { crate::web::strip_tags(&text) } else { text.trim().to_string() }
}

// --- IMAP ----------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct Entete {
    pub uid: u64,
    pub de: String,
    pub sujet: String,
    pub date: String,
    pub lu: bool,
}

/// Analyse une réponse `UID FETCH … (FLAGS BODY.PEEK[HEADER.FIELDS (FROM SUBJECT DATE)])`.
pub fn parse_fetch(resp: &str) -> Vec<Entete> {
    let mut out = vec![];
    let mut cur: Option<(u64, bool, String)> = None;
    let flush = |cur: &mut Option<(u64, bool, String)>, out: &mut Vec<Entete>| {
        if let Some((uid, lu, h)) = cur.take() {
            out.push(Entete {
                uid,
                de: decode_header(&header_value(&h, "From")),
                sujet: decode_header(&header_value(&h, "Subject")),
                date: header_value(&h, "Date"),
                lu,
            });
        }
    };
    for line in resp.replace("\r\n", "\n").lines() {
        if line.starts_with("* ") && line.contains("FETCH") {
            flush(&mut cur, &mut out);
            let uid = line.split("UID ").nth(1).and_then(|r| r.split(|c: char| !c.is_ascii_digit()).next()).and_then(|n| n.parse().ok()).unwrap_or(0);
            cur = Some((uid, line.contains("\\Seen"), String::new()));
        } else if let Some((_, _, h)) = &mut cur {
            if line != ")" {
                h.push_str(line);
                h.push('\n');
            }
        }
    }
    flush(&mut cur, &mut out);
    out.sort_by(|a, b| b.uid.cmp(&a.uid));
    out
}

fn mailbox(c: &Compte) -> String {
    format!("{}/INBOX", c.imap.trim_end_matches('/'))
}

/// Derniers mails (non lus seulement si `non_lus`).
pub fn list(c: &Compte, non_lus: bool, n: usize) -> Result<Vec<Entete>, String> {
    let search = curl(c, &mailbox(c), &["-X", if non_lus { "UID SEARCH UNSEEN" } else { "UID SEARCH ALL" }], None)?;
    let mut uids: Vec<u64> = search.split_whitespace().filter_map(|w| w.parse().ok()).collect();
    uids.sort_unstable();
    let last: Vec<String> = uids.iter().rev().take(n.clamp(1, 30)).map(|u| u.to_string()).collect();
    if last.is_empty() {
        return Ok(vec![]);
    }
    let cmd = format!("UID FETCH {} (FLAGS BODY.PEEK[HEADER.FIELDS (FROM SUBJECT DATE)])", last.join(","));
    Ok(parse_fetch(&curl(c, &mailbox(c), &["-X", &cmd], None)?))
}

/// Texte d'un mail (sans le marquer comme lu).
pub fn read(c: &Compte, uid: u64) -> Result<String, String> {
    let resp = curl(c, &mailbox(c), &["-X", &format!("UID FETCH {uid} (BODY.PEEK[])")], None)?;
    let start = resp.find('\n').map(|i| i + 1).unwrap_or(0);
    let raw = resp[start..].trim_end().trim_end_matches(')').to_string();
    let head = raw.replace("\r\n", "\n");
    let headers = head.split("\n\n").next().unwrap_or("");
    let body: String = body_text(&raw).chars().take(6000).collect();
    Ok(format!(
        "De : {}\nÀ : {}\nDate : {}\nObjet : {}\n\n{}",
        decode_header(&header_value(headers, "From")),
        decode_header(&header_value(headers, "To")),
        header_value(headers, "Date"),
        decode_header(&header_value(headers, "Subject")),
        body
    ))
}

// --- SMTP ----------------------------------------------------------------------------

fn encode_header(s: &str) -> String {
    if s.is_ascii() { s.to_string() } else { format!("=?UTF-8?B?{}?=", base64::engine::general_purpose::STANDARD.encode(s)) }
}

pub fn compose(c: &Compte, a: &[String], sujet: &str, texte: &str) -> String {
    let date = chrono::Local::now().to_rfc2822();
    let id = format!("<{}.{}@lkatta>", chrono::Local::now().timestamp_millis(), std::process::id());
    format!(
        "From: {}\r\nTo: {}\r\nSubject: {}\r\nDate: {date}\r\nMessage-ID: {id}\r\nMIME-Version: 1.0\r\n\
         Content-Type: text/plain; charset=UTF-8\r\nContent-Transfer-Encoding: 8bit\r\n\r\n{}\r\n",
        c.adresse,
        a.join(", "),
        encode_header(sujet),
        texte.replace("\r\n", "\n").replace('\n', "\r\n")
    )
}

pub fn send(c: &Compte, a: &[String], sujet: &str, texte: &str) -> Result<String, String> {
    if a.is_empty() || a.iter().any(|x| !x.contains('@') || x.contains(['\r', '\n', ' '])) {
        return Err("destinataire invalide".into());
    }
    let msg = compose(c, a, sujet, texte);
    let mut args: Vec<String> = vec!["--mail-from".into(), c.adresse.clone()];
    for r in a {
        args.push("--mail-rcpt".into());
        args.push(r.clone());
    }
    args.push("--upload-file".into());
    args.push("-".into());
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    curl(c, &c.smtp, &refs, Some(msg.as_bytes()))?;
    Ok(format!("Mail envoyé à {} : « {sujet} »", a.join(", ")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guesses_servers() {
        let g = Compte::guess("raantss@gmail.com");
        assert_eq!((g.imap.as_str(), g.smtp.as_str(), g.nom.as_str()), ("imaps://imap.gmail.com", "smtps://smtp.gmail.com:465", "Gmail"));
        assert_eq!(Compte::guess("a@univ-antananarivo.mg").imap, "imaps://imap.univ-antananarivo.mg");
        assert_eq!(host_of("smtps://smtp.gmail.com:465"), "smtp.gmail.com");
    }

    #[test]
    fn decodes_headers_and_bodies() {
        assert_eq!(decode_header("=?UTF-8?B?UsOpdW5pb24gZGVtYWlu?="), "Réunion demain");
        assert_eq!(decode_header("=?ISO-8859-1?Q?Caf=E9_ce_soir?="), "Café ce soir");
        assert_eq!(decode_header("Re: =?UTF-8?Q?=C3=A9t=C3=A9?= ok"), "Re: été ok");
        let raw = "Content-Type: multipart/alternative; boundary=\"XX\"\n\n--XX\nContent-Type: text/plain; charset=utf-8\nContent-Transfer-Encoding: quoted-printable\n\nBonjour =C3=A0 tous=\n !\n--XX\nContent-Type: text/html\n\n<p>Bonjour</p>\n--XX--\n";
        assert_eq!(body_text(raw), "Bonjour à tous !");
    }

    #[test]
    fn parses_fetch_response() {
        let resp = "* 3 FETCH (UID 812 FLAGS () BODY[HEADER.FIELDS (FROM SUBJECT DATE)] {80}\r\nFrom: Hery <hery@ex.mg>\r\nSubject: =?UTF-8?B?UsOpdW5pb24=?=\r\nDate: Mon, 9 Nov 2026 08:00:00 +0300\r\n\r\n)\r\n* 4 FETCH (UID 815 FLAGS (\\Seen) BODY[HEADER.FIELDS (FROM SUBJECT DATE)] {40}\r\nFrom: a@b.c\r\nSubject: Salut\r\n\r\n)\r\n";
        let e = parse_fetch(resp);
        assert_eq!(e.len(), 2);
        assert_eq!((e[0].uid, e[0].lu, e[0].sujet.as_str()), (815, true, "Salut"));
        assert_eq!((e[1].uid, e[1].lu, e[1].sujet.as_str(), e[1].de.as_str()), (812, false, "Réunion", "Hery <hery@ex.mg>"));
    }

    #[test]
    fn composes_utf8() {
        let c = Compte::guess("moi@gmail.com");
        let m = compose(&c, &["hery@ex.mg".into()], "Réunion", "Bonjour,\nà demain.");
        assert!(m.contains("Subject: =?UTF-8?B?"));
        assert!(m.contains("To: hery@ex.mg\r\n"));
        assert!(m.ends_with("Bonjour,\r\nà demain.\r\n"));
    }
}
