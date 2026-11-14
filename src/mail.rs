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
        .map_err(|_| t!("secret-tool absent (paquet libsecret)").to_string())?;
    let p = String::from_utf8_lossy(&o.stdout).trim_end_matches('\n').to_string();
    if p.is_empty() {
        return Err(t!("pas de mot de passe pour {} : lancer « lkatta compte-mail {} »", c.adresse, c.adresse));
    }
    Ok(p)
}

/// Enregistre un mot de passe tapé dans le panneau de réglages (envoyé au trousseau par
/// l'entrée standard, jamais écrit ailleurs).
pub fn set_password(adresse: &str, pass: &str) -> Result<(), String> {
    crate::cles::store(&format!("L Katta : {adresse}"), &["service", "lkatta", "compte", adresse], pass)
}

pub fn remove_password(adresse: &str) {
    crate::cles::clear(&["service", "lkatta", "compte", adresse]);
}

/// Enregistre le mot de passe dans le trousseau (saisi par l'utilisateur dans le terminal).
pub fn store_secret(adresse: &str) -> Result<(), String> {
    let st = Command::new("secret-tool")
        .args(["store", &format!("--label=L Katta : {adresse}"), "service", "lkatta", "compte", adresse])
        .status()
        .map_err(|_| t!("secret-tool absent (paquet libsecret)").to_string())?;
    if st.success() { Ok(()) } else { Err(t!("mot de passe non enregistré").into()) }
}

fn host_of(url: &str) -> String {
    url.split("://").nth(1).unwrap_or(url).split(['/', ':']).next().unwrap_or("").to_string()
}

/// Lance curl avec les identifiants dans un fichier netrc temporaire (jamais en argument).
fn curl(c: &Compte, url: &str, args: &[&str], stdin: Option<&[u8]>) -> Result<String, String> {
    let pass = secret(c)?;
    let dir = paths::runtime_dir();
    let _ = std::fs::create_dir_all(&dir);
    // Un fichier par appel (deux lectures en parallèle ne se marchent pas dessus), retiré
    // quoi qu'il arrive, même si le fil panique.
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let netrc = dir.join(format!("netrc-{}-{}", std::process::id(), N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
    struct Retire(std::path::PathBuf);
    impl Drop for Retire {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let _garde = Retire(netrc.clone());
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
    let out = out.map_err(|e| e.to_string())?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        let hint = if err.contains("Login denied") || err.contains("(67)") {
            t!(" — identifiants refusés (Gmail : il faut un « mot de passe d'application »)")
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
    corps_texte(raw, 0)
}

/// (Profondeur bornée : un mail aux parties emboîtées sans fin ne doit ni figer ni faire
/// tomber le maki.)
fn corps_texte(raw: &str, profondeur: usize) -> String {
    if profondeur > 8 {
        return String::new();
    }
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
                return corps_texte(p.trim_start_matches('\n'), profondeur + 1);
            }
        }
        return parts.first().map(|p| corps_texte(p.trim_start_matches('\n'), profondeur + 1)).unwrap_or_default();
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

/// Message brut (sans le marquer comme lu).
fn brut(c: &Compte, uid: u64) -> Result<String, String> {
    let resp = curl(c, &mailbox(c), &["-X", &format!("UID FETCH {uid} (BODY.PEEK[])")], None)?;
    let start = resp.find('\n').map(|i| i + 1).unwrap_or(0);
    let raw = resp[start..].trim_end().trim_end_matches(')').to_string();
    if raw.trim().is_empty() {
        return Err(t!("pas de mail n°{}", uid));
    }
    Ok(raw)
}

fn entetes(raw: &str) -> String {
    let head = raw.replace("\r\n", "\n");
    head.split("\n\n").next().unwrap_or("").to_string()
}

/// Texte d'un mail (sans le marquer comme lu), avec la liste de ses pièces jointes.
pub fn read(c: &Compte, uid: u64) -> Result<String, String> {
    let raw = brut(c, uid)?;
    let headers = entetes(&raw);
    let body: String = body_text(&raw).chars().take(6000).collect();
    let pj = pieces_jointes(&raw);
    let pj = if pj.is_empty() {
        String::new()
    } else {
        format!("\n\nPièces jointes : {}", pj.iter().map(|p| format!("{} ({})", p.nom, taille_lisible(p.donnees.len()))).collect::<Vec<_>>().join(", "))
    };
    Ok(format!(
        "De : {}\nÀ : {}\nDate : {}\nObjet : {}\n\n{}{pj}",
        decode_header(&header_value(&headers, "From")),
        decode_header(&header_value(&headers, "To")),
        header_value(&headers, "Date"),
        decode_header(&header_value(&headers, "Subject")),
        body
    ))
}

fn taille_lisible(n: usize) -> String {
    if n >= 1024 * 1024 { format!("{:.1} Mo", n as f64 / 1048576.0) } else { format!("{} Ko", n.div_ceil(1024)) }
}

// --- pièces jointes ----------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct PieceJointe {
    pub nom: String,
    pub donnees: Vec<u8>,
}

/// Feuilles d'un message MIME : (en-têtes, corps encodé).
fn feuilles(raw: &str, out: &mut Vec<(String, String)>, profondeur: usize) {
    let raw = raw.replace("\r\n", "\n");
    let (headers, body) = raw.split_once("\n\n").unwrap_or((&raw, ""));
    let ctype = header_value(headers, "Content-Type");
    if ctype.to_lowercase().starts_with("multipart/") && profondeur < 8 {
        let boundary = param(&ctype, "boundary");
        if !boundary.is_empty() {
            let sep = format!("--{boundary}");
            for p in body.split(sep.as_str()).skip(1).filter(|p| !p.starts_with("--")) {
                feuilles(p.trim_start_matches('\n'), out, profondeur + 1);
            }
            return;
        }
    }
    out.push((headers.to_string(), body.to_string()));
}

/// `filename*=UTF-8''r%C3%A9sum%C3%A9.pdf` (RFC 2231) ou `filename="=?UTF-8?…?="` (RFC 2047).
fn nom_fichier(headers: &str) -> Option<String> {
    let disp = header_value(headers, "Content-Disposition");
    let ctype = header_value(headers, "Content-Type");
    for (v, k) in [(&disp, "filename"), (&ctype, "name")] {
        let etoile = param(v, &format!("{k}*"));
        if !etoile.is_empty() {
            let enc = etoile.splitn(3, '\'').nth(2).unwrap_or(&etoile);
            let b = qp_like_pct(enc);
            return Some(String::from_utf8_lossy(&b).into_owned());
        }
        let n = param(v, k);
        if !n.is_empty() {
            return Some(decode_header(&n));
        }
    }
    disp.to_lowercase().starts_with("attachment").then(|| "piece-jointe".to_string())
}

fn qp_like_pct(s: &str) -> Vec<u8> {
    let b = s.as_bytes();
    let mut out = vec![];
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() + 1 {
            if let Ok(v) = u8::from_str_radix(std::str::from_utf8(&b[i + 1..(i + 3).min(b.len())]).unwrap_or("zz"), 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    out
}

/// Nom sans chemin ni caractère dangereux (un expéditeur ne choisit pas où l'on écrit).
fn nom_sur(n: &str) -> String {
    let n: String = n.rsplit(['/', '\\']).next().unwrap_or(n).chars().filter(|c| !c.is_control()).collect();
    let n = n.trim().trim_start_matches('.').to_string();
    if n.is_empty() { "piece-jointe".into() } else { n.chars().take(150).collect() }
}

pub fn pieces_jointes(raw: &str) -> Vec<PieceJointe> {
    let mut f = vec![];
    feuilles(raw, &mut f, 0);
    f.into_iter()
        .filter_map(|(h, corps)| {
            let nom = nom_fichier(&h)?;
            let cte = header_value(&h, "Content-Transfer-Encoding").to_lowercase();
            let donnees = match cte.as_str() {
                "base64" => base64::engine::general_purpose::STANDARD.decode(corps.split_whitespace().collect::<String>()).ok()?,
                "quoted-printable" => qp_decode(&corps, false),
                _ => corps.into_bytes(),
            };
            Some(PieceJointe { nom: nom_sur(&nom), donnees })
        })
        .collect()
}

/// Dossier de téléchargement de l'utilisateur (nom localisé selon la langue du système).
pub fn dossier_telechargements() -> std::path::PathBuf {
    let xdg = Command::new("xdg-user-dir").arg("DOWNLOAD").output().ok().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).filter(|s| !s.is_empty());
    let home = paths::home_dir();
    match xdg.map(std::path::PathBuf::from) {
        Some(p) if p != home => p,
        _ => home.join("Downloads"),
    }
}

/// Chemin libre : `rapport.pdf`, sinon `rapport (2).pdf`…
fn chemin_libre(dossier: &std::path::Path, nom: &str) -> std::path::PathBuf {
    let p = dossier.join(nom);
    if !p.exists() {
        return p;
    }
    let (base, ext) = match nom.rsplit_once('.') {
        Some((b, e)) if !b.is_empty() => (b.to_string(), format!(".{e}")),
        _ => (nom.to_string(), String::new()),
    };
    (2..1000).map(|i| dossier.join(format!("{base} ({i}){ext}"))).find(|p| !p.exists()).unwrap_or(p)
}

/// Noms des pièces jointes d'un mail (pour demander l'accord avant d'enregistrer).
pub fn noms_pieces(c: &Compte, uid: u64) -> Result<Vec<String>, String> {
    Ok(pieces_jointes(&brut(c, uid)?).into_iter().map(|p| p.nom).collect())
}

/// Enregistre les pièces jointes d'un mail (toutes, ou celles dont le nom contient `filtre`).
pub fn enregistrer_pieces(c: &Compte, uid: u64, filtre: &str) -> Result<Vec<std::path::PathBuf>, String> {
    let f = filtre.trim().to_lowercase();
    let pj: Vec<PieceJointe> = pieces_jointes(&brut(c, uid)?).into_iter().filter(|p| f.is_empty() || p.nom.to_lowercase().contains(&f)).collect();
    if pj.is_empty() {
        return Err(t!("aucune pièce jointe correspondante dans le mail n°{}", uid));
    }
    let d = dossier_telechargements();
    std::fs::create_dir_all(&d).map_err(|e| e.to_string())?;
    let mut faits = vec![];
    for p in pj {
        let dest = chemin_libre(&d, &p.nom);
        std::fs::write(&dest, &p.donnees).map_err(|e| format!("{} : {e}", dest.display()))?;
        faits.push(dest);
    }
    Ok(faits)
}

// --- réponses et brouillons --------------------------------------------------------------

/// Un mail à envoyer ou à ranger dans les brouillons.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Brouillon {
    pub a: Vec<String>,
    pub cc: Vec<String>,
    pub sujet: String,
    pub texte: String,
    pub pieces: Vec<std::path::PathBuf>,
    /// (In-Reply-To, References) quand c'est une réponse.
    pub en_reponse: Option<(String, String)>,
}

/// Adresses d'un en-tête « Nom <a@b>, c@d ».
pub fn adresses(v: &str) -> Vec<String> {
    let mut out = vec![];
    let mut dans_guillemets = false;
    let mut cur = String::new();
    for ch in v.chars() {
        match ch {
            '"' => dans_guillemets = !dans_guillemets,
            ',' if !dans_guillemets => out.push(std::mem::take(&mut cur)),
            c => cur.push(c),
        }
    }
    out.push(cur);
    out.into_iter()
        .filter_map(|x| {
            let x = x.trim();
            let a = match (x.rfind('<'), x.rfind('>')) {
                (Some(i), Some(j)) if j > i => &x[i + 1..j],
                _ => x,
            };
            let a = a.trim();
            (a.contains('@') && !a.contains(char::is_whitespace)).then(|| a.to_string())
        })
        .collect()
}

/// Prépare la réponse à un mail : destinataires, « Re: », fil de discussion, citation.
pub fn preparer_reponse(c: &Compte, uid: u64, texte: &str, tous: bool) -> Result<Brouillon, String> {
    Ok(reponse_depuis(&brut(c, uid)?, &c.adresse, texte, tous))
}

pub fn reponse_depuis(raw: &str, moi: &str, texte: &str, tous: bool) -> Brouillon {
    let h = entetes(raw);
    let rt = header_value(&h, "Reply-To");
    let de = header_value(&h, "From");
    let a = adresses(if rt.is_empty() { &de } else { &rt });
    let moi_l = moi.to_lowercase();
    let mut cc = vec![];
    if tous {
        for x in adresses(&header_value(&h, "To")).into_iter().chain(adresses(&header_value(&h, "Cc"))) {
            let xl = x.to_lowercase();
            if xl != moi_l && !a.iter().any(|y| y.to_lowercase() == xl) && !cc.iter().any(|y: &String| y.to_lowercase() == xl) {
                cc.push(x);
            }
        }
    }
    let sujet0 = decode_header(&header_value(&h, "Subject"));
    let low = sujet0.to_lowercase();
    let sujet = if low.starts_with("re:") || low.starts_with("re :") { sujet0.clone() } else { format!("Re: {sujet0}") };
    let id = header_value(&h, "Message-ID");
    let refs = header_value(&h, "References");
    let refs = [refs, id.clone()].into_iter().filter(|x| !x.is_empty()).collect::<Vec<_>>().join(" ");
    let cite: String = body_text(raw).lines().take(60).map(|l| format!("> {l}\n")).collect();
    let quand = header_value(&h, "Date");
    let qui = decode_header(&de);
    let texte = format!("{}\n\n{}\n{cite}", texte.trim_end(), t!("Le {}, {} a écrit :", quand, qui));
    Brouillon { a, cc, sujet, texte, pieces: vec![], en_reponse: (!id.is_empty()).then_some((id, refs)) }
}

fn type_mime(p: &std::path::Path) -> &'static str {
    match p.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase().as_str() {
        "pdf" => "application/pdf",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "txt" | "md" => "text/plain",
        "csv" => "text/csv",
        "html" | "htm" => "text/html",
        "zip" => "application/zip",
        "ics" => "text/calendar",
        "odt" => "application/vnd.oasis.opendocument.text",
        "ods" => "application/vnd.oasis.opendocument.spreadsheet",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "mp3" => "audio/mpeg",
        "mp4" => "video/mp4",
        _ => "application/octet-stream",
    }
}

/// Taille maximale des pièces jointes (la plupart des serveurs refusent au-delà de 25 Mo).
pub const MAX_PIECES: u64 = 20 * 1024 * 1024;

/// Message complet (texte seul, ou multipart/mixed avec les pièces jointes).
pub fn composer(c: &Compte, b: &Brouillon) -> Result<String, String> {
    valider(&b.a)?;
    valider(&b.cc)?;
    let date = chrono::Local::now().to_rfc2822();
    let id = format!("<{}.{}@lkatta>", chrono::Local::now().timestamp_millis(), std::process::id());
    let mut h = format!("From: {}\r\nTo: {}\r\n", c.adresse, b.a.join(", "));
    if !b.cc.is_empty() {
        h.push_str(&format!("Cc: {}\r\n", b.cc.join(", ")));
    }
    h.push_str(&format!("Subject: {}\r\nDate: {date}\r\nMessage-ID: {id}\r\n", encode_header(&b.sujet)));
    if let Some((irt, refs)) = &b.en_reponse {
        let (irt, refs) = (ids_surs(irt), ids_surs(refs));
        if !irt.is_empty() {
            h.push_str(&format!("In-Reply-To: {irt}\r\nReferences: {refs}\r\n"));
        }
    }
    h.push_str("MIME-Version: 1.0\r\n");
    let texte = b.texte.replace("\r\n", "\n").replace('\n', "\r\n");
    if b.pieces.is_empty() {
        return Ok(format!("{h}Content-Type: text/plain; charset=UTF-8\r\nContent-Transfer-Encoding: 8bit\r\n\r\n{texte}\r\n"));
    }
    let total: u64 = b.pieces.iter().filter_map(|p| std::fs::metadata(p).ok()).map(|m| m.len()).sum();
    if total > MAX_PIECES {
        return Err(t!("pièces jointes trop lourdes ({} Mo, 20 Mo au plus)", total / 1048576));
    }
    let frontiere = format!("=_lkatta_{}", chrono::Local::now().timestamp_nanos_opt().unwrap_or(0));
    let mut m = format!("{h}Content-Type: multipart/mixed; boundary=\"{frontiere}\"\r\n\r\n");
    m.push_str(&format!("--{frontiere}\r\nContent-Type: text/plain; charset=UTF-8\r\nContent-Transfer-Encoding: 8bit\r\n\r\n{texte}\r\n"));
    for p in &b.pieces {
        let donnees = std::fs::read(p).map_err(|e| format!("{} : {e}", p.display()))?;
        let nom = nom_sur(&p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default());
        let pct: String = nom.bytes().map(|o| if o.is_ascii_alphanumeric() || b".-_".contains(&o) { (o as char).to_string() } else { format!("%{o:02X}") }).collect();
        let b64 = base64::engine::general_purpose::STANDARD.encode(&donnees);
        let lignes: Vec<&str> = b64.as_bytes().chunks(76).map(|ch| std::str::from_utf8(ch).unwrap_or("")).collect();
        m.push_str(&format!(
            "--{frontiere}\r\nContent-Type: {}; name=\"{}\"\r\nContent-Disposition: attachment; filename*=UTF-8''{pct}\r\nContent-Transfer-Encoding: base64\r\n\r\n{}\r\n",
            type_mime(p),
            encode_header(&nom),
            lignes.join("\r\n")
        ));
    }
    m.push_str(&format!("--{frontiere}--\r\n"));
    Ok(m)
}

fn valider(adrs: &[String]) -> Result<(), String> {
    if adrs.iter().any(|x| !x.contains('@') || x.len() > 254 || x.chars().any(|c| c.is_control() || c.is_whitespace() || "<>,;:\"()".contains(c))) {
        return Err(t!("destinataire invalide").into());
    }
    Ok(())
}

/// Envoie un brouillon (après l'accord de l'utilisateur, demandé par l'appelant).
pub fn envoyer(c: &Compte, b: &Brouillon) -> Result<String, String> {
    if b.a.is_empty() {
        return Err(t!("destinataire invalide").into());
    }
    valider(&b.a)?;
    valider(&b.cc)?;
    let msg = composer(c, b)?;
    let mut args: Vec<String> = vec!["--mail-from".into(), c.adresse.clone()];
    for r in b.a.iter().chain(&b.cc) {
        args.push("--mail-rcpt".into());
        args.push(r.clone());
    }
    args.push("--upload-file".into());
    args.push("-".into());
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    curl(c, &c.smtp, &refs, Some(msg.as_bytes()))?;
    Ok(t!("Mail envoyé à {} : « {} »", b.a.iter().chain(&b.cc).cloned().collect::<Vec<_>>().join(", "), b.sujet))
}

/// Dossier des brouillons du serveur (attribut \Drafts, sinon un nom courant).
pub fn dossier_brouillons_depuis(liste: &str) -> Option<String> {
    let nom = |l: &str| -> Option<String> {
        let l = l.trim_end();
        if let Some(stripped) = l.strip_suffix('"') {
            stripped.rfind('"').map(|i| stripped[i + 1..].to_string())
        } else {
            l.rsplit(' ').next().map(str::to_string)
        }
    };
    let lignes: Vec<&str> = liste.lines().filter(|l| l.starts_with("* LIST")).collect();
    lignes.iter().find(|l| l.contains("\\Drafts")).and_then(|l| nom(l)).or_else(|| {
        lignes.iter().filter_map(|l| nom(l)).find(|n| {
            let b = n.rsplit(['/', '.']).next().unwrap_or(n).to_lowercase();
            matches!(b.as_str(), "drafts" | "brouillons" | "draft" | "entwürfe" | "borradores" | "bozze" | "concepten")
        })
    })
}

/// Range un brouillon dans le dossier des brouillons du compte (rien n'est envoyé).
pub fn ranger_brouillon(c: &Compte, b: &Brouillon) -> Result<String, String> {
    valider(&b.a)?;
    let liste = curl(c, &format!("{}/", c.imap.trim_end_matches('/')), &["-X", "LIST \"\" \"*\""], None)?;
    let dossier = dossier_brouillons_depuis(&liste).unwrap_or_else(|| "Drafts".into());
    let chemin: String = dossier
        .bytes()
        .map(|o| if o.is_ascii_alphanumeric() || b"-_.~/".contains(&o) { (o as char).to_string() } else { format!("%{o:02X}") })
        .collect();
    let msg = composer(c, b)?;
    curl(c, &format!("{}/{chemin}", c.imap.trim_end_matches('/')), &["--upload-file", "-"], Some(msg.as_bytes()))?;
    Ok(t!("Brouillon « {} » rangé dans {} ({}) : à relire et envoyer depuis ta messagerie.", b.sujet, dossier, c.adresse))
}

// --- SMTP ----------------------------------------------------------------------------

/// Valeur d'en-tête sûre : aucun retour à la ligne ni caractère de contrôle n'y survit (sinon
/// un objet « …\r\nBcc: x » ajouterait un destinataire caché), et tout ce qui n'est pas de
/// l'ASCII simple est encodé.
fn encode_header(s: &str) -> String {
    let s: String = s.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
    let s = s.trim();
    if s.is_ascii() { s.to_string() } else { format!("=?UTF-8?B?{}?=", base64::engine::general_purpose::STANDARD.encode(s)) }
}

/// Identifiants de message repris d'un mail reçu : seulement des `<…>` sans espace ni contrôle.
fn ids_surs(v: &str) -> String {
    v.split_whitespace().filter(|x| x.starts_with('<') && x.ends_with('>') && x.len() < 300 && !x.chars().any(|c| c.is_control())).collect::<Vec<_>>().join(" ")
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

    const MAIL: &str = "From: Hery <hery@ex.mg>\r\nTo: moi@gmail.com, Voahangy <voa@ex.mg>\r\nCc: \"Rakoto, Jean\" <jean@ex.mg>\r\nSubject: =?UTF-8?B?UsOpdW5pb24=?=\r\nDate: Mon, 9 Nov 2026 08:00:00 +0300\r\nMessage-ID: <abc@ex.mg>\r\nContent-Type: multipart/mixed; boundary=\"B\"\r\n\r\n--B\r\nContent-Type: text/plain; charset=utf-8\r\n\r\nOrdre du jour ci-joint.\r\n--B\r\nContent-Type: application/pdf; name=\"odj.pdf\"\r\nContent-Disposition: attachment; filename*=UTF-8''ordre%20du%20jour%C3%A9.pdf\r\nContent-Transfer-Encoding: base64\r\n\r\nJVBERi0xLjQK\r\n--B\r\nContent-Type: text/plain; name=\"../../.bashrc\"\r\nContent-Disposition: attachment\r\n\r\nrm -rf\r\n--B--\r\n";

    #[test]
    fn finds_attachments_safely() {
        let pj = pieces_jointes(MAIL);
        assert_eq!(pj.len(), 2);
        assert_eq!(pj[0].nom, "ordre du jouré.pdf");
        assert_eq!(pj[0].donnees, b"%PDF-1.4\n");
        // Un nom choisi par l'expéditeur ne sort jamais du dossier de téléchargement.
        assert_eq!(pj[1].nom, "bashrc");
        assert_eq!(body_text(MAIL), "Ordre du jour ci-joint.");
    }

    #[test]
    fn prepares_replies() {
        let r = reponse_depuis(MAIL, "moi@gmail.com", "D'accord, j'y serai.", true);
        assert_eq!(r.a, vec!["hery@ex.mg"]);
        assert_eq!(r.cc, vec!["voa@ex.mg", "jean@ex.mg"]);
        assert_eq!(r.sujet, "Re: Réunion");
        assert_eq!(r.en_reponse, Some(("<abc@ex.mg>".into(), "<abc@ex.mg>".into())));
        assert!(r.texte.starts_with("D'accord, j'y serai.\n\n"));
        assert!(r.texte.contains("> Ordre du jour ci-joint."));
        assert!(reponse_depuis(MAIL, "moi@gmail.com", "ok", false).cc.is_empty());
        let c = Compte::guess("moi@gmail.com");
        let m = composer(&c, &r).unwrap();
        assert!(m.contains("In-Reply-To: <abc@ex.mg>\r\n") && m.contains("Cc: voa@ex.mg, jean@ex.mg\r\n"));
    }

    #[test]
    fn attaches_files_and_finds_drafts() {
        let f = std::env::temp_dir().join(format!("lkatta-pj-{}-résumé.txt", std::process::id()));
        std::fs::write(&f, "bonjour").unwrap();
        let c = Compte::guess("moi@gmail.com");
        let b = Brouillon { a: vec!["a@b.c".into()], sujet: "PJ".into(), texte: "voir".into(), pieces: vec![f.clone()], ..Default::default() };
        let m = composer(&c, &b).unwrap();
        assert!(m.contains("multipart/mixed") && m.contains("Content-Disposition: attachment; filename*=UTF-8''lkatta-pj-"));
        assert_eq!(pieces_jointes(&m)[0].donnees, b"bonjour");
        let _ = std::fs::remove_file(f);
        let gmail = "* LIST (\\HasNoChildren) \"/\" \"INBOX\"\r\n* LIST (\\Drafts \\HasNoChildren) \"/\" \"[Gmail]/Brouillons\"\r\n";
        assert_eq!(dossier_brouillons_depuis(gmail).as_deref(), Some("[Gmail]/Brouillons"));
        assert_eq!(dossier_brouillons_depuis("* LIST () \".\" INBOX.Drafts\r\n").as_deref(), Some("INBOX.Drafts"));
        assert_eq!(adresses("\"Rakoto, Jean\" <jean@ex.mg>, voa@ex.mg"), vec!["jean@ex.mg", "voa@ex.mg"]);
    }

    #[test]
    fn headers_cannot_be_injected() {
        let c = Compte::guess("moi@gmail.com");
        let b = Brouillon { a: vec!["a@b.c".into()], sujet: "Bonjour\r\nBcc: voleur@x.y".into(), texte: "t".into(), ..Default::default() };
        let m = composer(&c, &b).unwrap();
        assert!(!m.contains("\r\nBcc:"), "{m}");
        // Objet piégé dans un mail reçu (RFC 2047) : la réponse ne gagne pas d'en-tête.
        let piege = "From: x@y.z\r\nSubject: =?UTF-8?B?Qm9uam91cg0KQmNjOiB2b2xldXJAeC55?=\r\nMessage-ID: <a@b>\r\n evil: 1\r\n\r\ncorps\r\n";
        let r = reponse_depuis(piege, "moi@gmail.com", "ok", false);
        let m = composer(&c, &r).unwrap();
        assert!(!m.contains("\r\nBcc:") && !m.to_lowercase().contains("\r\nevil"), "{m}");
        for mauvais in ["a@b.c\r\nBcc: x@y.z", "a b@c.d", "a@b.c,x@y.z"] {
            assert!(composer(&c, &Brouillon { a: vec![mauvais.into()], ..Default::default() }).is_err(), "{mauvais}");
            assert!(composer(&c, &Brouillon { a: vec!["a@b.c".into()], cc: vec![mauvais.into()], ..Default::default() }).is_err());
        }
        // Parties emboîtées sans fin : pas de plantage.
        let mut profond = String::new();
        for i in 0..5000 {
            profond.push_str(&format!("Content-Type: multipart/mixed; boundary=\"b{i}\"\n\n--b{i}\n"));
        }
        let _ = body_text(&profond);
    }

    #[test]
    fn composes_utf8() {
        let c = Compte::guess("moi@gmail.com");
        let b = Brouillon { a: vec!["hery@ex.mg".into()], sujet: "Réunion".into(), texte: "Bonjour,\nà demain.".into(), ..Default::default() };
        let m = composer(&c, &b).unwrap();
        assert!(m.contains("Subject: =?UTF-8?B?"));
        assert!(m.contains("To: hery@ex.mg\r\n"));
        assert!(m.ends_with("Bonjour,\r\nà demain.\r\n"));
    }
}
