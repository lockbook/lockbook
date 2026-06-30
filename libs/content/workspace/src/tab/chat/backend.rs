//! Provider-neutral chat backend seam.
//!
//! The agentic loop in [`super::harness`] talks only to [`ChatBackend`]; the
//! provider is a detail behind it. One cloud implementor wraps any rig
//! `CompletionModel` (Anthropic, or any OpenAI-compatible endpoint — Cerebras,
//! Groq, Together, …), and a future on-device model implements the same trait
//! against native ML. Keeping our own request/response types — rather than
//! threading rig's generics through the loop — is what lets a non-HTTP local
//! backend be a first-class peer instead of something bolted onto an HTTP
//! trait.

use async_trait::async_trait;
use lb_rs::model::chat::Usage;

/// One message in the model-facing conversation, in transcript order. The
/// loop builds the full history out of these every turn; backends translate
/// to their wire format.
#[derive(Clone, Debug)]
pub enum ChatMsg {
    /// Owner-typed text.
    User(String),
    /// An assistant turn: prose plus any tool calls it requested. Both may be
    /// present (the model can narrate while calling a tool).
    Assistant { text: String, tool_calls: Vec<ToolCall> },
    /// The result fed back for a previously-requested tool call, correlated by
    /// `id`. A denied call still produces one of these (`DENIED_RESULT`).
    ToolResult { id: String, content: String },
}

/// A tool call requested by the model.
#[derive(Clone, Debug)]
pub struct ToolCall {
    /// Provider-supplied id, used to correlate the matching [`ChatMsg::ToolResult`].
    pub id: String,
    pub name: String,
    pub args: serde_json::Value,
}

/// A tool the model may call, advertised on every completion request.
#[derive(Clone, Debug)]
pub struct ToolSchema {
    pub name: String,
    pub description: String,
    /// JSON Schema for the call arguments.
    pub parameters: serde_json::Value,
}

/// One completion step: the whole conversation so far plus the tool menu.
pub struct CompletionReq {
    pub system: String,
    pub messages: Vec<ChatMsg>,
    pub tools: Vec<ToolSchema>,
    pub max_tokens: u32,
}

/// The model's response to one completion step. `tool_calls` empty means the
/// turn is done and `text` is the final answer; otherwise the loop runs the
/// calls (gated by approval) and steps again.
pub struct CompletionResp {
    pub text: String,
    pub tool_calls: Vec<ToolCall>,
    /// Token usage of this single completion request. The loop sums these
    /// across the steps of one turn.
    pub usage: Usage,
}

/// One pickable model. `id` is the canonical request value (never abbreviated;
/// it is what `/chat.json` stores); `label` is the human display name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelChoice {
    pub id: String,
    pub label: String,
}

/// A provider the loop can drive. Errors are surfaced to the user as chat
/// error rows, so the message is the user-facing string.
#[async_trait]
pub trait ChatBackend: Send + Sync {
    /// Run one completion step over the given conversation and tool menu.
    async fn complete(&self, req: CompletionReq) -> Result<CompletionResp, String>;

    /// Models this provider offers, newest first. Best-effort: the settings
    /// picker falls back to a free-text id field when this fails or is empty.
    async fn list_models(&self) -> Result<Vec<ModelChoice>, String>;
}
