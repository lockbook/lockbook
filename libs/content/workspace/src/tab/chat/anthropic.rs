//! Chat backend over Anthropic's native Messages API, streaming via SSE.
//! Native (rather than the OpenAI-compat layer) for prompt caching: the
//! request opts into Anthropic's top-level automatic caching, which places
//! the cache breakpoint on the transcript's tail so each turn reads the
//! previous turn's prefix.

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

pub struct AnthropicBackend {
    client: reqwest::Client,
    base_url: String,
    api_key: String,
    model: String,
}

pub fn anthropic(provider: &Provider) -> AnthropicBackend {
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(10))
        .build()
        .expect("reqwest client");
    AnthropicBackend {
        client,
        base_url: provider.base_url.trim_end_matches('/').to_string(),
        api_key: provider.api_key.clone().unwrap_or_default(),
        model: provider.model.clone(),
    }
}

/// One SSE `data:` payload — the fields we read from the event union.
#[derive(Deserialize)]
struct Event {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    message: Option<MessageStart>,
    #[serde(default)]
    delta: Option<Delta>,
    #[serde(default)]
    usage: Option<WireUsage>,
    #[serde(default)]
    error: Option<WireError>,
}

#[derive(Deserialize)]
struct MessageStart {
    #[serde(default)]
    usage: Option<WireUsage>,
}

#[derive(Deserialize)]
struct Delta {
    #[serde(default)]
    text: Option<String>,
}

#[derive(Deserialize)]
struct WireUsage {
    #[serde(default)]
    input_tokens: Option<u64>,
    #[serde(default)]
    output_tokens: Option<u64>,
    #[serde(default)]
    cache_creation_input_tokens: Option<u64>,
    #[serde(default)]
    cache_read_input_tokens: Option<u64>,
}

#[derive(Deserialize)]
struct WireError {
    #[serde(default)]
    message: String,
}

impl AnthropicBackend {
    pub(super) async fn complete(
        &self, req: CompletionReq, deltas: UnboundedSender<String>,
    ) -> Result<Usage, String> {
        let messages: Vec<_> = req
            .messages
            .iter()
            .map(|msg| match msg {
                ChatMsg::User(text) => json!({ "role": "user", "content": text }),
                ChatMsg::Assistant(text) => json!({ "role": "assistant", "content": text }),
            })
            .collect();

        let mut body = json!({
            "model": self.model,
            "max_tokens": req.max_tokens,
            "messages": messages,
            "stream": true,
            // Automatic caching: the API places the breakpoint on the last
            // cacheable block, so a growing transcript accrues cache hits
            // turn over turn.
            "cache_control": { "type": "ephemeral" },
        });
        if !req.system.is_empty() {
            body["system"] = json!(req.system);
        }

        // Rate limits (429) and transient server errors (5xx) back off and
        // retry a couple of times before surfacing. Always pre-stream, so a
        // retry can't duplicate delivered text.
        let mut attempts: u32 = 0;
        let resp = loop {
            attempts += 1;
            let resp = self
                .client
                .post(format!("{}/messages", self.base_url))
                .header("x-api-key", &self.api_key)
                .header("anthropic-version", "2023-06-01")
                .json(&body)
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
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("{status}: {}", body.chars().take(500).collect::<String>()));
        };

        // SSE: `message_start` carries input/cache usage, `content_block_delta`
        // the text, `message_delta` the output count, `message_stop` the end.
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
                let Ok(event) = serde_json::from_str::<Event>(payload) else { continue };
                match event.kind.as_str() {
                    "message_start" => {
                        if let Some(u) = event.message.and_then(|m| m.usage) {
                            usage.input = u.input_tokens.unwrap_or(0);
                            usage.cache_read = u.cache_read_input_tokens.unwrap_or(0);
                            usage.cache_write = u.cache_creation_input_tokens.unwrap_or(0);
                        }
                    }
                    "content_block_delta" => {
                        if let Some(text) = event.delta.and_then(|d| d.text) {
                            if !text.is_empty() {
                                let _ = deltas.send(text);
                            }
                        }
                    }
                    "message_delta" => {
                        if let Some(u) = event.usage {
                            usage.output = u.output_tokens.unwrap_or(usage.output);
                        }
                    }
                    "message_stop" => return Ok(usage),
                    "error" => {
                        let message = event.error.map(|e| e.message).unwrap_or_default();
                        return Err(format!("provider error: {message}"));
                    }
                    _ => {}
                }
            }
        }
        Ok(usage)
    }
}

/// `GET /models` on the native API (needs Anthropic's own auth headers),
/// blocking — for the picker's fetch thread. The listing reports each
/// model's context window (`max_input_tokens`), which feeds the usage ring.
pub fn list_models_blocking(provider: &Provider) -> Result<Vec<ModelInfo>, String> {
    #[derive(Deserialize)]
    struct Listing {
        #[serde(default)]
        data: Vec<ModelEntry>,
    }
    #[derive(Deserialize)]
    struct ModelEntry {
        id: String,
        #[serde(default)]
        display_name: Option<String>,
        #[serde(default)]
        max_input_tokens: Option<u64>,
    }

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client
        .get(format!("{}/models", provider.base_url.trim_end_matches('/')))
        .header("x-api-key", provider.api_key.clone().unwrap_or_default())
        .header("anthropic-version", "2023-06-01")
        .send()
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(resp.status().to_string());
    }
    let listing: Listing = resp.json().map_err(|e| e.to_string())?;
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

/// One row per model family (Opus, Sonnet, …) rather than every dated
/// snapshot. The family key is the id minus its numeric tokens (versions,
/// date stamps), and the API sorts by release date newest-first, so the
/// first id seen per key is the family's latest.
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
    use super::super::openai::mock::serve_once;
    use super::*;

    const SSE_ANTHROPIC: &str = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n\
         event: message_start\n\
         data: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":10,\"cache_read_input_tokens\":6,\"cache_creation_input_tokens\":4}}}\n\n\
         event: content_block_delta\n\
         data: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"Hel\"}}\n\n\
         data: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"lo\"}}\n\n\
         data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":2}}\n\n\
         data: {\"type\":\"message_stop\"}\n\n";

    fn complete(base_url: &str) -> (Result<Usage, String>, Vec<String>) {
        let backend = anthropic(&Provider {
            name: "anthropic".into(),
            display_name: None,
            kind: "anthropic".into(),
            base_url: base_url.into(),
            model: "claude-opus-4-8".into(),
            api_key: Some("test-key".into()),
        });
        let req = CompletionReq {
            system: "system".into(),
            messages: vec![ChatMsg::User("hi".into())],
            max_tokens: 100,
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

    /// Version tokens sit in different spots across generations
    /// (claude-3-7-sonnet vs claude-sonnet-5) and some ids carry date
    /// stamps; all snapshots of a family collapse to the newest (first).
    #[test]
    fn one_model_per_family() {
        let ids = [
            "claude-opus-4-8",
            "claude-opus-4-7",
            "claude-sonnet-5",
            "claude-haiku-4-5-20251001",
            "claude-sonnet-4-6",
            "claude-3-7-sonnet-20250219",
            "claude-3-5-haiku-20241022",
        ];
        let models = ids
            .into_iter()
            .map(|id| ModelInfo { id: id.into(), display_name: None, window: None })
            .collect();
        let kept: Vec<String> = latest_per_family(models)
            .into_iter()
            .map(|m| m.id)
            .collect();
        assert_eq!(kept, ["claude-opus-4-8", "claude-sonnet-5", "claude-haiku-4-5-20251001"]);
    }

    #[test]
    fn streams_deltas_and_usage() {
        let base = serve_once(SSE_ANTHROPIC);
        let (result, deltas) = complete(&base);
        let usage = result.unwrap();
        assert_eq!(deltas.join(""), "Hello");
        assert_eq!((usage.input, usage.output, usage.cache_read, usage.cache_write), (10, 2, 6, 4));
    }

    /// Same guarantee as the OpenAI backend: a chunk boundary mid-character
    /// doesn't corrupt the delta.
    #[test]
    fn utf8_survives_chunk_split_mid_character() {
        const SSE_CRAB: &str = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n\
             data: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"a\u{1F980}b\"}}\n\n\
             data: {\"type\":\"message_stop\"}\n\n";
        let crab_start = SSE_CRAB.find('\u{1F980}').unwrap();
        let base = super::super::openai::mock::serve_split(SSE_CRAB, crab_start + 2);
        let (result, deltas) = complete(&base);
        assert!(result.is_ok(), "{result:?}");
        assert_eq!(deltas.join(""), "a\u{1F980}b");
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
        assert!(deltas.is_empty());
    }
}
