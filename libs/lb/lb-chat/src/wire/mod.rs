//! Provider-neutral request and completion types, and the three wire
//! dialects that carry them: OpenAI-compatible chat completions, the
//! Responses API, and Anthropic messages.

pub mod anthropic;
pub mod openai;
pub mod responses;

use std::time::Duration;

use lb_rs::model::chat::Usage;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::mpsc::UnboundedSender;

use crate::provider::{Kind, Provider};

#[derive(Default)]
pub struct Request {
    pub system: String,
    pub turns: Vec<Turn>,
    pub tools: Vec<ToolSchema>,
    /// The chat's `Settings::effort`; each dialect says it its own way.
    pub effort: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Turn {
    User(String),
    Assistant { text: String, calls: Vec<Call> },
    ToolResults(Vec<ToolResult>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Call {
    pub id: String,
    pub name: String,
    pub args: Value,
    pub echo: Option<Echo>,
}

/// Something a provider attached to a call and expects back with it: Google
/// signs each function call. It means nothing to any other provider.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Echo {
    pub provider: String,
    pub content: Value,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ToolResult {
    pub id: String,
    pub text: String,
    pub ok: bool,
}

/// A flat JSON-schema object with `additionalProperties: false`.
#[derive(Clone, Debug, PartialEq)]
pub struct ToolSchema {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

/// A piece of a completion as it streams.
#[derive(Clone, Debug, PartialEq)]
pub enum Piece {
    Text(String),
    /// Whatever the provider shows of the model's thinking.
    Thinking(String),
}

/// A call the provider made and answered itself: a web search, code it ran.
#[derive(Clone, Debug, PartialEq)]
pub struct Served {
    pub name: String,
    pub args: Value,
    /// What came of it, as markdown: sources as a list of links, output as text.
    pub result: String,
    /// What the provider wants back to stand by what it then said.
    pub echo: Option<Echo>,
}

#[derive(Debug, Default)]
pub struct Completion {
    pub text: String,
    pub thinking: String,
    /// In the order the provider made them, all before `text`.
    pub served: Vec<Served>,
    pub calls: Vec<Call>,
    pub usage: Usage,
}

pub const STREAM_IDLE: Duration = Duration::from_secs(120);
const MAX_ATTEMPTS: u32 = 3;

/// Run one completion, sending its pieces as they arrive. Dropping the
/// future cancels the request; the caller keeps the pieces it has seen.
pub async fn complete(
    client: &reqwest::Client, provider: &Provider, req: &Request, deltas: &UnboundedSender<Piece>,
) -> Result<Completion, String> {
    match provider.kind {
        Kind::OpenAi if provider.responses() => {
            responses::complete(client, provider, req, deltas).await
        }
        Kind::OpenAi => openai::complete(client, provider, req, deltas).await,
        Kind::Anthropic => anthropic::complete(client, provider, req, deltas).await,
    }
}

/// A source as a line of a markdown list; one with no title of its own is
/// named for its address.
pub(crate) fn link(title: &str, url: &str) -> String {
    let bare = url.split("://").last().unwrap_or(url).trim_end_matches('/');
    let named = !title.is_empty() && title.parse::<u32>().is_err();
    format!("- [{}]({url})\n", if named { title } else { bare })
}

/// Empty text is `{}`; malformed text is wrapped as `{"raw": text}` so the
/// tool can answer with a steering error instead of the turn dying.
pub fn parse_args(text: &str) -> Value {
    if text.trim().is_empty() {
        serde_json::json!({})
    } else {
        serde_json::from_str(text).unwrap_or_else(|_| serde_json::json!({ "raw": text }))
    }
}

/// Sends `body`, retrying rate limits and server errors a few times before
/// the stream starts. Non-success responses surface their body.
pub(crate) async fn send(
    client: &reqwest::Client, url: &str, headers: &[(&str, String)], body: &Value,
) -> Result<reqwest::Response, String> {
    let mut attempts = 0;
    loop {
        attempts += 1;
        let mut request = client.post(url).json(body);
        for (name, value) in headers {
            request = request.header(*name, value);
        }
        let resp = request.send().await.map_err(unsent)?;
        let status = resp.status();
        if status.is_success() {
            return Ok(resp);
        }
        if (status.as_u16() == 429 || status.is_server_error()) && attempts < MAX_ATTEMPTS {
            let wait = resp
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(2 * attempts as u64)
                .min(30);
            tokio::time::sleep(Duration::from_secs(wait)).await;
            continue;
        }
        return Err(explain(status, &resp.text().await.unwrap_or_default()));
    }
}

/// Why a request got no response. A server that is not there reads as its
/// address; reqwest's own text for that is "error sending request".
pub(crate) fn unsent(err: reqwest::Error) -> String {
    let target = err.url().and_then(|url| {
        let host = url.host_str()?;
        Some(
            url.port()
                .map_or(host.to_string(), |port| format!("{host}:{port}")),
        )
    });
    match target {
        Some(target) if err.is_connect() || err.is_timeout() => format!("can't reach {target}"),
        _ => format!("request failed: {}", err.without_url()),
    }
}

/// A failed response as a sentence: the status and the message inside the
/// provider's JSON, whichever of the usual envelopes it came in.
pub(crate) fn explain(status: reqwest::StatusCode, body: &str) -> String {
    let message = serde_json::from_str::<Value>(body).ok().and_then(|v| {
        let v = match v {
            Value::Array(mut items) if !items.is_empty() => items.swap_remove(0),
            v => v,
        };
        let error = v.get("error").unwrap_or(&v);
        error
            .get("message")
            .or_else(|| v.get("message"))
            .or(Some(error))
            .and_then(Value::as_str)
            .map(str::to_string)
    });
    let detail: String = message
        .unwrap_or_else(|| body.trim().to_string())
        .chars()
        .take(500)
        .collect();
    if detail.is_empty() { status.to_string() } else { format!("{status}: {detail}") }
}

/// Splits a byte stream into SSE `data:` payloads. Bytes are decoded per
/// complete line, since network chunks split multi-byte characters.
#[derive(Default)]
pub(crate) struct Sse {
    buf: Vec<u8>,
}

impl Sse {
    pub(crate) fn push(&mut self, bytes: &[u8]) -> Vec<String> {
        self.buf.extend_from_slice(bytes);
        let mut out = Vec::new();
        while let Some(nl) = self.buf.iter().position(|&b| b == b'\n') {
            let line: Vec<u8> = self.buf.drain(..=nl).collect();
            let line = String::from_utf8_lossy(&line);
            if let Some(payload) = line.trim().strip_prefix("data:") {
                out.push(payload.trim().to_string());
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Providers wrap what went wrong in JSON of a few shapes; the user
    /// reads the sentence, not the envelope.
    #[test]
    fn errors_read_as_their_message() {
        let status = reqwest::StatusCode::NOT_FOUND;
        for (body, message) in [
            (
                "{\n  \"error\": {\n    \"message\": \"Use the v1/responses endpoint.\",\n    \"type\": \"x\"\n  }\n}",
                "Use the v1/responses endpoint.",
            ),
            ("[{\"error\": {\"code\": 404, \"message\": \"High demand.\"}}]", "High demand."),
            (
                "{\"message\":\"Model is archived.\",\"type\":\"model_archived_error\"}",
                "Model is archived.",
            ),
            ("{\"error\":\"invalid api key\"}", "invalid api key"),
            ("no key", "no key"),
        ] {
            assert_eq!(explain(status, body), format!("404 Not Found: {message}"), "{body}");
        }
        assert_eq!(explain(status, "  "), "404 Not Found");
    }

    #[test]
    fn sse_splits_lines_and_survives_a_split_character() {
        let mut sse = Sse::default();
        let text = "data: {\"a\":\"é\"}\n\ndata: [DONE]\n";
        let bytes = text.as_bytes();
        let cut = text.find('é').unwrap() + 1;
        let mut got = sse.push(&bytes[..cut]);
        got.extend(sse.push(&bytes[cut..]));
        assert_eq!(got, ["{\"a\":\"é\"}", "[DONE]"]);
    }

    #[test]
    fn args_parse_or_wrap() {
        assert_eq!(parse_args(""), serde_json::json!({}));
        assert_eq!(parse_args("{\"a\":1}"), serde_json::json!({"a": 1}));
        assert_eq!(parse_args("{nope"), serde_json::json!({"raw": "{nope"}));
    }
}
