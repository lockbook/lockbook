//! What a provider can run: its `/models` listing, pruned to chat models and
//! to each family's newest, with a readable name where the endpoint offers
//! none. Chat works without this (the provider file names a model); it feeds
//! the picker.

use std::sync::Mutex;

use serde::Deserialize;

use crate::provider::{Kind, Place, Provider};
use crate::wire::{explain, unsent};

/// One entry from a provider's listing. `id` is what goes on the wire.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelInfo {
    pub id: String,
    /// Human-readable name (Anthropic's `display_name`, OpenRouter's `name`).
    pub display_name: Option<String>,
    /// Context window in tokens, when the endpoint reports one.
    pub window: Option<u64>,
}

impl ModelInfo {
    #[cfg(test)]
    fn bare(id: &str) -> Self {
        Self { id: id.to_string(), display_name: None, window: None }
    }

    /// The display name when the endpoint offers one, else the id made
    /// readable.
    pub fn label(&self) -> String {
        self.display_name
            .clone()
            .unwrap_or_else(|| prettify(&self.id))
    }
}

pub async fn list_models(provider: &Provider) -> Result<Vec<ModelInfo>, String> {
    if provider.needs_key {
        return Err(format!("{} needs an API key", provider.label()));
    }
    match provider.kind {
        Kind::Anthropic => list_anthropic(provider).await,
        Kind::OpenAi => list_openai(provider).await,
    }
}

/// For callers on a plain thread.
pub fn list_models_blocking(provider: &Provider) -> Result<Vec<ModelInfo>, String> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    rt.block_on(list_models(provider))
}

/// What each `(base_url, model)` asked about reported, a failure included.
static WINDOWS: Mutex<Vec<(String, String, Option<u64>)>> = Mutex::new(Vec::new());

/// The window of the provider's model on a server of the user's own, which
/// may be far smaller than the model's. Asks the listing once a process;
/// call from a plain thread.
pub fn window(provider: &Provider) -> Option<u64> {
    if provider.place() == Place::Internet {
        return None;
    }
    let known = |windows: &[(String, String, Option<u64>)]| {
        windows
            .iter()
            .find(|(url, model, _)| *url == provider.base_url && *model == provider.model)
            .map(|(_, _, window)| *window)
    };
    if let Some(window) = known(&WINDOWS.lock().unwrap()) {
        return window;
    }
    let window = list_models_blocking(provider)
        .ok()
        .and_then(|models| models.into_iter().find(|m| m.id == provider.model)?.window);
    let model = (provider.base_url.clone(), provider.model.clone(), window);
    WINDOWS.lock().unwrap().push(model);
    window
}

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())
}

/// `GET /models` on the OpenAI shape, which most hosts speak.
async fn list_openai(provider: &Provider) -> Result<Vec<ModelInfo>, String> {
    /// Together returns a bare array where everyone else wraps in `data`.
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Listing {
        Wrapped { data: Vec<Entry> },
        Bare(Vec<Entry>),
    }
    #[derive(Deserialize)]
    struct Entry {
        id: String,
        /// OpenRouter's `name`, Together's `display_name`; bare OpenAI-shape
        /// hosts send neither.
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
        /// llama.cpp: `n_ctx` is the window the server was started with.
        #[serde(default)]
        meta: Option<Meta>,
        /// Unix seconds; hosts that don't say sort last, in their own order.
        #[serde(default)]
        created: i64,
    }

    #[derive(Deserialize)]
    struct Meta {
        #[serde(default)]
        n_ctx: Option<u64>,
    }

    let url = format!("{}/models", provider.base_url.trim_end_matches('/'));
    let mut request = client()?.get(&url);
    if let Some(key) = &provider.api_key {
        request = request.bearer_auth(key);
    }
    let resp = request.send().await.map_err(unsent)?;
    if !resp.status().is_success() {
        return Err(explain(resp.status(), &resp.text().await.unwrap_or_default()));
    }
    // Something else answers here: a web page, another API.
    let listing: Listing = resp
        .json()
        .await
        .map_err(|_| format!("no model list at {url}"))?;
    let mut entries = match listing {
        Listing::Wrapped { data } => data,
        Listing::Bare(entries) => entries,
    };
    entries.sort_by_key(|m| std::cmp::Reverse(m.created));
    let mut models: Vec<ModelInfo> = entries
        .into_iter()
        // Speech, embedding, and image models can't run here; hide them. The
        // provider file still runs any id typed into it.
        .filter(|m| {
            m.kind
                .as_deref()
                .is_none_or(|k| matches!(k, "chat" | "language" | "code"))
                && is_chat_id(&m.id)
        })
        .map(|m| ModelInfo {
            id: m.id,
            // Some hosts echo the id as the name ("Qwen/Qwen3.8-27B"); a
            // name worth showing has no org prefix.
            display_name: m
                .display_name
                .or(m.name)
                .filter(|n| !n.is_empty() && (n.contains(' ') || !n.contains('/'))),
            window: m
                .context_length
                .or(m.max_context_length)
                .or(m.meta.and_then(|meta| meta.n_ctx)),
        })
        .collect();
    // These first-party hosts list every generation and dated snapshot with
    // the version inline; prune to each family's newest. Google namespaces
    // ids under "models/" (the wire accepts bare names).
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

/// `GET /models` on Anthropic's own API, which sorts newest first and names
/// every model.
async fn list_anthropic(provider: &Provider) -> Result<Vec<ModelInfo>, String> {
    #[derive(Deserialize)]
    struct Listing {
        #[serde(default)]
        data: Vec<Entry>,
    }
    #[derive(Deserialize)]
    struct Entry {
        id: String,
        #[serde(default)]
        display_name: Option<String>,
        #[serde(default)]
        max_input_tokens: Option<u64>,
    }

    let resp = client()?
        .get(format!("{}/models", provider.base_url.trim_end_matches('/')))
        .header("x-api-key", provider.api_key.clone().unwrap_or_default())
        .header("anthropic-version", "2023-06-01")
        .send()
        .await
        .map_err(unsent)?;
    if !resp.status().is_success() {
        return Err(explain(resp.status(), &resp.text().await.unwrap_or_default()));
    }
    let listing: Listing = resp.json().await.map_err(|e| e.to_string())?;
    Ok(latest_per_family(
        listing
            .data
            .into_iter()
            .map(|m| ModelInfo {
                id: m.id,
                display_name: m.display_name.filter(|n| !n.is_empty()),
                window: m.max_input_tokens,
            })
            .collect(),
    ))
}

/// No token naming a non-chat product (speech, live audio, embeddings,
/// image, video and music generation, moderation, legacy completions,
/// research and computer-use agents). Best-effort: a missed token shows an
/// extra row; a false positive is recoverable through the provider file.
pub fn is_chat_id(id: &str) -> bool {
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
        "instruct",
        "live",
        "translate",
        "sora",
        "video",
        "imagine",
        "banana",
        "lyria",
        "robotics",
        "research",
        "antigravity",
        "computer",
        "orpheus",
        "safeguard",
    ];
    !id.split(['-', '/', ':', '.', '_'])
        .any(|token| NON_CHAT.contains(&token.to_ascii_lowercase().as_str()))
}

/// A readable name from an id: "gpt-5.5" is "GPT 5.5", "openai/gpt-oss-120b"
/// is "GPT OSS 120B", "claude-opus-5-5-20260401" is "Claude Opus 5.5".
/// Acronyms are cosmetic: an unlisted one reads "Gpt", never wrong.
pub fn prettify(id: &str) -> String {
    const ACRONYMS: &[&str] = &["gpt", "oss", "tts", "ai", "api", "it", "vl", "gguf", "mlx"];
    let bare = id.rsplit('/').next().unwrap_or(id);
    // A tag of `latest` on a local model says nothing.
    let bare = bare.strip_suffix(":latest").unwrap_or(bare);
    let digits = |t: &str| t.chars().all(|c| c.is_ascii_digit());
    let mut words: Vec<String> = Vec::new();
    // "non-reasoning" and "multi-agent" keep their hyphen.
    let mut hyphenate = false;
    for token in strip_snapshot_date(bare).split(['-', ':', '_']) {
        if token.is_empty() {
            continue;
        }
        // A month-and-day stamp inside a name ("grok-4.20-0309-reasoning").
        if !words.is_empty() && is_month_day(token) {
            continue;
        }
        // A one- or two-digit token after a number is its minor version.
        if token.len() <= 2 && digits(token) {
            if let Some(last) = words
                .last_mut()
                .filter(|w| w.chars().all(|c| c.is_ascii_digit() || c == '.'))
            {
                last.push('.');
                last.push_str(token);
                continue;
            }
        }
        let lower = token.to_ascii_lowercase();
        let (head, tail) = lower.split_at(lower.len().min(1));
        // A letter and a number: "R1", "E2". OpenAI's o-series stays "o3".
        let short_code = lower.len() <= 3
            && head.chars().all(|c| c.is_ascii_alphabetic())
            && !tail.is_empty()
            && digits(tail);
        let word = if short_code && head == "o" {
            lower.clone()
        } else if ACRONYMS.contains(&lower.as_str()) || is_size(&lower) || short_code {
            token.to_uppercase()
        } else {
            capitalize(token)
        };
        match words.last_mut().filter(|_| hyphenate) {
            Some(last) => {
                last.push('-');
                last.push_str(&word);
            }
            None => words.push(word),
        }
        hyphenate = matches!(lower.as_str(), "non" | "multi");
    }
    words.join(" ")
}

/// Drops a trailing "-YYYYMMDD" or "-YYYY-MM-DD", which names a snapshot
/// rather than a version. Shorter numbers stay: they are versions.
fn strip_snapshot_date(id: &str) -> &str {
    let digits = |t: &str, n: usize| t.len() == n && t.chars().all(|c| c.is_ascii_digit());
    let tokens: Vec<&str> = id.split('-').collect();
    let cut = match tokens.as_slice() {
        [.., y, m, d] if digits(y, 4) && digits(m, 2) && digits(d, 2) => 11,
        [.., ymd] if digits(ymd, 8) => 9,
        _ => 0,
    };
    if cut < id.len() { &id[..id.len() - cut] } else { id }
}

/// Four digits that read as a month and a day.
fn is_month_day(token: &str) -> bool {
    token.len() == 4
        && token.chars().all(|c| c.is_ascii_digit())
        && (1..=12).contains(&token[..2].parse::<u8>().unwrap_or(0))
        && (1..=31).contains(&token[2..].parse::<u8>().unwrap_or(0))
}

/// A parameter count or context size: "120b", "e2b", "270m", "16k".
fn is_size(token: &str) -> bool {
    let Some(body) = token.strip_suffix(['b', 'm', 'k']) else { return false };
    let digits = body
        .strip_prefix(|c: char| c.is_ascii_alphabetic())
        .unwrap_or(body);
    !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit() || c == '.')
}

fn capitalize(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// Trailing date-stamp tokens ("-2024-08-06", "-20241022", "-0125"): all
/// digits in date-shaped widths, dropped so a snapshot's date can't pose as
/// a version.
fn strip_date_suffix(id: &str) -> &str {
    let mut end = id.len();
    for token in id.split('-').rev() {
        let date_shaped =
            matches!(token.len(), 2 | 4 | 6 | 8) && token.chars().all(|c| c.is_ascii_digit());
        if !date_shaped || end == token.len() {
            break;
        }
        end -= token.len() + 1;
    }
    &id[..end]
}

/// One row per family, keeping the highest version. A family is the id's
/// non-numeric tokens, its version the first numeric token ("gemini-3.5-flash"
/// is gemini-flash at 3.5). Rolling aliases ("gemini-flash-latest") are their
/// own family. A version tie prefers the shorter id, so "gpt-5.5" beats its
/// dated snapshot.
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

/// One row per family from a newest-first listing: the first id seen for
/// each set of non-numeric tokens is the family's latest.
fn latest_per_family(models: Vec<ModelInfo>) -> Vec<ModelInfo> {
    let mut seen = std::collections::HashSet::new();
    models
        .into_iter()
        .filter(|m| {
            let family: Vec<&str> =
                m.id.split('-')
                    .filter(|t| t.chars().any(|c| c.is_alphabetic()))
                    .collect();
            seen.insert(family.join("-"))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An address that answers, but not with a model list: a web page, say.
    #[test]
    fn an_answer_that_is_no_listing_says_so() {
        const PAGE: &str =
            "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nConnection: close\r\n\r\n<html></html>";
        let base_url = format!("{}/v1", crate::mock::serve_once(PAGE));
        let provider = Provider {
            name: "own".into(),
            display_name: None,
            kind: Kind::OpenAi,
            base_url: base_url.clone(),
            api_key: None,
            needs_key: false,
            model: String::new(),
            effort: None,
        };
        assert_eq!(
            list_models_blocking(&provider).unwrap_err(),
            format!("no model list at {base_url}/models")
        );
    }

    /// Nothing listening at the address: the error names the address, not
    /// reqwest's "error sending request".
    #[test]
    fn a_server_that_is_not_there_says_so() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        let provider = Provider {
            name: "own".into(),
            display_name: None,
            kind: Kind::OpenAi,
            base_url: format!("http://127.0.0.1:{port}/v1"),
            api_key: None,
            needs_key: false,
            model: String::new(),
            effort: None,
        };
        assert_eq!(
            list_models_blocking(&provider).unwrap_err(),
            format!("can't reach 127.0.0.1:{port}")
        );
    }

    /// llama.cpp's listing, asked once: the mock answers one connection.
    #[test]
    fn a_local_servers_window_is_asked_for_once() {
        let body = r#"{"object":"list","data":[{"id":"small","created":1,"meta":{"n_ctx":8192,"n_ctx_train":524288}}]}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let provider = Provider {
            name: "own".into(),
            display_name: None,
            kind: Kind::OpenAi,
            base_url: crate::mock::serve_once(&response),
            api_key: None,
            needs_key: false,
            model: "small".into(),
            effort: None,
        };
        assert_eq!(window(&provider), Some(8192));
        assert_eq!(window(&provider), Some(8192));
        let elsewhere = Provider { base_url: "https://api.example.com/v1".into(), ..provider };
        assert_eq!(window(&elsewhere), None);
    }

    fn ids(models: Vec<ModelInfo>) -> Vec<String> {
        models.into_iter().map(|m| m.id).collect()
    }

    #[test]
    fn prettified_ids() {
        for (id, label) in [
            ("gpt-5.5", "GPT 5.5"),
            ("openai/gpt-oss-120b", "GPT OSS 120B"),
            ("grok-4.3-fast", "Grok 4.3 Fast"),
            ("o4-mini", "o4 Mini"),
            ("o3", "o3"),
            ("o1-pro", "o1 Pro"),
            ("grok-4.20-0309-non-reasoning", "Grok 4.20 Non-Reasoning"),
            ("grok-4.20-multi-agent-0309", "Grok 4.20 Multi-Agent"),
            ("0309", "0309"),
            ("llama3", "Llama3"),
            ("claude-opus-5-5-20260401", "Claude Opus 5.5"),
            ("claude-sonnet-4-5", "Claude Sonnet 4.5"),
            ("gemini-2.5-pro", "Gemini 2.5 Pro"),
            ("qwen3:8b", "Qwen3 8B"),
            ("deepseek-r1", "Deepseek R1"),
            ("vendor/model-10", "Model 10"),
            ("claude-sonnet-4-10", "Claude Sonnet 4.10"),
            ("gpt-4-1106-preview", "GPT 4 Preview"),
            ("phi-4-2048", "Phi 4 2048"),
            ("gpt-5.5-2026-04-23", "GPT 5.5"),
            ("20260401", "20260401"),
            ("gemma4:e2b", "Gemma4 E2B"),
            ("gemma3:270m", "Gemma3 270M"),
            ("gpt-3.5-turbo-16k", "GPT 3.5 Turbo 16K"),
            ("gpt-5-search-api", "GPT 5 Search API"),
            ("gemma-4-31b-it", "Gemma 4 31B IT"),
            ("mistral-small:latest", "Mistral Small"),
            ("gemini-flash-latest", "Gemini Flash Latest"),
            ("Qwen/Qwen3.8-27B", "Qwen3.8 27B"),
        ] {
            assert_eq!(prettify(id), label, "{id}");
        }
    }

    #[test]
    fn non_chat_products_are_hidden() {
        for id in
            ["gpt-5.5", "claude-opus-5-5", "chat-latest", "gemini-3.5-flash-lite", "gpt-5.3-codex"]
        {
            assert!(is_chat_id(id), "{id}");
        }
        for id in [
            "whisper-1",
            "text-embedding-3-small",
            "dall-e-3",
            "gpt-4o-realtime-preview",
            "sora-2-pro",
            "gpt-live-1",
            "gpt-3.5-turbo-instruct",
            "grok-imagine-video-1.5",
            "nano-banana-pro-preview",
            "lyria-3.5",
            "gemini-3.8-live",
            "gemini-robotics-er-2-preview",
            "deep-research-preview-04-2026",
            "antigravity-preview-latest",
            "gemini-2.5-computer-use-preview-10-2025",
            "gemini-3.5-live-translate-preview",
            "canopylabs/orpheus-v1-english",
            "openai/gpt-oss-safeguard-20b",
        ] {
            assert!(!is_chat_id(id), "{id}");
        }
    }

    /// A listing over the wire: non-chat entries dropped, newest first,
    /// names from the endpoint where it gives them.
    #[test]
    fn a_listing_is_filtered_named_and_newest_first() {
        let body = "{\"data\":[\
            {\"id\":\"old-chat\",\"created\":100},\
            {\"id\":\"whisper-1\",\"created\":300},\
            {\"id\":\"new-chat\",\"created\":200,\"name\":\"Newest\"},\
            {\"id\":\"org/model-7b\",\"created\":50,\"name\":\"Org/Model-7B\"},\
            {\"id\":\"undated\"}]}";
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let provider = Provider {
            name: "p".into(),
            display_name: None,
            needs_key: false,
            kind: Kind::OpenAi,
            base_url: crate::mock::serve_once(&response),
            api_key: Some("k".into()),
            model: String::new(),
            effort: None,
        };
        let models = list_models_blocking(&provider).unwrap();
        let listed: Vec<(String, String)> = models
            .into_iter()
            .map(|m| (m.id.clone(), m.label()))
            .collect();
        assert_eq!(
            listed,
            [
                ("new-chat".to_string(), "Newest".to_string()),
                ("old-chat".to_string(), "Old Chat".to_string()),
                ("org/model-7b".to_string(), "Model 7B".to_string()),
                ("undated".to_string(), "Undated".to_string()),
            ]
        );
    }

    #[test]
    fn a_refused_listing_says_why() {
        let response = "HTTP/1.1 401 Unauthorized\r\nContent-Length: 41\r\nConnection: close\r\n\r\n{\"error\":{\"message\":\"Invalid API Key\"}}\n\n\n";
        let provider = Provider {
            name: "p".into(),
            display_name: None,
            needs_key: false,
            kind: Kind::OpenAi,
            base_url: crate::mock::serve_once(response),
            api_key: Some("k".into()),
            model: String::new(),
            effort: None,
        };
        assert_eq!(
            list_models_blocking(&provider).unwrap_err(),
            "401 Unauthorized: Invalid API Key"
        );
    }

    /// A version tie (alias vs dated snapshot) keeps the shorter id; a lone
    /// dated id survives as its family's only member.
    #[test]
    fn alias_beats_dated_snapshot() {
        let models =
            ["gpt-5.5-2026-04-23", "gpt-5.5", "gpt-4o-2024-08-06", "gpt-4o", "o1-20241217"]
                .into_iter()
                .map(ModelInfo::bare)
                .collect();
        assert_eq!(ids(newest_per_family(models)), ["gpt-5.5", "gpt-4o", "o1-20241217"]);
    }

    /// Versions live mid-id and old generations coexist with snapshots and
    /// "-latest" aliases; each family keeps its highest.
    #[test]
    fn newest_per_family_by_version() {
        let models = [
            "gemini-2.0-flash-001",
            "gemini-2.5-flash",
            "gemini-3.5-flash",
            "gemini-flash-latest",
            "gemini-2.5-pro",
            "text-embedding-004",
            "aqa",
        ]
        .into_iter()
        .map(ModelInfo::bare)
        .collect();
        assert_eq!(
            ids(newest_per_family(models)),
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
    fn a_newest_first_listing_keeps_the_first_of_each_family() {
        let models = ["claude-opus-5-5-20260401", "claude-opus-5-20251001", "claude-sonnet-4-5"]
            .into_iter()
            .map(ModelInfo::bare)
            .collect();
        assert_eq!(
            ids(latest_per_family(models)),
            ["claude-opus-5-5-20260401", "claude-sonnet-4-5"]
        );
    }
}
