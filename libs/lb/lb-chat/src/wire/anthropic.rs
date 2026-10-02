//! Anthropic messages, native: prompt caching over the whole conversation,
//! structured tool use, and thinking that is handed back as it came.

use std::collections::BTreeMap;

use futures::StreamExt;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::mpsc::UnboundedSender;

use super::{Call, Completion, Echo, Request, STREAM_IDLE, Sse, Turn, parse_args, send};
use crate::provider::Provider;

const MAX_TOKENS: u32 = 16_384;
const SMALL_MAX_TOKENS: u32 = 4096;
/// Thinking counts against the output cap, so a request that asks for it
/// gets more room. Every model that thinks allows this much.
const THINKING_MAX_TOKENS: u32 = 64_000;
/// What a model that thinks against a budget is given to think with.
const THINKING_BUDGET: u32 = 4096;
/// Sent with a thinking request: thinking between tool calls where a budget
/// model can, and leave to drop a block instead of refusing the request.
const THINKING_BETAS: &str = "interleaved-thinking-2025-05-14,thinking-binding-controls-2026-08-01";

#[derive(Deserialize)]
struct Event {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    index: Option<usize>,
    #[serde(default)]
    message: Option<Message>,
    #[serde(default)]
    content_block: Option<Block>,
    #[serde(default)]
    delta: Option<Delta>,
    #[serde(default)]
    usage: Option<WireUsage>,
    #[serde(default)]
    error: Option<WireError>,
}

#[derive(Deserialize)]
struct Message {
    #[serde(default)]
    usage: Option<WireUsage>,
}

#[derive(Deserialize)]
struct Block {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    name: Option<String>,
    /// A `redacted_thinking` block's sealed contents.
    #[serde(default)]
    data: Option<String>,
}

#[derive(Deserialize)]
struct Delta {
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    partial_json: Option<String>,
    #[serde(default)]
    thinking: Option<String>,
    #[serde(default)]
    signature: Option<String>,
}

/// A thinking block as it arrived and where it sat among the message's text
/// and calls. The API wants each back unmodified and in place with the tool
/// results, so the first call carries them as its echo.
struct Thought {
    after_text: bool,
    calls_before: usize,
    block: Value,
}

impl Thought {
    /// The thoughts `calls` came with, in the order they arrived.
    fn of(provider: &Provider, calls: &[Call]) -> Vec<Thought> {
        calls
            .iter()
            .filter_map(|call| call.echo.as_ref())
            .filter(|echo| echo.provider == provider.name)
            .filter_map(|echo| echo.content["thinking"].as_array())
            .flatten()
            .map(|thought| Thought {
                after_text: thought["after_text"] == true,
                calls_before: thought["calls_before"].as_u64().unwrap_or(0) as usize,
                block: thought["block"].clone(),
            })
            .collect()
    }

    fn echo(thoughts: &[Thought], provider: &Provider) -> Echo {
        let thinking: Vec<Value> = thoughts
            .iter()
            .map(|t| {
                json!({
                    "after_text": t.after_text,
                    "calls_before": t.calls_before,
                    "block": t.block,
                })
            })
            .collect();
        Echo { provider: provider.name.clone(), content: json!({ "thinking": thinking }) }
    }
}

#[derive(Deserialize)]
struct WireUsage {
    #[serde(default)]
    input_tokens: Option<u64>,
    #[serde(default)]
    output_tokens: Option<u64>,
    #[serde(default)]
    cache_read_input_tokens: Option<u64>,
    #[serde(default)]
    cache_creation_input_tokens: Option<u64>,
}

#[derive(Deserialize)]
struct WireError {
    #[serde(default)]
    message: String,
}

pub(super) fn body(provider: &Provider, req: &Request, max_tokens: u32) -> Value {
    let messages: Vec<Value> = req
        .turns
        .iter()
        .map(|turn| match turn {
            Turn::User(text) => json!({ "role": "user", "content": text }),
            Turn::Assistant { text, calls } if !calls.is_empty() => {
                // Every turn's thinking goes back: one left out would end
                // the cached prefix there.
                let thoughts = match req.effort {
                    Some(_) => Thought::of(provider, calls),
                    None => Vec::new(),
                };
                let at = |after_text: Option<bool>, calls_before: usize| {
                    thoughts
                        .iter()
                        .filter(move |t| t.calls_before == calls_before)
                        .filter(move |t| after_text.is_none_or(|after| after == t.after_text))
                        .map(|t| t.block.clone())
                };
                let mut blocks: Vec<Value> = at(Some(false), 0).collect();
                if !text.is_empty() {
                    blocks.push(json!({ "type": "text", "text": text }));
                }
                blocks.extend(at(Some(true), 0));
                for (n, c) in calls.iter().enumerate() {
                    blocks.push(json!({ "type": "tool_use", "id": c.id, "name": c.name, "input": c.args }));
                    blocks.extend(at(None, n + 1));
                }
                json!({ "role": "assistant", "content": blocks })
            }
            Turn::Assistant { text, .. } => json!({ "role": "assistant", "content": text }),
            Turn::ToolResults(results) => {
                let blocks: Vec<Value> = results
                    .iter()
                    .map(|r| {
                        let mut b = json!({ "type": "tool_result", "tool_use_id": r.id, "content": r.text });
                        if !r.ok {
                            b["is_error"] = json!(true);
                        }
                        b
                    })
                    .collect();
                json!({ "role": "user", "content": blocks })
            }
        })
        .collect();

    let mut body = json!({
        "model": provider.model,
        "max_tokens": max_tokens,
        "messages": messages,
        "stream": true,
        // The API sets the cache breakpoint on the last block, so each
        // round of a run reads everything before it from cache.
        "cache_control": { "type": "ephemeral" },
    });
    if !req.system.is_empty() {
        body["system"] = json!(req.system);
    }
    if !req.tools.is_empty() {
        body["tools"] = req
            .tools
            .iter()
            .map(|t| json!({ "name": t.name, "description": t.description, "input_schema": t.parameters }))
            .collect();
    }
    // "on" is a model that thinks against a budget, which must fit under
    // the output cap; any other value is an effort for one that paces
    // itself. A block the API can no longer place is dropped, not refused.
    let binding = json!({ "prefix_mismatch_behavior": "drop_block" });
    match req.effort.as_deref() {
        Some("on") if THINKING_BUDGET < max_tokens => {
            body["thinking"] = json!({
                "type": "enabled",
                "budget_tokens": THINKING_BUDGET,
                "block_binding": binding,
            });
        }
        Some("on") | None => {}
        Some(effort) => {
            body["thinking"] = json!({ "type": "adaptive", "block_binding": binding });
            body["output_config"] = json!({ "effort": effort });
        }
    }
    body
}

pub async fn complete(
    client: &reqwest::Client, provider: &Provider, req: &Request, deltas: &UnboundedSender<String>,
) -> Result<Completion, String> {
    let key = provider
        .api_key
        .clone()
        .ok_or("this provider needs an API key")?;
    let mut headers = vec![("x-api-key", key), ("anthropic-version", "2023-06-01".into())];
    if req.effort.is_some() {
        headers.push(("anthropic-beta", THINKING_BETAS.into()));
    }
    let url = format!("{}/messages", provider.base_url);

    let room = if req.effort.is_some() { THINKING_MAX_TOKENS } else { MAX_TOKENS };
    let resp = match send(client, &url, &headers, &body(provider, req, room)).await {
        Err(e) if e.starts_with("400") && e.contains("max_tokens") => {
            send(client, &url, &headers, &body(provider, req, SMALL_MAX_TOKENS)).await?
        }
        other => other?,
    };

    let mut out = Completion::default();
    // index → (id, name, accumulated json)
    let mut blocks: BTreeMap<usize, (String, String, String)> = BTreeMap::new();
    let mut thoughts: BTreeMap<usize, Thought> = BTreeMap::new();
    let mut after_text = false;
    let mut sse = Sse::default();
    let mut stream = resp.bytes_stream();
    loop {
        let item = tokio::time::timeout(STREAM_IDLE, stream.next())
            .await
            .map_err(|_| "provider stopped responding mid-stream".to_string())?;
        let Some(bytes) = item else { break };
        let bytes = bytes.map_err(|e| format!("stream failed: {e}"))?;
        for payload in sse.push(&bytes) {
            let Ok(event) = serde_json::from_str::<Event>(&payload) else { continue };
            match event.kind.as_str() {
                "message_start" => {
                    if let Some(u) = event.message.and_then(|m| m.usage) {
                        out.usage.input = u.input_tokens.unwrap_or(0);
                        out.usage.cache_read = u.cache_read_input_tokens.unwrap_or(0);
                        out.usage.cache_write = u.cache_creation_input_tokens.unwrap_or(0);
                    }
                }
                "content_block_start" => {
                    let Some(block) = event.content_block else { continue };
                    let index = event.index.unwrap_or(0);
                    let calls_before = blocks.len();
                    let thought = |block: Value| Thought { after_text, calls_before, block };
                    match block.kind.as_str() {
                        "text" => after_text = true,
                        "tool_use" => {
                            let call = (
                                block.id.unwrap_or_default(),
                                block.name.unwrap_or_default(),
                                String::new(),
                            );
                            blocks.insert(index, call);
                        }
                        "thinking" => {
                            let empty =
                                json!({ "type": "thinking", "thinking": "", "signature": "" });
                            thoughts.insert(index, thought(empty));
                        }
                        "redacted_thinking" => {
                            let sealed = json!({ "type": "redacted_thinking", "data": block.data });
                            thoughts.insert(index, thought(sealed));
                        }
                        _ => {}
                    }
                }
                "content_block_delta" => {
                    let Some(delta) = event.delta else { continue };
                    if let Some(text) = delta.text.filter(|t| !t.is_empty()) {
                        out.text.push_str(&text);
                        let _ = deltas.send(text);
                    }
                    if let Some(fragment) = delta.partial_json {
                        if let Some((_, _, args)) = event.index.and_then(|i| blocks.get_mut(&i)) {
                            args.push_str(&fragment);
                        }
                    }
                    if let Some(thought) = event.index.and_then(|i| thoughts.get_mut(&i)) {
                        let more = [("thinking", delta.thinking), ("signature", delta.signature)];
                        for (field, more) in more {
                            if let Some(more) = more {
                                let so_far = thought.block[field].as_str().unwrap_or_default();
                                thought.block[field] = json!(format!("{so_far}{more}"));
                            }
                        }
                    }
                }
                "message_delta" => {
                    if let Some(u) = event.usage {
                        out.usage.output = u.output_tokens.unwrap_or(out.usage.output);
                    }
                }
                "message_stop" => {
                    out.calls = finish(blocks, thoughts, req.effort.as_ref().map(|_| provider));
                    return Ok(out);
                }
                "error" => {
                    let message = event.error.map(|e| e.message).unwrap_or_default();
                    return Err(format!("provider error: {message}"));
                }
                _ => {}
            }
        }
    }
    out.calls = finish(blocks, thoughts, req.effort.as_ref().map(|_| provider));
    Ok(out)
}

/// The calls a message made. When its thinking is wanted back, `echo` names
/// who for, and the first call carries it.
fn finish(
    blocks: BTreeMap<usize, (String, String, String)>, thoughts: BTreeMap<usize, Thought>,
    echo: Option<&Provider>,
) -> Vec<Call> {
    let mut calls: Vec<Call> = blocks
        .into_values()
        .map(|(id, name, args)| Call { id, name, args: parse_args(&args), echo: None })
        .collect();
    let thoughts: Vec<Thought> = thoughts.into_values().collect();
    if let (Some(provider), Some(first), false) = (echo, calls.first_mut(), thoughts.is_empty()) {
        first.echo = Some(Thought::echo(&thoughts, provider));
    }
    calls
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock;
    use crate::provider::Kind;
    use lb_rs::model::chat::Usage;

    const SSE: &str = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n\
        data: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":5,\"cache_read_input_tokens\":2}}}\n\n\
        data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\"}}\n\n\
        data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hel\"}}\n\n\
        data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"lo\"}}\n\n\
        data: {\"type\":\"content_block_start\",\"index\":1,\"content_block\":{\"type\":\"tool_use\",\"id\":\"t1\",\"name\":\"read\"}}\n\n\
        data: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"path\\\":\"}}\n\n\
        data: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"\\\"/a\\\"}\"}}\n\n\
        data: {\"type\":\"message_delta\",\"usage\":{\"output_tokens\":9}}\n\n\
        data: {\"type\":\"message_stop\"}\n\n";

    fn provider(base_url: &str) -> Provider {
        Provider {
            name: "mock".into(),
            display_name: None,
            needs_key: false,
            kind: Kind::Anthropic,
            base_url: base_url.into(),
            api_key: Some("k".into()),
            model: "m".into(),
            effort: None,
        }
    }

    fn run(base_url: &str, req: Request) -> (Result<Completion, String>, Vec<String>) {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let client = reqwest::Client::new();
        let result = rt.block_on(complete(&client, &provider(base_url), &req, &tx));
        let mut deltas = Vec::new();
        while let Ok(d) = rx.try_recv() {
            deltas.push(d);
        }
        (result, deltas)
    }

    #[test]
    fn streams_text_usage_and_tool_use() {
        let req = Request {
            system: "s".into(),
            turns: vec![Turn::User("hi".into())],
            ..Default::default()
        };
        let (result, deltas) = run(&mock::serve_once(SSE), req);
        let c = result.unwrap();
        assert_eq!(deltas, ["Hel", "lo"]);
        assert_eq!(c.text, "Hello");
        assert_eq!(c.usage, Usage { input: 5, output: 9, cache_read: 2, cache_write: 0 });
        assert_eq!(
            c.calls,
            [Call {
                id: "t1".into(),
                name: "read".into(),
                args: json!({"path": "/a"}),
                echo: None
            }]
        );
    }

    #[test]
    fn the_request_is_cached_to_its_end_and_results_are_user_blocks() {
        let (url, rx) = mock::serve_capturing(SSE);
        let req = Request {
            system: "sys".into(),
            turns: vec![
                Turn::User("u".into()),
                Turn::Assistant {
                    text: "t".into(),
                    calls: vec![Call {
                        id: "t1".into(),
                        name: "read".into(),
                        args: json!({}),
                        echo: None,
                    }],
                },
                Turn::ToolResults(vec![super::super::ToolResult {
                    id: "t1".into(),
                    text: "r".into(),
                    ok: false,
                }]),
            ],
            ..Default::default()
        };
        run(&url, req).0.unwrap();
        let sent: Value = serde_json::from_str(&rx.recv().unwrap()).unwrap();
        assert_eq!(sent["cache_control"]["type"], "ephemeral");
        assert_eq!(sent["system"], "sys");
        assert_eq!(sent["messages"][1]["content"][1]["type"], "tool_use");
        assert_eq!(sent["messages"][2]["role"], "user");
        assert_eq!(sent["messages"][2]["content"][0]["is_error"], true);
    }
    /// A reply that thinks, says something, notes its progress, and calls a
    /// tool, as the API streams it.
    const SSE_THINKING: &str = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n\
        data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"thinking\",\"thinking\":\"\",\"signature\":\"\"}}\n\n\
        data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"Today is \"}}\n\n\
        data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"the first.\"}}\n\n\
        data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"signature_delta\",\"signature\":\"SIG-A\"}}\n\n\
        data: {\"type\":\"content_block_start\",\"index\":1,\"content_block\":{\"type\":\"text\"}}\n\n\
        data: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"text_delta\",\"text\":\"Looking.\"}}\n\n\
        data: {\"type\":\"content_block_start\",\"index\":2,\"content_block\":{\"type\":\"redacted_thinking\",\"data\":\"SEALED\"}}\n\n\
        data: {\"type\":\"content_block_start\",\"index\":3,\"content_block\":{\"type\":\"tool_use\",\"id\":\"t1\",\"name\":\"read\"}}\n\n\
        data: {\"type\":\"content_block_delta\",\"index\":3,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{}\"}}\n\n\
        data: {\"type\":\"content_block_start\",\"index\":4,\"content_block\":{\"type\":\"thinking\",\"thinking\":\"\",\"signature\":\"\"}}\n\n\
        data: {\"type\":\"content_block_delta\",\"index\":4,\"delta\":{\"type\":\"signature_delta\",\"signature\":\"SIG-B\"}}\n\n\
        data: {\"type\":\"content_block_start\",\"index\":5,\"content_block\":{\"type\":\"tool_use\",\"id\":\"t2\",\"name\":\"list\"}}\n\n\
        data: {\"type\":\"message_stop\"}\n\n";

    fn asked(effort: Option<&str>) -> Request {
        Request {
            system: "s".into(),
            turns: vec![Turn::User("hi".into())],
            effort: effort.map(str::to_string),
            ..Default::default()
        }
    }

    /// What the model thought goes back exactly as it came and where it
    /// came, with the tool results: shown or hidden or sealed, before the
    /// text, after it, and between the calls.
    #[test]
    fn thinking_goes_back_as_it_came() {
        let (url, sent) = mock::serve(vec![SSE_THINKING.to_string()]);
        let mut req = asked(Some("high"));
        let reply = run(&url, asked(Some("high"))).0.unwrap();
        let first: Value = serde_json::from_str(&sent.recv().unwrap()).unwrap();
        assert_eq!(first["thinking"]["type"], "adaptive");

        let results = reply
            .calls
            .iter()
            .map(|c| super::super::ToolResult { id: c.id.clone(), text: "r".into(), ok: true })
            .collect();
        req.turns
            .push(Turn::Assistant { text: reply.text.clone(), calls: reply.calls.clone() });
        req.turns.push(Turn::ToolResults(results));
        let again = body(&provider(&url), &req, MAX_TOKENS);
        assert_eq!(
            again["messages"][1]["content"],
            json!([
                { "type": "thinking", "thinking": "Today is the first.", "signature": "SIG-A" },
                { "type": "text", "text": "Looking." },
                { "type": "redacted_thinking", "data": "SEALED" },
                { "type": "tool_use", "id": "t1", "name": "read", "input": {} },
                { "type": "thinking", "thinking": "", "signature": "SIG-B" },
                { "type": "tool_use", "id": "t2", "name": "list", "input": {} },
            ])
        );

        // It still goes once the user has spoken again, so the request
        // starts as the last one did; none goes when the chat asks for none.
        req.turns.push(Turn::User("thanks".into()));
        let later = body(&provider(&url), &req, MAX_TOKENS);
        assert_eq!(later["messages"][1], again["messages"][1]);
        req.turns.pop();
        req.effort = None;
        let unasked = body(&provider(&url), &req, MAX_TOKENS);
        assert_eq!(unasked["messages"][1]["content"].as_array().unwrap().len(), 3);
    }

    /// A chat that asks for no thinking keeps none of what arrives anyway.
    #[test]
    fn unasked_thinking_is_not_kept() {
        let reply = run(&mock::serve_once(SSE_THINKING), asked(None)).0.unwrap();
        assert_eq!(reply.text, "Looking.");
        assert!(reply.calls.iter().all(|c| c.echo.is_none()));
    }

    /// "on" switches on a model that thinks against a budget; anything else
    /// is an effort for one that paces itself; nothing set, nothing said.
    #[test]
    fn the_effort_setting_asks_for_thinking() {
        let sent =
            |effort, max_tokens| body(&provider("http://unused"), &asked(effort), max_tokens);
        let on = sent(Some("on"), MAX_TOKENS);
        assert_eq!(on["thinking"]["type"], "enabled");
        assert_eq!(on["thinking"]["budget_tokens"], THINKING_BUDGET);
        assert_eq!(on["output_config"], Value::Null);

        let high = sent(Some("high"), MAX_TOKENS);
        assert_eq!(high["thinking"]["type"], "adaptive");
        assert_eq!(high["output_config"]["effort"], "high");
        assert_eq!(high["thinking"]["block_binding"]["prefix_mismatch_behavior"], "drop_block");

        assert_eq!(sent(None, MAX_TOKENS)["thinking"], Value::Null);
        // A budget that would not fit under the output cap is not asked for.
        assert_eq!(sent(Some("on"), SMALL_MAX_TOKENS)["thinking"], Value::Null);
    }
}
