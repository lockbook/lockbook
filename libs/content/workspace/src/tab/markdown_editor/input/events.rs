use crate::tab::markdown_editor::appearance::Appearance;
use crate::tab::markdown_editor::ast::Ast;
use crate::tab::markdown_editor::bounds::Bounds;
use crate::tab::markdown_editor::buffer::{Buffer, EditorMutation};
use crate::tab::markdown_editor::debug::DebugInfo;
use crate::tab::markdown_editor::galleys::Galleys;
use crate::tab::markdown_editor::input;
use crate::tab::markdown_editor::input::canonical::Modification;
use crate::tab::markdown_editor::input::click_checker::ClickChecker;
use crate::tab::markdown_editor::input::cursor::PointerState;
use egui::Event;
use std::time::Instant;

/// combines `events` and `custom_events` into a single set of events
pub fn combine(
    events: &[Event], custom_events: &[Modification], click_checker: impl ClickChecker + Copy,
    touch_mode: bool, appearance: &Appearance, pointer_state: &mut PointerState,
) -> Vec<Modification> {
    let canonical_egui_events = events.iter().filter_map(|e| {
        input::canonical::calc(
            e,
            click_checker,
            pointer_state,
            Instant::now(),
            touch_mode,
            appearance,
        )
    });

    custom_events
        .iter()
        .cloned()
        .chain(canonical_egui_events)
        .collect::<Vec<Modification>>()
}

/// processes `combined_events` and returns a boolean representing whether text was updated, new contents for clipboard
/// (optional), and a link that was opened (optional)
pub fn process(
    combined_events: &[Modification], galleys: &Galleys, bounds: &Bounds, ast: &Ast,
    buffer: &mut Buffer, debug: &mut DebugInfo, appearance: &mut Appearance,
) -> (bool, Option<String>, Option<String>) {
    combined_events
        .iter()
        .cloned()
        .map(|m| match input::mutation::calc(m, &buffer.current, galleys, bounds, ast) {
            EditorMutation::Buffer(mutations) if mutations.is_empty() => (false, None, None),
            EditorMutation::Buffer(mutations) => buffer.apply(mutations, debug, appearance),
            EditorMutation::Undo => {
                buffer.undo(debug, appearance);
                (true, None, None)
            }
            EditorMutation::Redo => {
                buffer.redo(debug, appearance);
                (true, None, None)
            }
        })
        .reduce(
            |(text_updated, to_clipboard, opened_url),
             (mutation_text_updated, mutation_to_clipboard, mutation_opened_url)| {
                (
                    text_updated || mutation_text_updated,
                    mutation_to_clipboard.or(to_clipboard),
                    mutation_opened_url.or(opened_url),
                )
            },
        )
        .unwrap_or_default()
}
