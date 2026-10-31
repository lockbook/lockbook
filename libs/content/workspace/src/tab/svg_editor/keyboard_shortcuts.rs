//! Canvas bindings and their discoverable labels share one source of truth.
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
    label: &'static str,
}

impl KeyboardShortcut {
    pub const fn new(
        key: Key, modifiers: Modifiers, action: CanvasAction, label: &'static str,
    ) -> Self {
        Self { key, modifiers, action, label }
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
    KeyboardShortcut::new(
        Key::Backspace,
        Modifiers::NONE,
        CanvasAction::DeleteSelection,
        "Delete selection",
    ),
    KeyboardShortcut::new(
        Key::Delete,
        Modifiers::NONE,
        CanvasAction::DeleteSelection,
        "Delete selection",
    ),
    KeyboardShortcut::new(Key::X, Modifiers::COMMAND, CanvasAction::CutSelection, "Cut selection"),
    KeyboardShortcut::new(
        Key::C,
        Modifiers::COMMAND,
        CanvasAction::CopySelection,
        "Copy selection",
    ),
    KeyboardShortcut::new(Key::A, Modifiers::COMMAND, CanvasAction::SelectAll, "Select all"),
    KeyboardShortcut::new(Key::Z, Modifiers::COMMAND, CanvasAction::Undo, "Undo"),
    KeyboardShortcut::new(
        Key::Z,
        Modifiers::COMMAND.plus(Modifiers::SHIFT),
        CanvasAction::Redo,
        "Redo",
    ),
    KeyboardShortcut::new(Key::B, Modifiers::NONE, CanvasAction::SetTool(Tool::Pen), "Pen"),
    KeyboardShortcut::new(Key::E, Modifiers::NONE, CanvasAction::SetTool(Tool::Eraser), "Eraser"),
    KeyboardShortcut::new(
        Key::S,
        Modifiers::NONE,
        CanvasAction::SetTool(Tool::Selection),
        "Selection",
    ),
    KeyboardShortcut::new(
        Key::Tab,
        Modifiers::NONE,
        CanvasAction::ToggleToolPopover,
        "Tool popover",
    ),
];

pub fn show_help(ui: &mut egui::Ui) {
    ui.label("Canvas shortcuts");
    for binding in BINDINGS {
        ui.horizontal(|ui| {
            ui.label(
                ui.ctx()
                    .format_shortcut(&egui::KeyboardShortcut::new(binding.modifiers, binding.key)),
            );
            ui.label(binding.label);
        });
    }
    ui.horizontal(|ui| {
        ui.label(
            ui.ctx()
                .format_shortcut(&egui::KeyboardShortcut::new(Modifiers::COMMAND, Key::V)),
        );
        ui.label("Paste");
    });
}
