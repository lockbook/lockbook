use crate::tab::{self, markdown_editor, ClipContent};
use egui::Context;
use lb_rs::text::buffer;
use lb_rs::Uuid;
use markdown_editor::input::{Event, Region};
use markdown_editor::Editor;
use std::time::Instant;

impl Editor {
    /// combines `events` and `custom_events` into a single set of events
    pub fn combine_events(
        &mut self, ctx: &egui::Context, events: Vec<egui::Event>, custom_events: Vec<crate::Event>,
        touch_mode: bool,
    ) -> Vec<Event> {
        let canonical_egui_events = events
            .into_iter()
            .filter_map(|e| self.calc_events(ctx, e, Instant::now(), touch_mode))
            .collect::<Vec<_>>();

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
        let mut ops = Vec::new();
        let mut response = buffer::Response::default();
        for event in combined_events {
            response |= self.calc_operations(ctx, event, &mut ops);
        }
        self.buffer.queue(ops);
        response |= self.buffer.update();
        response.into()
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
