//! Provider-neutral completion backend for the chat agent. Streaming-first:
//! a backend delivers text deltas as they arrive and returns the turn's
//! usage when the stream ends. Tool calls will extend [`CompletionReq`] and
//! the delta channel in a later increment; on-device inference slots in as
//! another [`Backend`] variant.

use lb_rs::model::chat::Usage;
use tokio::sync::mpsc::UnboundedSender;

use super::openai::OpenAiBackend;

/// One completion request: the whole conversation so far.
pub struct CompletionReq {
    pub system: String,
    pub messages: Vec<ChatMsg>,
    pub max_tokens: u32,
}

/// A transcript message in model context.
#[derive(Clone)]
pub enum ChatMsg {
    User(String),
    Assistant(String),
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
    OpenAi(OpenAiBackend),
}

impl Backend {
    /// Run one completion, sending each text delta on `deltas` as it arrives.
    /// Returns the turn's token usage once the stream completes. The caller
    /// accumulates deltas itself, so cancelling (dropping) this future keeps
    /// the text streamed so far.
    pub async fn complete(
        &self, req: CompletionReq, deltas: UnboundedSender<String>,
    ) -> Result<Usage, String> {
        match self {
            Backend::OpenAi(b) => b.complete(req, deltas).await,
        }
    }
}
