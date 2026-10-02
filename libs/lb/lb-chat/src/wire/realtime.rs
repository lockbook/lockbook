//! The realtime API, as OpenAI serves it: a socket that carries the user's
//! voice up and the model's down, the server deciding whose turn it is.
//! This module speaks the protocol; `voice` holds the conversation.

use futures::{SinkExt, StreamExt};
use lb_rs::model::chat::Usage;
use serde_json::{Value, json};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::{Error as WsError, Message};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

use super::{Call, ToolSchema, Turn, explain, parse_args};
use crate::provider::Provider;

/// Samples a second, both ways, as PCM16 mono.
pub const RATE: u32 = 24_000;
/// Bytes of audio in a millisecond.
pub const BYTES_PER_MS: u64 = RATE as u64 / 1000 * 2;
const VOICE: &str = "marin";
const TRANSCRIBER: &str = "gpt-4o-mini-transcribe";

/// What the server said, as far as the conversation cares.
#[derive(Debug, PartialEq)]
pub enum Incoming {
    /// The session is set up as asked.
    Ready,
    SpeechStarted,
    /// The user's turn `item` is in the conversation; its words follow.
    Committed {
        item: String,
    },
    /// What the user said in `item`, or nothing when it could not be made out.
    Heard {
        item: String,
        text: Option<String>,
    },
    ResponseCreated,
    /// More of the reply `item`, as audio.
    Audio {
        item: String,
        pcm: Vec<u8>,
    },
    /// More of the reply `item`, as words.
    Transcript {
        item: String,
        text: String,
    },
    /// A response is over, finished or not: the replies it spoke, the calls
    /// it made, and what it cost. `detail` says why when it was not finished.
    ResponseDone {
        status: String,
        detail: String,
        spoken: Vec<String>,
        calls: Vec<Call>,
        usage: Usage,
    },
    Error(String),
}

pub struct Socket {
    ws: WebSocketStream<MaybeTlsStream<TcpStream>>,
}

impl Socket {
    /// Opens a session with `model` at the provider's realtime endpoint.
    pub async fn connect(provider: &Provider, model: &str) -> Result<Socket, String> {
        let mut request = endpoint(&provider.base_url, model)
            .into_client_request()
            .map_err(|e| e.to_string())?;
        if let Some(key) = &provider.api_key {
            let bearer = format!("Bearer {key}")
                .parse()
                .map_err(|_| "the API key does not fit in a header")?;
            request.headers_mut().insert("Authorization", bearer);
        }
        let (ws, _) = connect_async(request).await.map_err(unopened)?;
        Ok(Socket { ws })
    }

    pub async fn send(&mut self, event: Value) -> Result<(), String> {
        self.ws
            .send(Message::Text(event.to_string()))
            .await
            .map_err(|e| e.to_string())
    }

    /// The next thing the server says that the conversation has a use
    /// for; nothing once the socket is closed.
    pub async fn next(&mut self) -> Option<Result<Incoming, String>> {
        loop {
            let text = match self.ws.next().await? {
                Ok(Message::Text(text)) => text,
                Ok(Message::Close(_)) => return None,
                Ok(_) => continue,
                Err(e) => return Some(Err(e.to_string())),
            };
            let Ok(event) = serde_json::from_str::<Value>(&text) else { continue };
            if let Some(incoming) = parse(&event) {
                return Some(Ok(incoming));
            }
        }
    }

    pub async fn close(mut self) {
        let _ = self.ws.close(None).await;
    }
}

/// The socket address of a provider's realtime endpoint.
fn endpoint(base_url: &str, model: &str) -> String {
    let socket = match base_url.split_once("://") {
        Some(("https", rest)) => format!("wss://{rest}"),
        Some(("http", rest)) => format!("ws://{rest}"),
        _ => base_url.to_string(),
    };
    format!("{socket}/realtime?model={model}")
}

/// Why the socket did not open: the server's refusal as its message, else
/// what went wrong on the way.
fn unopened(err: WsError) -> String {
    match err {
        WsError::Http(response) => {
            let body = response
                .body()
                .as_deref()
                .map(String::from_utf8_lossy)
                .unwrap_or_default();
            explain(response.status(), &body)
        }
        other => other.to_string(),
    }
}

/// The session as the conversation wants it: the server listens for the
/// end of a thought rather than a pause, answers on its own, and gives
/// way when spoken over.
pub fn session(instructions: &str, tools: &[ToolSchema]) -> Value {
    let tools: Vec<Value> = tools
        .iter()
        .map(|t| {
            json!({ "type": "function", "name": t.name, "description": t.description, "parameters": t.parameters })
        })
        .collect();
    json!({
        "type": "session.update",
        "session": {
            "type": "realtime",
            "instructions": instructions,
            "output_modalities": ["audio"],
            "audio": {
                "input": {
                    "format": { "type": "audio/pcm", "rate": RATE },
                    "turn_detection": {
                        "type": "semantic_vad",
                        "create_response": true,
                        "interrupt_response": true,
                    },
                    "transcription": { "model": TRANSCRIBER },
                },
                "output": { "format": { "type": "audio/pcm", "rate": RATE }, "voice": VOICE },
            },
            "tools": tools,
            "tool_choice": "auto",
        }
    })
}

/// The conversation so far, an event per item, for a session that opens
/// mid-chat. Pictures are not carried.
pub fn items(turns: &[Turn]) -> Vec<Value> {
    let item = |item: Value| json!({ "type": "conversation.item.create", "item": item });
    let mut out = Vec::new();
    for turn in turns {
        match turn {
            Turn::User(text) => out.push(item(json!({
                "type": "message", "role": "user",
                "content": [{ "type": "input_text", "text": text }],
            }))),
            Turn::Assistant { text, calls } => {
                if !text.is_empty() {
                    out.push(item(json!({
                        "type": "message", "role": "assistant",
                        "content": [{ "type": "output_text", "text": text }],
                    })));
                }
                for call in calls {
                    out.push(item(json!({
                        "type": "function_call", "call_id": call_id(&call.id),
                        "name": call.name, "arguments": call.args.to_string(),
                    })));
                }
            }
            Turn::ToolResults(results) => {
                for result in results {
                    out.push(output(&call_id(&result.id), &result.text));
                }
            }
        }
    }
    out
}

/// A call id the server takes: at most 32 characters, which a line's id
/// is without its hyphens.
fn call_id(id: &str) -> String {
    id.replace('-', "")
}

pub fn audio(pcm: &[u8]) -> Value {
    json!({ "type": "input_audio_buffer.append", "audio": base64::encode(pcm) })
}

/// Tells the server the user heard reply `item` only up to `ms`.
pub fn truncate(item: &str, ms: u64) -> Value {
    json!({ "type": "conversation.item.truncate", "item_id": item, "content_index": 0, "audio_end_ms": ms })
}

pub fn output(call_id: &str, text: &str) -> Value {
    json!({
        "type": "conversation.item.create",
        "item": { "type": "function_call_output", "call_id": call_id, "output": text },
    })
}

pub fn respond() -> Value {
    json!({ "type": "response.create" })
}

/// What an event means to the conversation; nothing for one it has no use
/// for.
pub(crate) fn parse(event: &Value) -> Option<Incoming> {
    let text = |v: &Value| v.as_str().unwrap_or_default().to_string();
    let item = text(&event["item_id"]);
    Some(match event["type"].as_str()? {
        "session.updated" => Incoming::Ready,
        "input_audio_buffer.speech_started" => Incoming::SpeechStarted,
        "input_audio_buffer.committed" => Incoming::Committed { item },
        "conversation.item.input_audio_transcription.completed" => {
            Incoming::Heard { item, text: Some(text(&event["transcript"])) }
        }
        "conversation.item.input_audio_transcription.failed" => {
            Incoming::Heard { item, text: None }
        }
        "response.created" => Incoming::ResponseCreated,
        "response.output_audio.delta" => {
            let pcm = base64::decode(event["delta"].as_str()?).ok()?;
            Incoming::Audio { item, pcm }
        }
        "response.output_audio_transcript.delta" => {
            Incoming::Transcript { item, text: text(&event["delta"]) }
        }
        "response.done" => {
            let response = &event["response"];
            let (mut spoken, mut calls) = (Vec::new(), Vec::new());
            for out in response["output"].as_array().into_iter().flatten() {
                match out["type"].as_str() {
                    Some("message") => spoken.push(text(&out["id"])),
                    Some("function_call") => calls.push(Call {
                        id: text(&out["call_id"]),
                        name: text(&out["name"]),
                        args: parse_args(out["arguments"].as_str().unwrap_or_default()),
                        echo: None,
                    }),
                    _ => {}
                }
            }
            let details = &response["status_details"];
            let detail = match details["error"]["message"].as_str() {
                Some(message) => message.to_string(),
                None => text(&details["reason"]),
            };
            let usage = &response["usage"];
            let n = |v: &Value| v.as_u64().unwrap_or_default();
            Incoming::ResponseDone {
                status: text(&response["status"]),
                detail,
                spoken,
                calls,
                usage: Usage {
                    input: n(&usage["input_tokens"]),
                    output: n(&usage["output_tokens"]),
                    cache_read: n(&usage["input_token_details"]["cached_tokens"]),
                    cache_write: 0,
                },
            }
        }
        "error" => Incoming::Error(text(&event["error"]["message"])),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::ToolResult;

    #[test]
    fn the_endpoint_is_the_base_url_as_a_socket() {
        assert_eq!(
            endpoint("https://api.openai.com/v1", "gpt-realtime"),
            "wss://api.openai.com/v1/realtime?model=gpt-realtime"
        );
        assert_eq!(endpoint("http://127.0.0.1:9/v1", "m"), "ws://127.0.0.1:9/v1/realtime?model=m");
    }

    #[test]
    fn a_finished_response_reads_as_its_replies_calls_and_cost() {
        let done = json!({
            "type": "response.done",
            "response": {
                "status": "cancelled",
                "status_details": { "type": "cancelled", "reason": "turn_detected" },
                "output": [
                    { "type": "message", "id": "m1", "role": "assistant" },
                    { "type": "function_call", "call_id": "c1", "name": "read", "arguments": "{\"path\":\"/a.md\"}" },
                ],
                "usage": { "input_tokens": 10, "output_tokens": 4, "input_token_details": { "cached_tokens": 3 } },
            }
        });
        let Some(Incoming::ResponseDone { status, detail, spoken, calls, usage }) = parse(&done)
        else {
            panic!()
        };
        assert_eq!((status.as_str(), detail.as_str()), ("cancelled", "turn_detected"));
        assert_eq!(spoken, ["m1"]);
        assert_eq!((calls[0].id.as_str(), calls[0].name.as_str()), ("c1", "read"));
        assert_eq!(calls[0].args, json!({ "path": "/a.md" }));
        assert_eq!(usage, Usage { input: 10, output: 4, cache_read: 3, cache_write: 0 });

        let failed = json!({ "type": "response.done", "response": { "status": "failed",
            "status_details": { "type": "failed", "error": { "message": "too long" } } } });
        assert!(
            matches!(parse(&failed), Some(Incoming::ResponseDone { detail, .. }) if detail == "too long")
        );
    }

    #[test]
    fn audio_and_words_carry_their_item_and_the_rest_is_passed_over() {
        let audio = json!({ "type": "response.output_audio.delta", "item_id": "m1", "delta": base64::encode([1u8, 2, 3]) });
        assert_eq!(parse(&audio), Some(Incoming::Audio { item: "m1".into(), pcm: vec![1, 2, 3] }));
        let words = json!({ "type": "response.output_audio_transcript.delta", "item_id": "m1", "delta": "Hi" });
        assert_eq!(
            parse(&words),
            Some(Incoming::Transcript { item: "m1".into(), text: "Hi".into() })
        );
        let heard = json!({ "type": "conversation.item.input_audio_transcription.completed", "item_id": "u1", "transcript": "hello" });
        assert_eq!(
            parse(&heard),
            Some(Incoming::Heard { item: "u1".into(), text: Some("hello".into()) })
        );
        assert_eq!(parse(&json!({ "type": "rate_limits.updated" })), None);
        assert_eq!(parse(&json!({ "type": "response.output_item.added", "item": {} })), None);
    }

    /// A session that opens on a chat already under way carries the chat
    /// as items, each kind as the server takes it.
    #[test]
    fn the_conversation_so_far_goes_as_items() {
        let id = "4d0b3f2a-0000-4000-8000-000000000001";
        let call = Call {
            id: id.into(),
            name: "read".into(),
            args: json!({ "path": "/a.md" }),
            echo: None,
        };
        let result = ToolResult { id: id.into(), text: "# A".into(), ok: true, media: vec![] };
        let turns = vec![
            Turn::User("look".into()),
            Turn::Assistant { text: "Sure.".into(), calls: vec![call] },
            Turn::ToolResults(vec![result]),
            Turn::Assistant { text: "It says A.".into(), calls: vec![] },
        ];
        let items: Vec<Value> = items(&turns)
            .into_iter()
            .map(|e| e["item"].clone())
            .collect();
        assert_eq!(items[0]["content"][0], json!({ "type": "input_text", "text": "look" }));
        assert_eq!(items[1]["content"][0], json!({ "type": "output_text", "text": "Sure." }));
        // A line's id is longer than a call id may be.
        let short = id.replace('-', "");
        assert_eq!(short.len(), 32);
        assert_eq!(
            items[2],
            json!({ "type": "function_call", "call_id": short, "name": "read", "arguments": "{\"path\":\"/a.md\"}" })
        );
        assert_eq!(
            items[3],
            json!({ "type": "function_call_output", "call_id": short, "output": "# A" })
        );
        assert_eq!(items[4]["role"], "assistant");
        assert_eq!(items.len(), 5);
    }

    #[test]
    fn the_session_listens_for_the_end_of_a_thought_and_gives_way() {
        let tool = ToolSchema {
            name: "read".into(),
            description: "d".into(),
            parameters: json!({ "type": "object" }),
        };
        let event = session("be brief", &[tool]);
        let session = &event["session"];
        assert_eq!(session["instructions"], "be brief");
        let detection = &session["audio"]["input"]["turn_detection"];
        assert_eq!(detection["type"], "semantic_vad");
        assert_eq!(
            (&detection["create_response"], &detection["interrupt_response"]),
            (&json!(true), &json!(true))
        );
        assert_eq!(session["audio"]["output"]["format"]["rate"], RATE);
        assert_eq!(session["tools"][0]["name"], "read");
        assert_eq!(session["tools"][0]["type"], "function");
    }
}
