//! Apple Intelligence through the bridge crate: the device's own model,
//! reached without a network. One native session is kept alive across a
//! round of tool calls, so a call costs no second prefill: the dialect
//! answers with the call, the driver runs it, and the next completion
//! hands the result back into the same session.

use std::sync::Mutex;
use std::time::Duration;

use tokio::sync::mpsc::UnboundedSender;

use super::{Call, Completion, Piece, Request, Turn};

/// What the one model is called on the wire.
pub const MODEL: &str = "on-device";
/// The model's window, input and output together.
pub const WINDOW: u64 = 8192;
/// Tokens a reply may run to.
const REPLY_TOKENS: u32 = 768;
/// How long a completion waits on the model between pieces.
const IDLE: Duration = Duration::from_secs(120);

/// Whether the device's model can be used, else why not.
pub fn available() -> Result<(), String> {
    native::available()
}

/// Runs one completion: a new session, or the one waiting on the result
/// of the call it made last time.
pub async fn complete(
    req: &Request, deltas: &UnboundedSender<Piece>,
) -> Result<Completion, String> {
    native::complete(req, deltas).await
}

/// The native session waiting on a tool result, and the call it made.
static PENDING: Mutex<Option<(native::Request, String)>> = Mutex::new(None);

fn take_pending() -> Option<(native::Request, String)> {
    PENDING.lock().unwrap_or_else(|e| e.into_inner()).take()
}

fn keep_pending(request: native::Request, call: String) {
    *PENDING.lock().unwrap_or_else(|e| e.into_inner()) = Some((request, call));
}

/// The answer to the pending call: the last tool result in the transcript,
/// which is the one the driver just ran.
fn last_result(req: &Request) -> Option<String> {
    req.turns.iter().rev().find_map(|turn| match turn {
        Turn::ToolResults(results) => results
            .last()
            .map(|r| if r.ok { r.text.clone() } else { format!("The tool failed: {}", r.text) }),
        _ => None,
    })
}

#[cfg(target_os = "macos")]
mod native {
    use lb_apple_ai::{Event, Input, Message, Tool};

    use super::*;
    use crate::wire::{Turn, parse_args};

    pub type Request = lb_apple_ai::Request;

    pub fn available() -> Result<(), String> {
        lb_apple_ai::availability()
    }

    /// The transcript as the bridge takes it: earlier tool results read as
    /// context the user supplied.
    fn messages(req: &Request_) -> Vec<Message> {
        let mut out = Vec::new();
        for turn in &req.turns {
            match turn {
                Turn::User(text) => out.push(Message { role: "user", content: text.clone() }),
                Turn::Assistant { text, .. } if !text.is_empty() => {
                    out.push(Message { role: "assistant", content: text.clone() })
                }
                Turn::Assistant { .. } => {}
                Turn::ToolResults(results) => {
                    for result in results {
                        let content = format!("Earlier tool result:\n{}", result.text);
                        out.push(Message { role: "user", content });
                    }
                }
            }
        }
        out
    }

    type Request_ = super::Request;

    pub async fn complete(
        req: &Request_, deltas: &UnboundedSender<Piece>,
    ) -> Result<Completion, String> {
        let mut request = match take_pending() {
            Some((request, call)) => {
                let answer = last_result(req).ok_or("the tool's result is missing")?;
                request.tool_result(&call, &answer)?;
                request
            }
            None => lb_apple_ai::start(Input {
                instructions: req.system.clone(),
                messages: messages(req),
                max_tokens: REPLY_TOKENS,
                tools: req
                    .tools
                    .iter()
                    .map(|t| Tool {
                        name: t.name.clone(),
                        description: t.description.clone(),
                        parameters: t.parameters.clone(),
                    })
                    .collect(),
            })?,
        };
        let mut done = Completion::default();
        loop {
            let event = tokio::time::timeout(IDLE, request.next())
                .await
                .map_err(|_| "Apple Intelligence stopped answering")?;
            match event {
                Some(Event::Delta(text)) => {
                    done.text.push_str(&text);
                    let _ = deltas.send(Piece::Text(text));
                }
                Some(Event::ToolCall(call)) => {
                    let id = call.id.clone();
                    done.calls.push(Call {
                        id: call.id,
                        name: call.name,
                        args: match call.args {
                            serde_json::Value::String(text) => parse_args(&text),
                            args => args,
                        },
                        echo: None,
                    });
                    keep_pending(request, id);
                    return Ok(done);
                }
                Some(Event::Done) => return Ok(done),
                Some(Event::Error(e)) => return Err(e),
                None => return Err("Apple Intelligence stopped without answering".into()),
            }
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod native {
    use super::*;

    pub struct Request;

    const ELSEWHERE: &str = "Apple Intelligence runs only in the Mac app for now";

    pub fn available() -> Result<(), String> {
        Err(ELSEWHERE.into())
    }

    pub async fn complete(
        _req: &Request, _deltas: &UnboundedSender<Piece>,
    ) -> Result<Completion, String> {
        Err(ELSEWHERE.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::ToolResult;

    #[test]
    fn the_pending_calls_answer_is_the_last_result() {
        let result = |id: &str, text: &str, ok: bool| ToolResult {
            id: id.into(),
            text: text.into(),
            ok,
            media: vec![],
        };
        let req = Request {
            turns: vec![
                Turn::User("q".into()),
                Turn::ToolResults(vec![result("a", "first", true)]),
                Turn::Assistant { text: String::new(), calls: vec![] },
                Turn::ToolResults(vec![result("b", "gone", false)]),
            ],
            ..Default::default()
        };
        assert_eq!(last_result(&req).as_deref(), Some("The tool failed: gone"));
        assert_eq!(last_result(&Request::default()), None);
    }
}
