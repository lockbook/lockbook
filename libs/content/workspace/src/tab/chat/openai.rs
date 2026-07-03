//! Chat backend over any OpenAI-compatible `/chat/completions` endpoint
//! (Cerebras, Groq, OpenRouter, Ollama, OpenAI itself, …), streaming via SSE.

use futures::StreamExt;
use lb_rs::model::chat::Usage;
use serde::Deserialize;
use serde_json::json;
use tokio::sync::mpsc::UnboundedSender;

use super::backend::{ChatMsg, CompletionReq, ModelInfo};
use super::settings::Provider;

/// Total request attempts (initial + backoff retries on 429/5xx).
const MAX_ATTEMPTS: u32 = 3;
/// Give up when the stream goes quiet this long mid-response.
const STREAM_IDLE: std::time::Duration = std::time::Duration::from_secs(60);

/// Host behaviors learned from their own error responses. Backends are
/// rebuilt every turn (resolve-at-send), so without process-wide memory
/// each turn would re-pay the probe request before the real one.
#[derive(Default, Clone, Copy)]
struct HostQuirks {
    /// Rejected the legacy `max_tokens` cap name (OpenAI's newer models
    /// take `max_completion_tokens`).
    renamed_cap: bool,
    /// Output cap that fit the host's per-minute token budget (Groq's free
    /// tier admission-checks prompt + cap against 8k TPM).
    cap: Option<u32>,
}

fn quirks(base_url: &str) -> HostQuirks {
    quirks_map()
        .lock()
        .ok()
        .and_then(|map| map.get(base_url).copied())
        .unwrap_or_default()
}

fn learn_quirk(base_url: &str, update: impl FnOnce(&mut HostQuirks)) {
    if let Ok(mut map) = quirks_map().lock() {
        update(map.entry(base_url.to_string()).or_default());
    }
}

fn quirks_map() -> &'static std::sync::Mutex<std::collections::HashMap<String, HostQuirks>> {
    static MAP: std::sync::OnceLock<
        std::sync::Mutex<std::collections::HashMap<String, HostQuirks>>,
    > = std::sync::OnceLock::new();
    MAP.get_or_init(Default::default)
}

/// The first run of digits after `tag`, for mining numbers out of error
/// prose ("Limit 8000, Requested 16175").
fn number_after(s: &str, tag: &str) -> Option<u64> {
    let rest = &s[s.find(tag)? + tag.len()..];
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

pub struct OpenAiBackend {
    client: reqwest::Client,
    base_url: String,
    /// Absent: no Authorization header (a keyless local server).
    api_key: Option<String>,
    model: String,
    /// Reasoning effort, sent as `reasoning_effort` when set.
    effort: Option<String>,
}

pub fn openai_compat(provider: &Provider) -> OpenAiBackend {
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(10))
        .build()
        .expect("reqwest client");
    OpenAiBackend {
        client,
        base_url: provider.base_url.trim_end_matches('/').to_string(),
        api_key: provider.api_key.clone(),
        model: provider.model.clone(),
        effort: provider.effort.clone(),
    }
}

/// One SSE `data:` payload.
#[derive(Deserialize)]
struct Chunk {
    #[serde(default)]
    choices: Vec<Choice>,
    #[serde(default)]
    usage: Option<WireUsage>,
}

#[derive(Deserialize)]
struct Choice {
    delta: Delta,
}

#[derive(Deserialize)]
struct Delta {
    #[serde(default)]
    content: Option<String>,
}

#[derive(Deserialize)]
struct WireUsage {
    #[serde(default)]
    prompt_tokens: u64,
    #[serde(default)]
    completion_tokens: u64,
    #[serde(default)]
    prompt_tokens_details: Option<PromptTokensDetails>,
}

#[derive(Deserialize)]
struct PromptTokensDetails {
    #[serde(default)]
    cached_tokens: u64,
}

impl OpenAiBackend {
    pub(super) async fn complete(
        &self, req: CompletionReq, deltas: UnboundedSender<String>,
    ) -> Result<Usage, String> {
        let mut messages = Vec::new();
        if !req.system.is_empty() {
            messages.push(json!({ "role": "system", "content": req.system }));
        }
        for msg in &req.messages {
            messages.push(match msg {
                ChatMsg::User(text) => json!({ "role": "user", "content": text }),
                ChatMsg::Assistant(text) => json!({ "role": "assistant", "content": text }),
            });
        }

        let known = quirks(&self.base_url);
        let mut cap_renamed = known.renamed_cap;
        let mut cap = known.cap.map_or(req.max_tokens, |c| c.min(req.max_tokens));
        let mut cap_shrunk = false;
        let mut cap_key = if cap_renamed { "max_completion_tokens" } else { "max_tokens" };
        let mut body = json!({
            "model": self.model,
            "messages": messages,
            "stream": true,
            "stream_options": { "include_usage": true },
        });
        body[cap_key] = json!(cap);
        // The de-facto reasoning knob across OpenAI-compatible endpoints
        // (OpenAI, Google's compat layer, xAI, Groq, OpenRouter, Together).
        if let Some(effort) = &self.effort {
            body["reasoning_effort"] = json!(effort);
        }

        // Rate limits (429) and transient server errors (5xx) back off and
        // retry a couple of times before surfacing. Always pre-stream, so a
        // retry can't duplicate delivered text.
        let mut attempts: u32 = 0;
        let resp = loop {
            attempts += 1;
            let mut request = self
                .client
                .post(format!("{}/chat/completions", self.base_url))
                .json(&body);
            if let Some(key) = &self.api_key {
                request = request.bearer_auth(key);
            }
            let resp = request
                .send()
                .await
                .map_err(|e| format!("request failed: {e}"))?;

            let status = resp.status();
            if status.is_success() {
                break resp;
            }
            if (status.as_u16() == 429 || status.is_server_error()) && attempts < MAX_ATTEMPTS {
                let wait = resp
                    .headers()
                    .get("retry-after")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(2 * attempts as u64)
                    .min(30);
                tokio::time::sleep(std::time::Duration::from_secs(wait)).await;
                continue;
            }
            let error_body = resp.text().await.unwrap_or_default();
            // OpenAI's newer models reject the legacy cap name and say
            // which to use; older-spec providers only know the legacy one.
            // The error is instruction enough — no per-provider table.
            if status.as_u16() == 400
                && !cap_renamed
                && error_body.contains("max_completion_tokens")
            {
                cap_renamed = true;
                learn_quirk(&self.base_url, |q| q.renamed_cap = true);
                body.as_object_mut()
                    .expect("body is an object")
                    .remove("max_tokens");
                cap_key = "max_completion_tokens";
                body[cap_key] = json!(cap);
                continue;
            }
            // Some hosts admission-check prompt + output cap against a
            // per-minute token budget (Groq free tier: 8k), which a big cap
            // overflows on its own. The error names both numbers; shrink
            // the cap by the overage (plus room for the prompt to grow next
            // turn) and re-send.
            if !cap_shrunk && error_body.contains("tokens per minute") {
                let over = number_after(&error_body, "Requested ")
                    .zip(number_after(&error_body, "Limit "))
                    .map(|(requested, limit)| requested.saturating_sub(limit));
                if let Some(over) = over {
                    let shrunk = (cap as u64).saturating_sub(over + 512) as u32;
                    if shrunk >= 256 {
                        cap_shrunk = true;
                        cap = shrunk;
                        learn_quirk(&self.base_url, |q| q.cap = Some(shrunk));
                        body[cap_key] = json!(cap);
                        continue;
                    }
                }
            }
            return Err(format!("{status}: {}", error_body.chars().take(500).collect::<String>()));
        };

        // SSE: buffer bytes, emit each `data: {json}` line; `data: [DONE]`
        // ends the stream. The usage-only chunk arrives last with no choices.
        // The buffer stays raw bytes, decoded per complete line — network
        // chunks split mid-character, and decoding a chunk at a time would
        // corrupt multi-byte glyphs into � (and persist them).
        let mut usage = Usage::default();
        let mut stream = resp.bytes_stream();
        let mut buf: Vec<u8> = Vec::new();
        loop {
            // A stop-button turn is cancelled by dropping this future, but a
            // silently wedged connection needs its own way out.
            let item = tokio::time::timeout(STREAM_IDLE, stream.next())
                .await
                .map_err(|_| "provider stopped responding mid-stream".to_string())?;
            let Some(bytes) = item else { break };
            let bytes = bytes.map_err(|e| format!("stream failed: {e}"))?;
            buf.extend_from_slice(&bytes);
            while let Some(nl) = buf.iter().position(|&b| b == b'\n') {
                let line_bytes: Vec<u8> = buf.drain(..=nl).collect();
                let line = String::from_utf8_lossy(&line_bytes);
                let line = line.trim();
                let Some(payload) = line.strip_prefix("data:").map(str::trim) else { continue };
                if payload == "[DONE]" {
                    return Ok(usage);
                }
                let Ok(chunk) = serde_json::from_str::<Chunk>(payload) else { continue };
                if let Some(u) = chunk.usage {
                    // The wire's cached_tokens is a subset of prompt_tokens;
                    // persisted Usage keeps input and cache_read disjoint
                    // (Anthropic's native fields already are).
                    let cached = u.prompt_tokens_details.map_or(0, |d| d.cached_tokens);
                    usage.input = u.prompt_tokens.saturating_sub(cached);
                    usage.output = u.completion_tokens;
                    usage.cache_read = cached;
                }
                if let Some(text) = chunk.choices.first().and_then(|c| c.delta.content.clone()) {
                    if !text.is_empty() {
                        let _ = deltas.send(text);
                    }
                }
            }
        }
        Ok(usage)
    }
}

/// `GET /models`, blocking — called from the picker's fetch thread, never
/// the UI thread. Chat works without it (the provider file's default model);
/// this only feeds the picker's list and, when the endpoint reports one
/// (OpenRouter's `context_length`, LM Studio's `max_context_length`), the
/// usage ring's context window.
pub fn list_models_blocking(provider: &Provider) -> Result<Vec<ModelInfo>, String> {
    /// Together returns a bare array where everyone else wraps in `data`.
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Listing {
        Wrapped { data: Vec<ModelEntry> },
        Bare(Vec<ModelEntry>),
    }
    #[derive(Deserialize)]
    struct ModelEntry {
        id: String,
        /// Human-readable name: OpenRouter's `name`, Together's
        /// `display_name`; bare-OpenAI-shape providers send neither.
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        display_name: Option<String>,
        /// Together's modality ("chat", "image", "embedding", …).
        #[serde(rename = "type", default)]
        kind: Option<String>,
        #[serde(default)]
        context_length: Option<u64>,
        #[serde(default)]
        max_context_length: Option<u64>,
    }

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;
    let mut request = client.get(format!("{}/models", provider.base_url.trim_end_matches('/')));
    if let Some(key) = &provider.api_key {
        request = request.bearer_auth(key);
    }
    let resp = request.send().map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(resp.status().to_string());
    }
    let listing: Listing = resp.json().map_err(|e| e.to_string())?;
    let entries = match listing {
        Listing::Wrapped { data } => data,
        Listing::Bare(entries) => entries,
    };
    let mut models: Vec<ModelInfo> = entries
        .into_iter()
        // The picker drives one product: streamed text chat. Speech,
        // embedding, and image models can't run here — hide them (the
        // provider file still runs any id typed into it).
        .filter(|m| {
            m.kind
                .as_deref()
                .is_none_or(|k| matches!(k, "chat" | "language" | "code"))
                && is_chat_id(&m.id)
        })
        .map(|m| {
            let display_name = m
                .display_name
                .or(m.name)
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| prettify(&m.id));
            ModelInfo {
                id: m.id,
                display_name: Some(display_name),
                window: m.context_length.or(m.max_context_length),
            }
        })
        .collect();
    // These first-party hosts list every generation and dated snapshot, and
    // their ids carry versions inline — prune to each family's latest.
    // Google additionally namespaces ids under "models/" (the wire accepts
    // bare names).
    const VERSIONED_HOSTS: &[&str] =
        &["generativelanguage.googleapis.com", "api.openai.com", "api.x.ai"];
    if VERSIONED_HOSTS
        .iter()
        .any(|h| provider.base_url.contains(h))
    {
        for m in &mut models {
            if let Some(bare) = m.id.strip_prefix("models/") {
                m.id = bare.to_string();
            }
        }
        models = newest_per_family(models);
    }
    Ok(models)
}

/// No token naming a non-chat product (speech, embeddings, image gen,
/// moderation, legacy completions). Subtractive and best-effort: a missed
/// token just shows an extra row, and a false positive is recoverable via
/// the provider file's model field.
fn is_chat_id(id: &str) -> bool {
    const NON_CHAT: &[&str] = &[
        "whisper",
        "tts",
        "audio",
        "realtime",
        "transcribe",
        "dall",
        "image",
        "imagen",
        "veo",
        "embedding",
        "embed",
        "moderation",
        "guard",
        "rerank",
        "aqa",
        "davinci",
        "babbage",
    ];
    !id.split(['-', '/', ':', '.', '_'])
        .any(|token| NON_CHAT.contains(&token.to_ascii_lowercase().as_str()))
}

/// Fallback label derived from the id when the endpoint offers no display
/// name: "gpt-5.5" → "GPT 5.5", "openai/gpt-oss-120b" → "GPT OSS 120B",
/// "grok-4.3-fast" → "Grok 4.3 Fast". The acronym list is cosmetic-only —
/// an unlisted acronym degrades to "Gpt", never to wrong behavior.
fn prettify(id: &str) -> String {
    const ACRONYMS: &[&str] = &["gpt", "oss", "tts", "ai"];
    let bare = id.rsplit('/').next().unwrap_or(id);
    bare.split('-')
        .map(|token| {
            let param_count = token.len() > 1
                && token.ends_with('b')
                && token[..token.len() - 1].chars().all(|c| c.is_ascii_digit());
            if ACRONYMS.contains(&token) || param_count {
                token.to_uppercase()
            } else {
                let mut chars = token.chars();
                match chars.next() {
                    Some(first) => first.to_uppercase().chain(chars).collect(),
                    None => String::new(),
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Trailing date-stamp tokens ("-2024-08-06", "-20241022", "-0125"): all
/// digits in date-shaped widths. Dropped before version comparison so a
/// snapshot's date can't masquerade as a version (bites when the real
/// version token isn't numeric — "gpt-4o-2024-08-06" vs "gpt-4o").
fn strip_date_suffix(id: &str) -> &str {
    let mut end = id.len();
    for token in id.split('-').rev() {
        let date_shaped =
            matches!(token.len(), 2 | 4 | 6 | 8) && token.chars().all(|c| c.is_ascii_digit());
        // `end == token.len()`: the whole id is date-shaped; keep it.
        if !date_shaped || end == token.len() {
            break;
        }
        end -= token.len() + 1;
    }
    &id[..end]
}

/// One row per model family, keeping the highest version. A family is the
/// id's non-numeric tokens, its version the first numeric token — these
/// hosts carry the version inline ("gemini-3.5-flash" → gemini-flash, 3.5),
/// unlike Anthropic's date-sorted listing. Rolling aliases
/// ("gemini-flash-latest") key as their own family and stay. A version tie
/// prefers the shorter id, so "gpt-5.5" beats its dated snapshot.
fn newest_per_family(models: Vec<ModelInfo>) -> Vec<ModelInfo> {
    let mut best: std::collections::HashMap<String, (f32, usize)> = Default::default();
    for (i, m) in models.iter().enumerate() {
        let mut family = Vec::new();
        let mut version = -1.0f32;
        for token in strip_date_suffix(&m.id).split('-') {
            match token.parse::<f32>() {
                Ok(v) if version < 0.0 => version = v,
                Ok(_) => {}
                Err(_) => family.push(token),
            }
        }
        match best.entry(family.join("-")) {
            std::collections::hash_map::Entry::Occupied(mut e) => {
                let (best_version, best_i) = *e.get();
                let newer = best_version < version
                    || (best_version == version && m.id.len() < models[best_i].id.len());
                if newer {
                    e.insert((version, i));
                }
            }
            std::collections::hash_map::Entry::Vacant(e) => {
                e.insert((version, i));
            }
        }
    }
    let keep: std::collections::HashSet<usize> = best.into_values().map(|(_, i)| i).collect();
    models
        .into_iter()
        .enumerate()
        .filter(|(i, _)| keep.contains(i))
        .map(|(_, m)| m)
        .collect()
}

/// Test double for any OpenAI-compatible provider: a real socket serving a
/// canned response, so tests exercise the full reqwest + SSE path.
#[cfg(test)]
pub(super) mod mock {
    use std::io::{Read, Write};
    use std::net::TcpListener;

    pub const SSE_HELLO: &str = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n\
         data: {\"choices\":[{\"delta\":{\"content\":\"Hel\"}}]}\n\n\
         data: {\"choices\":[{\"delta\":{\"content\":\"lo\"}}]}\n\n\
         data: {\"choices\":[],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":2,\
         \"prompt_tokens_details\":{\"cached_tokens\":3}}}\n\n\
         data: [DONE]\n\n";

    /// Serve `response` verbatim to the first connection, closing after — the
    /// close delimits the body, standing in for a finished SSE stream.
    pub fn serve_once(response: &'static str) -> String {
        serve_seq(vec![response])
    }

    /// Serve `response` in two writes split at byte `split_at`, with a pause
    /// between — a chunk boundary landing wherever the caller aims it,
    /// including mid-character.
    pub fn serve_split(response: &'static str, split_at: usize) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            let (mut sock, _) = listener.accept().unwrap();
            let mut buf = Vec::new();
            let mut tmp = [0u8; 4096];
            loop {
                let n = sock.read(&mut tmp).unwrap();
                buf.extend_from_slice(&tmp[..n]);
                if buf.windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }
            let bytes = response.as_bytes();
            sock.write_all(&bytes[..split_at]).unwrap();
            sock.flush().unwrap();
            std::thread::sleep(std::time::Duration::from_millis(50));
            sock.write_all(&bytes[split_at..]).unwrap();
        });
        format!("http://{addr}")
    }

    /// Serve each response to one connection, in order.
    pub fn serve_seq(responses: Vec<&'static str>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            for response in responses {
                let (mut sock, _) = listener.accept().unwrap();
                // Read the request through its content-length body so the
                // write isn't racing the client's send.
                let mut buf = Vec::new();
                let mut tmp = [0u8; 4096];
                loop {
                    let n = sock.read(&mut tmp).unwrap();
                    buf.extend_from_slice(&tmp[..n]);
                    let Some(end) = buf.windows(4).position(|w| w == b"\r\n\r\n") else { continue };
                    let headers = String::from_utf8_lossy(&buf[..end]).to_lowercase();
                    let len = headers
                        .lines()
                        .find_map(|l| l.strip_prefix("content-length:"))
                        .map_or(0, |v| v.trim().parse::<usize>().unwrap());
                    if buf.len() >= end + 4 + len {
                        break;
                    }
                }
                sock.write_all(response.as_bytes()).unwrap();
            }
        });
        format!("http://{addr}")
    }

    /// Serve `response` and hand the request's JSON body back on the
    /// channel, for asserting what the backend actually sent on the wire.
    pub fn serve_capturing(response: &'static str) -> (String, std::sync::mpsc::Receiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let (mut sock, _) = listener.accept().unwrap();
            let mut buf = Vec::new();
            let mut tmp = [0u8; 4096];
            let body = loop {
                let n = sock.read(&mut tmp).unwrap();
                buf.extend_from_slice(&tmp[..n]);
                let Some(end) = buf.windows(4).position(|w| w == b"\r\n\r\n") else { continue };
                let headers = String::from_utf8_lossy(&buf[..end]).to_lowercase();
                let len = headers
                    .lines()
                    .find_map(|l| l.strip_prefix("content-length:"))
                    .map_or(0, |v| v.trim().parse::<usize>().unwrap());
                if buf.len() >= end + 4 + len {
                    break String::from_utf8_lossy(&buf[end + 4..end + 4 + len]).into_owned();
                }
            };
            let _ = tx.send(body);
            sock.write_all(response.as_bytes()).unwrap();
        });
        (format!("http://{addr}"), rx)
    }
}

#[cfg(test)]
mod tests {
    use super::mock::{SSE_HELLO, serve_once};
    use super::*;

    fn complete(base_url: &str) -> (Result<Usage, String>, Vec<String>) {
        complete_with(base_url, 100)
    }

    fn complete_with(base_url: &str, max_tokens: u32) -> (Result<Usage, String>, Vec<String>) {
        let backend = openai_compat(&Provider {
            name: "mock".into(),
            display_name: None,
            kind: "openai".into(),
            base_url: base_url.into(),
            model: "test-model".into(),
            api_key: Some("test-key".into()),
            effort: None,
        });
        let req = CompletionReq {
            system: "system".into(),
            messages: vec![ChatMsg::User("hi".into())],
            max_tokens,
        };
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let result = rt.block_on(backend.complete(req, tx));
        let mut deltas = Vec::new();
        while let Ok(d) = rx.try_recv() {
            deltas.push(d);
        }
        (result, deltas)
    }

    /// A configured effort rides the wire as `reasoning_effort`; an absent
    /// one sends nothing (so non-reasoning providers are untouched).
    #[test]
    fn effort_maps_to_reasoning_effort() {
        let provider = |effort: Option<&str>| Provider {
            name: "mock".into(),
            display_name: None,
            kind: "openai".into(),
            base_url: String::new(),
            model: "test-model".into(),
            api_key: Some("test-key".into()),
            effort: effort.map(str::to_string),
        };
        let run = |p: &Provider| {
            let (base, rx) = super::mock::serve_capturing(SSE_HELLO);
            let mut p = p.clone();
            p.base_url = base;
            let backend = openai_compat(&p);
            let req = CompletionReq {
                system: "s".into(),
                messages: vec![ChatMsg::User("hi".into())],
                max_tokens: 100,
            };
            let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(backend.complete(req, tx))
                .unwrap();
            rx.recv().unwrap()
        };
        assert!(run(&provider(Some("high"))).contains("\"reasoning_effort\":\"high\""));
        assert!(!run(&provider(None)).contains("reasoning_effort"));
    }

    /// A 400 naming `max_completion_tokens` re-sends with the renamed cap
    /// instead of surfacing — OpenAI's newer models reject the legacy name.
    #[test]
    fn renames_token_cap_when_rejected() {
        let unsupported = "HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\n\
             Content-Length: 82\r\nConnection: close\r\n\r\n\
             {\"error\":{\"message\":\"Use 'max_completion_tokens' instead.\",\"param\":\"max_tokens\"}}\n";
        let base = super::mock::serve_seq(vec![unsupported, SSE_HELLO]);
        let (result, deltas) = complete(&base);
        assert!(result.is_ok(), "{result:?}");
        assert_eq!(deltas.join(""), "Hello");
    }

    /// A token-budget rejection (some hosts admission-check prompt + cap
    /// against per-minute limits) shrinks the cap by the stated overage and
    /// re-sends instead of surfacing.
    #[test]
    fn shrinks_token_cap_to_budget() {
        let body = r#"{"error":{"message":"Request too large for model `x` in organization `o` service tier `on_demand` on tokens per minute (TPM): Limit 8000, Requested 16175, please reduce your message size and try again.","type":"tokens","code":"rate_limit_exceeded"}}"#;
        let resp = format!(
            "HTTP/1.1 413 Payload Too Large\r\nContent-Type: application/json\r\n\
             Content-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        let base = super::mock::serve_seq(vec![Box::leak(resp.into_boxed_str()), SSE_HELLO]);
        let (result, deltas) = complete_with(&base, 16_000);
        assert!(result.is_ok(), "{result:?}");
        assert_eq!(deltas.join(""), "Hello");
    }

    /// Non-chat products hide from the picker; chat ids survive, including
    /// vision-capable chat models whose ids don't say "image".
    #[test]
    fn non_chat_ids_filtered() {
        for id in [
            "whisper-large-v3",
            "gpt-image-1",
            "dall-e-3",
            "text-embedding-004",
            "gemini-2.5-flash-preview-tts",
            "veo-2.0-generate-001",
            "meta-llama/llama-guard-4-12b",
            "nomic-embed-text",
        ] {
            assert!(!is_chat_id(id), "{id} should be filtered");
        }
        for id in ["gpt-5.5", "grok-4.3", "gemini-3.5-flash", "gpt-4o", "chat-latest"] {
            assert!(is_chat_id(id), "{id} should survive");
        }
    }

    /// Derived labels: acronyms upper, parameter counts upper, words
    /// capitalized, org prefix dropped.
    #[test]
    fn prettified_ids() {
        for (id, label) in [
            ("gpt-5.5", "GPT 5.5"),
            ("openai/gpt-oss-120b", "GPT OSS 120B"),
            ("grok-4.3-fast", "Grok 4.3 Fast"),
            ("o4-mini", "O4 Mini"),
            ("llama3", "Llama3"),
        ] {
            assert_eq!(prettify(id), label);
        }
    }

    /// A version tie (alias vs dated snapshot — dates are numeric tokens,
    /// so both collapse to one family) keeps the shorter id.
    #[test]
    fn alias_beats_dated_snapshot() {
        let models =
            ["gpt-5.5-2026-04-23", "gpt-5.5", "gpt-4o-2024-08-06", "gpt-4o", "o1-20241217"]
                .into_iter()
                .map(|id| ModelInfo { id: id.into(), display_name: None, window: None })
                .collect();
        let kept: Vec<String> = newest_per_family(models)
            .into_iter()
            .map(|m| m.id)
            .collect();
        // "4o" isn't numeric, so without date stripping the snapshot's 2024
        // would pose as the family version and beat the alias. The lone
        // dated id survives as its family's only member.
        assert_eq!(kept, ["gpt-5.5", "gpt-4o", "o1-20241217"]);
    }

    /// Versions live mid-id and old generations coexist with dated
    /// snapshots and "-latest" aliases; each family keeps its highest.
    #[test]
    fn newest_per_family_by_version() {
        let ids = [
            "gemini-2.0-flash-001",
            "gemini-2.5-flash",
            "gemini-3.5-flash",
            "gemini-flash-latest",
            "gemini-2.5-pro",
            "text-embedding-004",
            "aqa",
        ];
        let models = ids
            .into_iter()
            .map(|id| ModelInfo { id: id.into(), display_name: None, window: None })
            .collect();
        let kept: Vec<String> = newest_per_family(models)
            .into_iter()
            .map(|m| m.id)
            .collect();
        assert_eq!(
            kept,
            [
                "gemini-3.5-flash",
                "gemini-flash-latest",
                "gemini-2.5-pro",
                "text-embedding-004",
                "aqa"
            ]
        );
    }

    #[test]
    fn streams_deltas_and_usage() {
        let base = serve_once(SSE_HELLO);
        let (result, deltas) = complete(&base);
        let usage = result.unwrap();
        assert_eq!(deltas.join(""), "Hello");
        // input excludes the cached subset: 10 prompt − 3 cached.
        assert_eq!((usage.input, usage.output, usage.cache_read), (7, 2, 3));
    }

    /// A chunk boundary inside a multi-byte character must not corrupt the
    /// delta — bytes buffer raw and decode per complete line.
    #[test]
    fn utf8_survives_chunk_split_mid_character() {
        const SSE_CRAB: &str = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n\
             data: {\"choices\":[{\"delta\":{\"content\":\"a\u{1F980}b\"}}]}\n\n\
             data: [DONE]\n\n";
        // Split two bytes into the four-byte 🦀.
        let crab_start = SSE_CRAB.find('\u{1F980}').unwrap();
        let base = super::mock::serve_split(SSE_CRAB, crab_start + 2);
        let (result, deltas) = complete(&base);
        assert!(result.is_ok(), "{result:?}");
        assert_eq!(deltas.join(""), "a\u{1F980}b");
    }

    /// A rate limit backs off (honoring Retry-After) and retries rather than
    /// surfacing; the retry succeeds normally.
    #[test]
    fn rate_limit_backs_off_and_retries() {
        let base = super::mock::serve_seq(vec![
            "HTTP/1.1 429 Too Many Requests\r\nRetry-After: 0\r\n\
             Content-Length: 0\r\nConnection: close\r\n\r\n",
            SSE_HELLO,
        ]);
        let (result, deltas) = complete(&base);
        assert!(result.is_ok(), "{result:?}");
        assert_eq!(deltas.join(""), "Hello");
    }

    #[test]
    fn error_status_surfaces_body() {
        let base = serve_once(
            "HTTP/1.1 401 Unauthorized\r\nContent-Type: application/json\r\n\
             Content-Length: 31\r\nConnection: close\r\n\r\n\
             {\"error\":\"invalid api key :(\"}\n",
        );
        let (result, deltas) = complete(&base);
        let err = result.unwrap_err();
        assert!(err.contains("401"), "{err}");
        assert!(err.contains("invalid api key"), "{err}");
        assert!(deltas.is_empty());
    }
}
