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
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use super::backend::{Backend, ChatMsg, CompletionReq};
use super::openai;
use super::settings::Provider;

/// Per-request output cap. Comfortably under Cerebras' free-tier 32k.
const MAX_TOKENS: u32 = 16_000;

/// UI → driver. Turn-running commands carry the provider resolved at send
/// time, so a config edit between turns just takes effect — nothing watches
/// the provider files and nothing is rebuilt.
enum Cmd {
    /// A user message to respond to; the caller already appended it to the
    /// transcript. `system` overrides the built-in preamble when the user
    /// has an instructions file.
    Say { text: String, provider: Provider, system: Option<String> },
    /// Cancel the turn in flight, keeping the text streamed so far.
    Stop,
    /// Re-run the last failed turn — with current config, so "fix the key,
    /// hit retry" works.
    Retry { provider: Provider, system: Option<String> },
    /// Replace model context wholesale — the UI's transcript mutations
    /// (delete, and later edit/regenerate) recompute the seed and hand the
    /// driver its new truth. Sent only while idle.
    Reseed(Vec<ChatMsg>),
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
    /// The turn failed; any partial text was discarded. Retryable.
    Error(String),
    TurnEnded,
}

/// UI-side handle to the driver thread. Dropping it closes the channels and
/// the driver exits.
pub struct Harness {
    /// A turn is in flight (set on `say`/`retry`, cleared on TurnEnded).
    pub busy: bool,
    /// Text streamed so far this turn.
    pub streaming: String,

    cmd_tx: UnboundedSender<Cmd>,
    events_rx: UnboundedReceiver<AgentEvent>,
}

/// Transcript-bound output of [`Harness::pump`].
pub enum HarnessUpdate {
    /// Append as an agent message stamped with the turn's usage.
    Reply { text: String, usage: Option<Usage> },
    /// Append as an error row.
    Error(String),
}

impl Harness {
    /// Spawn the driver thread. `history` seeds model context from the
    /// persisted transcript. No provider is bound here — each turn runs with
    /// the one its command carries, and a misconfigured provider isn't
    /// pre-screened: the turn fails and surfaces as a retryable error row,
    /// which beats a silently dormant agent.
    pub fn new(ctx: egui::Context, history: Vec<ChatMsg>) -> Self {
        let (cmd_tx, cmd_rx) = unbounded_channel();
        let (events_tx, events_rx) = unbounded_channel();

        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("tokio runtime");
            rt.block_on(run(history, ctx, cmd_rx, events_tx));
        });

        Self { busy: false, streaming: String::new(), cmd_tx, events_rx }
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

    /// Replace model context after a transcript mutation. Call only while
    /// idle — a reseed during a turn would race the in-flight reply.
    pub fn reseed(&mut self, history: Vec<ChatMsg>) {
        let _ = self.cmd_tx.send(Cmd::Reseed(history));
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
                AgentEvent::Error(e) => {
                    self.streaming.clear();
                    updates.push(HarnessUpdate::Error(e));
                }
                AgentEvent::TurnEnded => self.busy = false,
            }
        }
        updates
    }
}

async fn run(
    mut history: Vec<ChatMsg>, ctx: egui::Context, mut cmd_rx: UnboundedReceiver<Cmd>,
    events_tx: UnboundedSender<AgentEvent>,
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
            // Not a turn; no busy state to clear.
            Cmd::Reseed(new_history) => {
                history = new_history;
                continue;
            }
            // A stale Stop lost the race with natural completion. `stop()`
            // never set busy, and an unpaired TurnEnded here could clear a
            // *newer* turn's busy flag mid-stream — say nothing.
            Cmd::Stop => continue,
            // An unretryable Retry did set busy; release it.
            Cmd::Retry { .. } => {
                send(AgentEvent::TurnEnded);
                continue;
            }
        };

        // The turn's backend, from the provider resolved at send time.
        let backend = match provider.kind.as_str() {
            "openai" => Backend::OpenAi(openai::openai_compat(&provider)),
            other => {
                send(AgentEvent::Error(format!("provider kind '{other}' is not supported yet")));
                send(AgentEvent::TurnEnded);
                continue;
            }
        };

        let req = CompletionReq {
            system: system.unwrap_or_else(preamble),
            messages: history.clone(),
            max_tokens: MAX_TOKENS,
        };
        let (delta_tx, mut delta_rx) = unbounded_channel();
        let fut = backend.complete(req, delta_tx);
        tokio::pin!(fut);

        // Forward deltas as they stream, accumulating the reply locally so a
        // stop keeps what's arrived. Outcome is the backend result, or None
        // on stop.
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
        // Deltas the backend sent before finishing but after our last poll.
        while let Ok(delta) = delta_rx.try_recv() {
            text.push_str(&delta);
            send(AgentEvent::Delta(delta));
        }

        match outcome {
            Some(Ok(usage)) => {
                history.push(ChatMsg::Assistant(text.clone()));
                send(AgentEvent::Reply { text, usage: Some(usage) });
            }
            // Stopped: keep the partial as the reply — the user watched it
            // stream and chose to cut it there.
            None => {
                if !text.trim().is_empty() {
                    history.push(ChatMsg::Assistant(text.clone()));
                }
                send(AgentEvent::Reply { text, usage: None });
            }
            // Failed: discard the partial so a retry re-runs the turn clean.
            Some(Err(e)) => send(AgentEvent::Error(e)),
        }
        // With a queued command the run continues straight into the next
        // turn; TurnEnded (busy=false) would flicker the stop button.
        if pending.is_empty() {
            send(AgentEvent::TurnEnded);
        }
    }
}

/// The prompt's editable substance — also the instructions-file template,
/// so customizing starts from what's actually sent. Tool guidance joins in
/// the tools increment. Privacy claims deliberately absent: the transcript
/// is sent to whatever provider the user configured, and a model told "this
/// app is E2EE" overclaims privacy in exactly the conversations where the
/// nuance matters.
pub const PREAMBLE: &str = "You are the user's personal assistant inside their lockbook: a file \
     tree of mostly-markdown notes, synced across their devices. You are \
     talking with them in a chat tab of the lockbook app. Your replies \
     render as markdown chat bubbles, so keep them short and conversational \
     — headers and long lists usually read poorly in a bubble. You don't \
     yet have tools to read or edit their notes; if asked, say so plainly.";

/// Default system prompt: [`PREAMBLE`] plus the date (day-granular, so the
/// provider's prompt cache misses at most once a day).
fn preamble() -> String {
    format!("{PREAMBLE} Today is {}.", chrono::Utc::now().format("%B %-d, %Y"))
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::super::openai::mock::{SSE_HELLO, serve_once};
    use super::*;

    /// End to end through the driver thread: say → streamed deltas → reply
    /// with usage → idle.
    #[test]
    fn say_streams_a_reply() {
        let provider = Provider {
            name: "mock".into(),
            display_name: None,
            kind: "openai".into(),
            base_url: serve_once(SSE_HELLO),
            model: "test-model".into(),
            api_key: Some("test-key".into()),
        };
        let mut harness = Harness::new(egui::Context::default(), Vec::new());

        harness.say("hi".into(), provider, None);
        assert!(harness.busy);

        let deadline = Instant::now() + Duration::from_secs(5);
        let mut replies = Vec::new();
        while Instant::now() < deadline && (harness.busy || replies.is_empty()) {
            for update in harness.pump() {
                match update {
                    HarnessUpdate::Reply { text, usage } => replies.push((text, usage)),
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
}
