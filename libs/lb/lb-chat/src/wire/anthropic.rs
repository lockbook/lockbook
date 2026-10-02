//! Anthropic messages, native: prompt caching on the system block and
//! structured tool use.

use std::collections::BTreeMap;

use futures::StreamExt;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::mpsc::UnboundedSender;

use super::{Call, Completion, Request, STREAM_IDLE, Sse, Turn, parse_args, send};
use crate::provider::Provider;

const MAX_TOKENS: u32 = 16_384;
const SMALL_MAX_TOKENS: u32 = 4096;

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
}

#[derive(Deserialize)]
struct Delta {
    #[serde(default)]
    text: Option<String>,
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
    let mut messages: Vec<Value> = req
        .turns
        .iter()
        .map(|turn| match turn {
            Turn::User(text) => json!({ "role": "user", "content": text }),
            Turn::Assistant { text, calls } if !calls.is_empty() => {
                let mut blocks = Vec::new();
                if !text.is_empty() {
                    blocks.push(json!({ "type": "text", "text": text }));
                }
                for c in calls {
                    blocks.push(json!({ "type": "tool_use", "id": c.id, "name": c.name, "input": c.args }));
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

    // Said only in the system prompt, the date is lost on a small model by
    // the time a note full of dates comes back. Said again where the model
    // answers from, and in the user's voice rather than a tool's, it holds.
    let newest = messages.last_mut().filter(|m| m["role"] == "user");
    if let Some(newest) = newest.filter(|_| !req.today.is_empty()) {
        let today = json!({ "type": "text", "text": req.today });
        match &mut newest["content"] {
            Value::Array(blocks) => blocks.push(today),
            text => *text = json!([{ "type": "text", "text": text.take() }, today]),
        }
    }

    let mut body = json!({
        "model": provider.model,
        "max_tokens": max_tokens,
        "messages": messages,
        "stream": true,
    });
    if !req.system.is_empty() {
        body["system"] = json!([{
            "type": "text",
            "text": req.system,
            "cache_control": { "type": "ephemeral" },
        }]);
    }
    if !req.tools.is_empty() {
        body["tools"] = req
            .tools
            .iter()
            .map(|t| json!({ "name": t.name, "description": t.description, "input_schema": t.parameters }))
            .collect();
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
    let headers = [("x-api-key", key), ("anthropic-version", "2023-06-01".into())];
    let url = format!("{}/messages", provider.base_url);

    let resp = match send(client, &url, &headers, &body(provider, req, MAX_TOKENS)).await {
        Err(e) if e.starts_with("400") && e.contains("max_tokens") => {
            send(client, &url, &headers, &body(provider, req, SMALL_MAX_TOKENS)).await?
        }
        other => other?,
    };

    let mut out = Completion::default();
    // index → (id, name, accumulated json)
    let mut blocks: BTreeMap<usize, (String, String, String)> = BTreeMap::new();
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
                    if let Some(block) = event.content_block.filter(|b| b.kind == "tool_use") {
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
                }
                "message_delta" => {
                    if let Some(u) = event.usage {
                        out.usage.output = u.output_tokens.unwrap_or(out.usage.output);
                    }
                }
                "message_stop" => {
                    out.calls = finish(blocks);
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
    out.calls = finish(blocks);
    Ok(out)
}

fn finish(blocks: BTreeMap<usize, (String, String, String)>) -> Vec<Call> {
    blocks
        .into_values()
        .map(|(id, name, args)| Call { id, name, args: parse_args(&args), echo: None })
        .collect()
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
            tools: vec![],
            today: String::new(),
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
    fn system_block_carries_cache_control_and_results_are_user_blocks() {
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
            tools: vec![],
            today: String::new(),
        };
        run(&url, req).0.unwrap();
        let sent: Value = serde_json::from_str(&rx.recv().unwrap()).unwrap();
        assert_eq!(sent["system"][0]["cache_control"]["type"], "ephemeral");
        assert_eq!(sent["messages"][1]["content"][1]["type"], "tool_use");
        assert_eq!(sent["messages"][2]["role"], "user");
        assert_eq!(sent["messages"][2]["content"][0]["is_error"], true);
    }

    /// The date follows whatever the model reads last, as text of its own:
    /// after the user's message, and after tool results rather than inside
    /// one. Only the newest turn carries it.
    #[test]
    fn the_date_is_said_again_after_the_newest_turn() {
        let today = "Today is Thursday, October 1, 2026.";
        let call = Call { id: "t1".into(), name: "read".into(), args: json!({}), echo: None };
        let result = super::super::ToolResult { id: "t1".into(), text: "r".into(), ok: true };
        let asked = vec![Turn::User("u".into())];
        let answered = vec![
            Turn::User("u".into()),
            Turn::Assistant { text: String::new(), calls: vec![call] },
            Turn::ToolResults(vec![result]),
        ];
        let sent = |turns: Vec<Turn>| {
            let req = Request { system: "s".into(), turns, tools: vec![], today: today.into() };
            body(&provider("http://unused"), &req, MAX_TOKENS)["messages"].clone()
        };
        let said = json!({ "type": "text", "text": today });

        let messages = sent(asked);
        assert_eq!(messages[0]["content"], json!([{ "type": "text", "text": "u" }, said]));

        let messages = sent(answered);
        assert_eq!(messages[0]["content"], "u");
        assert_eq!(messages[2]["content"][0]["type"], "tool_result");
        assert_eq!(messages[2]["content"][0]["content"], "r");
        assert_eq!(messages[2]["content"][1], said);
    }
}
