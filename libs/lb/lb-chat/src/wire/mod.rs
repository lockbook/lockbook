//! Provider-neutral request and completion types, and the two wire dialects
//! that carry them: OpenAI-compatible chat completions and Anthropic messages.

pub mod anthropic;
pub mod openai;

use std::time::Duration;

use lb_rs::model::chat::Usage;
use serde_json::Value;
use tokio::sync::mpsc::UnboundedSender;

use crate::provider::{Kind, Provider};

pub struct Request {
    pub system: String,
    pub turns: Vec<Turn>,
    pub tools: Vec<ToolSchema>,
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

#[derive(Debug, Default)]
pub struct Completion {
    pub text: String,
    pub calls: Vec<Call>,
    pub usage: Usage,
}

pub const STREAM_IDLE: Duration = Duration::from_secs(120);
const MAX_ATTEMPTS: u32 = 3;

/// Run one completion, sending text deltas as they arrive. Dropping the
/// future cancels the request; the caller keeps the deltas it has seen.
pub async fn complete(
    client: &reqwest::Client, provider: &Provider, req: &Request, deltas: &UnboundedSender<String>,
) -> Result<Completion, String> {
    match provider.kind {
        Kind::OpenAi => openai::complete(client, provider, req, deltas).await,
        Kind::Anthropic => anthropic::complete(client, provider, req, deltas).await,
    }
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
        let resp = request
            .send()
            .await
            .map_err(|e| format!("request failed: {e}"))?;
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
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("{status}: {}", text.chars().take(500).collect::<String>()));
    }
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
