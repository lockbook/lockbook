use crate::appearance::Appearance;
use crate::ast::Ast;
use crate::buffer::{Buffer, EditorMutation};
use crate::debug::DebugInfo;
use crate::galleys::Galleys;
use crate::input;
use crate::input::canonical::Modification;
use crate::input::click_checker::EditorClickChecker;
use crate::input::cursor::PointerState;
use crate::layouts::Layouts;
use egui::{Event, Vec2};
use std::time::Instant;

/// processes `events` and returns a boolean representing whether text was updated, new contents for clipboard
/// (optional), and a link that was opened (optional)
#[allow(clippy::too_many_arguments)]
pub fn process(
    events: &[Event], ast: &Ast, layouts: &Layouts, galleys: &Galleys, appearance: &Appearance,
    ui_size: Vec2, buffer: &mut Buffer, debug: &mut DebugInfo, pointer_state: &mut PointerState,
) -> (bool, Option<String>, Option<String>) {
    let click_checker = EditorClickChecker { ui_size, galleys, buffer, ast, appearance };
    events
        .iter()
        .filter_map(|e| input::canonical::calc(e, &click_checker, pointer_state, Instant::now()))
        .collect::<Vec<Modification>>()
        .into_iter()
        .map(|m| match input::mutation::calc(m, layouts, &buffer.current, galleys) {
            EditorMutation::Buffer(mutations) if mutations.is_empty() => (false, None, None),
            EditorMutation::Buffer(mutations) => buffer.apply(mutations, debug),
            EditorMutation::Undo => {
                buffer.undo(debug);
                (true, None, None)
            }
            EditorMutation::Redo => {
                buffer.redo(debug);
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
