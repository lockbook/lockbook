//! OpenAI-compatible chat completions: OpenAI, xAI, Groq, Cerebras, Together,
//! OpenRouter, Google's compatibility layer, and every self-hosted server.

use std::sync::Mutex;

use futures::StreamExt;
use lb_rs::model::chat::Usage;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::mpsc::UnboundedSender;

use super::{Call, Completion, Echo, Piece, Request, STREAM_IDLE, Sse, Turn, parse_args, send};
use crate::provider::{Provider, host};

const GOOGLE: &str = "generativelanguage.googleapis.com";
/// What Google accepts in place of a signature on a call it did not make.
const UNSIGNED: &str = "skip_thought_signature_validator";

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

#[derive(Deserialize, Default)]
struct Delta {
    #[serde(default)]
    content: Option<String>,
    /// Thinking, as llama.cpp, xAI, and DeepSeek name it.
    #[serde(default)]
    reasoning_content: Option<Value>,
    /// Thinking, as OpenRouter, Groq, Cerebras, and Ollama name it.
    #[serde(default)]
    reasoning: Option<Value>,
    #[serde(default)]
    tool_calls: Vec<CallDelta>,
}

#[derive(Deserialize)]
struct CallDelta {
    #[serde(default)]
    index: usize,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    function: Option<FunctionDelta>,
    /// Google's compatibility layer: the call's thought signature.
    #[serde(default)]
    extra_content: Option<Value>,
}

#[derive(Deserialize)]
struct FunctionDelta {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    arguments: Option<String>,
}

#[derive(Deserialize)]
struct WireUsage {
    #[serde(default)]
    prompt_tokens: u64,
    #[serde(default)]
    completion_tokens: u64,
    #[serde(default)]
    prompt_tokens_details: Option<PromptDetails>,
}

#[derive(Deserialize)]
struct PromptDetails {
    #[serde(default)]
    cached_tokens: u64,
}

#[derive(Default)]
struct Partial {
    id: String,
    name: String,
    args: String,
    extra: Option<Value>,
}

/// `(base_url, model)` pairs that refused tools unless reasoning is off, as
/// OpenAI's models from GPT-5.6 on do at this endpoint.
static REASONING_OFF: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());

fn reasoning_off(provider: &Provider) -> bool {
    let known = REASONING_OFF.lock().unwrap();
    known
        .iter()
        .any(|(url, model)| *url == provider.base_url && *model == provider.model)
}

/// What a model says through us when it cannot use tools at this endpoint
/// with reasoning on or off, as GPT-6 Astra cannot.
fn responses_only(provider: &Provider) -> String {
    format!(
        "{} uses tools only through OpenAI's Responses API, which is not supported yet",
        provider.model
    )
}

pub(crate) fn body(provider: &Provider, req: &Request) -> Value {
    let mut messages = Vec::new();
    if !req.system.is_empty() {
        messages.push(json!({ "role": "system", "content": req.system }));
    }
    for turn in &req.turns {
        match turn {
            Turn::User(text) => messages.push(json!({ "role": "user", "content": text })),
            Turn::Assistant { text, calls } => {
                let mut m = json!({ "role": "assistant" });
                m["content"] =
                    if text.is_empty() && !calls.is_empty() { Value::Null } else { json!(text) };
                if !calls.is_empty() {
                    m["tool_calls"] = calls
                        .iter()
                        .map(|c| {
                            let mut call = json!({
                                "id": c.id,
                                "type": "function",
                                "function": { "name": c.name, "arguments": c.args.to_string() },
                            });
                            if let Some(echo) =
                                c.echo.as_ref().filter(|e| e.provider == provider.name)
                            {
                                call["extra_content"] = echo.content.clone();
                            } else if host(&provider.base_url) == GOOGLE {
                                // A call Google did not sign: one the driver
                                // made, or another provider's.
                                call["extra_content"] =
                                    json!({ "google": { "thought_signature": UNSIGNED } });
                            }
                            call
                        })
                        .collect();
                }
                messages.push(m);
            }
            Turn::ToolResults(results) => {
                for r in results {
                    messages
                        .push(json!({ "role": "tool", "tool_call_id": r.id, "content": r.text }));
                }
                // A tool's answer is text here, so what it read to be
                // looked at follows as the user's.
                let seen: Vec<Value> = results
                    .iter()
                    .flat_map(|r| &r.media)
                    .map(|i| match i.is_pdf() {
                        true => {
                            let file = json!({ "filename": "document.pdf", "file_data": i.url() });
                            json!({ "type": "file", "file": file })
                        }
                        false => json!({ "type": "image_url", "image_url": { "url": i.url() } }),
                    })
                    .collect();
                if !seen.is_empty() {
                    messages.push(json!({ "role": "user", "content": seen }));
                }
            }
        }
    }
    let mut body = json!({
        "model": provider.model,
        "messages": messages,
        "stream": true,
        "stream_options": { "include_usage": true },
    });
    if !req.tools.is_empty() {
        body["tools"] = req
            .tools
            .iter()
            .map(|t| {
                json!({
                    "type": "function",
                    "function": {
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.parameters,
                    },
                })
            })
            .collect();
    }
    if !req.tools.is_empty() && reasoning_off(provider) {
        body["reasoning_effort"] = json!("none");
    } else if let Some(effort) = &req.effort {
        body["reasoning_effort"] = json!(effort);
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
    let url = format!("{}/chat/completions", provider.base_url);
    let resp = match send(client, &url, &headers, &body(provider, req)).await {
        // The model takes tools here only with reasoning off and says so;
        // once that works it is asked that way from the start.
        Err(e)
            if e.starts_with("400")
                && e.contains("reasoning_effort")
                && e.contains("'none'")
                && !reasoning_off(provider) =>
        {
            let mut unreasoned = body(provider, req);
            unreasoned["reasoning_effort"] = json!("none");
            let resp = send(client, &url, &headers, &unreasoned)
                .await
                .map_err(|e| if e.starts_with("400") { responses_only(provider) } else { e })?;
            let model = (provider.base_url.clone(), provider.model.clone());
            REASONING_OFF.lock().unwrap().push(model);
            resp
        }
        other => other?,
    };

    let mut out = Completion::default();
    let mut partials: Vec<Partial> = Vec::new();
    let mut sse = Sse::default();
    let mut stream = resp.bytes_stream();
    loop {
        let item = tokio::time::timeout(STREAM_IDLE, stream.next())
            .await
            .map_err(|_| "provider stopped responding mid-stream".to_string())?;
        let Some(bytes) = item else { break };
        let bytes = bytes.map_err(|e| format!("stream failed: {e}"))?;
        for payload in sse.push(&bytes) {
            if payload == "[DONE]" {
                out.calls = finish(partials, &provider.name);
                return Ok(out);
            }
            let Ok(chunk) = serde_json::from_str::<Chunk>(&payload) else { continue };
            if let Some(u) = chunk.usage {
                let cached = u.prompt_tokens_details.map_or(0, |d| d.cached_tokens);
                out.usage = Usage {
                    input: u.prompt_tokens.saturating_sub(cached),
                    output: u.completion_tokens,
                    cache_read: cached,
                    cache_write: 0,
                };
            }
            let Some(choice) = chunk.choices.into_iter().next() else { continue };
            let thinking = [&choice.delta.reasoning_content, &choice.delta.reasoning];
            for text in thinking.into_iter().flatten().filter_map(Value::as_str) {
                out.thinking.push_str(text);
                let _ = deltas.send(Piece::Thinking(text.to_string()));
            }
            if let Some(text) = choice.delta.content.filter(|t| !t.is_empty()) {
                out.text.push_str(&text);
                let _ = deltas.send(Piece::Text(text));
            }
            for frag in choice.delta.tool_calls {
                if partials.len() <= frag.index {
                    partials.resize_with(frag.index + 1, Default::default);
                }
                let call = &mut partials[frag.index];
                if let Some(id) = frag.id {
                    call.id = id;
                }
                if frag.extra_content.is_some() {
                    call.extra = frag.extra_content;
                }
                if let Some(f) = frag.function {
                    if let Some(name) = f.name {
                        call.name = name;
                    }
                    if let Some(args) = f.arguments {
                        call.args.push_str(&args);
                    }
                }
            }
        }
    }
    out.calls = finish(partials, &provider.name);
    Ok(out)
}

fn finish(partials: Vec<Partial>, provider: &str) -> Vec<Call> {
    partials
        .into_iter()
        .filter(|p| !p.name.is_empty())
        .enumerate()
        .map(|(i, p)| Call {
            id: if p.id.is_empty() { format!("call_{i}") } else { p.id },
            name: p.name,
            args: parse_args(&p.args),
            echo: p
                .extra
                .map(|content| Echo { provider: provider.to_string(), content }),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::{self, SSE_HELLO};
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

    /// Google wants every call signed. One it did not make, such as the
    /// read of an attached note, goes with the placeholder it accepts;
    /// nobody else is sent one.
    #[test]
    fn a_call_google_did_not_sign_goes_with_its_placeholder() {
        let call = Call { id: "c".into(), name: "read".into(), args: json!({}), echo: None };
        let req = Request {
            turns: vec![
                Turn::User("hi".into()),
                Turn::Assistant { text: String::new(), calls: vec![call] },
            ],
            ..Default::default()
        };
        let sent = |base_url: &str| {
            let sent = body(&provider(base_url), &req);
            let asked = sent["messages"].as_array().unwrap().last().unwrap();
            asked["tool_calls"][0]["extra_content"].clone()
        };
        let google = sent("https://generativelanguage.googleapis.com/v1beta/openai");
        assert_eq!(google["google"]["thought_signature"], UNSIGNED);
        assert_eq!(sent("https://api.x.ai/v1"), Value::Null);
    }

    /// Thinking comes beside the reply under either of two names.
    #[test]
    fn thinking_streams_apart_from_the_reply() {
        const SSE: &str = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n\
            data: {\"choices\":[{\"delta\":{\"reasoning_content\":\"9:40 and \"}}]}\n\n\
            data: {\"choices\":[{\"delta\":{\"reasoning\":\"2:35\",\"content\":null}}]}\n\n\
            data: {\"choices\":[{\"delta\":{\"content\":\"12:15\"}}]}\n\ndata: [DONE]\n\n";
        let (result, pieces) = run(&mock::serve_once(SSE), hi());
        let c = result.unwrap();
        assert_eq!((c.thinking.as_str(), c.text.as_str()), ("9:40 and 2:35", "12:15"));
        assert_eq!(pieces[0], Piece::Thinking("9:40 and ".into()));
    }

    #[test]
    fn streams_deltas_and_usage() {
        let (result, deltas) = run(&mock::serve_once(SSE_HELLO), hi());
        let c = result.unwrap();
        assert_eq!(deltas, [Piece::Text("Hel".into()), Piece::Text("lo".into())]);
        assert_eq!(c.text, "Hello");
        assert_eq!(c.usage, Usage { input: 7, output: 2, cache_read: 3, cache_write: 0 });
    }

    #[test]
    fn utf8_survives_a_chunk_split_mid_character() {
        const SSE: &str = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n\
            data: {\"choices\":[{\"delta\":{\"content\":\"héllo\"}}]}\n\ndata: [DONE]\n\n";
        let split = SSE.find('é').unwrap() + 1;
        let (result, _) = run(&mock::serve_split(SSE, split), hi());
        assert_eq!(result.unwrap().text, "héllo");
    }

    #[test]
    fn tool_calls_accumulate_across_fragments() {
        const SSE: &str = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n\
            data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"c1\",\"function\":{\"name\":\"read\",\"arguments\":\"{\\\"pa\"}}]}}]}\n\n\
            data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"th\\\":\\\"/a\\\"}\"}}]}}]}\n\n\
            data: [DONE]\n\n";
        let (result, _) = run(&mock::serve_once(SSE), hi());
        let c = result.unwrap();
        assert_eq!(
            c.calls,
            [Call {
                id: "c1".into(),
                name: "read".into(),
                args: json!({"path": "/a"}),
                echo: None
            }]
        );
    }

    #[test]
    fn call_turns_and_results_ride_the_wire() {
        let (url, rx) = mock::serve_capturing(SSE_HELLO);
        let req = Request {
            system: String::new(),
            turns: vec![
                Turn::User("u".into()),
                Turn::Assistant {
                    text: String::new(),
                    calls: vec![Call {
                        id: "c1".into(),
                        name: "read".into(),
                        args: json!({"path": "/a"}),
                        echo: None,
                    }],
                },
                Turn::ToolResults(vec![ToolResult {
                    id: "c1".into(),
                    text: "contents".into(),
                    ok: true,
                    media: vec![],
                }]),
            ],
            ..Default::default()
        };
        run(&url, req).0.unwrap();
        let sent: Value = serde_json::from_str(&rx.recv().unwrap()).unwrap();
        let messages = sent["messages"].as_array().unwrap();
        assert_eq!(messages[1]["content"], Value::Null);
        assert_eq!(messages[1]["tool_calls"][0]["function"]["arguments"], "{\"path\":\"/a\"}");
        assert_eq!(messages[2]["role"], "tool");
        assert_eq!(messages[2]["tool_call_id"], "c1");
    }

    #[test]
    fn error_status_surfaces_body() {
        const ERR: &str = "HTTP/1.1 400 Bad Request\r\nContent-Length: 13\r\nConnection: close\r\n\r\nbad request!!";
        let (result, _) = run(&mock::serve_once(ERR), hi());
        let err = result.unwrap_err();
        assert!(err.starts_with("400") && err.contains("bad request!!"), "{err}");
    }
    /// OpenAI's newer models refuse tools at this endpoint unless reasoning
    /// is off. The refusal is retried that way, and the model is asked that
    /// way from then on.
    #[test]
    fn a_model_that_takes_tools_only_unreasoned_is_asked_that_way() {
        const REFUSAL: &str = "HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n\
            {\"error\":{\"message\":\"Function tools with reasoning_effort are not supported for m in /v1/chat/completions. To use function tools, use /v1/responses or set reasoning_effort to 'none'.\",\"param\":\"reasoning_effort\"}}";
        let replies = vec![REFUSAL.to_string(), SSE_HELLO.to_string(), SSE_HELLO.to_string()];
        let (url, rx) = mock::serve(replies);
        let tool = ToolSchema {
            name: "read".into(),
            description: "d".into(),
            parameters: json!({ "type": "object" }),
        };
        let asked = || Request { tools: vec![tool.clone()], ..hi() };

        assert_eq!(run(&url, asked()).0.unwrap().text, "Hello");
        assert_eq!(run(&url, asked()).0.unwrap().text, "Hello");
        let efforts: Vec<Value> = rx
            .try_iter()
            .map(|sent| serde_json::from_str::<Value>(&sent).unwrap()["reasoning_effort"].clone())
            .collect();
        assert_eq!(efforts, [Value::Null, json!("none"), json!("none")]);
    }

    /// A model that refuses tools with reasoning off as well cannot use them
    /// here at all. It says so in plain words, and is not remembered.
    #[test]
    fn a_model_that_takes_no_tools_here_says_so() {
        const REFUSAL: &str = "HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n\
            {\"error\":{\"message\":\"Function tools with reasoning_effort are not supported for m in /v1/chat/completions. To use function tools, use /v1/responses or set reasoning_effort to 'none'.\"}}";
        const NO_NONE: &str = "HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n\
            {\"error\":{\"message\":\"Unsupported value: 'reasoning_effort' does not support 'none' with this model.\"}}";
        let replies = vec![REFUSAL.to_string(), NO_NONE.to_string(), REFUSAL.to_string()];
        let (url, rx) = mock::serve(replies);
        let tool = ToolSchema {
            name: "read".into(),
            description: "d".into(),
            parameters: json!({ "type": "object" }),
        };
        let asked = Request { tools: vec![tool], ..hi() };

        let err = run(&url, asked).0.unwrap_err();
        assert_eq!(
            err,
            "m uses tools only through OpenAI's Responses API, which is not supported yet"
        );
        assert!(!reasoning_off(&provider(&url)));
        assert_eq!(rx.try_iter().count(), 2);
    }

    #[test]
    fn the_effort_setting_rides_as_reasoning_effort() {
        let set = Request { effort: Some("high".into()), ..hi() };
        let provider = provider("http://unused");
        assert_eq!(body(&provider, &set)["reasoning_effort"], "high");
        assert_eq!(body(&provider, &hi())["reasoning_effort"], Value::Null);
    }
}
