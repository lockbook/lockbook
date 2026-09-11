//! Grok over the xAI Responses API.
//!
//! Typed chat POSTs `/v1/responses` (HTTP + SSE). Voice will use the Realtime
//! websocket. Both speak the same event names (`response.output_text.delta`,
//! `response.function_call_arguments.done`, `response.output_item.done`).
//! [`apply_event`] is that shared fold — keep transport (SSE vs WS) out of it.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};

use futures::StreamExt;
use lb_rs::model::chat::{ItemKind, Status, Transcript, Usage};
use serde_json::{Value, json};

use tracing::{debug, error, info, warn};

use super::tools;

pub const BASE_URL: &str = "https://api.x.ai/v1";
/// Flagship chat/code model. `grok-4-latest` is an older alias (post-May-2026
/// retirement it has pointed at grok-4.3, not 4.6).
pub const MODEL: &str = "grok-4.6";
pub const MODELS: &[(&str, &str)] = &[("Grok 4.6", "grok-4.6"), ("Grok 4.5", "grok-4.5")];

pub fn model_label(id: &str) -> &str {
    MODELS
        .iter()
        .find(|(_, m)| *m == id)
        .map(|(l, _)| *l)
        .unwrap_or(id)
}

pub fn canonical_model(id: &str) -> String {
    if MODELS.iter().any(|(_, m)| *m == id) { id.to_string() } else { MODEL.to_string() }
}
/// Agentic web/X search can sit quiet between events for a while.
const STREAM_IDLE: std::time::Duration = std::time::Duration::from_secs(300);

#[derive(Debug, Clone, PartialEq)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub args: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TurnEv {
    Text(String),
    Usage(Usage),
    /// Responses `id` — next client-tool round sends `previous_response_id`.
    ResponseId(String),
    /// Client tools the UI must run, then start a follow-up.
    ToolCalls(Vec<ToolCall>),
    /// Server-side search (already executed). Show as a tool row.
    ServerTool {
        id: String,
        name: String,
        args: Value,
    },
    Done,
    Err(String),
}

#[derive(Default)]
struct PartialCall {
    call_id: String,
    name: String,
    args: String,
}

/// Accumulator for one Responses stream. Realtime can reuse [`apply_event`].
#[derive(Default)]
pub struct StreamState {
    response_id: Option<String>,
    items: HashMap<String, PartialCall>,
    client: Vec<ToolCall>,
    seen_server: HashSet<String>,
    finished: bool,
}

impl StreamState {
    fn note_id(&mut self, id: &str) -> Option<TurnEv> {
        if id.is_empty() || self.response_id.as_deref() == Some(id) {
            return None;
        }
        self.response_id = Some(id.to_string());
        Some(TurnEv::ResponseId(id.to_string()))
    }

    fn finish(&mut self) -> Vec<TurnEv> {
        if self.finished {
            return Vec::new();
        }
        self.finished = true;
        if self.client.is_empty() {
            vec![TurnEv::Done]
        } else {
            info!(n = self.client.len(), "chat emitting client tools");
            vec![TurnEv::ToolCalls(self.client.clone())]
        }
    }
}

/// Rebuild Responses `input` from the local transcript. Server search is
/// omitted (no `function_call_output` for those). Voice will not use this —
/// the socket already has the conversation.
pub fn input_from_transcript(t: &Transcript) -> Vec<Value> {
    let mut out = Vec::new();
    for item in &t.items {
        if item.status.in_flight() {
            continue;
        }
        if item.status == Status::Cancelled && !item.has_text() {
            continue;
        }
        match item.kind {
            ItemKind::User if item.has_text() => {
                out.push(json!({ "role": "user", "content": item.text() }));
            }
            ItemKind::Assistant if item.status == Status::Done || item.has_text() => {
                for tool in t.items.iter().filter(|i| {
                    i.kind == ItemKind::Tool
                        && i.parent == Some(item.id)
                        && !i.status.in_flight()
                        && !tools::is_server(i.meta.tool_kind.as_deref().unwrap_or(""))
                }) {
                    let id = tool
                        .meta
                        .wire_id
                        .clone()
                        .unwrap_or_else(|| tool.id.to_string());
                    let name = tool.meta.tool_kind.clone().unwrap_or_default();
                    let args = tool.meta.args.clone().unwrap_or(json!({}));
                    out.push(json!({
                        "type": "function_call",
                        "call_id": id,
                        "name": name,
                        "arguments": args.to_string(),
                    }));
                    if tool.status == Status::Done || tool.status == Status::Failed {
                        out.push(json!({
                            "type": "function_call_output",
                            "call_id": id,
                            "output": tool.text(),
                        }));
                    }
                }
                if item.has_text() {
                    out.push(json!({ "role": "assistant", "content": item.text() }));
                }
            }
            _ => {}
        }
    }
    out
}

/// Client-tool results for a `previous_response_id` follow-up. HTTP-only:
/// Realtime sends each `function_call_output` on the socket as it finishes.
pub fn tool_outputs_from_transcript(t: &Transcript) -> Vec<Value> {
    let Some(asst) = t.items.iter().rev().find(|i| i.kind == ItemKind::Assistant) else {
        return Vec::new();
    };
    t.items
        .iter()
        .filter(|i| {
            i.kind == ItemKind::Tool
                && i.parent == Some(asst.id)
                && i.status.terminal()
                && !tools::is_server(i.meta.tool_kind.as_deref().unwrap_or(""))
        })
        .map(|i| {
            json!({
                "type": "function_call_output",
                "call_id": i.meta.wire_id.clone().unwrap_or_else(|| i.id.to_string()),
                "output": i.text(),
            })
        })
        .collect()
}

pub async fn stream(
    bearer: &str, model: &str, input: Vec<Value>, instructions: Option<&str>,
    previous_response_id: Option<&str>, cancel: &AtomicBool, mut tx: impl FnMut(TurnEv),
) {
    let client = match reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(10))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            error!(error = %e, "chat http client");
            tx(TurnEv::Err(e.to_string()));
            return;
        }
    };
    let n_tools = tools::typed_tools()
        .as_array()
        .map(|a| a.len())
        .unwrap_or(0);
    info!(
        model,
        n_input = input.len(),
        n_tools,
        follow_up = previous_response_id.is_some(),
        "chat responses request"
    );
    let mut body = json!({
        "model": model,
        "input": input,
        "stream": true,
        "store": true,
        "tools": tools::typed_tools(),
        "include": [
            "web_search_call.action.sources",
            "code_interpreter_call.outputs",
        ],
    });
    if let Some(id) = previous_response_id {
        body["previous_response_id"] = json!(id);
    } else if let Some(sys) = instructions {
        body["instructions"] = json!(sys);
    }
    let resp = match send(&client, bearer, &body).await {
        Ok(r) => r,
        Err(e) => {
            error!(error = %e, "chat responses transport");
            tx(TurnEv::Err(e));
            return;
        }
    };
    if !resp.status().is_success() {
        let status = resp.status();
        let err = resp.text().await.unwrap_or_default();
        let snippet: String = err.chars().take(400).collect();
        error!(%status, body = %snippet, "chat responses http error");
        tx(TurnEv::Err(format!("{status}: {snippet}")));
        return;
    }

    let mut stream = resp.bytes_stream();
    let mut buf: Vec<u8> = Vec::new();
    let mut st = StreamState::default();
    let mut event_ty: Option<String> = None;
    loop {
        if cancel.load(Ordering::Relaxed) {
            info!("chat stream cancelled");
            return;
        }
        let item = match tokio::time::timeout(STREAM_IDLE, stream.next()).await {
            Ok(item) => item,
            Err(_) => {
                error!("chat stream idle timeout");
                tx(TurnEv::Err("provider stopped responding mid-stream".into()));
                return;
            }
        };
        let Some(bytes) = item else { break };
        let bytes = match bytes {
            Ok(b) => b,
            Err(e) => {
                error!(error = %e, "chat stream read failed");
                tx(TurnEv::Err(format!("stream failed: {e}")));
                return;
            }
        };
        buf.extend_from_slice(&bytes);
        while let Some(nl) = buf.iter().position(|&b| b == b'\n') {
            let line_bytes: Vec<u8> = buf.drain(..=nl).collect();
            let line = String::from_utf8_lossy(&line_bytes);
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Some(ev) = line.strip_prefix("event:") {
                event_ty = Some(ev.trim().to_string());
                continue;
            }
            let Some(payload) = line.strip_prefix("data:") else {
                continue;
            };
            let payload = payload.trim();
            if payload == "[DONE]" {
                for ev in st.finish() {
                    tx(ev);
                }
                return;
            }
            let mut v: Value = match serde_json::from_str(payload) {
                Ok(v) => v,
                Err(e) => {
                    debug!(
                        error = %e,
                        payload = %payload.chars().take(200).collect::<String>(),
                        "chat sse skip"
                    );
                    continue;
                }
            };
            if v.get("type").and_then(|t| t.as_str()).is_none() {
                if let Some(t) = &event_ty {
                    v["type"] = json!(t);
                }
            }
            for ev in apply_event(&v, &mut st) {
                if cancel.load(Ordering::Relaxed) {
                    info!("chat stream cancelled");
                    return;
                }
                match &ev {
                    TurnEv::Err(e) => {
                        error!(error = %e, "chat sse error event");
                        tx(ev);
                        return;
                    }
                    _ => tx(ev),
                }
            }
        }
    }
    for ev in st.finish() {
        tx(ev);
    }
}

async fn send(
    client: &reqwest::Client, bearer: &str, body: &Value,
) -> Result<reqwest::Response, String> {
    client
        .post(format!("{BASE_URL}/responses"))
        .bearer_auth(bearer)
        .json(body)
        .send()
        .await
        .map_err(|e| format!("request failed: {e}"))
}

/// Fold one Responses-family event. HTTP SSE and Realtime WS feed this.
pub fn apply_event(ev: &Value, st: &mut StreamState) -> Vec<TurnEv> {
    if let Some(err) = ev.get("error") {
        let msg = err
            .get("message")
            .and_then(|m| m.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| err.to_string());
        return vec![TurnEv::Err(msg)];
    }
    let ty = ev.get("type").and_then(|v| v.as_str()).unwrap_or("");
    match ty {
        "response.failed" => {
            let msg = ev
                .pointer("/response/error/message")
                .and_then(|m| m.as_str())
                .unwrap_or("response failed")
                .to_string();
            vec![TurnEv::Err(msg)]
        }
        "response.created" | "response.in_progress" => id_from(ev)
            .and_then(|id| st.note_id(id))
            .into_iter()
            .collect(),
        "response.output_text.delta" => {
            let delta = ev.get("delta").and_then(|d| d.as_str()).unwrap_or("");
            if delta.is_empty() { Vec::new() } else { vec![TurnEv::Text(delta.to_string())] }
        }
        "response.output_item.added" | "response.output_item.done" => {
            let done = ty.ends_with("done");
            ev.get("item")
                .map(|item| handle_item(item, st, done))
                .unwrap_or_default()
        }
        "response.function_call_arguments.delta" => {
            if let Some(id) = ev.get("item_id").and_then(|v| v.as_str()) {
                if let Some(d) = ev.get("delta").and_then(|v| v.as_str()) {
                    st.items.entry(id.to_string()).or_default().args.push_str(d);
                }
            }
            Vec::new()
        }
        "response.function_call_arguments.done" => finish_function(ev, st),
        "response.completed" => {
            let mut out = Vec::new();
            if let Some(id) = id_from(ev) {
                if let Some(ev) = st.note_id(id) {
                    out.push(ev);
                }
            }
            if let Some(u) = usage_from(ev.get("response").unwrap_or(ev)) {
                out.push(TurnEv::Usage(u));
            }
            out.extend(st.finish());
            out
        }
        _ if ty.contains("web_search")
            || ty.contains("x_search")
            || ty.contains("code_interpreter")
            || ty.contains("code_execution") =>
        {
            ev.get("item")
                .map(|item| {
                    handle_item(item, st, ty.ends_with("completed") || ty.ends_with("done"))
                })
                .unwrap_or_default()
        }
        _ => Vec::new(),
    }
}

fn id_from(ev: &Value) -> Option<&str> {
    ev.pointer("/response/id")
        .and_then(|v| v.as_str())
        .or_else(|| ev.get("id").and_then(|v| v.as_str()))
        .filter(|s| !s.is_empty())
}

fn usage_from(v: &Value) -> Option<Usage> {
    let u = v.get("usage")?;
    let input = u.get("input_tokens").and_then(|x| x.as_u64()).unwrap_or(0);
    let output = u.get("output_tokens").and_then(|x| x.as_u64()).unwrap_or(0);
    let cached = u
        .pointer("/input_tokens_details/cached_tokens")
        .and_then(|x| x.as_u64())
        .unwrap_or(0);
    Some(Usage { input: input.saturating_sub(cached), output, cache_read: cached, cache_write: 0 })
}

fn handle_item(item: &Value, st: &mut StreamState, done: bool) -> Vec<TurnEv> {
    let ty = item.get("type").and_then(|v| v.as_str()).unwrap_or("");
    let item_id = item
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    match ty {
        "function_call" => {
            let p = st.items.entry(item_id).or_default();
            if let Some(id) = item.get("call_id").and_then(|v| v.as_str()) {
                p.call_id = id.to_string();
            }
            if let Some(name) = item.get("name").and_then(|v| v.as_str()) {
                p.name = name.to_string();
            }
            if let Some(args) = item.get("arguments").and_then(|v| v.as_str()) {
                if p.args.len() < args.len() {
                    p.args = args.to_string();
                }
            }
            if done && !p.name.is_empty() {
                let ev = json!({
                    "item_id": item.get("id").cloned().unwrap_or(Value::Null),
                    "call_id": p.call_id,
                    "name": p.name,
                    "arguments": p.args,
                });
                return finish_function(&ev, st);
            }
            Vec::new()
        }
        "message" | "reasoning" => Vec::new(),
        other => {
            if !done {
                return Vec::new();
            }
            if other.is_empty() || tools::is_client(other) {
                return Vec::new();
            }
            let id = if item_id.is_empty() {
                item.get("call_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or(other)
                    .to_string()
            } else {
                item_id
            };
            emit_server(st, id, other, search_args(item))
        }
    }
}

fn search_args(item: &Value) -> Value {
    let mut out = serde_json::Map::new();
    let take = |out: &mut serde_json::Map<String, Value>, key: &str, v: &Value| {
        if !v.is_null() {
            out.entry(key.to_string()).or_insert(v.clone());
        }
    };
    for key in ["code", "input", "outputs", "query", "sources", "citations", "results", "url"] {
        if let Some(v) = item.get(key) {
            take(&mut out, key, v);
        }
    }
    if let Some(action) = item.get("action").and_then(|a| a.as_object()) {
        for (k, v) in action {
            if !v.is_null() {
                out.entry(k.clone()).or_insert(v.clone());
            }
        }
    }
    Value::Object(out)
}

fn finish_function(ev: &Value, st: &mut StreamState) -> Vec<TurnEv> {
    let item_id = ev.get("item_id").and_then(|v| v.as_str()).unwrap_or("");
    let partial = st.items.get(item_id);
    let name = ev
        .get("name")
        .and_then(|v| v.as_str())
        .or_else(|| partial.map(|p| p.name.as_str()))
        .unwrap_or("")
        .to_string();
    if name.is_empty() {
        return Vec::new();
    }
    let call_id = ev
        .get("call_id")
        .and_then(|v| v.as_str())
        .or_else(|| {
            partial
                .map(|p| p.call_id.as_str())
                .filter(|s| !s.is_empty())
        })
        .unwrap_or(item_id);
    let call_id = if call_id.is_empty() { name.clone() } else { call_id.to_string() };
    let args = match ev.get("arguments") {
        Some(Value::String(s)) if !s.is_empty() => {
            serde_json::from_str(s).unwrap_or_else(|_| json!({ "raw": s }))
        }
        Some(other) if !other.is_null() && other != &json!({}) => other.clone(),
        _ => {
            let raw = partial.map(|p| p.args.as_str()).unwrap_or("");
            if raw.is_empty() {
                json!({})
            } else {
                serde_json::from_str(raw).unwrap_or_else(|_| json!({ "raw": raw }))
            }
        }
    };
    if tools::is_client(&name) {
        if st.client.iter().any(|c| c.id == call_id) {
            return Vec::new();
        }
        st.client.push(ToolCall { id: call_id, name, args });
        Vec::new()
    } else {
        emit_server(st, call_id, &name, args)
    }
}

fn emit_server(st: &mut StreamState, id: String, name: &str, args: Value) -> Vec<TurnEv> {
    if !st.seen_server.insert(id.clone()) {
        return Vec::new();
    }
    if tools::is_server(name) {
        info!(name, "chat server tool");
    } else {
        warn!(name, "chat unknown tool treated as server");
    }
    vec![TurnEv::ServerTool { id, name: name.to_string(), args }]
}

#[cfg(test)]
mod tests {
    use super::*;
    use lb_rs::model::chat::{Content, Event, EventBody, ItemMeta};

    fn apply(ty: &str, rest: Value) -> (StreamState, Vec<TurnEv>) {
        let mut v = rest;
        v["type"] = json!(ty);
        let mut st = StreamState::default();
        let out = apply_event(&v, &mut st);
        (st, out)
    }

    #[test]
    fn text_delta() {
        let (_, out) = apply("response.output_text.delta", json!({ "delta": "Hi" }));
        assert_eq!(out, vec![TurnEv::Text("Hi".into())]);
    }

    #[test]
    fn client_tool_waits_for_completed() {
        let mut st = StreamState::default();
        let out = apply_event(
            &json!({
                "type": "response.function_call_arguments.done",
                "item_id": "fc_1",
                "call_id": "c1",
                "name": "list",
                "arguments": "{\"path\":\"/\"}"
            }),
            &mut st,
        );
        assert!(out.is_empty());
        let out = apply_event(
            &json!({
                "type": "response.completed",
                "response": { "id": "resp_1", "usage": { "input_tokens": 10, "output_tokens": 4 } }
            }),
            &mut st,
        );
        assert!(
            out.iter()
                .any(|e| matches!(e, TurnEv::ResponseId(id) if id == "resp_1"))
        );
        match out.last() {
            Some(TurnEv::ToolCalls(c)) => {
                assert_eq!(c.len(), 1);
                assert_eq!(c[0].name, "list");
                assert_eq!(c[0].args["path"], "/");
            }
            other => panic!("expected ToolCalls, got {other:?}"),
        }
    }

    #[test]
    fn web_search_item_emits_immediately() {
        let mut st = StreamState::default();
        let out = apply_event(
            &json!({
                "type": "response.output_item.done",
                "item": {
                    "type": "web_search_call",
                    "id": "ws_1",
                    "status": "completed",
                    "action": { "type": "search", "query": "xai" }
                }
            }),
            &mut st,
        );
        match &out[..] {
            [TurnEv::ServerTool { id, name, args }] => {
                assert_eq!(id, "ws_1");
                assert_eq!(name, "web_search_call");
                assert_eq!(args["query"], "xai");
            }
            other => panic!("expected ServerTool, got {other:?}"),
        }
        let again = apply_event(
            &json!({
                "type": "response.output_item.done",
                "item": { "type": "web_search_call", "id": "ws_1", "action": { "query": "xai" } }
            }),
            &mut st,
        );
        assert!(again.is_empty());
        let end = apply_event(
            &json!({ "type": "response.completed", "response": { "id": "r" } }),
            &mut st,
        );
        assert!(matches!(end.last(), Some(TurnEv::Done)));
    }

    #[test]
    fn web_search_waits_for_done() {
        let mut st = StreamState::default();
        let out = apply_event(
            &json!({
                "type": "response.output_item.added",
                "item": {
                    "type": "web_search_call",
                    "id": "ws_1",
                    "action": { "query": "xai" }
                }
            }),
            &mut st,
        );
        assert!(out.is_empty());
    }

    #[test]
    fn code_interpreter_item_emits_immediately() {
        let mut st = StreamState::default();
        let out = apply_event(
            &json!({
                "type": "response.output_item.done",
                "item": {
                    "type": "code_interpreter_call",
                    "id": "ci_1",
                    "code": "print(1+1)\nprint(2)",
                    "outputs": [{ "type": "logs", "logs": "2\n" }]
                }
            }),
            &mut st,
        );
        match &out[..] {
            [TurnEv::ServerTool { id, name, args }] => {
                assert_eq!(id, "ci_1");
                assert_eq!(name, "code_interpreter_call");
                assert_eq!(args["code"], "print(1+1)\nprint(2)");
                assert_eq!(args["outputs"][0]["logs"], "2\n");
            }
            other => panic!("expected ServerTool, got {other:?}"),
        }
    }

    #[test]
    fn web_search_keeps_sources() {
        let mut st = StreamState::default();
        let out = apply_event(
            &json!({
                "type": "response.output_item.done",
                "item": {
                    "type": "web_search_call",
                    "id": "ws_2",
                    "action": {
                        "type": "search",
                        "query": "xai",
                        "sources": [
                            { "url": "https://x.ai", "title": "xAI" }
                        ]
                    }
                }
            }),
            &mut st,
        );
        match &out[..] {
            [TurnEv::ServerTool { name, args, .. }] => {
                assert_eq!(name, &"web_search_call");
                assert_eq!(args["query"], "xai");
                assert_eq!(args["sources"][0]["url"], "https://x.ai");
            }
            other => panic!("expected ServerTool, got {other:?}"),
        }
    }

    #[test]
    fn server_function_name_is_not_a_client_follow_up() {
        let mut st = StreamState::default();
        let out = apply_event(
            &json!({
                "type": "response.function_call_arguments.done",
                "call_id": "s1",
                "name": "web_search",
                "arguments": "{\"query\":\"xai\"}"
            }),
            &mut st,
        );
        assert!(matches!(&out[..], [TurnEv::ServerTool { name, .. }] if name == "web_search"));
        let end = apply_event(&json!({ "type": "response.completed" }), &mut st);
        assert!(matches!(end.last(), Some(TurnEv::Done)));
    }

    #[test]
    fn input_omits_provider_search() {
        let user = lb_rs::Uuid::from_u128(1);
        let asst = lb_rs::Uuid::from_u128(2);
        let web = lb_rs::Uuid::from_u128(3);
        let list = lb_rs::Uuid::from_u128(4);
        let t = Transcript::fold(&[
            Event::new("a", 1, EventBody::Open { item: user, parent: None, kind: ItemKind::User }),
            Event::new(
                "a",
                2,
                EventBody::Replace {
                    item: user,
                    blocks: vec![Content::Text { text: "hi".into() }],
                },
            ),
            Event::new("a", 3, EventBody::SetStatus { item: user, status: Status::Done }),
            Event::new(
                "a",
                4,
                EventBody::Open { item: asst, parent: Some(user), kind: ItemKind::Assistant },
            ),
            Event::new(
                "a",
                5,
                EventBody::Replace {
                    item: asst,
                    blocks: vec![Content::Text { text: "ok".into() }],
                },
            ),
            Event::new("a", 6, EventBody::SetStatus { item: asst, status: Status::Done }),
            Event::new(
                "a",
                7,
                EventBody::Open { item: web, parent: Some(asst), kind: ItemKind::Tool },
            ),
            Event::new(
                "a",
                8,
                EventBody::SetMeta {
                    item: web,
                    meta: ItemMeta {
                        title: Some("web xai".into()),
                        tool_kind: Some("web_search".into()),
                        wire_id: Some("s1".into()),
                        args: Some(json!({"query": "xai"})),
                    },
                },
            ),
            Event::new("a", 9, EventBody::SetStatus { item: web, status: Status::Done }),
            Event::new(
                "a",
                10,
                EventBody::Open { item: list, parent: Some(asst), kind: ItemKind::Tool },
            ),
            Event::new(
                "a",
                11,
                EventBody::SetMeta {
                    item: list,
                    meta: ItemMeta {
                        title: Some("list /".into()),
                        tool_kind: Some("list".into()),
                        wire_id: Some("c1".into()),
                        args: Some(json!({})),
                    },
                },
            ),
            Event::new(
                "a",
                12,
                EventBody::Replace {
                    item: list,
                    blocks: vec![Content::Text { text: "notes/".into() }],
                },
            ),
            Event::new("a", 13, EventBody::SetStatus { item: list, status: Status::Done }),
        ]);
        let input = input_from_transcript(&t);
        assert_eq!(input[0]["role"], "user");
        assert_eq!(input[1]["type"], "function_call");
        assert_eq!(input[1]["name"], "list");
        assert_eq!(input[2]["type"], "function_call_output");
        assert_eq!(input[2]["call_id"], "c1");
        assert_eq!(input[3]["role"], "assistant");
        assert_eq!(input.len(), 4);
        let outs = tool_outputs_from_transcript(&t);
        assert_eq!(outs.len(), 1);
        assert_eq!(outs[0]["call_id"], "c1");
    }
}
