//! Agent harness for the chat tab: a rig multi-turn loop against Anthropic
//! on its own thread, with lockbook-document tools gated by per-call user
//! approval. Ported from the foundry agent spike.
//!
//! The split mirrors the spike: [`Harness`] is the UI-side handle — its
//! vocabulary is *user* actions only (say, approve, deny). Everything the
//! agent does arrives as [`AgentEvent`]s over a channel from the driver and
//! is folded into chat state by [`Harness::pump`], called every frame.
//!
//! Nothing here is persisted. Final assistant messages are appended to the
//! chat transcript by the caller; tool requests, busy state, and errors are
//! per-session overlay state that never reaches the synced `.chat` file.

use std::collections::HashMap;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use lb_rs::Lb;
use lb_rs::blocking::Lb as BlockingLb;
use lb_rs::model::chat::Usage;
use lb_rs::search::{ContentSearcher, PathSearcher};
use rig::agent::{Agent, AgentBuilder, HookAction, PromptHook, ToolCallHookAction};
use rig::client::{CompletionClient, ModelListingClient, ProviderClient};
use rig::completion::{CompletionModel, Prompt, ToolDefinition};
use rig::message::Message as RigMessage;
use rig::providers::anthropic;
use rig::tool::Tool;
use serde::Deserialize;
use serde_json::json;
use tokio::sync::Mutex as AsyncMutex;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

pub const DEFAULT_MODEL: &str = "claude-opus-4-8";
const MAX_TURNS: usize = 30;

/// Documents larger than this are refused by `read_file` to keep single tool
/// results from flooding the context window.
const READ_CAP_BYTES: usize = 256 * 1024;

/// A tool call awaiting (or executing after) the user's decision.
#[derive(Clone, Debug)]
pub struct ToolCall {
    pub id: u64,
    pub name: String,
    /// Human summary of the call arguments (usually the path).
    pub detail: String,
    /// The raw call arguments, for the persisted transcript record.
    pub args: serde_json::Value,
}

/// Cap on a persisted tool result. Beyond this the transcript record is
/// truncated — a restored agent can re-read the rare huge file.
const TOOL_RESULT_PERSIST_CAP: usize = 32 * 1024;

/// What rig feeds the model for a denied call (`ToolCallHookAction::skip`);
/// persisted records must say the same so a restored context matches.
pub const DENIED_RESULT: &str = "The user denied permission for this tool call.";

/// One pickable model. `id` is the canonical request value — older models
/// only exist under dated ids (`claude-sonnet-4-5-20250929`), so ids must
/// never be abbreviated and are what `/chat.json` stores. `label` is the
/// API's human-readable display name ("Claude Sonnet 4.5").
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelChoice {
    pub id: String,
    pub label: String,
}

/// Driver → UI notifications.
enum AgentEvent {
    /// A tool call wants permission. The driver blocks until the user
    /// decides, so at most one of these is outstanding at a time.
    ToolRequest(ToolCall),
    /// The approved tool finished executing; `result` is the (possibly
    /// truncated) text the model saw.
    ToolFinished {
        result: String,
    },
    /// The model's final answer for a turn, with the turn's token usage
    /// (aggregated across every completion request the loop made for it).
    AssistantText {
        text: String,
        usage: Usage,
    },
    TurnEnded,
    Error(String),
    /// Models available from the provider, fetched at startup (newest first).
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
    /// Models the settings picker can offer (empty until the driver's
    /// startup fetch lands).
    pub models: Vec<ModelChoice>,

    user_tx: UnboundedSender<String>,
    decision_tx: UnboundedSender<bool>,
    events_rx: UnboundedReceiver<AgentEvent>,
}

impl Harness {
    /// Spawn the driver thread, or `None` when no API key is configured (in
    /// `/chat.json` or the environment). `history` seeds the conversation
    /// from the persisted transcript — including tool round-trips, so the
    /// restored context is equivalent to the live one it replaces.
    pub fn new(
        core: BlockingLb, ctx: egui::Context, username: String, history: Vec<SeedMsg>,
        settings: super::settings::ChatSettings,
    ) -> Option<Self> {
        let api_key = settings.api_key()?;
        let model = settings.model().to_string();

        let (user_tx, user_rx) = unbounded_channel();
        let (decision_tx, decision_rx) = unbounded_channel();
        let (events_tx, events_rx) = unbounded_channel();

        let mut rig_history = Vec::new();
        for seed in history {
            match seed {
                SeedMsg::User(text) => rig_history.push(RigMessage::user(text)),
                SeedMsg::Agent(text) => rig_history.push(RigMessage::assistant(text)),
                // A tool round-trip replays as the assistant's call followed
                // by its result. Not byte-identical to the original turn
                // (text and calls may have shared one message), but the same
                // information in a shape every provider accepts.
                SeedMsg::Tool { id, name, args, result } => {
                    rig_history.push(RigMessage::Assistant {
                        id: None,
                        content: rig::OneOrMany::one(rig::message::AssistantContent::tool_call(
                            id.clone(),
                            name,
                            args,
                        )),
                    });
                    rig_history.push(RigMessage::tool_result(id, result));
                }
            }
        }
        let history = rig_history;

        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .expect("tokio runtime");
            rt.block_on(run(
                core,
                ctx,
                username,
                api_key,
                model,
                history,
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

    /// Deny the pending call, returning it so the caller can record the
    /// denial in the transcript.
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
    /// The model's final answer — append as an agent message stamped with
    /// the turn's usage.
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
    core: BlockingLb, ctx: egui::Context, username: String, api_key: String, model: String,
    mut history: Vec<RigMessage>, mut user_rx: UnboundedReceiver<String>,
    decision_rx: UnboundedReceiver<bool>, events_tx: UnboundedSender<AgentEvent>,
) {
    let send = |ev: AgentEvent| {
        let _ = events_tx.send(ev);
        ctx.request_repaint();
    };

    let client = match anthropic::Client::from_val(api_key) {
        Ok(c) => c,
        Err(e) => {
            send(AgentEvent::Error(format!("anthropic client failed: {e}")));
            return;
        }
    };

    let preamble = format!(
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
         Examples of good behavior:\n\
         - \"what did I write about the boat repair?\" → `search_content` for 'boat \
         repair', \
         read the best match, answer with a short summary and the path.\n\
         - \"add milk to my grocery list\" → find the list (`search_paths` for \
         'grocer'), \
         `read_file`, then `write_file` the whole list back with milk added.\n\
         - \"clean up my recipes folder\" → `list_paths`, propose a reorganization in \
         chat first, and only after {username} agrees, do the moves/renames.\n\
         \n\
         Every tool call is shown to {username} for explicit approval before it runs. \
         Batch your thinking so you need few calls, and don't ask for permission in \
         chat — the approval flow is the permission. A denied call is a deliberate \
         decision, not an error: respect it, don't retry it, and adjust your plan.\n\
         \n\
         When you reference a document in a reply, link it so {username} can open \
         it: a markdown link whose URL is the bare lockbook path, like \
         [todo](/notes/todo.md). Never use file://, lb://, or any other scheme — \
         these are lockbook paths, not filesystem paths, and schemed links render \
         as broken. If something is ambiguous (several plausible lists, an unclear \
         target folder), ask one short clarifying question instead of guessing."
    );

    let hook = ApprovalHook {
        inner: Arc::new(HookShared {
            events: events_tx.clone(),
            ctx: ctx.clone(),
            decisions: AsyncMutex::new(decision_rx),
            next_id: AtomicU64::new(1),
            in_flight: Mutex::new(HashMap::new()),
        }),
    };

    // Fetch the model list concurrently; chat works before (and without) it —
    // the settings picker just falls back to a free-text id field.
    {
        let client = client.clone();
        let events = events_tx.clone();
        let ctx = ctx.clone();
        tokio::spawn(async move {
            match client.list_models().await {
                Ok(list) => {
                    let choices: Vec<ModelChoice> = list
                        .iter()
                        .map(|m| ModelChoice {
                            id: m.id.clone(),
                            label: m.display_name().to_string(),
                        })
                        .collect();
                    let _ = events.send(AgentEvent::Models(choices));
                    ctx.request_repaint();
                }
                Err(e) => tracing::warn!("chat agent: model listing failed: {e}"),
            }
        });
    }

    let agent = make_agent(&client, &model, &preamble, hook, core);

    while let Some(text) = user_rx.recv().await {
        let result = agent
            .prompt(text.clone())
            .with_history(history.clone())
            .extended_details()
            .await;
        match result {
            Ok(resp) => {
                // `messages` holds the messages this run added (prompt, tool
                // round-trips, final answer); appending keeps `history` the
                // full conversation. The same round-trips are persisted to
                // the transcript, so a restart reconstructs this context.
                if let Some(new) = resp.messages {
                    history.extend(new);
                }
                let u = resp.usage;
                send(AgentEvent::AssistantText {
                    text: resp.output,
                    usage: Usage {
                        input: u.input_tokens,
                        output: u.output_tokens,
                        cache_read: u.cached_input_tokens,
                        cache_write: u.cache_creation_input_tokens,
                    },
                });
            }
            Err(e) => {
                // keep parity with the transcript, which has the user's text
                history.push(RigMessage::user(text));
                send(AgentEvent::Error(e.to_string()));
            }
        };
        send(AgentEvent::TurnEnded);
    }
}

fn make_agent(
    client: &anthropic::Client, model: &str, preamble: &str, hook: ApprovalHook, core: BlockingLb,
) -> Agent<anthropic::completion::CompletionModel, ApprovalHook> {
    // Automatic prompt caching: a top-level cache_control on every request;
    // the API advances the breakpoint as history grows. 1h TTL, not the 5m
    // default: a human-paced chat is one request per turn with minutes of
    // reading/typing between sends, so 5m entries expire before the next
    // request ever reads them (all cache write, zero cache read).
    let model = client.completion_model(model).with_automatic_caching_1h();
    let lb = core.async_lb().clone();
    AgentBuilder::new(model)
        .preamble(preamble)
        .max_tokens(16_000)
        .default_max_turns(MAX_TURNS)
        .hook(hook)
        .tool(ReadFile { core: lb.clone() })
        .tool(WriteFile { core: lb.clone() })
        .tool(ListDir { core: lb.clone() })
        .tool(ListPaths { core: lb.clone() })
        .tool(MoveFile { core: lb.clone() })
        .tool(RenameFile { core: lb.clone() })
        .tool(CreateFolder { core: lb.clone() })
        .tool(StatFile { core: lb.clone() })
        .tool(ReadPdf { core: lb.clone() })
        .tool(SearchContent { core })
        .tool(SearchPaths { core: lb })
        .build()
}

// -- Approval hook --

#[derive(Clone)]
struct ApprovalHook {
    inner: Arc<HookShared>,
}

struct HookShared {
    events: UnboundedSender<AgentEvent>,
    ctx: egui::Context,
    /// Single shared decision stream. Sound because tool calls run with
    /// concurrency 1 (rig's default, which we keep): each request consumes
    /// exactly the next decision. The async mutex would also serialize
    /// overlapping requests if that default ever changed.
    decisions: AsyncMutex<UnboundedReceiver<bool>>,
    next_id: AtomicU64,
    /// rig's internal call id → tool call id, so the post-execution hook can
    /// signal completion of the right call.
    in_flight: Mutex<HashMap<String, u64>>,
}

impl<M: CompletionModel> PromptHook<M> for ApprovalHook {
    async fn on_tool_call(
        &self, tool_name: &str, _tool_call_id: Option<String>, internal_call_id: &str, args: &str,
    ) -> ToolCallHookAction {
        let id = self.inner.next_id.fetch_add(1, Ordering::Relaxed);
        self.inner
            .in_flight
            .lock()
            .unwrap()
            .insert(internal_call_id.to_string(), id);
        let _ = self.inner.events.send(AgentEvent::ToolRequest(ToolCall {
            id,
            name: tool_name.to_string(),
            detail: detail_for(tool_name, args),
            args: serde_json::from_str(args).unwrap_or(serde_json::Value::Null),
        }));
        self.inner.ctx.request_repaint();

        let approved = {
            let mut rx = self.inner.decisions.lock().await;
            // A closed channel means the UI is gone; treat as denial.
            rx.recv().await.unwrap_or(false)
        };
        if approved {
            ToolCallHookAction::cont()
        } else {
            self.inner
                .in_flight
                .lock()
                .unwrap()
                .remove(internal_call_id);
            ToolCallHookAction::skip("The user denied permission for this tool call.")
        }
    }

    async fn on_tool_result(
        &self, _tool_name: &str, _tool_call_id: Option<String>, internal_call_id: &str,
        _args: &str, result: &str,
    ) -> HookAction {
        let id = self
            .inner
            .in_flight
            .lock()
            .unwrap()
            .remove(internal_call_id);
        if id.is_some() {
            let result = if result.len() > TOOL_RESULT_PERSIST_CAP {
                let mut truncated: String = result.chars().take(TOOL_RESULT_PERSIST_CAP).collect();
                truncated.push_str("\n(result truncated for transcript)");
                truncated
            } else {
                result.to_string()
            };
            let _ = self.inner.events.send(AgentEvent::ToolFinished { result });
            self.inner.ctx.request_repaint();
        }
        HookAction::cont()
    }
}

/// Human one-liner for a tool call's arguments.
fn detail_for(tool_name: &str, args: &str) -> String {
    let parsed: Option<serde_json::Value> = serde_json::from_str(args).ok();
    let field = |name: &str| {
        parsed
            .as_ref()
            .and_then(|v| v.get(name))
            .and_then(|v| v.as_str())
            .map(str::to_string)
    };
    match tool_name {
        "read_file" | "read_pdf" | "stat" | "create_folder" => field("path").unwrap_or_default(),
        "write_file" => {
            let path = field("path").unwrap_or_default();
            let bytes = field("content").map(|c| c.len()).unwrap_or(0);
            format!("{path} ({bytes} bytes)")
        }
        "list_dir" => field("path").unwrap_or_else(|| "/".to_string()),
        "list_paths" => "/".to_string(),
        "move_file" => format!(
            "{} → {}",
            field("path").unwrap_or_default(),
            field("new_parent").unwrap_or_default()
        ),
        "rename_file" => format!(
            "{} → {}",
            field("path").unwrap_or_default(),
            field("new_name").unwrap_or_default()
        ),
        "search_content" | "search_paths" => field("query").unwrap_or_default(),
        _ => {
            let line = args.lines().next().unwrap_or("");
            line.chars().take(80).collect()
        }
    }
}

// -- Tools --

#[derive(Debug)]
struct ToolFailure(String);

impl fmt::Display for ToolFailure {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ToolFailure {}

impl From<lb_rs::model::errors::LbErr> for ToolFailure {
    fn from(e: lb_rs::model::errors::LbErr) -> Self {
        ToolFailure(e.to_string())
    }
}

struct ReadFile {
    core: Lb,
}

#[derive(Deserialize)]
struct ReadArgs {
    path: String,
}

impl Tool for ReadFile {
    const NAME: &'static str = "read_file";
    type Error = ToolFailure;
    type Args = ReadArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Read a UTF-8 text document from the lockbook.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Lockbook path, e.g. /notes/todo.md" }
                },
                "required": ["path"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let file = self.core.get_by_path(&args.path).await?;
        let bytes = self.core.read_document(file.id, false).await?;
        if bytes.len() > READ_CAP_BYTES {
            return Err(ToolFailure(format!(
                "document is {} bytes, over the {READ_CAP_BYTES}-byte read cap",
                bytes.len()
            )));
        }
        String::from_utf8(bytes).map_err(|_| ToolFailure("document is not UTF-8".into()))
    }
}

struct WriteFile {
    core: Lb,
}

#[derive(Deserialize)]
struct WriteArgs {
    path: String,
    content: String,
}

impl Tool for WriteFile {
    const NAME: &'static str = "write_file";
    type Error = ToolFailure;
    type Args = WriteArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Write a UTF-8 text document in the lockbook, replacing it if it \
                          exists. Parent folders are created as needed."
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Lockbook path, e.g. /notes/todo.md" },
                    "content": { "type": "string", "description": "The full document content to write" }
                },
                "required": ["path", "content"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let file = match self.core.get_by_path(&args.path).await {
            Ok(file) => file,
            Err(_) => self.core.create_at_path(&args.path).await?,
        };
        self.core
            .write_document(file.id, args.content.as_bytes())
            .await?;
        Ok(format!("wrote {} bytes to {}", args.content.len(), args.path))
    }
}

struct ListDir {
    core: Lb,
}

#[derive(Deserialize)]
struct ListArgs {
    #[serde(default)]
    path: Option<String>,
}

impl Tool for ListDir {
    const NAME: &'static str = "list_dir";
    type Error = ToolFailure;
    type Args = ListArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "List the entries of a lockbook folder (non-recursive). Folders \
                          are suffixed with '/'."
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Folder path; omit for the root" }
                },
                "required": []
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let folder = match args.path.as_deref() {
            None | Some("/") | Some("") => self.core.root().await?,
            Some(path) => self.core.get_by_path(path).await?,
        };
        let children = self.core.get_children(&folder.id).await?;
        let mut entries: Vec<String> = children
            .into_iter()
            .map(|f| if f.is_folder() { format!("{}/", f.name) } else { f.name })
            .collect();
        entries.sort();
        if entries.is_empty() { Ok("(empty)".to_string()) } else { Ok(entries.join("\n")) }
    }
}

/// Cap on entries returned by `list_paths` and results returned by `search`,
/// so one call can't flood the context window.
const LIST_CAP: usize = 500;
const SEARCH_RESULT_CAP: usize = 10;
const SNIPPETS_PER_RESULT: usize = 3;

struct ListPaths {
    core: Lb,
}

#[derive(Deserialize)]
struct NoArgs {}

impl Tool for ListPaths {
    const NAME: &'static str = "list_paths";
    type Error = ToolFailure;
    type Args = NoArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "List every path in the lockbook (documents and folders). Prefer \
                          this over walking folders with list_dir."
                .to_string(),
            parameters: json!({ "type": "object", "properties": {}, "required": [] }),
        }
    }

    async fn call(&self, _args: Self::Args) -> Result<Self::Output, Self::Error> {
        let mut paths = self.core.list_paths(None).await?;
        paths.sort();
        let total = paths.len();
        paths.truncate(LIST_CAP);
        let mut out = paths.join("\n");
        if total > LIST_CAP {
            out.push_str(&format!("\n(+{} more)", total - LIST_CAP));
        }
        Ok(out)
    }
}

struct MoveFile {
    core: Lb,
}

#[derive(Deserialize)]
struct MoveArgs {
    path: String,
    new_parent: String,
}

impl Tool for MoveFile {
    const NAME: &'static str = "move_file";
    type Error = ToolFailure;
    type Args = MoveArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Move a document or folder into another folder (same name).".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Current lockbook path" },
                    "new_parent": { "type": "string", "description": "Destination folder path" }
                },
                "required": ["path", "new_parent"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let file = self.core.get_by_path(&args.path).await?;
        let parent = self.core.get_by_path(&args.new_parent).await?;
        self.core.move_file(&file.id, &parent.id).await?;
        Ok(format!("moved {} into {}", args.path, args.new_parent))
    }
}

struct RenameFile {
    core: Lb,
}

#[derive(Deserialize)]
struct RenameArgs {
    path: String,
    new_name: String,
}

impl Tool for RenameFile {
    const NAME: &'static str = "rename_file";
    type Error = ToolFailure;
    type Args = RenameArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Rename a document or folder in place (same parent).".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Current lockbook path" },
                    "new_name": { "type": "string", "description": "New name, e.g. notes.md" }
                },
                "required": ["path", "new_name"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let file = self.core.get_by_path(&args.path).await?;
        self.core.rename_file(&file.id, &args.new_name).await?;
        Ok(format!("renamed {} to {}", args.path, args.new_name))
    }
}

struct CreateFolder {
    core: Lb,
}

#[derive(Deserialize)]
struct CreateFolderArgs {
    path: String,
}

impl Tool for CreateFolder {
    const NAME: &'static str = "create_folder";
    type Error = ToolFailure;
    type Args = CreateFolderArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Create a folder (and any missing parents).".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Folder path, e.g. /projects/q3" }
                },
                "required": ["path"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        // a trailing slash is how create_at_path knows it's a folder
        let path = format!("{}/", args.path.trim_end_matches('/'));
        self.core.create_at_path(&path).await?;
        Ok(format!("created {path}"))
    }
}

struct StatFile {
    core: Lb,
}

#[derive(Deserialize)]
struct StatArgs {
    path: String,
}

impl Tool for StatFile {
    const NAME: &'static str = "stat";
    type Error = ToolFailure;
    type Args = StatArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Metadata for a document or folder: type, size, last modified, \
                          sharing. Cheaper than reading the document."
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Lockbook path" }
                },
                "required": ["path"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let file = self.core.get_by_path(&args.path).await?;
        let kind = if file.is_folder() { "folder" } else { "document" };
        let modified = chrono::DateTime::from_timestamp_millis(file.last_modified as i64)
            .map(|dt| dt.format("%Y-%m-%d %H:%M UTC").to_string())
            .unwrap_or_default();
        let shares = if file.shares.is_empty() {
            "not shared".to_string()
        } else {
            format!("shared with {} user(s)", file.shares.len())
        };
        Ok(format!(
            "{}: {kind}, {} bytes, modified {modified} by {}, {shares}",
            args.path, file.size_bytes, file.last_modified_by
        ))
    }
}

/// `SearchResult.parent_path` has no trailing slash (except at the root), so
/// joining naively yields "/notesfile.md".
fn join_result_path(parent: &str, filename: &str) -> String {
    format!("{}/{filename}", parent.trim_end_matches('/'))
}

struct SearchContent {
    core: BlockingLb,
}

#[derive(Deserialize)]
struct SearchArgs {
    query: String,
}

impl Tool for SearchContent {
    const NAME: &'static str = "search_content";
    type Error = ToolFailure;
    type Args = SearchArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Full-text search across all markdown documents. Returns matching \
                          paths with snippets, best matches first."
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Words to search for (case-insensitive)" }
                },
                "required": ["query"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let core = self.core.clone();
        // ContentSearcher reads every .md through the blocking Lb (its own
        // worker threads + blocking calls on this one), so it can't run on a
        // tokio worker.
        let out = tokio::task::spawn_blocking(move || {
            let mut searcher = ContentSearcher::new(&core);
            searcher.query(&args.query);

            let mut out = String::new();
            for result in searcher.results().iter().take(SEARCH_RESULT_CAP) {
                out.push_str(&join_result_path(&result.parent_path, &result.filename));
                out.push('\n');
                for m in result.content_matches.iter().take(SNIPPETS_PER_RESULT) {
                    if let Some((pre, hit, post)) = searcher.snippet(result.id, &m.range, 40) {
                        out.push_str(&format!("  …{pre}{hit}{post}…\n"));
                    }
                }
            }
            let total = searcher.results().len();
            if total > SEARCH_RESULT_CAP {
                out.push_str(&format!("(+{} more matching documents)", total - SEARCH_RESULT_CAP));
            }
            if out.is_empty() { "(no matches)".to_string() } else { out }
        })
        .await
        .map_err(|e| ToolFailure(format!("search task failed: {e}")))?;
        Ok(out)
    }
}

struct SearchPaths {
    core: Lb,
}

impl Tool for SearchPaths {
    const NAME: &'static str = "search_paths";
    type Error = ToolFailure;
    type Args = SearchArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Fuzzy search over file and folder paths (like a filename \
                          picker). Returns the best-matching paths."
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Approximate name or path fragment" }
                },
                "required": ["query"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let mut searcher = PathSearcher::new(&self.core).await;
        searcher.query(&args.query);
        let out = searcher
            .results()
            .iter()
            .take(20)
            .map(|r| join_result_path(&r.parent_path, &r.filename))
            .collect::<Vec<_>>()
            .join("\n");
        if out.is_empty() { Ok("(no matches)".to_string()) } else { Ok(out) }
    }
}

struct ReadPdf {
    core: Lb,
}

impl Tool for ReadPdf {
    const NAME: &'static str = "read_pdf";
    type Error = ToolFailure;
    type Args = ReadArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Extract the text of a PDF document in the lockbook. Works for \
                          text-based PDFs; scanned/image-only PDFs yield little or nothing."
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Lockbook path, e.g. /papers/attention.pdf" }
                },
                "required": ["path"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let file = self.core.get_by_path(&args.path).await?;
        let bytes = self.core.read_document(file.id, false).await?;
        // CPU-heavy parse off the async workers; panics in the parser (it
        // has them) surface as a join error rather than killing the driver.
        let text = tokio::task::spawn_blocking(move || pdf_extract::extract_text_from_mem(&bytes))
            .await
            .map_err(|e| ToolFailure(format!("pdf parse crashed: {e}")))?
            .map_err(|e| ToolFailure(format!("pdf parse failed: {e}")))?;
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Err(ToolFailure("no extractable text (scanned or image-only pdf?)".into()));
        }
        if trimmed.len() > READ_CAP_BYTES {
            let mut end = READ_CAP_BYTES;
            while !trimmed.is_char_boundary(end) {
                end -= 1;
            }
            return Ok(format!("{}\n(truncated)", &trimmed[..end]));
        }
        Ok(trimmed.to_string())
    }
}
