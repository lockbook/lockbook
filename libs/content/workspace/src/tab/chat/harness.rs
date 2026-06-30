//! Agent harness for the chat tab: an agentic loop over a provider-neutral
//! [`ChatBackend`] (Anthropic, Cerebras, …, and a future on-device model), run
//! on its own thread. The UI-side [`Harness`] speaks only user actions (say,
//! approve, deny); everything the agent does arrives as [`AgentEvent`]s over a
//! channel and is folded into chat state by [`Harness::pump`], called each
//! frame.
//!
//! Nothing here is persisted. Final assistant messages are appended to the
//! transcript by the caller; pending tool calls, busy state, and errors are
//! per-session overlay state that never reaches the synced `.chat` file.

use lb_rs::blocking::Lb as BlockingLb;
use lb_rs::model::chat::Usage;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use super::backend::{ChatBackend, ChatMsg, CompletionReq};
use super::settings::ChatSettings;
use super::{rig_backend, tools};

pub use super::backend::ModelChoice;
pub use super::settings::DEFAULT_MODEL;

/// Per-request output cap. Comfortably under Cerebras' free-tier 32k.
const MAX_TOKENS: u32 = 16_000;

/// Cap on tool round-trips within one turn, so a confused model can't loop
/// forever burning tokens (and the user's approvals).
const MAX_TURNS: usize = 30;

/// Cap on a persisted tool result. Beyond this the transcript record is
/// truncated — a restored agent can re-read the rare huge file.
const TOOL_RESULT_PERSIST_CAP: usize = 32 * 1024;

/// What a denied tool call feeds the model; persisted records must match so a
/// restored context is consistent.
pub const DENIED_RESULT: &str = "The user denied permission for this tool call.";

/// A tool call awaiting (or executing after) the user's decision.
#[derive(Clone, Debug)]
pub struct ToolCall {
    pub id: u64,
    pub name: String,
    /// Human summary of the call arguments (usually the path).
    pub detail: String,
    /// Raw call arguments, for the persisted transcript record.
    pub args: serde_json::Value,
}

/// Driver → UI notifications.
enum AgentEvent {
    /// A tool call wants permission. The driver blocks until the user decides,
    /// so at most one is outstanding at a time.
    ToolRequest(ToolCall),
    /// The approved tool finished; `result` is the text the model saw.
    ToolFinished {
        result: String,
    },
    /// The model's final answer for a turn, with the turn's token usage.
    AssistantText {
        text: String,
        usage: Usage,
    },
    TurnEnded,
    Error(String),
    /// Models available from the provider (newest first), for the picker.
    Models(Vec<ModelChoice>),
}

/// UI-side handle to the driver thread. Dropping it closes the channels and
/// the driver exits after the turn in flight.
pub struct Harness {
    /// A turn is in flight (set on `say`, cleared on TurnEnded).
    pub busy: bool,
    /// Tool call awaiting the user's approve/deny decision.
    pub pending: Option<ToolCall>,
    /// Approved tool currently executing.
    pub running: Option<ToolCall>,
    /// Models the settings picker can offer (empty until a listing lands).
    pub models: Vec<ModelChoice>,

    user_tx: UnboundedSender<String>,
    decision_tx: UnboundedSender<bool>,
    events_rx: UnboundedReceiver<AgentEvent>,
}

impl Harness {
    /// Spawn the driver thread, or `None` when no API key is configured (in
    /// `/chat.json` or the provider env var). `history` seeds the conversation
    /// from the persisted transcript.
    pub fn new(
        core: BlockingLb, ctx: egui::Context, username: String, history: Vec<SeedMsg>,
        settings: ChatSettings,
    ) -> Option<Self> {
        let api_key = settings.api_key()?;
        let base_url = settings.base_url();
        let model = settings.model().to_string();
        let kind = settings.kind();

        let (user_tx, user_rx) = unbounded_channel();
        let (decision_tx, decision_rx) = unbounded_channel();
        let (events_tx, events_rx) = unbounded_channel();

        let mut chat_history = Vec::new();
        for seed in history {
            match seed {
                SeedMsg::User(text) => chat_history.push(ChatMsg::User(text)),
                SeedMsg::Agent(text) => {
                    chat_history.push(ChatMsg::Assistant { text, tool_calls: Vec::new() })
                }
                // A tool round-trip replays as the assistant's call followed
                // by its result — the same information in a shape every
                // provider accepts.
                SeedMsg::Tool { id, name, args, result } => {
                    chat_history.push(ChatMsg::Assistant {
                        text: String::new(),
                        tool_calls: vec![super::backend::ToolCall { id: id.clone(), name, args }],
                    });
                    chat_history.push(ChatMsg::ToolResult { id, content: result });
                }
            }
        }

        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .expect("tokio runtime");
            rt.block_on(run(
                core,
                username,
                kind,
                api_key,
                base_url,
                model,
                chat_history,
                ctx,
                user_rx,
                decision_rx,
                events_tx,
            ));
        });

        Some(Self {
            busy: false,
            pending: None,
            running: None,
            models: Vec::new(),
            user_tx,
            decision_tx,
            events_rx,
        })
    }

    /// Send a user message to the agent. The caller has already appended it
    /// to the transcript.
    pub fn say(&mut self, text: String) {
        self.busy = true;
        let _ = self.user_tx.send(text);
    }

    pub fn approve(&mut self) {
        if let Some(call) = self.pending.take() {
            self.running = Some(call);
            let _ = self.decision_tx.send(true);
        }
    }

    /// Deny the pending call, returning it so the caller can record the denial.
    pub fn deny(&mut self) -> Option<ToolCall> {
        let call = self.pending.take();
        if call.is_some() {
            let _ = self.decision_tx.send(false);
        }
        call
    }

    /// Drain driver events into transcript-bound updates. Cheap when nothing
    /// arrived.
    pub fn pump(&mut self) -> Vec<HarnessUpdate> {
        let mut updates = Vec::new();
        while let Ok(ev) = self.events_rx.try_recv() {
            match ev {
                AgentEvent::ToolRequest(call) => self.pending = Some(call),
                AgentEvent::ToolFinished { result } => {
                    if let Some(call) = self.running.take() {
                        updates.push(HarnessUpdate::ToolDone { call, result });
                    }
                }
                AgentEvent::AssistantText { text, usage } => {
                    updates.push(HarnessUpdate::Reply { text, usage })
                }
                AgentEvent::TurnEnded => self.busy = false,
                AgentEvent::Error(e) => {
                    self.busy = false;
                    updates.push(HarnessUpdate::Error(e));
                }
                AgentEvent::Models(list) => self.models = list,
            }
        }
        updates
    }
}

/// Transcript-bound output of [`Harness::pump`].
pub enum HarnessUpdate {
    /// The model's final answer — append as an agent message stamped with the
    /// turn's usage.
    Reply { text: String, usage: Usage },
    /// An approved tool call finished — append as a tool record.
    ToolDone { call: ToolCall, result: String },
    /// The turn failed — append as an error row.
    Error(String),
}

/// A transcript message bound for the model's context, in transcript order.
pub enum SeedMsg {
    User(String),
    Agent(String),
    Tool { id: String, name: String, args: serde_json::Value, result: String },
}

#[allow(clippy::too_many_arguments)]
async fn run(
    core: BlockingLb, username: String, kind: String, api_key: String, base_url: String,
    model: String, mut history: Vec<ChatMsg>, ctx: egui::Context,
    mut user_rx: UnboundedReceiver<String>, mut decision_rx: UnboundedReceiver<bool>,
    events_tx: UnboundedSender<AgentEvent>,
) {
    let send = |ev: AgentEvent| {
        let _ = events_tx.send(ev);
        ctx.request_repaint();
    };

    // Select the backend by provider kind. A future on-device `local` slots
    // in here.
    let built = match kind.as_str() {
        "openai" => rig_backend::openai_compat(&api_key, &base_url, &model),
        "anthropic" => rig_backend::anthropic(&api_key, &model),
        other => Err(format!("provider kind '{other}' is not supported yet")),
    };
    let backend: std::sync::Arc<dyn ChatBackend> = match built {
        Ok(b) => b.into(),
        Err(e) => {
            send(AgentEvent::Error(format!("provider setup failed: {e}")));
            return;
        }
    };

    // Fetch the model list concurrently; chat works before (and without) it —
    // the picker falls back to a free-text id field.
    {
        let backend = backend.clone();
        let events = events_tx.clone();
        let ctx = ctx.clone();
        tokio::spawn(async move {
            match backend.list_models().await {
                Ok(models) if !models.is_empty() => {
                    let _ = events.send(AgentEvent::Models(models));
                    ctx.request_repaint();
                }
                Ok(_) => {}
                Err(e) => tracing::warn!("chat: model listing failed: {e}"),
            }
        });
    }

    let lb = core.async_lb().clone();
    let tool_schemas = tools::schemas();
    let preamble = preamble(&username);
    // Monotonic UI id for approval rows, distinct from the model's tool-call id.
    let mut next_tool_id: u64 = 0;

    while let Some(text) = user_rx.recv().await {
        history.push(ChatMsg::User(text));
        // One user message can drive several completion requests (tool
        // round-trips); the turn's usage is their sum.
        let mut usage = Usage::default();

        for turn in 0..=MAX_TURNS {
            if turn == MAX_TURNS {
                send(AgentEvent::Error(format!("stopped after {MAX_TURNS} tool round-trips")));
                break;
            }
            let req = CompletionReq {
                system: preamble.clone(),
                messages: history.clone(),
                tools: tool_schemas.clone(),
                max_tokens: MAX_TOKENS,
            };
            let resp = match backend.complete(req).await {
                Ok(r) => r,
                Err(e) => {
                    send(AgentEvent::Error(e));
                    break;
                }
            };
            usage.input += resp.usage.input;
            usage.output += resp.usage.output;
            usage.cache_read += resp.usage.cache_read;
            usage.cache_write += resp.usage.cache_write;

            if resp.tool_calls.is_empty() {
                history
                    .push(ChatMsg::Assistant { text: resp.text.clone(), tool_calls: Vec::new() });
                send(AgentEvent::AssistantText { text: resp.text, usage });
                break;
            }

            // The assistant's tool-call turn (any narration + the calls).
            history.push(ChatMsg::Assistant {
                text: resp.text.clone(),
                tool_calls: resp.tool_calls.clone(),
            });

            // Run the calls one at a time, each gated by the user. The shared
            // decision channel is sound because exactly one request is
            // outstanding at a time.
            for call in resp.tool_calls {
                next_tool_id += 1;
                send(AgentEvent::ToolRequest(ToolCall {
                    id: next_tool_id,
                    name: call.name.clone(),
                    detail: tools::detail_for(&call.name, &call.args),
                    args: call.args.clone(),
                }));
                // A closed channel means the UI is gone; treat as denial.
                let approved = decision_rx.recv().await.unwrap_or(false);

                let result = if approved {
                    let result = tools::dispatch(&lb, &core, &call.name, &call.args).await;
                    // The model sees the full result; the transcript record
                    // (and the UI row) gets the capped copy.
                    send(AgentEvent::ToolFinished { result: truncate_for_persist(&result) });
                    result
                } else {
                    // Denial is recorded by the UI side (Harness::deny); only
                    // the model context is updated here.
                    DENIED_RESULT.to_string()
                };
                history.push(ChatMsg::ToolResult { id: call.id.clone(), content: result });
            }
        }
        send(AgentEvent::TurnEnded);
    }
}

/// Cap a tool result bound for the synced transcript; the live model context
/// keeps the full text.
fn truncate_for_persist(result: &str) -> String {
    if result.len() > TOOL_RESULT_PERSIST_CAP {
        let mut t: String = result.chars().take(TOOL_RESULT_PERSIST_CAP).collect();
        t.push_str("\n(result truncated for transcript)");
        t
    } else {
        result.to_string()
    }
}

/// System prompt: identity, file-tree tool guidance, and the approval contract.
fn preamble(username: &str) -> String {
    format!(
        "You are {username}'s personal assistant inside their lockbook: a private, \
         end-to-end-encrypted file tree of mostly-markdown notes, synced across their \
         devices. You are talking with {username} in a chat tab of the lockbook app. \
         Your replies render as markdown chat bubbles, so keep them short and \
         conversational — headers and long lists usually read poorly in a bubble.\n\
         \n\
         You have tools over the file tree. Paths look like /notes/todo.md (always \
         absolute, '/' is the root). Guidance on choosing:\n\
         - To find something: `search_content` (full-text over markdown) when you \
         have words to look for; `search_paths` (fuzzy filename match) when you know \
         roughly what it's called; `list_paths` (the whole tree at once) when \
         browsing structure. Prefer these over walking folders with `list_dir`.\n\
         - Before editing a document, `read_file` it first — `write_file` replaces the \
         entire document, so write back the full intended content, not a fragment.\n\
         - `read_pdf` extracts the text of a PDF so you can summarize or answer \
         questions about it.\n\
         - `stat` answers metadata questions (size, last modified, shared?) without \
         pulling the document into context.\n\
         - `move_file` / `rename_file` / `create_folder` reorganize the tree.\n\
         \n\
         Every tool call is shown to {username} for explicit approval before it runs. \
         Batch your thinking so you need few calls, and don't ask for permission in \
         chat — the approval flow is the permission. A denied call is a deliberate \
         decision, not an error: respect it, don't retry it, and adjust your plan.\n\
         \n\
         When you reference a document in a reply, link it so {username} can open it: \
         a markdown link whose URL is the bare lockbook path, like [todo](/notes/todo.md). \
         Never use file://, lb://, or any other scheme. If something is ambiguous \
         (several plausible lists, an unclear target folder), ask one short clarifying \
         question instead of guessing."
    )
}
