//! Provider-neutral completion backend for the chat agent. Streaming-first:
//! a backend delivers text deltas as they arrive and returns the turn's
//! outcome — any tool calls the model requested, plus usage — when the
//! stream ends. On-device inference slots in as another [`Backend`] variant.

use lb_rs::model::chat::Usage;
use tokio::sync::mpsc::UnboundedSender;

use super::anthropic::AnthropicBackend;
use super::openai::OpenAiBackend;

/// One completion request: the whole conversation so far.
pub struct CompletionReq {
    pub system: String,
    pub messages: Vec<ChatMsg>,
    pub max_tokens: u32,
    /// Tools advertised to the model. Keep the set and order stable across a
    /// conversation — tools render ahead of everything else in the provider's
    /// prompt, so any change invalidates its cache.
    pub tools: Vec<ToolSchema>,
}

/// A transcript message in model context.
#[derive(Clone, Debug)]
pub enum ChatMsg {
    User(String),
    /// A model turn: streamed text plus any tool calls it requested.
    Assistant {
        text: String,
        tool_calls: Vec<ToolCall>,
    },
    /// One tool call's outcome, echoed back under the provider's call id.
    ToolResult {
        id: String,
        content: String,
        is_error: bool,
    },
}

/// A tool the model may call: name, guidance, and a JSON Schema for its
/// arguments (flat object, `additionalProperties: false` — strict-compatible
/// on both wire dialects).
pub struct ToolSchema {
    pub name: &'static str,
    pub description: &'static str,
    pub parameters: serde_json::Value,
}

/// One tool invocation requested by the model.
#[derive(Clone, Debug, PartialEq)]
pub struct ToolCall {
    /// Provider-assigned call id; the matching [`ChatMsg::ToolResult`] echoes
    /// it.
    pub id: String,
    pub name: String,
    /// Parsed arguments. When the model streams malformed argument JSON the
    /// text arrives as `{"raw": <text>}`, so dispatch can answer with a
    /// steering error instead of the turn dying.
    pub args: serde_json::Value,
}

/// Parse a tool call's accumulated argument string into [`ToolCall::args`],
/// the same way on both wire dialects: empty text is no arguments (`{}`),
/// and malformed JSON is wrapped raw (`{"raw": <text>}`) so dispatch answers
/// with a steering error instead of the turn dying.
pub fn parse_tool_args(args: &str) -> serde_json::Value {
    if args.trim().is_empty() {
        serde_json::json!({})
    } else {
        serde_json::from_str(args).unwrap_or_else(|_| serde_json::json!({ "raw": args }))
    }
}

/// A finished (unstopped) completion.
#[derive(Debug)]
pub struct Completion {
    pub tool_calls: Vec<ToolCall>,
    pub usage: Usage,
}

/// One entry from a provider's `/models` listing. `id` is what goes on the
/// wire; everything else is optional metadata some endpoints report.
pub struct ModelInfo {
    pub id: String,
    /// Human-readable name (Anthropic's `display_name`, OpenRouter's `name`).
    pub display_name: Option<String>,
    /// Context window in tokens, when the endpoint reports one.
    pub window: Option<u64>,
}

impl ModelInfo {
    /// What the picker shows: the display name when the endpoint offers one,
    /// the honest id otherwise.
    pub fn label(&self) -> &str {
        self.display_name.as_deref().unwrap_or(&self.id)
    }
}

/// The turn's completion backend — one variant per provider protocol. A
/// concrete enum rather than `Box<dyn>` so `complete` is a native `async fn`;
/// the provider set is closed and chosen by `provider.kind` at the call site.
pub enum Backend {
    Anthropic(AnthropicBackend),
    OpenAi(OpenAiBackend),
}

impl Backend {
    /// Run one completion, sending each text delta on `deltas` as it arrives.
    /// Returns the turn's tool calls and token usage once the stream
    /// completes. The caller accumulates deltas itself, so cancelling
    /// (dropping) this future keeps the text streamed so far.
    pub async fn complete(
        &self, req: CompletionReq, deltas: UnboundedSender<String>,
    ) -> Result<Completion, String> {
        match self {
            Backend::Anthropic(b) => b.complete(req, deltas).await,
            Backend::OpenAi(b) => b.complete(req, deltas).await,
        }
    }
}
