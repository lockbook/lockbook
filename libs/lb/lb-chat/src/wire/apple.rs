//! Apple Intelligence through the bridge crate: the device's own model,
//! reached without a network. One native session is kept alive across a
//! round of tool calls, so a call costs no second prefill: the dialect
//! answers with the call, the driver runs it, and the next completion
//! hands the result back into the same session, if that is what the next
//! completion is; anything else starts afresh and the parked session goes.

use tokio::sync::mpsc::UnboundedSender;

use super::{Completion, Piece, Request};

/// What the one model is called on the wire.
pub const MODEL: &str = "on-device";
/// The model's window, input and output together.
pub const WINDOW: u64 = 8192;

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

/// The answer to the call a session is parked on, when the transcript ends
/// with that call and its result. Calls are matched by what they asked,
/// since a line's id is not the server's.
#[cfg(any(target_os = "macos", target_os = "ios", test))]
fn answered(req: &Request, name: &str, args: &serde_json::Value) -> Option<String> {
    use super::Turn;
    let n = req.turns.len();
    let before = n.checked_sub(2).and_then(|i| req.turns.get(i));
    let (Some(Turn::Assistant { calls, .. }), Some(Turn::ToolResults(results))) =
        (before, req.turns.last())
    else {
        return None;
    };
    let (call, result) = (calls.last()?, results.last()?);
    if call.name != name || call.args != *args {
        return None;
    }
    Some(if result.ok { result.text.clone() } else { format!("The tool failed: {}", result.text) })
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
mod native {
    use std::sync::Mutex;
    use std::time::Duration;

    use lb_apple_ai::{Event, Input, Message, Tool};

    use super::*;
    use crate::wire::{Call, Turn, parse_args};

    /// Tokens a reply may run to.
    const REPLY_TOKENS: u32 = 768;
    /// How long a completion waits on the model between pieces.
    const IDLE: Duration = Duration::from_secs(120);

    /// The native session waiting on a tool result, and the call it made.
    static PENDING: Mutex<Option<(lb_apple_ai::Request, Call)>> = Mutex::new(None);

    pub fn available() -> Result<(), String> {
        lb_apple_ai::availability()
    }

    /// The transcript as the bridge takes it: earlier tool results read as
    /// context the user supplied.
    fn messages(req: &Request) -> Vec<Message> {
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

    pub async fn complete(
        req: &Request, deltas: &UnboundedSender<Piece>,
    ) -> Result<Completion, String> {
        let parked = PENDING.lock().unwrap_or_else(|e| e.into_inner()).take();
        let resumed = parked.and_then(|(request, call)| {
            let answer = answered(req, &call.name, &call.args)?;
            Some((request, call.id, answer))
        });
        let mut request = match resumed {
            Some((request, id, answer)) => {
                request.tool_result(&id, &answer)?;
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
                    let call = Call {
                        id: call.id,
                        name: call.name,
                        args: match call.args {
                            serde_json::Value::String(text) => parse_args(&text),
                            args => args,
                        },
                        echo: None,
                    };
                    done.calls.push(call.clone());
                    *PENDING.lock().unwrap_or_else(|e| e.into_inner()) = Some((request, call));
                    return Ok(done);
                }
                Some(Event::Done) => return Ok(done),
                Some(Event::Error(e)) => return Err(e),
                None => return Err("Apple Intelligence stopped without answering".into()),
            }
        }
    }
}

#[cfg(not(any(target_os = "macos", target_os = "ios")))]
mod native {
    use super::*;

    const ELSEWHERE: &str = "Apple Intelligence runs only on Apple devices";

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
    use serde_json::json;

    use super::*;
    use crate::wire::{Call, ToolResult, Turn};

    /// A parked session goes on only when the transcript ends with its own
    /// call answered; a different call, or a new message, starts afresh.
    #[test]
    fn a_parked_call_is_answered_only_by_its_own_result() {
        let call = |name: &str| Call {
            id: "c".into(),
            name: name.into(),
            args: json!({ "path": "/a.md" }),
            echo: None,
        };
        let result = |text: &str, ok: bool| ToolResult {
            id: "x".into(),
            text: text.into(),
            ok,
            media: vec![],
        };
        let ended_with = |turns: Vec<Turn>| Request { turns, ..Default::default() };
        let args = json!({ "path": "/a.md" });

        let answered_read = ended_with(vec![
            Turn::User("q".into()),
            Turn::Assistant { text: String::new(), calls: vec![call("read")] },
            Turn::ToolResults(vec![result("# A", true)]),
        ]);
        assert_eq!(answered(&answered_read, "read", &args).as_deref(), Some("# A"));
        assert_eq!(answered(&answered_read, "search", &args), None);
        assert_eq!(answered(&answered_read, "read", &json!({ "path": "/b.md" })), None);

        let failed = ended_with(vec![
            Turn::Assistant { text: String::new(), calls: vec![call("read")] },
            Turn::ToolResults(vec![result("gone", false)]),
        ]);
        assert_eq!(answered(&failed, "read", &args).as_deref(), Some("The tool failed: gone"));

        let new_message = ended_with(vec![
            Turn::Assistant { text: String::new(), calls: vec![call("read")] },
            Turn::ToolResults(vec![result("# A", true)]),
            Turn::User("another question".into()),
        ]);
        assert_eq!(answered(&new_message, "read", &args), None);
        assert_eq!(answered(&Request::default(), "read", &args), None);
    }
}
