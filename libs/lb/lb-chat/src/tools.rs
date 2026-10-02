//! What the model may call. The driver advertises `schemas()` and dispatches
//! each call through `call()`; the vault toolset lives with the territory
//! rules, this module only fixes the shape.

use lb_rs::model::chat::{Chat, Mention};

use crate::wire::{Call, Media, ToolSchema};

pub enum ToolOutcome {
    Done {
        text: String,
        ok: bool,
    },
    /// The run ends and the model never learns why.
    Abort {
        text: String,
    },
}

impl ToolOutcome {
    pub fn ok(text: impl Into<String>) -> Self {
        Self::Done { text: text.into(), ok: true }
    }

    pub fn err(text: impl Into<String>) -> Self {
        Self::Done { text: text.into(), ok: false }
    }
}

pub trait Tools: Send {
    fn schemas(&self) -> Vec<ToolSchema>;
    /// Called before each completion with the chat as it stands.
    fn prepare(&mut self, _chat: &Chat, _user: &str, _working_dir: &str) {}
    fn call(&mut self, call: &Call) -> ToolOutcome;
    /// The `AGENTS.md` of each folder from the root down to `working_dir`,
    /// as (path, text): the user's standing instructions for work there.
    fn instructions(&mut self, _working_dir: &str) -> Vec<(String, String)> {
        Vec::new()
    }
    /// The picture or PDF at `path` as a model is shown it, a picture
    /// brought down in size; nothing when it is gone or out of reach.
    fn media(&mut self, _path: &str) -> Option<Media> {
        None
    }
    /// Keeps something the model made as a file named `name` in the
    /// chat's `imports` folder, and answers with the path it got.
    fn keep(&mut self, _name: &str, _bytes: &[u8]) -> Result<String, String> {
        Err("there is nowhere to keep a file".into())
    }
    /// Where a file the user attached is now.
    fn locate(&mut self, mention: &Mention) -> String {
        mention.path.clone()
    }
    /// The device cannot reach the network right now, so the web tools
    /// will not work; a model on the device goes on without them.
    fn offline(&self) -> bool {
        false
    }
}

/// Whether `path` names a picture: something to be looked at, not read.
pub fn pictured(path: &str) -> bool {
    let name = path.to_lowercase();
    [".png", ".jpg", ".jpeg", ".gif", ".webp", ".bmp", ".svg"]
        .iter()
        .any(|ext| name.ends_with(ext))
}

/// Whether `path` names something a model is shown and not read to: a
/// picture or a PDF.
pub fn shown(path: &str) -> bool {
    pictured(path) || path.to_lowercase().ends_with(".pdf")
}

pub struct NoTools;

impl Tools for NoTools {
    fn schemas(&self) -> Vec<ToolSchema> {
        Vec::new()
    }

    fn call(&mut self, call: &Call) -> ToolOutcome {
        ToolOutcome::err(format!("no tool named {}", call.name))
    }
}
