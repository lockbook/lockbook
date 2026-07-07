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

use super::backend::{ChatMsg, Completion, CompletionReq, ModelInfo, ToolCall, parse_tool_args};
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
    effort: Option<String>,
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
        effort: provider.effort.clone(),
    }
}

/// One SSE `data:` payload — the fields we read from the event union.
#[derive(Deserialize)]
struct Event {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    message: Option<MessageStart>,
    /// Which content block a `content_block_*` event belongs to.
    #[serde(default)]
    index: Option<usize>,
    #[serde(default)]
    content_block: Option<ContentBlockStart>,
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

/// A new content block opening. Tool-use blocks carry the call's id and
/// name here; their arguments follow as `input_json_delta` fragments.
#[derive(Deserialize)]
struct ContentBlockStart {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    name: Option<String>,
}

#[derive(Deserialize)]
struct Delta {
    #[serde(default)]
    text: Option<String>,
    /// A fragment of a tool-use block's argument JSON.
    #[serde(default)]
    partial_json: Option<String>,
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
    ) -> Result<Completion, String> {
        let messages: Vec<_> = req
            .messages
            .iter()
            .map(|msg| match msg {
                ChatMsg::User(text) => json!({ "role": "user", "content": text }),
                ChatMsg::Assistant { text, tool_calls } if !tool_calls.is_empty() => {
                    // A call turn is a content-block array; an empty text
                    // block is invalid on this wire, so it's included only
                    // when the model actually said something.
                    let mut blocks = Vec::new();
                    if !text.is_empty() {
                        blocks.push(json!({ "type": "text", "text": text }));
                    }
                    for c in tool_calls {
                        blocks.push(json!({
                            "type": "tool_use", "id": c.id, "name": c.name, "input": c.args,
                        }));
                    }
                    json!({ "role": "assistant", "content": blocks })
                }
                ChatMsg::Assistant { text, .. } => {
                    json!({ "role": "assistant", "content": text })
                }
                ChatMsg::ToolResult { id, content, is_error } => {
                    let mut block = json!({
                        "type": "tool_result", "tool_use_id": id, "content": content,
                    });
                    if *is_error {
                        block["is_error"] = json!(true);
                    }
                    json!({ "role": "user", "content": [block] })
                }
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
        if !req.tools.is_empty() {
            body["tools"] = req
                .tools
                .iter()
                .map(|t| {
                    json!({
                        "name": t.name,
                        "description": t.description,
                        "input_schema": t.parameters,
                        "strict": true,
                    })
                })
                .collect::<Vec<_>>()
                .into();
            // One call at a time — matches the harness's single-decision
            // approval channel.
            body["tool_choice"] = json!({ "type": "auto", "disable_parallel_tool_use": true });
        }
        // Anthropic controls reasoning depth through effort on adaptive
        // thinking (Opus 4.6+), not `reasoning_effort`. Values: low / medium
        // / high / xhigh / max.
        if let Some(effort) = &self.effort {
            body["thinking"] = json!({ "type": "adaptive" });
            body["output_config"] = json!({ "effort": effort });
        }

        // Rate limits (429) and transient server errors (5xx) back off and
        // retry a couple of times before surfacing. Always pre-stream, so a
        // retry can't duplicate delivered text.
        let mut attempts: u32 = 0;
        // `strict: true` needs a model that supports it (and a conforming
        // schema). A model that rejects the key 400s naming it — strip it
        // from every tool and re-send once, the way the OpenAI adapter falls
        // back to `plain_tools`.
        let mut strict_stripped = false;
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
            let error_body = resp.text().await.unwrap_or_default();
            if !strict_stripped
                && !req.tools.is_empty()
                && status.as_u16() == 400
                && error_body.contains("strict")
            {
                strict_stripped = true;
                if let Some(tools) = body["tools"].as_array_mut() {
                    for tool in tools {
                        if let Some(obj) = tool.as_object_mut() {
                            obj.remove("strict");
                        }
                    }
                }
                continue;
            }
            return Err(format!("{status}: {}", error_body.chars().take(500).collect::<String>()));
        };

        // SSE: `message_start` carries input/cache usage, `content_block_delta`
        // the text, `message_delta` the output count, `message_stop` the end.
        // Tool calls open as `content_block_start` (id + name) and accumulate
        // argument JSON through `input_json_delta` fragments keyed by block
        // index. The buffer stays raw bytes, decoded per complete line —
        // network chunks split mid-character, and decoding a chunk at a time
        // would corrupt multi-byte glyphs into � (and persist them).
        let mut usage = Usage::default();
        // Tool-use blocks in progress, keyed by block index: (id, name, args).
        let mut blocks: std::collections::BTreeMap<usize, (String, String, String)> =
            Default::default();
        let finish = |blocks: std::collections::BTreeMap<usize, (String, String, String)>,
                      usage: Usage| {
            let tool_calls = blocks
                .into_values()
                .map(|(id, name, args)| ToolCall { id, name, args: parse_tool_args(&args) })
                .collect();
            Completion { tool_calls, usage }
        };
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
                    "content_block_start" => {
                        if let Some(block) = event.content_block {
                            if block.kind == "tool_use" {
                                blocks.insert(
                                    event.index.unwrap_or(0),
                                    (
                                        block.id.unwrap_or_default(),
                                        block.name.unwrap_or_default(),
                                        String::new(),
                                    ),
                                );
                            }
                        }
                    }
                    "content_block_delta" => {
                        if let Some(delta) = event.delta {
                            if let Some(text) = delta.text {
                                if !text.is_empty() {
                                    let _ = deltas.send(text);
                                }
                            }
                            if let Some(fragment) = delta.partial_json {
                                if let Some((_, _, args)) =
                                    event.index.and_then(|i| blocks.get_mut(&i))
                                {
                                    args.push_str(&fragment);
                                }
                            }
                        }
                    }
                    "message_delta" => {
                        if let Some(u) = event.usage {
                            usage.output = u.output_tokens.unwrap_or(usage.output);
                        }
                    }
                    "message_stop" => return Ok(finish(blocks, usage)),
                    "error" => {
                        let message = event.error.map(|e| e.message).unwrap_or_default();
                        return Err(format!("provider error: {message}"));
                    }
                    _ => {}
                }
            }
        }
        Ok(finish(blocks, usage))
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

    fn complete(base_url: &str) -> (Result<Completion, String>, Vec<String>) {
        let req = CompletionReq {
            system: "system".into(),
            messages: vec![ChatMsg::User("hi".into())],
            max_tokens: 100,
            tools: vec![],
        };
        complete_req(base_url, req)
    }

    fn complete_req(
        base_url: &str, req: CompletionReq,
    ) -> (Result<Completion, String>, Vec<String>) {
        let backend = anthropic(&Provider {
            name: "anthropic".into(),
            display_name: None,
            kind: "anthropic".into(),
            base_url: base_url.into(),
            model: "claude-opus-4-8".into(),
            api_key: Some("test-key".into()),
            effort: None,
        });
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

    /// Effort maps to adaptive thinking + `output_config.effort`, not the
    /// OpenAI-compat `reasoning_effort` key.
    #[test]
    fn effort_maps_to_output_config() {
        let (base, rx) = super::super::openai::mock::serve_capturing(SSE_ANTHROPIC);
        let backend = anthropic(&Provider {
            name: "anthropic".into(),
            display_name: None,
            kind: "anthropic".into(),
            base_url: base,
            model: "claude-opus-4-8".into(),
            api_key: Some("k".into()),
            effort: Some("high".into()),
        });
        let req = CompletionReq {
            system: "s".into(),
            messages: vec![ChatMsg::User("hi".into())],
            max_tokens: 100,
            tools: vec![],
        };
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(backend.complete(req, tx))
            .unwrap();
        let body = rx.recv().unwrap();
        assert!(body.contains("\"output_config\":{\"effort\":\"high\"}"), "{body}");
        assert!(body.contains("\"thinking\":{\"type\":\"adaptive\"}"), "{body}");
    }

    #[test]
    fn streams_deltas_and_usage() {
        let base = serve_once(SSE_ANTHROPIC);
        let (result, deltas) = complete(&base);
        let completion = result.unwrap();
        assert_eq!(deltas.join(""), "Hello");
        assert!(completion.tool_calls.is_empty());
        let usage = completion.usage;
        assert_eq!((usage.input, usage.output, usage.cache_read, usage.cache_write), (10, 2, 6, 4));
    }

    /// Tools ride the wire in the native shape (`input_schema`, `strict`)
    /// with parallel calls disabled; call turns re-serialize as content-block
    /// arrays and results as `tool_result` user turns, with `is_error` only
    /// when set.
    #[test]
    fn tools_and_call_turns_ride_the_wire() {
        let (base, body_rx) = super::super::openai::mock::serve_capturing(SSE_ANTHROPIC);
        let req = CompletionReq {
            system: "s".into(),
            messages: vec![
                ChatMsg::User("add milk".into()),
                ChatMsg::Assistant {
                    text: String::new(),
                    tool_calls: vec![ToolCall {
                        id: "toolu_1".into(),
                        name: "read_note".into(),
                        args: json!({ "path": "/todo.md" }),
                    }],
                },
                ChatMsg::ToolResult {
                    id: "toolu_1".into(),
                    content: "error: not found".into(),
                    is_error: true,
                },
            ],
            max_tokens: 100,
            tools: vec![super::super::backend::ToolSchema {
                name: "read_note",
                description: "Read a note.",
                parameters: json!({
                    "type": "object",
                    "properties": { "path": { "type": "string" } },
                    "required": ["path"],
                    "additionalProperties": false,
                }),
            }],
        };
        let (result, _) = complete_req(&base, req);
        assert!(result.is_ok(), "{result:?}");
        let body: serde_json::Value = serde_json::from_str(&body_rx.recv().unwrap()).unwrap();
        assert_eq!(body["tools"][0]["name"], "read_note");
        assert!(body["tools"][0].get("input_schema").is_some());
        assert_eq!(body["tools"][0]["strict"], true);
        assert_eq!(body["tool_choice"]["type"], "auto");
        assert_eq!(body["tool_choice"]["disable_parallel_tool_use"], true);
        // Call turn: no empty text block, one tool_use block with the parsed
        // input object (not a string).
        let call_turn = &body["messages"][1]["content"];
        assert_eq!(call_turn.as_array().unwrap().len(), 1);
        assert_eq!(call_turn[0]["type"], "tool_use");
        assert_eq!(call_turn[0]["id"], "toolu_1");
        assert_eq!(call_turn[0]["input"]["path"], "/todo.md");
        // Result turn: a user message wrapping one tool_result block.
        let result_turn = &body["messages"][2];
        assert_eq!(result_turn["role"], "user");
        assert_eq!(result_turn["content"][0]["type"], "tool_result");
        assert_eq!(result_turn["content"][0]["tool_use_id"], "toolu_1");
        assert_eq!(result_turn["content"][0]["is_error"], true);
    }

    /// A tool-use block opens with id + name, accumulates argument JSON
    /// through `input_json_delta` fragments, and lands as a parsed call at
    /// message_stop — while text deltas keep streaming.
    #[test]
    fn tool_use_accumulates_across_fragments() {
        const SSE_TOOL: &str = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n\
             data: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":10}}}\n\n\
             data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n\
             data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Checking.\"}}\n\n\
             data: {\"type\":\"content_block_start\",\"index\":1,\"content_block\":{\"type\":\"tool_use\",\"id\":\"toolu_9\",\"name\":\"read_note\",\"input\":{}}}\n\n\
             data: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"path\\\":\"}}\n\n\
             data: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"\\\"/todo.md\\\"}\"}}\n\n\
             data: {\"type\":\"content_block_stop\",\"index\":1}\n\n\
             data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"tool_use\"},\"usage\":{\"output_tokens\":7}}\n\n\
             data: {\"type\":\"message_stop\"}\n\n";
        // Split inside the second argument fragment.
        let split_at = SSE_TOOL.find("/todo.md").unwrap() + 3;
        let base = super::super::openai::mock::serve_split(SSE_TOOL, split_at);
        let (result, deltas) = complete(&base);
        let completion = result.unwrap();
        assert_eq!(deltas.join(""), "Checking.");
        assert_eq!(
            completion.tool_calls,
            vec![ToolCall {
                id: "toolu_9".into(),
                name: "read_note".into(),
                args: json!({ "path": "/todo.md" }),
            }]
        );
        assert_eq!(completion.usage.output, 7);
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
