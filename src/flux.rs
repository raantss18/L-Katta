//! Réponses en continu (API compatible OpenAI, `stream: true`) : lecture des événements
//! SSE, texte transmis au fil de l'eau, appels d'outils réassemblés. Si le fournisseur
//! répond en JSON classique malgré tout, on lit le JSON.

use std::io::{BufRead, BufReader, Read};

use serde_json::{Value, json};

/// Lit une réponse (SSE ou JSON) et rend (message assistant, modèle annoncé).
pub fn lire(r: impl Read, sse: bool, on_delta: &mut dyn FnMut(&str)) -> Result<(Value, Option<String>), String> {
    if !sse {
        let v: Value = serde_json::from_reader(r).map_err(|e| format!("réponse illisible : {e}"))?;
        if let Some(e) = v.pointer("/error/message").and_then(Value::as_str) {
            return Err(e.to_string());
        }
        let mut msg = v.pointer("/choices/0/message").cloned().ok_or("réponse sans message")?;
        if let Some(f) = v.pointer("/choices/0/finish_reason").and_then(Value::as_str) {
            msg["finish_reason"] = json!(f);
        }
        if let Some(t) = msg.get("content").and_then(Value::as_str) {
            on_delta(t);
        }
        return Ok((msg, v.get("model").and_then(Value::as_str).map(String::from)));
    }
    let mut texte = String::new();
    let mut appels: Vec<Value> = vec![];
    let mut modele = None;
    let mut vu = false;
    let mut fin: Option<String> = None;
    for line in BufReader::new(r).lines() {
        let line = line.map_err(|e| format!("flux coupé : {e}"))?;
        let Some(data) = line.strip_prefix("data:") else { continue };
        let data = data.trim();
        if data == "[DONE]" {
            break;
        }
        let Ok(v) = serde_json::from_str::<Value>(data) else { continue };
        if let Some(e) = v.pointer("/error/message").and_then(Value::as_str) {
            return Err(e.to_string());
        }
        if modele.is_none() {
            modele = v.get("model").and_then(Value::as_str).map(String::from);
        }
        if let Some(f) = v.pointer("/choices/0/finish_reason").and_then(Value::as_str) {
            fin = Some(f.to_string());
        }
        let Some(d) = v.pointer("/choices/0/delta") else { continue };
        vu = true;
        if let Some(t) = d.get("content").and_then(Value::as_str) {
            if !t.is_empty() {
                texte.push_str(t);
                on_delta(t);
            }
        }
        for tc in d.get("tool_calls").and_then(Value::as_array).into_iter().flatten() {
            let i = tc.get("index").and_then(Value::as_u64).unwrap_or(appels.len().saturating_sub(1) as u64) as usize;
            while appels.len() <= i {
                appels.push(json!({"id": "", "type": "function", "function": {"name": "", "arguments": ""}}));
            }
            let a = &mut appels[i];
            if let Some(id) = tc.get("id").and_then(Value::as_str) {
                a["id"] = json!(id);
            }
            if let Some(f) = tc.get("function") {
                if let Some(n) = f.get("name").and_then(Value::as_str) {
                    let cur = a["function"]["name"].as_str().unwrap_or("").to_string();
                    a["function"]["name"] = json!(cur + n);
                }
                match f.get("arguments") {
                    Some(Value::String(s)) => {
                        let cur = a["function"]["arguments"].as_str().unwrap_or("").to_string();
                        a["function"]["arguments"] = json!(cur + s);
                    }
                    Some(o @ Value::Object(_)) => a["function"]["arguments"] = json!(o.to_string()),
                    _ => {}
                }
            }
        }
    }
    if !vu {
        return Err("flux vide".into());
    }
    let mut msg = json!({"role": "assistant", "content": texte});
    appels.retain(|a| !a["function"]["name"].as_str().unwrap_or("").is_empty());
    for (k, a) in appels.iter_mut().enumerate() {
        if a["id"].as_str().unwrap_or("").is_empty() {
            a["id"] = json!(format!("appel_{k}"));
        }
    }
    if !appels.is_empty() {
        msg["tool_calls"] = json!(appels);
    }
    if let Some(f) = fin {
        msg["finish_reason"] = json!(f);
    }
    Ok((msg, modele))
}

/// Texte à montrer pendant le flux : sans les blocs `<think>` (même pas encore fermés).
pub fn visible(s: &str) -> String {
    let mut out = crate::brain::clean(s);
    if let Some(i) = out.find("<think>") {
        out.truncate(i);
    }
    out.trim_start().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assembles_text_and_tool_calls() {
        let sse = "data: {\"model\":\"m1\",\"choices\":[{\"delta\":{\"content\":\"Bon\"}}]}\n\n\
                   data: {\"choices\":[{\"delta\":{\"content\":\"jour\"}}]}\n\n\
                   data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"c1\",\"function\":{\"name\":\"commande\",\"arguments\":\"{\\\"comm\"}}]}}]}\n\n\
                   data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"ande\\\":\\\"ls\\\"}\"}}]}}]}\n\n\
                   data: [DONE]\n";
        let mut vu = String::new();
        let (m, modele) = lire(sse.as_bytes(), true, &mut |d| vu.push_str(d)).unwrap();
        assert_eq!(vu, "Bonjour");
        assert_eq!(modele.as_deref(), Some("m1"));
        assert_eq!(m["content"], "Bonjour");
        assert_eq!(m["tool_calls"][0]["function"]["name"], "commande");
        assert_eq!(m["tool_calls"][0]["function"]["arguments"], "{\"commande\":\"ls\"}");
        let coupe = "data: {\"choices\":[{\"delta\":{\"content\":\"Il était\"}}]}\n\ndata: {\"choices\":[{\"delta\":{},\"finish_reason\":\"length\"}]}\n\ndata: [DONE]\n";
        let (m, _) = lire(coupe.as_bytes(), true, &mut |_| {}).unwrap();
        assert_eq!(m["finish_reason"], "length");
    }

    #[test]
    fn reads_plain_json_and_errors() {
        let j = r#"{"model":"x","choices":[{"message":{"role":"assistant","content":"Salut"}}]}"#;
        let mut vu = String::new();
        let (m, _) = lire(j.as_bytes(), false, &mut |d| vu.push_str(d)).unwrap();
        assert_eq!((m["content"].as_str(), vu.as_str()), (Some("Salut"), "Salut"));
        assert!(lire("data: {\"error\":{\"message\":\"quota\"}}\n".as_bytes(), true, &mut |_| {}).is_err());
        assert!(lire("".as_bytes(), true, &mut |_| {}).is_err());
        assert_eq!(visible("<think>hmm</think>Oui"), "Oui");
        assert_eq!(visible("Oui <think>en cours"), "Oui ");
    }
}
