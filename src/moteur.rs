//! Moteur intégré : choisit tout seul le meilleur modèle d'IA gratuit pour chaque demande,
//! et passe au suivant quand l'un est saturé. Porté de Tontolo (catalogue, classifieur,
//! routeur, quotas), sans Python ni serveur : L Katta fonctionne seul, sans compte.
//!
//! - Catalogue : liste de secours embarquée (assets/seed.json, issue de Tontolo), puis
//!   rafraîchie en ligne auprès des fournisseurs gratuits sans clé (LLM7, OVHcloud, Kilo,
//!   Pollinations), d'OpenRouter si une clé existe, et d'Ollama s'il tourne en local.
//! - Classifieur : devine le genre de demande (code, maths, rédaction…).
//! - Routeur : trie par qualité pour ce genre, fiabilité observée, disponibilité ; un
//!   modèle saturé est mis en pause (délai croissant, ou jusqu'à minuit si quota du jour).

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use regex_lite::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::paths;

const SEED: &str = include_str!("../assets/seed.json");
const REFRESH_SECS: u64 = 12 * 3600;

fn now() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Fournisseur {
    pub id: String,
    pub nom: String,
    pub base_url: String,
    /// Fournisseur à clé : la clé est lue dans le trousseau au moment de l'appel, jamais
    /// enregistrée dans le catalogue.
    #[serde(default)]
    pub avec_cle: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Modele {
    pub provider: String,
    pub model_id: String,
    pub name: String,
    pub tools: bool,
    pub vision: bool,
    pub quality: HashMap<String, f32>,
    #[serde(default)]
    pub local: bool,
    /// Fournisseur payant : jamais choisi tout seul (sauf « payants en premier »).
    #[serde(default)]
    pub payant: bool,
}

impl Modele {
    pub fn key(&self) -> String {
        format!("{}:{}", self.provider, self.model_id)
    }
    pub fn score(&self, cat: &str) -> f32 {
        self.quality.get(cat).or(self.quality.get("chat")).copied().unwrap_or(0.4)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Catalogue {
    pub mis_a_jour: f64,
    pub fournisseurs: Vec<Fournisseur>,
    pub modeles: Vec<Modele>,
}

/// État d'un modèle : pause éventuelle et fiabilité observée.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct Etat {
    /// En pause jusqu'à (secondes Unix).
    until: f64,
    /// « pause » ou « quota du jour épuisé ».
    raison: String,
    echecs: u32,
    ok: f32,
    total: f32,
}

#[derive(Default, Serialize, Deserialize)]
struct Etats {
    modeles: HashMap<String, Etat>,
}

fn catalogue_file() -> std::path::PathBuf {
    paths::data_dir().join("catalogue.json")
}
fn etats_file() -> std::path::PathBuf {
    paths::data_dir().join("moteur-etats.json")
}

fn etats() -> &'static Mutex<Etats> {
    static E: OnceLock<Mutex<Etats>> = OnceLock::new();
    E.get_or_init(|| {
        Mutex::new(std::fs::read_to_string(etats_file()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default())
    })
}

fn save_etats(e: &Etats) {
    if let Ok(s) = serde_json::to_string(e) {
        let _ = paths::write_private(&etats_file(), s.as_bytes());
    }
}

// --- catalogue ------------------------------------------------------------------------

/// Catalogue de secours embarqué (fournisseurs compatibles OpenAI seulement).
pub fn seed() -> Catalogue {
    let v: Value = serde_json::from_str(SEED).unwrap_or_default();
    let mut fournisseurs = vec![];
    for (id, p) in v["providers"].as_object().into_iter().flatten() {
        let base = p["base_url"].as_str().unwrap_or("");
        if base.contains("huggingface.co") || base.is_empty() {
            continue;
        }
        fournisseurs.push(Fournisseur {
            id: id.clone(),
            nom: p["name"].as_str().unwrap_or(id).to_string(),
            base_url: base.trim_end_matches('/').to_string(),
            avec_cle: false,
        });
    }
    let modeles = v["models"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|m| fournisseurs.iter().any(|f| Some(f.id.as_str()) == m["provider"].as_str()))
        .filter(|m| m["is_free"].as_bool().unwrap_or(false) && !m["needs_key"].as_bool().unwrap_or(false))
        .filter(|m| m["output_modalities"].as_array().is_none_or(|o| o.iter().any(|x| x == "text")))
        .map(|m| Modele {
            provider: m["provider"].as_str().unwrap_or("").into(),
            model_id: m["model_id"].as_str().unwrap_or("").into(),
            name: m["name"].as_str().unwrap_or("").into(),
            tools: m["supports_tools"].as_bool().unwrap_or(false),
            vision: m["input_modalities"].as_array().is_some_and(|i| i.iter().any(|x| x == "image")),
            quality: m["quality"].as_object().map(|q| q.iter().filter_map(|(k, v)| v.as_f64().map(|f| (k.clone(), f as f32))).collect()).unwrap_or_default(),
            local: false,
            payant: false,
        })
        .collect();
    Catalogue { mis_a_jour: 0.0, fournisseurs, modeles }
}

/// Catalogue courant : copie rafraîchie s'il y en a une, sinon le secours embarqué.
pub fn catalogue() -> Catalogue {
    std::fs::read_to_string(catalogue_file())
        .ok()
        .and_then(|s| serde_json::from_str::<Catalogue>(&s).ok())
        .filter(|c| !c.modeles.is_empty())
        .unwrap_or_else(seed)
}

fn agent(secs: u64) -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(4)))
        .timeout_global(Some(Duration::from_secs(secs)))
        .http_status_as_error(false)
        .user_agent(concat!("lkatta/", env!("CARGO_PKG_VERSION")))
        .build()
        .into()
}

fn get_json(url: &str, key: &str) -> Option<Value> {
    let mut req = agent(15).get(url);
    if url.starts_with("https://api.anthropic.com/") {
        // L'API native d'Anthropic (liste des modèles) veut son propre en-tête.
        req = req.header("x-api-key", key).header("anthropic-version", "2023-06-01");
    } else if !key.is_empty() {
        req = req.header("Authorization", &format!("Bearer {key}"));
    }
    let mut r = req.call().ok()?;
    if !r.status().is_success() {
        return None;
    }
    r.body_mut().with_config().limit(20 * 1024 * 1024).read_json().ok()
}

/// Qualité estimée d'un modèle absent du catalogue : par famille (même début de nom)
/// sinon d'après sa taille (« 70b »…).
fn estimate(name: &str, known: &[Modele]) -> HashMap<String, f32> {
    let n = name.to_lowercase();
    let stem: String = n.split(['-', ':', '/', '.', '_']).take(2).collect::<Vec<_>>().join("-");
    if let Some(m) = known.iter().find(|m| {
        let k = m.name.to_lowercase();
        stem.len() > 4 && k.split(['-', ':', '/', '.', '_']).take(2).collect::<Vec<_>>().join("-") == stem
    }) {
        return m.quality.iter().map(|(k, v)| (k.clone(), v * 0.95)).collect();
    }
    let size = Regex::new(r"(\d+(?:\.\d+)?)\s*b\b").ok().and_then(|re| re.captures(&n)).and_then(|c| c[1].parse::<f32>().ok()).unwrap_or(8.0);
    let q = (0.3 + 0.12 * size.max(1.0).log10()).min(0.72);
    ["chat", "code", "math", "reasoning", "writing", "translation", "search", "agentic"].iter().map(|c| (c.to_string(), q)).collect()
}

fn is_text_chat(id: &str) -> bool {
    let l = id.to_lowercase();
    !["embed", "bge", "guard", "whisper", "diffusion", "tts", "flux", "lyria", "rerank", "moderation", "image", "veo", "dall-e", "audio", "realtime", "transcribe", "sora", "babbage", "davinci"]
        .iter()
        .any(|w| l.contains(w))
}

/// Rafraîchit le catalogue en ligne (sans bloquer : à lancer dans un fil).
pub fn refresh() -> Result<usize, String> {
    let base = seed();
    let mut fournisseurs = base.fournisseurs.clone();
    let mut modeles: Vec<Modele> = vec![];
    let mut seen_provider = vec![];
    let push = |list: &mut Vec<Modele>, provider: &str, id: &str, name: &str, tools: bool, vision: bool, known: &[Modele]| {
        if id.is_empty() || !is_text_chat(id) || list.iter().any(|m| m.provider == provider && m.model_id == id) {
            return;
        }
        let quality = known
            .iter()
            .find(|m| m.provider == provider && m.model_id == id)
            .map(|m| m.quality.clone())
            .unwrap_or_else(|| estimate(name, known));
        let payant = crate::cles::fournisseur(provider).is_some_and(|f| f.payant);
        list.push(Modele { provider: provider.into(), model_id: id.into(), name: name.into(), tools, vision, quality, local: provider.ends_with("_local"), payant });
    };
    for f in &base.fournisseurs {
        let Some(v) = get_json(&format!("{}/models", f.base_url), "") else { continue };
        seen_provider.push(f.id.clone());
        match f.id.as_str() {
            "pollinations" => {
                let items = v.as_array().or(v["data"].as_array()).cloned().unwrap_or_default();
                for m in &items {
                    if m["tier"].as_str().is_some_and(|t| t != "anonymous") || m["output_modalities"].as_array().is_some_and(|o| !o.iter().any(|x| x == "text")) {
                        continue;
                    }
                    let id = m["name"].as_str().or(m["id"].as_str()).unwrap_or("");
                    let known = base.modeles.iter().find(|k| k.provider == f.id && k.model_id == id);
                    let tools = m["tools"].as_bool().or(known.map(|k| k.tools)).unwrap_or(false);
                    push(&mut modeles, &f.id, id, m["description"].as_str().unwrap_or(id), tools, m["vision"].as_bool().unwrap_or(false), &base.modeles);
                }
            }
            _ => {
                for m in v["data"].as_array().into_iter().flatten() {
                    let id = m["id"].as_str().unwrap_or("");
                    // Kilo : seulement les gratuits ; LLM7 : pas le niveau « pro » payant ;
                    // OVHcloud : tout est gratuit.
                    let free = match f.id.as_str() {
                        "kilo" => m["isFree"].as_bool().unwrap_or(false) || id.ends_with(":free"),
                        "llm7" => m["tier"].as_str() != Some("pro") && m["model_type"].as_str().is_none_or(|t| t == "chat"),
                        _ => m["context_length"].as_i64().is_none_or(|c| c > 0),
                    };
                    if !free {
                        continue;
                    }
                    let tools = m["supported_parameters"].as_array().map(|p| p.iter().any(|x| x == "tools")).or(m["tools_calling"].as_bool()).unwrap_or(true);
                    let vision = m["architecture"]["input_modalities"].as_array().is_some_and(|i| i.iter().any(|x| x == "image"));
                    push(&mut modeles, &f.id, id, m["name"].as_str().unwrap_or(id), tools, vision, &base.modeles);
                }
            }
        }
    }
    // Fournisseurs injoignables : on garde leurs modèles du secours.
    for m in &base.modeles {
        if !seen_provider.contains(&m.provider) && !modeles.iter().any(|x| x.key() == m.key()) {
            modeles.push(m.clone());
        }
    }
    // Fournisseurs à clé (OpenRouter, Groq, Gemini, Mistral…) : ceux dont la clé est connue.
    for f in crate::cles::FOURNISSEURS {
        let Some(k) = crate::cles::cle(f.id) else { continue };
        let url = f.models_url.map(String::from).unwrap_or_else(|| format!("{}/models", f.base_url));
        let Some(v) = get_json(&url, &k) else { continue };
        fournisseurs.push(Fournisseur { id: f.id.into(), nom: f.nom.into(), base_url: f.base_url.into(), avec_cle: true });
        let items = v.as_array().or(v["data"].as_array()).or(v["models"].as_array()).cloned().unwrap_or_default();
        for m in &items {
            let id = m["id"].as_str().or(m["name"].as_str()).unwrap_or("");
            let id = id.strip_prefix("models/").unwrap_or(id);
            if f.free_suffix.is_some_and(|suf| !id.ends_with(suf)) {
                continue;
            }
            if f.id == "openrouter" && m["pricing"]["prompt"].as_str().is_some_and(|p| p != "0") {
                continue;
            }
            let tools = m["supported_parameters"].as_array().map(|p| p.iter().any(|x| x == "tools")).unwrap_or(true);
            let vision = m["architecture"]["input_modalities"].as_array().is_some_and(|i| i.iter().any(|x| x == "image"));
            let name = m["name"].as_str().or(m["display_name"].as_str()).unwrap_or(id);
            push(&mut modeles, f.id, id, name, tools, vision, &base.modeles);
        }
    }
    // Ollama local : modèles installés (utilisés surtout sans réseau).
    if let Some(v) = get_json("http://127.0.0.1:11434/api/tags", "") {
        fournisseurs.push(Fournisseur { id: "ollama_local".into(), nom: "Ollama (local)".into(), base_url: "http://127.0.0.1:11434/v1".into(), avec_cle: false });
        for m in v["models"].as_array().into_iter().flatten() {
            let id = m["name"].as_str().unwrap_or("");
            push(&mut modeles, "ollama_local", id, &format!("{id} (local)"), true, false, &base.modeles);
        }
    }
    if modeles.is_empty() {
        return Err(t!("aucun fournisseur joignable").into());
    }
    let n = modeles.len();
    let c = Catalogue { mis_a_jour: now(), fournisseurs, modeles };
    let s = serde_json::to_string(&c).map_err(|e| e.to_string())?;
    paths::write_private(&catalogue_file(), s.as_bytes()).map_err(|e| e.to_string())?;
    Ok(n)
}

/// Rafraîchit si le catalogue a plus de 12 h.
pub fn refresh_if_old() {
    if now() - catalogue().mis_a_jour > REFRESH_SECS as f64 {
        let _ = refresh();
    }
}

// --- classifieur -------------------------------------------------------------------------

/// Genre de la demande, d'après des mots-clés pondérés (français, anglais, malgache).
pub fn classify(text: &str) -> &'static str {
    static RULES: OnceLock<Vec<(&'static str, Regex, f32)>> = OnceLock::new();
    let rules = RULES.get_or_init(|| {
        let r = |cat: &'static str, p: &str, w: f32| (cat, Regex::new(&format!("(?i){p}")).expect("regex"), w);
        vec![
            r("code", r"```", 3.0),
            r("code", r"\b(python|javascript|typescript|java|rust|golang|kotlin|dart|flutter|c\+\+|sql|bash|regex|html|css|react|django|fastapi|docker)\b", 2.0),
            r("code", r"\b(code|coder|fonction|function|bug|d[ée]bogue|debug|compile|script|classe|m[ée]thode|api|refactor|stack ?trace|exception|traceback)\b", 1.5),
            r("code", r"(def |class |import |#include|console\.log|public static)", 3.0),
            r("code", r"\b(kaody|programma)\b", 2.0),
            r("math", r"\$[^$]+\$|\\\(|\\\[|\\frac|\\int|\\sum|\\sqrt", 3.0),
            r("math", r"\b([ée]quation|int[ée]grale|d[ée]riv[ée]e|th[ée]or[èe]me|d[ée]montre|d[ée]monstration|matrice|probabilit[ée]|limite|polyn[ôo]me|calcule|r[ée]sous)\b", 2.0),
            r("math", r"\b(equation|integral|derivative|theorem|prove|proof|matrix|probability|solve|calculate|compute)\b", 2.0),
            r("math", r"\d+\s*[-+*/^]\s*\d+", 1.0),
            r("math", r"\b(kajy|fitoviana|porofo|latex|tikz|alg[èe]bre)\b", 2.0),
            r("translation", r"\b(tradui[st]|traduction|traduire|en anglais|en fran[çc]ais|en malgache|translate|translation|adikao|dikao)\b", 4.0),
            r("writing", r"\b(r[ée]dige|[ée]cris|po[èe]me|lettre|histoire|conte|article|discours|dissertation|r[ée]sum[ée]|slogan|courriel|e-?mail|write|poem|letter|story|essay|soraty|tantara|tononkalo|taratasy)\b", 2.0),
            r("writing", r"\b(dossier|manuel|cours|rapport|m[ée]moire|document|chapitres?)\b", 1.5),
            r("search", r"\b(actualit[ée]s?|aujourd'hui|derni[èe]res? (nouvelles|infos)|cette semaine|en ce moment|m[ée]t[ée]o|prix actuel|r[ée]cemment|qui a gagn[ée]|news|today|latest|weather|vaovao|androany)\b", 3.0),
            r("search", r"\b(cherche|recherche|trouve)\b.*\b(sur (le )?web|en ligne|internet|sources?)\b", 3.0),
            r("reasoning", r"\b(pourquoi|analyse|compare|raisonne|logique|[ée]nigme|strat[ée]gie|avantages et inconv[ée]nients|[ée]tape par [ée]tape|why|analy[sz]e|reason|step by step|nahoana|hazavao)\b", 1.5),
        ]
    });
    let mut scores: HashMap<&'static str, f32> = HashMap::new();
    for (cat, re, w) in rules {
        if re.is_match(text) {
            *scores.entry(cat).or_default() += w;
        }
    }
    scores.into_iter().filter(|(_, s)| *s >= 1.5).max_by(|a, b| a.1.total_cmp(&b.1)).map(|(c, _)| c).unwrap_or("chat")
}

// --- routeur ---------------------------------------------------------------------------

/// État lisible d'un modèle (pour le sélecteur) : `None` = disponible.
pub fn pause_of(key: &str) -> Option<String> {
    let e = etats().lock().ok()?;
    e.modeles.get(key).filter(|s| s.until > now()).map(|s| s.raison.clone())
}

fn reliability(key: &str) -> f32 {
    let e = etats().lock();
    let (ok, total) = e.ok().and_then(|e| e.modeles.get(key).map(|s| (s.ok, s.total))).unwrap_or((0.0, 0.0));
    (ok + 4.0) / (total + 5.0)
}

/// Modèles à essayer, du meilleur au moins bon, pour un genre de demande. Les modèles
/// payants ne viennent que choisis exprès, ou en tête si `payants` (« payants en premier »).
pub fn route(cat: &str, preferred: &str, cat_: &Catalogue, payants: bool) -> Vec<Modele> {
    let choisi = |m: &Modele| preferred != "auto" && !preferred.is_empty() && (m.key() == preferred || m.model_id == preferred);
    let mut v: Vec<(f32, Modele)> = cat_
        .modeles
        .iter()
        .filter(|m| m.tools && pause_of(&m.key()).is_none() && (!m.payant || payants || choisi(m)))
        .map(|m| {
            // Les modèles locaux passent après les gratuits en ligne (plus lents ici).
            let local = if m.local { -0.25 } else { 0.0 };
            let payant = if m.payant { 1.0 } else { 0.0 };
            (0.75 * m.score(cat) + 0.25 * reliability(&m.key()) + local + payant, m.clone())
        })
        .collect();
    v.sort_by(|a, b| b.0.total_cmp(&a.0));
    let mut out: Vec<Modele> = v.into_iter().map(|(_, m)| m).collect();
    if preferred != "auto" && !preferred.is_empty() {
        if let Some(i) = out.iter().position(|m| m.key() == preferred || m.model_id == preferred) {
            let m = out.remove(i);
            out.insert(0, m);
        }
    }
    out
}

/// Clé refusée (401/403) : tous les modèles du fournisseur en pause pour 12 h.
fn pause_provider(provider: &str, models: &[Modele]) {
    let Ok(mut e) = etats().lock() else { return };
    for m in models.iter().filter(|m| m.provider == provider) {
        let st = e.modeles.entry(m.key()).or_default();
        st.until = now() + 12.0 * 3600.0;
        st.raison = "clé refusée".into();
    }
    save_etats(&e);
}

fn record(key: &str, ok: bool, status: u16, msg: &str) {
    let Ok(mut e) = etats().lock() else { return };
    let st = e.modeles.entry(key.to_string()).or_default();
    st.ok = st.ok * 0.97 + if ok { 1.0 } else { 0.0 };
    st.total = st.total * 0.97 + 1.0;
    if ok {
        st.echecs = 0;
        st.until = 0.0;
    } else if status != 400 {
        st.echecs += 1;
        let low = msg.to_lowercase();
        if low.contains("daily") || low.contains("per day") || low.contains("quota") {
            // Quota du jour : jusqu'à minuit UTC.
            let t = now();
            st.until = (t / 86400.0).floor() * 86400.0 + 86400.0;
            st.raison = "quota du jour épuisé".into();
        } else {
            st.until = now() + (30.0 * 2f64.powi(st.echecs.min(6) as i32 - 1)).min(1800.0);
            st.raison = "en pause".into();
        }
    }
    save_etats(&e);
}

/// Réglages d'une complétion.
#[derive(Debug, Clone)]
pub struct Options {
    /// Modèles payants autorisés (et essayés en premier).
    pub payants: bool,
    pub max_tokens: u32,
}

impl Default for Options {
    fn default() -> Self {
        Self { payants: false, max_tokens: 1800 }
    }
}

/// Une complétion par le moteur intégré : essaie les meilleurs modèles l'un après l'autre.
/// Renvoie le message de l'assistant et le modèle qui a répondu.
/// Complète la conversation ; le texte arrive au fil de l'eau dans `on_delta` (une chaîne
/// vide veut dire « on recommence avec un autre modèle »).
pub fn complete(messages: &[Value], tools: &Value, preferred: &str, opts: &Options, on_delta: &mut dyn FnMut(&str)) -> Result<(Value, String), String> {
    let cat_ = catalogue();
    let last_user = messages.iter().rev().find(|m| m["role"] == "user").and_then(|m| m["content"].as_str()).unwrap_or("");
    let cat = classify(last_user);
    let mut chain = route(cat, preferred, &cat_, opts.payants);
    chain.truncate(5);
    // Repli local : si aucun modèle en ligne ne répond, le meilleur modèle installé sur la
    // machine (Ollama) est essayé en dernier, même sans réseau.
    if !chain.iter().any(|m| m.local) {
        if let Some(l) = route(cat, "auto", &cat_, false).into_iter().find(|m| m.local) {
            chain.push(l);
        }
    }
    if chain.is_empty() {
        return Err(t!("tous les modèles gratuits sont en pause : réessaie dans un moment").into());
    }
    let mut errors = vec![];
    for m in chain.iter() {
        let Some(f) = cat_.fournisseurs.iter().find(|f| f.id == m.provider) else { continue };
        let mut body = json!({
            "model": m.model_id,
            "messages": messages,
            "temperature": 0.4,
            "max_tokens": opts.max_tokens,
            "stream": true,
        });
        if tools.as_array().is_some_and(|t| !t.is_empty()) {
            body["tools"] = tools.clone();
            body["tool_choice"] = json!("auto");
        }
        let mut req = agent(if m.local { 300 } else { 120 }).post(format!("{}/chat/completions", f.base_url)).header("Content-Type", "application/json");
        if f.avec_cle {
            let Some(k) = crate::cles::cle(&f.id) else { continue };
            req = req.header("Authorization", &format!("Bearer {k}"));
            if f.id == "anthropic" {
                req = req.header("x-api-key", &k);
            }
        }
        let key = m.key();
        on_delta("");
        match req.send_json(&body) {
            Ok(mut r) => {
                let status = r.status().as_u16();
                let sse = r.headers().get("content-type").and_then(|v| v.to_str().ok()).is_some_and(|c| c.contains("event-stream"));
                let lu = if status < 400 { Some(crate::flux::lire(r.body_mut().as_reader(), sse, on_delta)) } else { None };
                match lu {
                    Some(Ok((msg, _))) => {
                        record(&key, true, status, "");
                        return Ok((msg, format!("{} · {}", f.nom, m.name)));
                    }
                    _ => {
                        let err = match lu {
                            Some(Err(e)) => e,
                            _ => r.body_mut().read_json::<Value>().ok().and_then(|v| v.pointer("/error/message").and_then(Value::as_str).map(String::from)).unwrap_or_else(|| "réponse vide".into()),
                        };
                        if status == 401 || status == 403 {
                            pause_provider(&m.provider, &cat_.modeles);
                        }
                        record(&key, false, status, &err);
                        errors.push(format!("{} : HTTP {status} {err}", m.name));
                    }
                }
            }
            Err(e) => {
                record(&key, false, 0, &e.to_string());
                errors.push(format!("{} : {e}", m.name));
            }
        }
    }
    Err(t!("Les modèles gratuits n'ont pas répondu ({}) :\n{}", cat, errors.join("\n")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seed_is_usable_without_keys() {
        let c = seed();
        assert!(c.fournisseurs.iter().all(|f| !f.avec_cle && f.base_url.starts_with("https://")));
        assert!(c.modeles.iter().filter(|m| m.tools).count() >= 10, "{}", c.modeles.len());
        assert!(!c.fournisseurs.iter().any(|f| f.base_url.contains("huggingface")));
    }

    #[test]
    fn classifies_requests() {
        assert_eq!(classify("corrige ce bug dans ma fonction python"), "code");
        assert_eq!(classify("résous l'équation x^2 + 3x = 4"), "math");
        assert_eq!(classify("traduis ce texte en anglais"), "translation");
        assert_eq!(classify("rédige une lettre de motivation"), "writing");
        assert_eq!(classify("salut, ça va ?"), "chat");
    }

    #[test]
    fn routes_by_category_and_preference() {
        let c = seed();
        let code = route("code", "auto", &c, false);
        assert!(!code.is_empty() && code.iter().all(|m| m.tools));
        // Le meilleur en code a la meilleure note de code (à fiabilité égale).
        let best = code[0].score("code");
        assert!(code.iter().all(|m| m.score("code") <= best + 0.3));
        let pick = code.last().unwrap().key();
        assert_eq!(route("code", &pick, &c, false)[0].key(), pick);
    }

    #[test]
    fn paid_models_only_when_chosen_or_allowed() {
        let mut c = seed();
        let mut p = c.modeles.iter().find(|m| m.tools).unwrap().clone();
        p.provider = "anthropic".into();
        p.model_id = "claude-test".into();
        p.payant = true;
        let key = p.key();
        c.modeles.push(p);
        assert!(!route("chat", "auto", &c, false).iter().any(|m| m.payant));
        assert_eq!(route("chat", &key, &c, false)[0].key(), key);
        assert_eq!(route("chat", "auto", &c, true)[0].key(), key);
    }

    #[test]
    fn estimates_unknown_models() {
        let q = estimate("Llama-3.3-70B-Instruct", &[]);
        let small = estimate("tiny-1b", &[]);
        assert!(q["chat"] > small["chat"]);
    }
}
