//! Cerveau : client d'API compatible OpenAI (moteur Tontolo en premier), avec repli
//! sur les moteurs suivants et boucle d'appels d'outils.

use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::{config::{Config, Moteur}, paths, profile, tools};

/// Ce que le cerveau signale pendant son travail.
#[derive(Debug, Clone)]
pub enum Event {
    /// Moteur qui répond (nom + modèle effectif).
    Engine(String),
    /// Un outil démarre (nom, résumé lisible).
    Tool(#[allow(dead_code)] String, String),
    /// Résultat abrégé d'un outil (nom, aperçu).
    #[allow(dead_code)]
    ToolDone(String, String),
}

/// L'interface (panneau ou terminal) : confirmations et suivi.
pub trait Host {
    /// Demande l'accord de l'utilisateur pour une action qui modifie la machine.
    fn confirm(&mut self, question: &str) -> bool;
    fn event(&mut self, e: Event);
}

pub struct Brain {
    cfg: Config,
    agent: ureq::Agent,
    /// Indice du dernier moteur qui a répondu (essayé en premier).
    preferred: usize,
}

const MAX_STEPS: usize = 10;

impl Brain {
    pub fn new(cfg: Config) -> Self {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_connect(Some(Duration::from_secs(3)))
            .timeout_global(Some(Duration::from_secs(180)))
            .http_status_as_error(false)
            .build()
            .into();
        Self { cfg, agent, preferred: 0 }
    }

    fn engines(&self) -> Vec<(usize, &Moteur)> {
        let mut v: Vec<(usize, &Moteur)> = self.cfg.moteur.iter().enumerate().filter(|(_, m)| m.actif).collect();
        v.sort_by_key(|(i, _)| if *i == self.preferred { 0 } else { 1 + *i });
        v
    }

    /// Envoie l'historique (sans message système) et complète la réponse, outils compris.
    /// L'historique reçoit les messages de l'assistant et des outils.
    pub fn reply(&mut self, history: &mut Vec<Value>, host: &mut dyn Host) -> Result<String, String> {
        let schemas = tools::schemas();
        let system = profile::system_prompt(&self.cfg);
        for _ in 0..MAX_STEPS {
            let mut messages = vec![json!({"role": "system", "content": system})];
            messages.extend(history.iter().cloned());
            let msg = self.complete(&messages, &schemas, host)?;
            let calls = msg.get("tool_calls").and_then(Value::as_array).cloned().unwrap_or_default();
            if calls.is_empty() {
                let text = clean(msg.get("content").and_then(Value::as_str).unwrap_or(""));
                history.push(json!({"role": "assistant", "content": text}));
                return Ok(text);
            }
            history.push(json!({
                "role": "assistant",
                "content": msg.get("content").cloned().unwrap_or(Value::Null),
                "tool_calls": calls,
            }));
            for call in &calls {
                let id = call.get("id").and_then(Value::as_str).unwrap_or("call").to_string();
                let f = call.get("function").cloned().unwrap_or_default();
                let name = f.get("name").and_then(Value::as_str).unwrap_or("").to_string();
                let args: Value = match f.get("arguments") {
                    Some(Value::String(s)) => serde_json::from_str(s).unwrap_or(json!({})),
                    Some(v @ Value::Object(_)) => v.clone(),
                    _ => json!({}),
                };
                let out = tools::execute(&name, &args, &self.cfg, host);
                host.event(Event::ToolDone(name.clone(), preview(&out)));
                history.push(json!({"role": "tool", "tool_call_id": id, "name": name, "content": out}));
            }
        }
        Err("Trop d'étapes d'outils d'affilée : je m'arrête là.".into())
    }

    /// Une requête de complétion, avec repli sur les moteurs suivants.
    fn complete(&mut self, messages: &[Value], schemas: &Value, host: &mut dyn Host) -> Result<Value, String> {
        let mut errors = vec![];
        for (idx, m) in self.engines().into_iter().map(|(i, m)| (i, m.clone())).collect::<Vec<_>>() {
            match self.try_engine(&m, messages, schemas) {
                Ok((msg, model)) => {
                    self.preferred = idx;
                    host.event(Event::Engine(format!("{} · {}", m.nom, model)));
                    return Ok(msg);
                }
                Err(e) => errors.push(format!("{} : {}", m.nom, e)),
            }
        }
        Err(format!("Aucun moteur n'a répondu.\n{}", errors.join("\n")))
    }

    fn try_engine(&self, m: &Moteur, messages: &[Value], schemas: &Value) -> Result<(Value, String), String> {
        let key = m.cle();
        if m.url.starts_with("https://") && key.is_none() {
            return Err("pas de clé".into());
        }
        let url = format!("{}/chat/completions", m.url.trim_end_matches('/'));
        let mut body = json!({
            "model": m.modele,
            "messages": messages,
            "tools": schemas,
            "tool_choice": "auto",
            "temperature": 0.4,
            "max_tokens": 1800,
        });
        if !m.replis.is_empty() {
            let mut all = vec![m.modele.clone()];
            all.extend(m.replis.iter().cloned());
            body["models"] = json!(all);
        }
        if m.gratuit_seulement && m.url.contains("openrouter.ai") {
            body["provider"] = json!({"max_price": {"prompt": 0, "completion": 0}});
        }
        let send = || {
            let mut req = self.agent.post(&url).header("Content-Type", "application/json");
            if let Some(k) = &key {
                req = req.header("Authorization", &format!("Bearer {k}"));
            }
            req.send_json(&body)
        };
        let mut resp = match send() {
            Ok(r) => r,
            Err(e) if is_connect_error(&e) && !m.demarrer.is_empty() => {
                start_engine(m)?;
                send().map_err(|e| e.to_string())?
            }
            Err(e) => return Err(e.to_string()),
        };
        let status = resp.status().as_u16();
        let v: Value = resp.body_mut().read_json().map_err(|e| format!("réponse illisible ({status}) : {e}"))?;
        if status >= 400 {
            let msg = v.pointer("/error/message").and_then(Value::as_str).unwrap_or("erreur");
            return Err(format!("HTTP {status} : {msg}"));
        }
        let msg = v.pointer("/choices/0/message").cloned().ok_or("réponse sans message")?;
        let model = v.get("model").and_then(Value::as_str).unwrap_or(&m.modele).to_string();
        Ok((msg, model))
    }

    /// Vérifie rapidement quels moteurs répondent (commande /moteurs).
    pub fn ping_all(&self) -> Vec<(String, Result<String, String>)> {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(6)))
            .http_status_as_error(false)
            .build()
            .into();
        self.cfg
            .moteur
            .iter()
            .filter(|m| m.actif)
            .map(|m| {
                let mut req = agent.get(format!("{}/models", m.url.trim_end_matches('/')));
                if let Some(k) = m.cle() {
                    req = req.header("Authorization", &format!("Bearer {k}"));
                }
                let r = match req.call() {
                    Ok(r) if r.status().is_success() => Ok(format!("joignable, modèle {}", m.modele)),
                    Ok(r) => Err(format!("HTTP {}", r.status().as_u16())),
                    Err(e) if is_connect_error(&e) && !m.demarrer.is_empty() => {
                        Err("éteint (sera démarré à la demande)".into())
                    }
                    Err(e) => Err(e.to_string()),
                };
                (m.nom.clone(), r)
            })
            .collect()
    }
}

/// Un modèle proposé dans le sélecteur.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelInfo {
    /// Identifiant à mettre dans la configuration.
    pub key: String,
    pub name: String,
    pub provider: String,
    /// Qualité par catégorie (0..1), si le moteur la connaît (Tontolo).
    pub quality: Vec<(String, f32)>,
    pub tools: bool,
    pub vision: bool,
    /// Indisponible pour l'instant (pause après quota, quota du jour épuisé…).
    pub pause: Option<String>,
}

impl ModelInfo {
    pub fn score(&self, cat: &str) -> f32 {
        self.quality.iter().find(|(k, _)| k == cat).map(|(_, v)| *v).unwrap_or(0.0)
    }
}

/// Catalogue riche de Tontolo (`/tontolo/catalog`) : modèles gratuits et disponibles.
pub fn parse_tontolo_catalog(v: &Value) -> Vec<ModelInfo> {
    let list: Vec<Value> = match v.get("models") {
        Some(Value::Array(a)) => a.clone(),
        Some(Value::Object(o)) => o.values().cloned().collect(),
        _ => vec![],
    };
    let mut out: Vec<ModelInfo> = list
        .iter()
        .filter(|m| m["free"].as_bool().unwrap_or(false) && m["usable"].as_bool().unwrap_or(true))
        .filter(|m| m["outputs"].as_array().is_none_or(|o| o.iter().any(|x| x == "text")))
        .map(|m| ModelInfo {
            key: m["key"].as_str().unwrap_or("").to_string(),
            name: m["name"].as_str().or(m["id"].as_str()).unwrap_or("").to_string(),
            provider: m["provider"].as_str().unwrap_or("").to_string(),
            quality: m["quality"]
                .as_object()
                .map(|q| q.iter().filter_map(|(k, v)| v.as_f64().map(|f| (k.clone(), f as f32))).collect())
                .unwrap_or_default(),
            tools: m["tools"].as_bool().unwrap_or(false),
            vision: m["vision"].as_bool().unwrap_or(false),
            pause: match m["status"].as_str().unwrap_or("available") {
                "available" => None,
                "exhausted_daily" => Some("quota du jour épuisé".into()),
                "cooldown" => Some("en pause".into()),
                other => Some(other.replace('_', " ")),
            },
        })
        .filter(|m| !m.key.is_empty())
        .collect();
    out.sort_by(|a, b| b.score("chat").total_cmp(&a.score("chat")));
    out
}

/// Modèles d'un moteur pour le sélecteur (catalogue détaillé pour Tontolo, sinon liste simple).
pub fn catalog(m: &Moteur) -> Result<Vec<ModelInfo>, String> {
    let base = m.url.trim_end_matches('/').trim_end_matches("/v1").to_string();
    if m.nom.to_lowercase().contains("tontolo") || m.url.contains(":8765") {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(15)))
            .http_status_as_error(false)
            .build()
            .into();
        let mut req = agent.get(format!("{base}/tontolo/catalog?usable=true&free=true&limit=1000"));
        if let Some(k) = m.cle() {
            req = req.header("Authorization", &format!("Bearer {k}"));
        }
        match req.call() {
            Ok(mut r) if r.status().is_success() => {
                let v: Value = r.body_mut().with_config().limit(40 * 1024 * 1024).read_json().map_err(|e| e.to_string())?;
                let list = parse_tontolo_catalog(&v);
                if !list.is_empty() {
                    return Ok(list);
                }
            }
            Err(e) if is_connect_error(&e) => return Err("Tontolo est éteint : il démarrera à la première question. « Automatique » reste disponible.".into()),
            _ => {}
        }
    }
    Ok(list_models(m)?
        .into_iter()
        .map(|id| ModelInfo {
            name: id.rsplit('/').next().unwrap_or(&id).trim_end_matches(":free").to_string(),
            provider: id.split('/').next().filter(|_| id.contains('/')).unwrap_or("").to_string(),
            key: id,
            quality: vec![],
            tools: true,
            vision: false,
            pause: None,
        })
        .collect())
}

/// Modèles proposés par un moteur (`GET /models`).
pub fn list_models(m: &Moteur) -> Result<Vec<String>, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(15)))
        .http_status_as_error(false)
        .build()
        .into();
    let mut req = agent.get(format!("{}/models", m.url.trim_end_matches('/')));
    if let Some(k) = m.cle() {
        req = req.header("Authorization", &format!("Bearer {k}"));
    }
    let mut r = req.call().map_err(|e| {
        if is_connect_error(&e) && !m.demarrer.is_empty() { "éteint (il démarre à la première question)".to_string() } else { e.to_string() }
    })?;
    if !r.status().is_success() {
        return Err(format!("HTTP {}", r.status().as_u16()));
    }
    let v: Value = r.body_mut().with_config().limit(20 * 1024 * 1024).read_json().map_err(|e| e.to_string())?;
    let mut ids: Vec<String> = v
        .get("data")
        .or(v.get("models"))
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|x| x.get("id").or(x.get("name")).and_then(Value::as_str).map(String::from)).collect())
        .unwrap_or_default();
    // OpenRouter : seulement les gratuits si le moteur l'exige.
    if m.gratuit_seulement && m.url.contains("openrouter.ai") {
        ids.retain(|i| i.ends_with(":free") || i == "openrouter/free");
    }
    Ok(ids)
}

fn is_connect_error(e: &ureq::Error) -> bool {
    matches!(e, ureq::Error::ConnectionFailed | ureq::Error::Io(_) | ureq::Error::HostNotFound)
        || e.to_string().to_lowercase().contains("connect")
}

/// Démarre un moteur local détaché et attend qu'il réponde (20 s au plus).
fn start_engine(m: &Moteur) -> Result<(), String> {
    use std::os::unix::process::CommandExt;
    let log = paths::data_dir().join("moteur.log");
    let _ = std::fs::create_dir_all(paths::data_dir());
    let out = std::fs::OpenOptions::new().create(true).append(true).open(&log).map_err(|e| e.to_string())?;
    let err = out.try_clone().map_err(|e| e.to_string())?;
    let mut cmd = std::process::Command::new("sh");
    cmd.args(["-c", &m.demarrer]).stdin(std::process::Stdio::null()).stdout(out).stderr(err);
    // Nouveau groupe de session : le moteur survit au panneau.
    unsafe {
        cmd.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
    cmd.spawn().map_err(|e| format!("démarrage impossible : {e}"))?;
    let agent: ureq::Agent =
        ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(2))).build().into();
    let url = format!("{}/models", m.url.trim_end_matches('/'));
    let t0 = Instant::now();
    while t0.elapsed() < Duration::from_secs(25) {
        if agent.get(&url).call().is_ok() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    Err(format!("démarré mais muet (voir {})", log.display()))
}

/// Retire les blocs de réflexion `<think>…</think>` que certains modèles renvoient.
pub fn clean(s: &str) -> String {
    let mut out = s.to_string();
    while let (Some(a), Some(b)) = (out.find("<think>"), out.find("</think>")) {
        if b < a {
            break;
        }
        out.replace_range(a..b + "</think>".len(), "");
    }
    out.trim().to_string()
}

fn preview(s: &str) -> String {
    let one: String = s.lines().take(3).collect::<Vec<_>>().join(" ⏎ ");
    if one.chars().count() > 140 { format!("{}…", one.chars().take(140).collect::<String>()) } else { one }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_catalog() {
        let v = json!({"models": [
            {"key": "ovh:a", "name": "A", "provider": "ovh", "free": true, "usable": true, "status": "available",
             "outputs": ["text"], "tools": true, "quality": {"chat": 0.5, "code": 0.9}},
            {"key": "ovh:b", "name": "B", "provider": "ovh", "free": true, "usable": true, "status": "available",
             "outputs": ["text"], "tools": false, "quality": {"chat": 0.8, "code": 0.2}},
            {"key": "p:img", "name": "Img", "free": true, "usable": true, "outputs": ["image"]},
            {"key": "p:paid", "name": "Paid", "free": false, "usable": true, "outputs": ["text"]}
        ]});
        let l = parse_tontolo_catalog(&v);
        assert_eq!(l.iter().map(|m| m.key.as_str()).collect::<Vec<_>>(), ["ovh:b", "ovh:a"]);
        assert_eq!(l[1].score("code"), 0.9);
        assert!(!l[0].tools);
    }

    #[test]
    fn strips_think() {
        assert_eq!(clean("<think>hmm</think>\nBonjour"), "Bonjour");
        assert_eq!(clean("Salut"), "Salut");
    }
}
