//! What the model may call. The driver advertises `schemas()` and dispatches
//! each call through `call()`; the vault toolset lives with the territory
//! rules, this module only fixes the shape.

use lb_rs::model::chat::Mention;

use crate::wire::{Call, ToolSchema};

pub struct ToolOutcome {
    pub text: String,
    pub ok: bool,
}

impl ToolOutcome {
    pub fn ok(text: impl Into<String>) -> Self {
        Self { text: text.into(), ok: true }
    }

    pub fn err(text: impl Into<String>) -> Self {
        Self { text: text.into(), ok: false }
    }
}

pub trait Tools: Send {
    fn schemas(&self) -> Vec<ToolSchema>;
    fn call(&mut self, call: &Call) -> ToolOutcome;
    /// Current text of a file the user attached, or nothing if it is gone.
    fn read_mention(&mut self, _mention: &Mention) -> Option<String> {
        None
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
