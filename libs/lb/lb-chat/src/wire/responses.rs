//! The Responses API, as OpenAI and xAI serve it: tools beside reasoning,
//! and reasoning that is handed back as it came.

use std::collections::BTreeMap;

use futures::StreamExt;
use lb_rs::model::chat::Usage;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::mpsc::UnboundedSender;

use super::{
    Call, Completion, Echo, Piece, Request, STREAM_IDLE, Served, Sse, Turn, link, parse_args, send,
};
use crate::provider::{Provider, host};

/// Tools each host runs itself, tried with a real request, and what to ask
/// for so their results come back.
const SERVER_TOOLS: &[(&str, &[&str], &[&str])] = &[
    (
        "api.openai.com",
        &["web_search", "code_interpreter", "image_generation"],
        &["web_search_call.action.sources", "code_interpreter_call.outputs"],
    ),
    ("api.x.ai", &["web_search", "x_search", "code_interpreter"], &[]),
];

#[derive(Deserialize)]
struct Event {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    output_index: usize,
    /// Which part of a reasoning item a thinking delta belongs to.
    #[serde(default, alias = "content_index")]
    summary_index: usize,
    #[serde(default)]
    delta: Option<String>,
    #[serde(default)]
    item: Option<Value>,
    #[serde(default)]
    response: Option<Response>,
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    error: Option<WireError>,
}

#[derive(Deserialize)]
struct Response {
    #[serde(default)]
    usage: Option<WireUsage>,
    #[serde(default)]
    error: Option<WireError>,
}

#[derive(Deserialize)]
struct WireUsage {
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
    #[serde(default)]
    input_tokens_details: Option<InputDetails>,
}

#[derive(Deserialize)]
struct InputDetails {
    #[serde(default)]
    cached_tokens: u64,
}

#[derive(Deserialize)]
struct WireError {
    #[serde(default)]
    message: String,
}

#[derive(Default)]
struct Partial {
    id: String,
    name: String,
    args: String,
}

/// The reasoning items `calls` came with, for the provider that made them.
fn reasoning<'a>(provider: &'a Provider, calls: &'a [Call]) -> impl Iterator<Item = Value> + 'a {
    calls
        .iter()
        .filter_map(|call| call.echo.as_ref())
        .filter(|echo| echo.provider == provider.name)
        .filter_map(|echo| echo.content["reasoning"].as_array())
        .flatten()
        .cloned()
}

/// What a provider ran itself, from the output item that reports it.
fn served(item: &Value) -> Option<Served> {
    let text = |v: &Value| v.as_str().unwrap_or_default().to_string();
    match item["type"].as_str()? {
        // A search, or a page it went on to open.
        "web_search_call" => {
            let action = &item["action"];
            if let Some(url) = action["url"].as_str().filter(|_| action["query"].is_null()) {
                let args = json!({ "url": url });
                return Some(Served {
                    name: "fetch".into(),
                    args,
                    result: String::new(),
                    echo: None,
                    file: None,
                });
            }
            action["query"].as_str()?;
            let sources = action["sources"].as_array().into_iter().flatten();
            let urls = sources
                .map(|s| text(&s["url"]))
                .filter(|url| !url.is_empty());
            let result: String = urls.map(|url| link("", &url)).collect();
            let args = json!({ "query": action["query"] });
            Some(Served { name: "web_search".into(), args, result, echo: None, file: None })
        }
        "code_interpreter_call" => {
            let logs = item["outputs"].as_array().into_iter().flatten();
            // xAI wraps what was printed in a JSON string of its own.
            let result: Vec<String> = logs
                .map(|o| text(&o["logs"]))
                .map(|l| match serde_json::from_str::<Value>(&l) {
                    Ok(v) if v["stdout"].is_string() => text(&v["stdout"]) + &text(&v["stderr"]),
                    _ => l,
                })
                .collect();
            let args = json!({ "code": item["code"] });
            let result = result.join("\n").trim().to_string();
            Some(Served { name: "code".into(), args, result, echo: None, file: None })
        }
        "image_generation_call" => {
            let bytes = base64::decode(item["result"].as_str()?).ok()?;
            let ext = item["output_format"].as_str().unwrap_or("png").to_string();
            let args = json!({ "prompt": item["revised_prompt"] });
            let name = super::images::NAME.into();
            Some(Served { name, args, result: String::new(), echo: None, file: Some((ext, bytes)) })
        }
        // xAI's searches of X come as calls to tools of its own.
        "custom_tool_call" if text(&item["name"]).starts_with("x_") => {
            let args = parse_args(&text(&item["input"]));
            Some(Served {
                name: "x_search".into(),
                args,
                result: String::new(),
                echo: None,
                file: None,
            })
        }
        _ => None,
    }
}

/// Adds what a message cites to the search that found it.
fn cite(out: &mut Completion, message: &Value) {
    let parts = message["content"].as_array().into_iter().flatten();
    let cited = parts.flat_map(|part| part["annotations"].as_array().into_iter().flatten());
    let searches = |s: &&mut Served| s.name == "web_search" || s.name == "x_search";
    let Some(search) = out.served.iter_mut().rfind(searches) else { return };
    for citation in cited {
        let url = citation["url"].as_str().unwrap_or_default();
        if !url.is_empty() && !search.result.contains(&format!("({url})")) {
            let title = citation["title"].as_str().unwrap_or_default();
            search.result = link(title, url) + &search.result;
        }
    }
}

pub(crate) fn body(provider: &Provider, req: &Request) -> Value {
    let mut input = Vec::new();
    for turn in &req.turns {
        match turn {
            Turn::User(text) => input.push(json!({ "role": "user", "content": text })),
            Turn::Assistant { text, calls } => {
                if !text.is_empty() {
                    input.push(json!({ "role": "assistant", "content": text }));
                }
                if req.effort.is_some() {
                    input.extend(reasoning(provider, calls));
                }
                for c in calls {
                    input.push(json!({
                        "type": "function_call",
                        "call_id": c.id,
                        "name": c.name,
                        "arguments": c.args.to_string(),
                    }));
                }
            }
            Turn::ToolResults(results) => {
                for r in results {
                    input.push(json!({
                        "type": "function_call_output",
                        "call_id": r.id,
                        "output": r.text,
                    }));
                }
                // What a call read to be looked at follows as the user's.
                let seen: Vec<Value> = results
                    .iter()
                    .flat_map(|r| &r.media)
                    .map(|i| match i.is_pdf() {
                        true => {
                            let (name, data) = ("document.pdf", i.url());
                            json!({ "type": "input_file", "filename": name, "file_data": data })
                        }
                        false => json!({ "type": "input_image", "image_url": i.url() }),
                    })
                    .collect();
                if !seen.is_empty() {
                    input.push(json!({ "role": "user", "content": seen }));
                }
            }
        }
    }
    let mut body = json!({
        "model": provider.model,
        "input": input,
        "stream": true,
        "store": false,
    });
    if !req.system.is_empty() {
        body["instructions"] = json!(req.system);
    }
    if !req.tools.is_empty() {
        body["tools"] = req
            .tools
            .iter()
            .map(|t| {
                json!({
                    "type": "function",
                    "name": t.name,
                    "description": t.description,
                    "parameters": t.parameters,
                    // Strict is the default here and wants every property
                    // required.
                    "strict": false,
                })
            })
            .collect();
    }
    let host = host(&provider.base_url);
    let own = SERVER_TOOLS.iter().find(|(h, ..)| *h == host);
    let mut include: Vec<&str> = Vec::new();
    if let Some((_, kinds, wanted)) = own {
        let tools = body["tools"].as_array().cloned().unwrap_or_default();
        let own = kinds.iter().map(|kind| match (*kind, host.as_str()) {
            ("code_interpreter", "api.openai.com") => {
                json!({ "type": kind, "container": { "type": "auto" } })
            }
            _ => json!({ "type": kind }),
        });
        body["tools"] = tools.into_iter().chain(own).collect();
        include.extend(*wanted);
    }
    // A model that does not reason refuses both of these, so they go only
    // where an effort has been shown to work.
    if let Some(effort) = &req.effort {
        body["reasoning"] = json!({ "effort": effort, "summary": "auto" });
        include.push("reasoning.encrypted_content");
    }
    if !include.is_empty() {
        body["include"] = json!(include);
    }
    body
}

pub async fn complete(
    client: &reqwest::Client, provider: &Provider, req: &Request, deltas: &UnboundedSender<Piece>,
) -> Result<Completion, String> {
    let mut headers = Vec::new();
    if let Some(key) = &provider.api_key {
        headers.push(("authorization", format!("Bearer {key}")));
    }
    let url = format!("{}/responses", provider.base_url);
    let resp = send(client, &url, &headers, &body(provider, req)).await?;
    let echo = req.effort.as_ref().map(|_| provider);

    let mut out = Completion::default();
    // Both keyed by output index.
    let mut partials: BTreeMap<usize, Partial> = BTreeMap::new();
    let mut reasoned: BTreeMap<usize, Value> = BTreeMap::new();
    // The part the last thinking text came in.
    let mut thinking_in: Option<(usize, usize, bool)> = None;
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
                "response.output_text.delta" => {
                    if let Some(text) = event.delta.filter(|t| !t.is_empty()) {
                        out.text.push_str(&text);
                        let _ = deltas.send(Piece::Text(text));
                    }
                }
                "response.reasoning_summary_text.delta" | "response.reasoning_text.delta" => {
                    let Some(text) = event.delta.filter(|t| !t.is_empty()) else { continue };
                    // A part of its own starts a paragraph of its own.
                    let summary = event.kind == "response.reasoning_summary_text.delta";
                    let part = (event.output_index, event.summary_index, summary);
                    let new_part = thinking_in.replace(part) != Some(part);
                    let text = match new_part && !out.thinking.is_empty() {
                        true => format!("\n\n{text}"),
                        false => text,
                    };
                    out.thinking.push_str(&text);
                    let _ = deltas.send(Piece::Thinking(text));
                }
                "response.function_call_arguments.delta" => {
                    if let Some(call) = partials.get_mut(&event.output_index) {
                        call.args.push_str(&event.delta.unwrap_or_default());
                    }
                }
                "response.output_item.added" | "response.output_item.done" => {
                    let Some(item) = event.item else { continue };
                    match item["type"].as_str() {
                        Some("function_call") => {
                            let call = partials.entry(event.output_index).or_default();
                            let field = |name: &str| item[name].as_str().filter(|v| !v.is_empty());
                            if let Some(id) = field("call_id").or(field("id")) {
                                call.id = id.to_string();
                            }
                            if let Some(name) = field("name") {
                                call.name = name.to_string();
                            }
                            if let Some(args) = field("arguments") {
                                call.args = args.to_string();
                            }
                        }
                        Some("reasoning") if event.kind.ends_with("done") => {
                            reasoned.insert(event.output_index, item);
                        }
                        Some("message") if event.kind.ends_with("done") => cite(&mut out, &item),
                        Some(_) if event.kind.ends_with("done") => out.served.extend(served(&item)),
                        _ => {}
                    }
                }
                "response.completed" | "response.incomplete" => {
                    if let Some(u) = event.response.and_then(|r| r.usage) {
                        let cached = u.input_tokens_details.map_or(0, |d| d.cached_tokens);
                        out.usage = Usage {
                            input: u.input_tokens.saturating_sub(cached),
                            output: u.output_tokens,
                            cache_read: cached,
                            cache_write: 0,
                        };
                    }
                    out.calls = finish(partials, reasoned, echo);
                    return Ok(out);
                }
                "response.failed" | "error" => {
                    let message = event
                        .response
                        .and_then(|r| r.error)
                        .or(event.error)
                        .map(|e| e.message)
                        .or(event.message)
                        .unwrap_or_default();
                    return Err(format!("provider error: {message}"));
                }
                _ => {}
            }
        }
    }
    out.calls = finish(partials, reasoned, echo);
    Ok(out)
}

/// The calls a response made. When its reasoning is wanted back, `echo`
/// names who for, and the first call carries it.
fn finish(
    partials: BTreeMap<usize, Partial>, reasoned: BTreeMap<usize, Value>, echo: Option<&Provider>,
) -> Vec<Call> {
    let mut calls: Vec<Call> = partials
        .into_values()
        .filter(|p| !p.name.is_empty())
        .enumerate()
        .map(|(i, p)| Call {
            id: if p.id.is_empty() { format!("call_{i}") } else { p.id },
            name: p.name,
            args: parse_args(&p.args),
            echo: None,
        })
        .collect();
    let reasoned: Vec<Value> = reasoned.into_values().collect();
    if let (Some(provider), Some(first), false) = (echo, calls.first_mut(), reasoned.is_empty()) {
        let content = json!({ "reasoning": reasoned });
        first.echo = Some(Echo { provider: provider.name.clone(), content });
    }
    calls
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock;
    use crate::provider::Kind;
    use crate::wire::{ToolResult, ToolSchema};

    fn provider(base_url: &str) -> Provider {
        Provider {
            name: "mock".into(),
            display_name: None,
            needs_key: false,
            kind: Kind::OpenAi,
            base_url: base_url.into(),
            api_key: Some("k".into()),
            model: "m".into(),
            effort: None,
        }
    }

    fn run(base_url: &str, req: Request) -> (Result<Completion, String>, Vec<Piece>) {
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

    fn hi() -> Request {
        Request { system: "s".into(), turns: vec![Turn::User("hi".into())], ..Default::default() }
    }

    fn thinking_hard() -> Request {
        Request { effort: Some("high".into()), ..hi() }
    }

    /// A streamed reply of `events`, each under its `event:` line.
    fn sse(events: &[Value]) -> String {
        let mut out =
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n"
                .to_string();
        for event in events {
            let kind = event["type"].as_str().unwrap();
            out.push_str(&format!("event: {kind}\ndata: {event}\n\n"));
        }
        out
    }

    fn rs1() -> Value {
        json!({
            "type": "reasoning",
            "id": "rs_1",
            "summary": [{ "type": "summary_text", "text": "Find the note." }],
            "encrypted_content": "sealed",
        })
    }

    /// A response that reasons in two summary parts, then calls `read`
    /// with arguments in two fragments.
    fn reasoned_call() -> String {
        let call = json!({
            "type": "function_call", "id": "fc_1", "call_id": "c1", "name": "read", "arguments": "",
        });
        let mut called = call.clone();
        called["arguments"] = json!("{\"path\":\"/a\"}");
        let thought = |part: usize, delta: &str| {
            json!({
                "type": "response.reasoning_summary_text.delta",
                "output_index": 0, "summary_index": part, "delta": delta,
            })
        };
        let args = |delta: &str| {
            json!({
                "type": "response.function_call_arguments.delta",
                "output_index": 1, "item_id": "fc_1", "delta": delta,
            })
        };
        let unsummarized = json!({ "type": "reasoning", "id": "rs_1", "summary": [] });
        let usage = json!({ "input_tokens": 4, "output_tokens": 9 });
        sse(&[
            json!({ "type": "response.created", "response": { "id": "resp_1" } }),
            json!({ "type": "response.output_item.added", "output_index": 0, "item": unsummarized }),
            thought(0, "Find "),
            thought(0, "the note."),
            thought(1, "Read it."),
            json!({ "type": "response.output_item.done", "output_index": 0, "item": rs1() }),
            json!({ "type": "response.output_item.added", "output_index": 1, "item": call }),
            args("{\"pa"),
            args("th\":\"/a\"}"),
            json!({ "type": "response.output_item.done", "output_index": 1, "item": called }),
            json!({ "type": "response.completed", "response": { "usage": usage } }),
        ])
    }

    #[test]
    fn text_streams_with_its_usage() {
        let usage = json!({
            "input_tokens": 10,
            "output_tokens": 2,
            "input_tokens_details": { "cached_tokens": 3 },
        });
        let reply = sse(&[
            json!({ "type": "response.output_text.delta", "output_index": 0, "delta": "Hel" }),
            json!({ "type": "response.output_text.delta", "output_index": 0, "delta": "lo" }),
            json!({ "type": "response.completed", "response": { "usage": usage } }),
        ]);
        let (result, deltas) = run(&mock::serve_once(&reply), hi());
        let c = result.unwrap();
        assert_eq!(deltas, [Piece::Text("Hel".into()), Piece::Text("lo".into())]);
        assert_eq!(c.text, "Hello");
        assert_eq!(c.usage, Usage { input: 7, output: 2, cache_read: 3, cache_write: 0 });
    }

    #[test]
    fn a_call_is_assembled_from_its_fragments() {
        let (result, _) = run(&mock::serve_once(&reasoned_call()), hi());
        let c = result.unwrap();
        let read =
            Call { id: "c1".into(), name: "read".into(), args: json!({"path": "/a"}), echo: None };
        assert_eq!(c.calls, [read]);
        assert_eq!(c.usage.output, 9);
    }

    /// Each summary part is a paragraph of the thinking, and the reasoning
    /// item rides on the call as it came.
    #[test]
    fn a_reasoning_summary_streams_as_thinking_and_is_kept() {
        let (result, pieces) = run(&mock::serve_once(&reasoned_call()), thinking_hard());
        let c = result.unwrap();
        assert_eq!(c.thinking, "Find the note.\n\nRead it.");
        assert_eq!(pieces[0], Piece::Thinking("Find ".into()));
        assert_eq!(pieces[2], Piece::Thinking("\n\nRead it.".into()));
        let echo = c.calls[0].echo.as_ref().unwrap();
        assert_eq!(echo.provider, "mock");
        assert_eq!(echo.content, json!({ "reasoning": [rs1()] }));
    }

    /// Reasoning goes back unmodified just before the calls it led to, and
    /// only to the provider it came from. A failed result is just its text.
    #[test]
    fn reasoning_goes_back_in_place_and_to_nobody_else() {
        let echo = Echo { provider: "mock".into(), content: json!({ "reasoning": [rs1()] }) };
        let call = Call {
            id: "c1".into(),
            name: "read".into(),
            args: json!({"path": "/a"}),
            echo: Some(echo),
        };
        let failed =
            ToolResult { id: "c1".into(), text: "no such note".into(), ok: false, media: vec![] };
        let turns = || {
            vec![
                Turn::User("u".into()),
                Turn::Assistant { text: "Looking.".into(), calls: vec![call.clone()] },
                Turn::ToolResults(vec![failed.clone()]),
            ]
        };
        let (url, rx) = mock::serve_capturing(&reasoned_call());
        run(&url, Request { turns: turns(), ..thinking_hard() })
            .0
            .unwrap();
        let sent: Value = serde_json::from_str(&rx.recv().unwrap()).unwrap();
        let mut input = vec![
            json!({ "role": "user", "content": "u" }),
            json!({ "role": "assistant", "content": "Looking." }),
            rs1(),
            json!({
                "type": "function_call", "call_id": "c1", "name": "read",
                "arguments": "{\"path\":\"/a\"}",
            }),
            json!({ "type": "function_call_output", "call_id": "c1", "output": "no such note" }),
        ];
        assert_eq!(sent["input"], json!(input));

        let other = Provider { name: "other".into(), ..provider("http://unused") };
        let req = Request { turns: turns(), ..thinking_hard() };
        input.remove(2);
        assert_eq!(body(&other, &req)["input"], json!(input));
    }

    #[test]
    fn the_request_is_stateless_with_flat_tools() {
        let tool = ToolSchema {
            name: "read".into(),
            description: "d".into(),
            parameters: json!({ "type": "object" }),
        };
        let sent = body(&provider("http://unused"), &Request { tools: vec![tool], ..hi() });
        assert_eq!(sent["instructions"], "s");
        assert_eq!((&sent["store"], &sent["stream"]), (&json!(false), &json!(true)));
        let flat = json!({
            "type": "function", "name": "read", "description": "d",
            "parameters": { "type": "object" }, "strict": false,
        });
        assert_eq!(sent["tools"], json!([flat]));
    }

    /// An effort asks for summaries and sealed reasoning with it; without
    /// one the request says nothing about reasoning.
    #[test]
    fn the_effort_setting_rides_as_reasoning() {
        let provider = provider("http://unused");
        let set = body(&provider, &thinking_hard());
        assert_eq!(set["reasoning"], json!({ "effort": "high", "summary": "auto" }));
        assert_eq!(set["include"], json!(["reasoning.encrypted_content"]));
        let unset = body(&provider, &hi());
        assert_eq!((&unset["reasoning"], &unset["include"]), (&Value::Null, &Value::Null));
    }

    #[test]
    fn a_failed_response_surfaces_its_message() {
        let error = json!({ "code": "server_error", "message": "The model overloaded." });
        let failed = json!({ "type": "response.failed", "response": { "error": error } });
        let flat = json!({ "type": "error", "code": "rate_limit", "message": "Slow down." });
        let nested = json!({ "type": "error", "error": { "message": "Bad key." } });
        for (event, message) in
            [(failed, "The model overloaded."), (flat, "Slow down."), (nested, "Bad key.")]
        {
            let (result, _) = run(&mock::serve_once(&sse(&[event])), hi());
            assert_eq!(result.unwrap_err(), format!("provider error: {message}"));
        }
    }

    /// Server-side search, and whatever else arrives unrecognized, passes by.
    #[test]
    fn unknown_events_and_items_are_passed_over() {
        let search = json!({ "type": "web_search_call", "id": "ws_1", "status": "completed" });
        let reply = sse(&[
            json!({ "type": "response.output_item.added", "output_index": 0, "item": search }),
            json!({ "type": "response.web_search_call.searching", "delta": { "odd": 1 } }),
            json!({ "type": "response.output_item.done", "output_index": 0, "item": search }),
            json!({ "type": "response.output_text.delta", "output_index": 1, "delta": "Found." }),
            json!({ "type": "response.completed", "response": {} }),
        ]);
        let (result, _) = run(&mock::serve_once(&reply), hi());
        let c = result.unwrap();
        assert_eq!((c.text.as_str(), c.calls.len()), ("Found.", 0));
    }

    #[test]
    fn the_host_picks_the_dialect() {
        for (base_url, responses) in [
            ("https://api.openai.com/v1", true),
            ("https://api.x.ai/v1", true),
            ("https://openrouter.ai/api/v1", false),
            ("https://api.groq.com/openai/v1", false),
            ("http://localhost:11434/v1", false),
        ] {
            assert_eq!(provider(base_url).responses(), responses, "{base_url}");
        }
        let anthropic = Provider { kind: Kind::Anthropic, ..provider("https://api.openai.com/v1") };
        assert!(!anthropic.responses());
    }
    /// What the provider ran itself is reported with what came of it: a
    /// search with its sources and what the reply cites, code with what it
    /// printed, a search of X with what it asked.
    #[test]
    fn what_the_provider_ran_comes_back_with_its_results() {
        let found = json!({ "type": "web_search_call", "action": {
            "query": "rust", "sources": [{ "url": "https://a.test/x/" }, { "type": "api" }] } });
        let ran = json!({ "type": "code_interpreter_call", "code": "print(2)", "outputs": [
            { "type": "logs", "logs": "{\"stdout\":\"2\\n\",\"stderr\":\"\"}" }] });
        let x = json!({ "type": "custom_tool_call", "name": "x_keyword_search",
            "input": "{\"query\":\"from:rustlang\"}" });
        assert_eq!(served(&found).unwrap().result, "- [a.test/x](https://a.test/x/)\n");
        let code = served(&ran).unwrap();
        assert_eq!((code.name.as_str(), code.result.as_str()), ("code", "2"));
        assert_eq!(served(&x).unwrap().args, json!({ "query": "from:rustlang" }));
        assert_eq!(served(&json!({ "type": "custom_tool_call", "name": "other" })), None);
        let opened = json!({ "type": "web_search_call", "action": { "type": "open_page", "url": "https://a.test" } });
        assert_eq!(served(&opened).unwrap().args, json!({ "url": "https://a.test" }));
        assert_eq!(
            served(&json!({ "type": "web_search_call", "action": { "type": "find" } })),
            None
        );

        let mut out = Completion { served: vec![served(&found).unwrap()], ..Default::default() };
        let cited = json!({ "content": [{ "annotations": [
            { "type": "url_citation", "url": "https://b.test", "title": "B" },
            { "type": "url_citation", "url": "https://a.test/x/", "title": "1" }] }] });
        cite(&mut out, &cited);
        let sources = "- [B](https://b.test)\n- [a.test/x](https://a.test/x/)\n";
        assert_eq!(out.served[0].result, sources);
    }

    /// A picture the provider made comes back as its bytes, to be kept.
    #[test]
    fn a_picture_the_provider_made_comes_as_a_file() {
        let made = json!({ "type": "image_generation_call", "result": "AAEC",
            "output_format": "png", "revised_prompt": "a red circle" });
        let made = served(&made).unwrap();
        assert_eq!(made.name, "generate_image");
        assert_eq!(made.args, json!({ "prompt": "a red circle" }));
        assert_eq!(made.file, Some(("png".into(), vec![0, 1, 2])));
    }

    /// Each host is offered the tools it runs itself, beside ours.
    #[test]
    fn each_host_is_offered_its_own_tools() {
        let kinds = |base_url: &str| -> Vec<String> {
            let tools = body(&provider(base_url), &Request::default())["tools"].clone();
            let tools = tools.as_array().cloned().unwrap_or_default();
            tools
                .iter()
                .map(|t| t["type"].as_str().unwrap().to_string())
                .collect()
        };
        let openai = ["web_search", "code_interpreter", "image_generation"];
        assert_eq!(kinds("https://api.openai.com/v1"), openai);
        assert_eq!(kinds("https://api.x.ai/v1"), ["web_search", "x_search", "code_interpreter"]);
        assert!(kinds("http://localhost:1/v1").is_empty());
    }
}
