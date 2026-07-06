//! Agent driver for the chat tab: one turn = one streamed completion over a
//! provider-neutral [`Backend`], run on its own thread. The UI-side
//! [`Harness`] speaks user actions (say, stop, retry); everything the agent
//! does arrives as events over a channel and is folded into chat state by
//! [`Harness::pump`], called each frame.
//!
//! Nothing here is persisted. Final replies are appended to the transcript by
//! the caller; the streaming text in flight is per-session overlay state that
//! never reaches the synced `.chat` file.
//!
//! Turn outcomes: a completed stream yields the reply with its usage; a
//! user stop yields whatever streamed so far (a deliberate acceptance — the
//! partial enters the transcript and model context); an error discards the
//! partial and yields an error row the UI offers to retry.

use lb_rs::model::chat::Usage;
use serde_json::Value;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use super::backend::{Backend, ChatMsg, CompletionReq};
use super::diff;
use super::settings::Provider;
use super::tools::{self, Buffers};
use super::{anthropic, openai};

/// Per-request output cap. Comfortably under Cerebras' free-tier 32k.
const MAX_TOKENS: u32 = 16_000;
/// Completions per user turn before we stop (a runaway tool loop backstop).
const MAX_TOOL_TURNS: u32 = 30;

/// UI → driver. Turn-running commands carry the provider resolved at send
/// time, so a config edit between turns just takes effect — nothing watches
/// the provider files and nothing is rebuilt.
enum Cmd {
    /// A user message to respond to; the caller already appended it to the
    /// transcript. `system` overrides the built-in preamble when the user
    /// has a prompt file.
    Say { text: String, provider: Provider, system: Option<String> },
    /// Cancel the turn in flight, keeping the text streamed so far.
    Stop,
    /// Re-run the last failed turn — with current config, so "fix the key,
    /// hit retry" works.
    Retry { provider: Provider, system: Option<String> },
    /// Replace model context wholesale — the UI's transcript mutations
    /// (delete, and later edit/regenerate) recompute the seed and hand the
    /// driver its new truth. Sent only while idle. Buffers are session state
    /// and survive a reseed.
    Reseed(Vec<ChatMsg>, Buffers),
    /// Approve the pending edit — it runs and then applies to the real note.
    Approve,
    /// Deny the pending edit; the model is told and steers.
    Deny,
}

/// Driver → UI.
enum AgentEvent {
    /// Streamed text: append to the live reply.
    Delta(String),
    /// The turn's final reply — append as an agent message. `usage` is absent
    /// when the stream was stopped before the provider reported it.
    Reply {
        text: String,
        usage: Option<Usage>,
    },
    /// An edit wants to run; the UI shows the approval card (with the
    /// proposed diff) and the turn blocks until approve/deny. Reads and
    /// listings run without a gate.
    ToolRequest {
        name: String,
        /// The call's raw arguments, for the card's adapted permission prose.
        args: Value,
        /// For edits: the proposed change as diff segments, or the steering
        /// error approval would hit. `None` for gated reads.
        preview: Option<Result<Vec<diff::Segment>, String>>,
    },
    /// A tool call resolved (ran, or was denied) — persist it.
    ToolDone(ToolOutcome),
    /// The turn failed; any partial text was discarded. Retryable.
    Error(String),
    TurnEnded,
}

/// A finished tool call, as the transcript should record it.
pub struct ToolOutcome {
    pub name: String,
    pub args: Value,
    /// The result the model saw (truncated on persist).
    pub result: String,
    /// A steering failure — the call had no successful buffer effect, so
    /// replay suppresses its mutation.
    pub is_error: bool,
    /// A harness-authored consequence of the user's approval (the automatic
    /// write-back after an approved edit), not a model call — the record
    /// persists and folds but stays out of model context.
    pub user_action: bool,
    /// Raw content to persist as this record's replay attachment (full, out of
    /// model context): a read/auto-open capture, or a merge-sync's merged text.
    pub capture: Option<String>,
}

/// UI-side handle to the driver thread. Dropping it closes the channels and
/// the driver exits.
pub struct Harness {
    /// A turn is in flight (set on `say`/`retry`, cleared on TurnEnded).
    pub busy: bool,
    /// Text streamed so far this turn.
    pub streaming: String,
    /// The edit awaiting the user's decision; `Some` drives the approval
    /// card.
    pub pending_tool: Option<PendingTool>,

    cmd_tx: UnboundedSender<Cmd>,
    events_rx: UnboundedReceiver<AgentEvent>,
}

/// An edit shown to the user for approval.
pub struct PendingTool {
    pub name: String,
    pub args: Value,
    /// For edits: the proposed change as diff segments, or the steering
    /// error approval would hit. `None` for gated reads.
    pub preview: Option<Result<Vec<diff::Segment>, String>>,
}

/// Transcript-bound output of [`Harness::pump`].
pub enum HarnessUpdate {
    /// Append as an agent message stamped with the turn's usage.
    Reply { text: String, usage: Option<Usage> },
    /// Append a tool record.
    Tool(ToolOutcome),
    /// Append as an error row.
    Error(String),
}

impl Harness {
    /// Spawn the driver thread. `history` seeds model context from the
    /// persisted transcript. No provider is bound here — each turn runs with
    /// the one its command carries, and a misconfigured provider isn't
    /// pre-screened: the turn fails and surfaces as a retryable error row,
    /// which beats a silently dormant agent.
    pub fn new(
        ctx: egui::Context, core: lb_rs::blocking::Lb, history: Vec<ChatMsg>, buffers: Buffers,
    ) -> Self {
        let (cmd_tx, cmd_rx) = unbounded_channel();
        let (events_tx, events_rx) = unbounded_channel();

        // The async core, for tool dispatch on the driver's own runtime.
        let lb = core.async_lb().clone();
        std::thread::spawn(move || {
            // Multi-threaded so tool dispatch's `spawn_blocking` (pdf, content
            // search) doesn't stall the driver.
            let rt = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .expect("tokio runtime");
            rt.block_on(run(history, buffers, lb, ctx, cmd_rx, events_tx));
        });

        Self { busy: false, streaming: String::new(), pending_tool: None, cmd_tx, events_rx }
    }

    /// Send a user message to the agent. The caller has already appended it
    /// to the transcript.
    pub fn say(&mut self, text: String, provider: Provider, system: Option<String>) {
        self.busy = true;
        let _ = self.cmd_tx.send(Cmd::Say { text, provider, system });
    }

    /// Cancel the turn in flight; the partial reply arrives as a normal
    /// `Reply` update.
    pub fn stop(&mut self) {
        let _ = self.cmd_tx.send(Cmd::Stop);
    }

    /// Re-run the last failed turn.
    pub fn retry(&mut self, provider: Provider, system: Option<String>) {
        self.busy = true;
        let _ = self.cmd_tx.send(Cmd::Retry { provider, system });
    }

    /// Approve the pending edit. No-op if nothing is pending.
    pub fn approve(&mut self) {
        if self.pending_tool.take().is_some() {
            let _ = self.cmd_tx.send(Cmd::Approve);
        }
    }

    /// Deny the pending edit.
    pub fn deny(&mut self) {
        if self.pending_tool.take().is_some() {
            let _ = self.cmd_tx.send(Cmd::Deny);
        }
    }

    /// Replace model context and rebuild the variable buffers after a
    /// transcript mutation. Call only while idle — a reseed during a turn
    /// would race the in-flight reply.
    pub fn reseed(&mut self, history: Vec<ChatMsg>, buffers: Buffers) {
        let _ = self.cmd_tx.send(Cmd::Reseed(history, buffers));
    }

    /// Drain driver events into transcript-bound updates. Cheap when nothing
    /// arrived.
    pub fn pump(&mut self) -> Vec<HarnessUpdate> {
        let mut updates = Vec::new();
        while let Ok(ev) = self.events_rx.try_recv() {
            match ev {
                AgentEvent::Delta(text) => self.streaming.push_str(&text),
                AgentEvent::Reply { text, usage } => {
                    self.streaming.clear();
                    if !text.trim().is_empty() {
                        updates.push(HarnessUpdate::Reply { text, usage });
                    }
                }
                AgentEvent::ToolRequest { name, args, preview } => {
                    self.pending_tool = Some(PendingTool { name, args, preview });
                }
                AgentEvent::ToolDone(outcome) => {
                    self.pending_tool = None;
                    updates.push(HarnessUpdate::Tool(outcome));
                }
                AgentEvent::Error(e) => {
                    self.streaming.clear();
                    updates.push(HarnessUpdate::Error(e));
                }
                // A turn can end while an edit is pending (stopped
                // mid-approval) — clear the card so it doesn't stick.
                AgentEvent::TurnEnded => {
                    self.busy = false;
                    self.pending_tool = None;
                }
            }
        }
        updates
    }
}

async fn run(
    mut history: Vec<ChatMsg>, mut buffers: Buffers, lb: lb_rs::Lb, ctx: egui::Context,
    mut cmd_rx: UnboundedReceiver<Cmd>, events_tx: UnboundedSender<AgentEvent>,
) {
    let send = |ev: AgentEvent| {
        let _ = events_tx.send(ev);
        ctx.request_repaint();
    };

    // Commands that arrived mid-turn, run before pulling from the channel
    // again — a Say landing while a turn streams is queued, never dropped.
    let mut pending = std::collections::VecDeque::new();
    loop {
        let cmd = match pending.pop_front() {
            Some(cmd) => cmd,
            None => match cmd_rx.recv().await {
                Some(cmd) => cmd,
                None => return,
            },
        };
        let (provider, system) = match cmd {
            Cmd::Say { text, provider, system } => {
                history.push(ChatMsg::User(text));
                (provider, system)
            }
            // Retry only makes sense while the last context entry is the
            // user message whose turn failed.
            Cmd::Retry { provider, system } if matches!(history.last(), Some(ChatMsg::User(_))) => {
                (provider, system)
            }
            // Not a turn; no busy state to clear. The rebuilt buffers replace
            // the branch (a mutation may have removed tool rows).
            Cmd::Reseed(new_history, new_buffers) => {
                history = new_history;
                buffers = new_buffers;
                continue;
            }
            // A stale Stop lost the race with natural completion; a stray
            // Approve/Deny arrived with no edit pending. None of these set
            // busy, and an unpaired TurnEnded could clear a *newer* turn's
            // busy flag mid-stream — say nothing.
            Cmd::Stop | Cmd::Approve | Cmd::Deny => continue,
            // An unretryable Retry did set busy; release it.
            Cmd::Retry { .. } => {
                send(AgentEvent::TurnEnded);
                continue;
            }
        };

        // Reads egress note content to the provider, so they gate on
        // approval too — except when the model runs on this machine and
        // nothing leaves it.
        let local = provider.is_local();
        // The turn's backend, from the provider resolved at send time; reused
        // across the turn's completions.
        let backend = match provider.kind.as_str() {
            "openai" => Backend::OpenAi(openai::openai_compat(&provider)),
            "anthropic" => Backend::Anthropic(anthropic::anthropic(&provider)),
            other => {
                send(AgentEvent::Error(format!("provider kind '{other}' is not supported yet")));
                send(AgentEvent::TurnEnded);
                continue;
            }
        };

        // A user turn spans one or more completions, threaded by tool calls:
        // complete → (tool call? show for approval, run it, feed the result
        // back, re-complete) → final text.
        let mut turns = 0u32;
        'turn: loop {
            turns += 1;
            let req = CompletionReq {
                system: system.clone().unwrap_or_else(preamble),
                messages: history.clone(),
                max_tokens: MAX_TOKENS,
                tools: tools::schemas(),
            };
            let (delta_tx, mut delta_rx) = unbounded_channel();
            let fut = backend.complete(req, delta_tx);
            tokio::pin!(fut);

            // Forward deltas as they stream, accumulating locally so a stop
            // keeps what's arrived. Outcome is the backend result, or None on
            // stop.
            let mut text = String::new();
            let outcome = loop {
                tokio::select! {
                    Some(delta) = delta_rx.recv() => {
                        text.push_str(&delta);
                        send(AgentEvent::Delta(delta));
                    }
                    result = &mut fut => break Some(result),
                    cmd = cmd_rx.recv() => match cmd {
                        Some(Cmd::Stop) | None => break None,
                        Some(other) => pending.push_back(other),
                    },
                }
            };
            // Deltas sent before finishing but after our last poll.
            while let Ok(delta) = delta_rx.try_recv() {
                text.push_str(&delta);
                send(AgentEvent::Delta(delta));
            }

            let completion = match outcome {
                Some(Ok(c)) => c,
                // Stopped: keep the partial as the reply — the user watched it
                // stream and chose to cut it there.
                None => {
                    if !text.trim().is_empty() {
                        history.push(ChatMsg::Assistant { text: text.clone(), tool_calls: vec![] });
                    }
                    send(AgentEvent::Reply { text, usage: None });
                    break 'turn;
                }
                // Failed: discard the partial so a retry re-runs clean.
                Some(Err(e)) => {
                    send(AgentEvent::Error(e));
                    break 'turn;
                }
            };

            let calls = completion.tool_calls.clone();
            history
                .push(ChatMsg::Assistant { text: text.clone(), tool_calls: completion.tool_calls });
            // The agent's words (may be empty when it only called a tool).
            send(AgentEvent::Reply { text, usage: Some(completion.usage) });

            if calls.is_empty() {
                break 'turn;
            }
            if turns >= MAX_TOOL_TURNS {
                send(AgentEvent::Error("stopped: too many tool calls this turn".into()));
                break 'turn;
            }

            // One call at a time on the wire, but handle a batch defensively —
            // every tool_use needs a matching result. Edits always block on
            // the user's approval (and, once approved and successful, apply
            // straight through to the real note); reads gate only when the
            // provider is remote.
            for call in calls {
                let is_edit = call.name == "edit_note";
                let gated = is_edit || !local;
                if gated {
                    let preview = if is_edit {
                        Some(tools::preview_edit(&lb, &buffers, &call.args).await)
                    } else {
                        None
                    };
                    send(AgentEvent::ToolRequest {
                        name: call.name.clone(),
                        args: call.args.clone(),
                        preview,
                    });

                    // Block on the decision, queuing unrelated commands.
                    let approved = loop {
                        match cmd_rx.recv().await {
                            Some(Cmd::Approve) => break Some(true),
                            Some(Cmd::Deny) => break Some(false),
                            Some(Cmd::Stop) | None => break None,
                            Some(other) => pending.push_back(other),
                        }
                    };
                    match approved {
                        // Stopped mid-approval: end the turn. The assistant's
                        // call is left unanswered in the driver's history, but
                        // the next Say reseeds from the transcript.
                        None => break 'turn,
                        // Denied: record the refusal (identical text live and
                        // persisted); the model adjusts.
                        Some(false) => {
                            history.push(ChatMsg::ToolResult {
                                id: call.id.clone(),
                                content: tools::DENIED_RESULT.into(),
                                is_error: false,
                            });
                            send(AgentEvent::ToolDone(ToolOutcome {
                                name: call.name,
                                args: call.args,
                                result: tools::DENIED_RESULT.into(),
                                // No buffer effect — replay skips it.
                                is_error: true,
                                capture: None,
                                user_action: false,
                            }));
                            continue;
                        }
                        Some(true) => {}
                    }
                }

                let out = tools::dispatch(&lb, &mut buffers, &call.name, &call.args).await;
                let edit_applied = is_edit && !out.is_error;
                history.push(ChatMsg::ToolResult {
                    id: call.id.clone(),
                    content: out.result.clone(),
                    is_error: out.is_error,
                });
                send(AgentEvent::ToolDone(ToolOutcome {
                    name: call.name,
                    args: call.args.clone(),
                    result: out.result,
                    is_error: out.is_error,
                    capture: out.capture,
                    user_action: false,
                }));

                // The approved edit lands on the real note now. The receipt
                // record persists and folds (it marks the buffer saved, and
                // captures a merge's inflow) but stays out of model context —
                // the model's picture is simply that edits apply.
                if edit_applied {
                    let synced = tools::sync_note(&lb, &mut buffers, &call.args).await;
                    if synced.is_error {
                        // Receipts don't render, so a failed write-back must
                        // surface as an error row. No record is right for
                        // replay too: the disk didn't change, so the buffer
                        // folding back dirty matches reality.
                        send(AgentEvent::Error(synced.result));
                    } else {
                        send(AgentEvent::ToolDone(ToolOutcome {
                            name: "sync_note".into(),
                            args: call.args,
                            result: synced.result,
                            is_error: false,
                            capture: synced.capture,
                            user_action: true,
                        }));
                    }
                }
            }
            // Re-complete with the tool results now in context.
        }

        // With a queued command the run continues straight into the next
        // turn; TurnEnded (busy=false) would flicker the stop button.
        if pending.is_empty() {
            send(AgentEvent::TurnEnded);
        }
    }
}

/// The prompt's editable substance — also the prompt-file template, so
/// customizing starts from what's actually sent. Privacy claims deliberately
/// absent: the transcript is sent to whatever provider the user configured,
/// and a model told "this app is E2EE" overclaims privacy in exactly the
/// conversations where the nuance matters.
pub const PREAMBLE: &str = "You are the user's personal assistant inside their lockbook: a file \
     tree of mostly-markdown notes, synced across their devices. You are \
     talking with them in a chat tab of the lockbook app. Your replies \
     render as markdown chat bubbles, so keep them short and conversational \
     — headers and long lists usually read poorly in a bubble. \
     You have tools to explore and edit their notes. read_note before \
     editing, and prefer editing over rewriting a whole note. Just use the \
     tools directly rather than asking permission in prose. Link to notes by \
     their bare lockbook path, like [todo](/notes/todo.md).";

/// Default system prompt: [`PREAMBLE`] plus the date (day-granular, so the
/// provider's prompt cache misses at most once a day).
fn preamble() -> String {
    format!("{PREAMBLE} Today is {}.", chrono::Utc::now().format("%B %-d, %Y"))
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use lb_rs::model::core_config::{ClientType, Config};

    use super::super::openai::mock::{SSE_HELLO, serve_once, serve_seq};
    use super::*;

    /// An offline core — enough to construct the harness. Tool dispatch
    /// against it (no account) returns steering errors, which is all these
    /// tests need.
    fn offline_core() -> lb_rs::blocking::Lb {
        lb_rs::blocking::Lb::init(Config {
            writeable_path: format!("/tmp/{}", lb_rs::Uuid::new_v4()),
            logs: false,
            stdout_logs: false,
            colored_logs: false,
            background_work: false,
            client_type: ClientType::Unknown,
        })
        .unwrap()
    }

    fn provider(base_url: String) -> Provider {
        Provider {
            name: "mock".into(),
            display_name: None,
            kind: "openai".into(),
            base_url,
            model: "test-model".into(),
            api_key: Some("test-key".into()),
            effort: None,
        }
    }

    /// Pump until idle (or timeout), collecting updates.
    fn drain(harness: &mut Harness) -> Vec<HarnessUpdate> {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut out = Vec::new();
        while Instant::now() < deadline && (harness.busy || out.is_empty()) {
            out.extend(harness.pump());
            std::thread::sleep(Duration::from_millis(5));
        }
        out
    }

    /// End to end through the driver thread: say → streamed deltas → reply
    /// with usage → idle.
    #[test]
    fn say_streams_a_reply() {
        let mut harness =
            Harness::new(egui::Context::default(), offline_core(), Vec::new(), Buffers::default());

        harness.say("hi".into(), provider(serve_once(SSE_HELLO)), None);
        assert!(harness.busy);

        let deadline = Instant::now() + Duration::from_secs(5);
        let mut replies = Vec::new();
        while Instant::now() < deadline && (harness.busy || replies.is_empty()) {
            for update in harness.pump() {
                match update {
                    HarnessUpdate::Reply { text, usage } => replies.push((text, usage)),
                    HarnessUpdate::Tool(_) => panic!("unexpected tool"),
                    HarnessUpdate::Error(e) => panic!("unexpected error: {e}"),
                }
            }
            std::thread::sleep(Duration::from_millis(5));
        }

        assert_eq!(replies.len(), 1, "expected one reply, got {replies:?}");
        let (text, usage) = &replies[0];
        assert_eq!(text, "Hello");
        // SSE_HELLO's 10 prompt tokens minus the 3-cached subset.
        assert_eq!(usage.unwrap().input, 7);
        assert!(!harness.busy);
        assert!(harness.streaming.is_empty());
    }

    /// SSE body for a single-call completion invoking `edit_note`.
    const SSE_EDIT_CALL: &str = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n\
         data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"type\":\"function\",\"function\":{\"name\":\"edit_note\",\"arguments\":\"{\\\"path\\\":\\\"/a.md\\\",\\\"old_str\\\":\\\"x\\\",\\\"new_str\\\":\\\"y\\\"}\"}}]}}]}\n\n\
         data: {\"choices\":[],\"usage\":{\"prompt_tokens\":5,\"completion_tokens\":1}}\n\n\
         data: [DONE]\n\n";

    /// An edit call raises the approval card; approval runs it (a steering
    /// error on the offline core, so no write-back receipt) and the turn
    /// completes.
    #[test]
    fn edit_requests_approval_then_runs() {
        let base = serve_seq(vec![SSE_EDIT_CALL, SSE_HELLO]);
        let mut harness =
            Harness::new(egui::Context::default(), offline_core(), Vec::new(), Buffers::default());
        harness.say("edit my note".into(), provider(base), None);

        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline && harness.pending_tool.is_none() {
            harness.pump();
            std::thread::sleep(Duration::from_millis(5));
        }
        let pending = harness.pending_tool.as_ref().expect("approval card");
        assert_eq!(pending.name, "edit_note");
        assert_eq!(pending.args["path"], "/a.md");
        // No account on the offline core → the preview carries the
        // steering error approval would hit.
        assert!(matches!(&pending.preview, Some(Err(e)) if e.starts_with("error:")));
        assert!(harness.busy);

        harness.approve();

        let updates = drain(&mut harness);
        let tools: Vec<_> = updates
            .iter()
            .filter_map(|u| match u {
                HarnessUpdate::Tool(t) => Some(t),
                _ => None,
            })
            .collect();
        assert_eq!(tools.len(), 1, "steering-errored edit gets no receipt");
        assert_eq!(tools[0].name, "edit_note");
        assert!(tools[0].result.starts_with("error:"), "{}", tools[0].result);
        assert!(!harness.busy);
        assert!(harness.pending_tool.is_none());
    }

    /// Denying an edit feeds the denial back without running it; the turn
    /// still completes.
    #[test]
    fn denied_edit_completes_without_running() {
        let base = serve_seq(vec![SSE_EDIT_CALL, SSE_HELLO]);
        let mut harness =
            Harness::new(egui::Context::default(), offline_core(), Vec::new(), Buffers::default());
        harness.say("do it".into(), provider(base), None);

        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline && harness.pending_tool.is_none() {
            harness.pump();
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(harness.pending_tool.is_some());
        harness.deny();

        let updates = drain(&mut harness);
        let tool = updates
            .iter()
            .find_map(|u| match u {
                HarnessUpdate::Tool(t) => Some(t),
                _ => None,
            })
            .expect("a denied tool row");
        assert_eq!(tool.result, tools::DENIED_RESULT);
        assert!(tool.is_error);
        assert!(!harness.busy);
    }

    /// The full tool round-trip: first completion emits a call → it runs
    /// ungated and feeds a result back → second completion returns the final
    /// reply → idle.
    #[test]
    fn tool_call_runs_ungated_then_completes() {
        // First response calls `list`; second (after the result) is the reply.
        const SSE_LIST_CALL: &str = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n\
             data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"type\":\"function\",\"function\":{\"name\":\"list\",\"arguments\":\"{}\"}}]}}]}\n\n\
             data: {\"choices\":[],\"usage\":{\"prompt_tokens\":5,\"completion_tokens\":1}}\n\n\
             data: [DONE]\n\n";
        let base = serve_seq(vec![SSE_LIST_CALL, SSE_HELLO]);
        let mut harness =
            Harness::new(egui::Context::default(), offline_core(), Vec::new(), Buffers::default());
        harness.say("what's in my vault?".into(), provider(base), None);

        // Drain to completion: a tool row, then the reply — no decision step.
        let updates = drain(&mut harness);
        let mut saw_tool = false;
        let mut reply = None;
        for u in updates {
            match u {
                HarnessUpdate::Tool(t) => {
                    assert_eq!(t.name, "list");
                    // No account on the offline core → a steering error result.
                    assert!(t.result.starts_with("error:"), "{}", t.result);
                    saw_tool = true;
                }
                HarnessUpdate::Reply { text, .. } => reply = Some(text),
                HarnessUpdate::Error(e) => panic!("unexpected error: {e}"),
            }
        }
        assert!(saw_tool, "expected a tool row");
        assert_eq!(reply.as_deref(), Some("Hello"));
        assert!(!harness.busy);
    }
}
