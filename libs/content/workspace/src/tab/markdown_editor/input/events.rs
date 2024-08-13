use crate::tab::{self, markdown_editor, ClipContent};
use egui::Context;
use lb_rs::Uuid;
use markdown_editor::input::canonical;
use markdown_editor::input::{Event, Region};
use markdown_editor::Editor;
use std::time::Instant;

use super::click_checker::EditorClickChecker;

impl Editor {
    /// combines `events` and `custom_events` into a single set of events
    pub fn combine_events(
        &mut self, events: &[egui::Event], custom_events: &[crate::Event], touch_mode: bool,
    ) -> Vec<Event> {
        let click_checker = EditorClickChecker {
            ui_rect: self.ui_rect,
            galleys: &self.galleys,
            buffer: &self.buffer,
            ast: &self.ast,
            appearance: &self.appearance,
            bounds: &self.bounds,
        };
        let canonical_egui_events = events.iter().filter_map(|e| {
            canonical::calc(
                e,
                &click_checker,
                &mut self.pointer_state,
                Instant::now(),
                touch_mode,
                &self.appearance,
            )
        });

        custom_events
            .iter()
            .cloned()
            .flat_map(|event| handle_custom_event(event, &mut self.core, self.file_id))
            .chain(canonical_egui_events)
            .collect()
    }

    /// Processes `combined_events`. Returns a (text_updated, selection_updated) pair.
    pub fn process_combined_events(
        &mut self, ctx: &Context, combined_events: Vec<Event>,
    ) -> (bool, bool) {
        for event in combined_events {
            let ops = self.calc_operations(&ctx, event);
            self.buffer.queue(ops);
        }
        self.buffer.update()
    }
}

fn handle_custom_event(event: crate::Event, core: &mut lb_rs::Core, file_id: Uuid) -> Vec<Event> {
    match event {
        crate::Event::Markdown(modification) => vec![modification],
        crate::Event::Drop { content, .. } | crate::Event::Paste { content, .. } => {
            let mut modifications = Vec::new();
            for clip in content {
                match clip {
                    ClipContent::Image(data) => {
                        let file = tab::import_image(core, file_id, &data);
                        let rel_path = tab::core_get_relative_path(core, file_id, file.id);
                        let markdown_image_link = format!("![{}]({})", file.name, rel_path);

                        modifications.push(Event::Replace {
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
