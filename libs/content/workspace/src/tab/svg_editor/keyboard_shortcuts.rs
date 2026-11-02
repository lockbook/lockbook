//! Canvas keyboard bindings.
use egui::{Key, Modifiers};

use super::Tool;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CanvasAction {
    DeleteSelection,
    CutSelection,
    CopySelection,
    SelectAll,
    Undo,
    Redo,
    SetTool(Tool),
    ToggleToolPopover,
}

#[derive(Debug)]
pub struct KeyboardShortcut {
    key: Key,
    modifiers: Modifiers,
    pub action: CanvasAction,
}

impl KeyboardShortcut {
    pub const fn new(key: Key, modifiers: Modifiers, action: CanvasAction) -> Self {
        Self { key, modifiers, action }
    }

    pub fn matches(&self, event: &egui::Event) -> bool {
        match *event {
            egui::Event::Key { key, modifiers, pressed: true, repeat: false, .. } => {
                key == self.key && modifiers.matches_exact(self.modifiers)
            }
            egui::Event::Copy => self.key == Key::C && self.modifiers == Modifiers::COMMAND,
            egui::Event::Cut => self.key == Key::X && self.modifiers == Modifiers::COMMAND,
            _ => false,
        }
    }
}

pub const BINDINGS: &[KeyboardShortcut] = &[
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
