//! Canvas keyboard bindings.
use egui::{Key, Modifiers};

use super::{CanvasAction, Tool};

#[derive(Debug)]
pub struct KeyboardShortcut {
    pub key: Key,
    pub modifiers: Modifiers,
    pub action: CanvasAction,
}

impl KeyboardShortcut {
    pub const fn new(key: Key, modifiers: Modifiers, action: CanvasAction) -> Self {
        Self { key, modifiers, action }
    }

    pub fn matches(&self, event: &egui::Event) -> bool {
        match *event {
            egui::Event::Key { key, modifiers, pressed: true, repeat, .. } => {
                key == self.key
                    && modifiers.matches_exact(self.modifiers)
                    && (!repeat || matches!(self.action, CanvasAction::AdjustBrushSize(_)))
            }
            egui::Event::Copy => self.key == Key::C && self.modifiers == Modifiers::COMMAND,
            egui::Event::Cut => self.key == Key::X && self.modifiers == Modifiers::COMMAND,
            _ => false,
        }
    }
}

pub const BINDINGS: &[KeyboardShortcut] = &[
    KeyboardShortcut::new(Key::OpenBracket, Modifiers::NONE, CanvasAction::AdjustBrushSize(-1.0)),
    KeyboardShortcut::new(Key::CloseBracket, Modifiers::NONE, CanvasAction::AdjustBrushSize(1.0)),
    KeyboardShortcut::new(Key::Backspace, Modifiers::NONE, CanvasAction::DeleteSelection),
    KeyboardShortcut::new(Key::Delete, Modifiers::NONE, CanvasAction::DeleteSelection),
    KeyboardShortcut::new(Key::X, Modifiers::COMMAND, CanvasAction::CutSelection),
    KeyboardShortcut::new(Key::C, Modifiers::COMMAND, CanvasAction::CopySelection),
    KeyboardShortcut::new(Key::A, Modifiers::COMMAND, CanvasAction::SelectAll),
    KeyboardShortcut::new(Key::Z, Modifiers::COMMAND, CanvasAction::Undo),
    KeyboardShortcut::new(Key::Z, Modifiers::COMMAND.plus(Modifiers::SHIFT), CanvasAction::Redo),
    KeyboardShortcut::new(Key::B, Modifiers::NONE, CanvasAction::SetTool(Tool::Pen)),
    KeyboardShortcut::new(Key::E, Modifiers::NONE, CanvasAction::SetTool(Tool::Eraser)),
    KeyboardShortcut::new(Key::S, Modifiers::NONE, CanvasAction::SetTool(Tool::Selection)),
    KeyboardShortcut::new(Key::Tab, Modifiers::NONE, CanvasAction::ToggleToolPopover),
];

/// Resolves keyboard and native clipboard events without changing editor state.
pub fn resolve(event: &egui::Event) -> Option<CanvasAction> {
    BINDINGS
        .iter()
        .find(|binding| binding.matches(event))
        .map(|binding| binding.action.clone())
}
