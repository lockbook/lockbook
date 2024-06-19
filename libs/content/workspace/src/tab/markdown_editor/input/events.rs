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
use crate::tab::{self, ClipContent};
use egui::Event;
use lb_rs::Uuid;
use std::time::Instant;

use super::canonical::Region;

/// combines `events` and `custom_events` into a single set of events
#[allow(clippy::too_many_arguments)]
pub fn combine(
    events: &[Event], custom_events: &[crate::Event], click_checker: impl ClickChecker + Copy,
    touch_mode: bool, appearance: &Appearance, pointer_state: &mut PointerState,
    core: &mut lb_rs::Core, open_file: Uuid,
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
        .flat_map(|event| handle_custom_event(event, core, open_file))
        .chain(canonical_egui_events)
        .collect()
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

fn handle_custom_event(
    event: crate::Event, core: &mut lb_rs::Core, open_file: Uuid,
) -> Vec<Modification> {
    match event {
        crate::Event::Markdown(modification) => vec![modification],
        crate::Event::Drop { content, .. } | crate::Event::Paste { content, .. } => {
            let mut modifications = Vec::new();
            for clip in content {
                match clip {
                    ClipContent::Image(data) => {
                        let file = tab::import_image(core, open_file, &data);
                        let markdown_image_link = format!("![{}](lb://{})", file.name, file.id);

                        modifications.push(Modification::Replace {
                            region: Region::Selection, // todo: more thoughtful location
                            text: markdown_image_link,
                        });
                    }
                    ClipContent::Files(..) => {
                        // todo: support file drop & paste
                        println!("unimplemented: editor file drop & paste");
                    }
                }
            }
            modifications
        }
    }
}
