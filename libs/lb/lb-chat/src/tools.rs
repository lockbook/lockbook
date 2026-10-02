//! What the model may call. The driver advertises `schemas()` and dispatches
//! each call through `call()`; the vault toolset lives with the territory
//! rules, this module only fixes the shape.

use lb_rs::model::chat::{Chat, Mention};

use crate::wire::{Call, ToolSchema};

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
    /// Where a file the user attached is now.
    fn locate(&mut self, mention: &Mention) -> String {
        mention.path.clone()
    }
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
