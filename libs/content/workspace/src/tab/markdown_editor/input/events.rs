use crate::tab::markdown_editor::bounds::{BoundCase, BoundExt as _};
use crate::tab::markdown_editor::input::Location;
use crate::tab::{self, markdown_editor, ClipContent, ExtendedInput as _};
use egui::{Context, EventFilter};
use lb_rs::text::buffer;
use lb_rs::text::offset_types::RangeExt as _;
use markdown_editor::input::{Event, Region};
use markdown_editor::Editor;

use super::canonical::translate_egui_keyboard_event;
use super::Bound;

impl Editor {
    pub fn process_events(&mut self, ctx: &Context) -> (bool, bool) {
        let mut ops = Vec::new();
        let mut response = buffer::Response::default();
        for event in self.get_cursor_fix_events() {
            response |= self.calc_operations(ctx, event, &mut ops);
        }
        for event in self.get_workspace_events(ctx) {
            response |= self.calc_operations(ctx, event, &mut ops);
        }
        for event in self.get_key_events(ctx) {
            response |= self.calc_operations(ctx, event, &mut ops);
        }
        for event in self.get_pointer_events(ctx) {
            response |= self.calc_operations(ctx, event, &mut ops);
        }
        self.buffer.queue(ops);
        response |= self.buffer.update();
        response.into()
    }

    fn get_cursor_fix_events(&self) -> Vec<Event> {
        // if the cursor is in an invalid location, move it to the next valid location
        let mut fixed_selection = self.buffer.current.selection;
        if let BoundCase::BetweenRanges { range_after, .. } =
            fixed_selection.0.bound_case(&self.bounds.text)
        {
            fixed_selection.0 = range_after.start();
        }
        if let BoundCase::BetweenRanges { range_after, .. } =
            fixed_selection.1.bound_case(&self.bounds.text)
        {
            fixed_selection.1 = range_after.start();
        }

        if fixed_selection != self.buffer.current.selection {
            vec![Event::Select { region: fixed_selection.into() }]
        } else {
            vec![]
        }
    }

    fn get_workspace_events(&self, ctx: &Context) -> Vec<Event> {
        let mut result = Vec::new();
        for event in ctx.pop_events() {
            match event {
                crate::Event::Markdown(modification) => result.push(modification),
                crate::Event::Drop { content, .. } | crate::Event::Paste { content, .. } => {
                    for clip in content {
                        match clip {
                            ClipContent::Image(data) => {
                                let file = tab::import_image(&self.core, self.file_id, &data);
                                let rel_path =
                                    tab::core_get_relative_path(&self.core, self.file_id, file.id);
                                let markdown_image_link = format!("![{}]({})", file.name, rel_path);

                                result.push(Event::Replace {
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
                }
            }
        }
        result
    }

    fn get_key_events(&self, ctx: &Context) -> Vec<Event> {
        let has_focus = ctx.memory(|m| m.focused().is_none()); // focused by default
        if has_focus {
            ctx.input(|r| {
                r.filtered_events(&EventFilter {
                    tab: true,
                    horizontal_arrows: true,
                    vertical_arrows: true,
                    escape: false,
                })
            })
            .into_iter()
            .filter_map(|e| translate_egui_keyboard_event(e))
            .collect::<Vec<_>>()
        } else {
            Vec::new()
        }
    }

    fn get_pointer_events(&self, ctx: &Context) -> Vec<Event> {
        if cfg!(target_os = "ios") {
            // iOS handles touch events using virtual keyboard FFI fn's
            return Vec::new();
        }
        for i in 0..self.galleys.galleys.len() {
            let galley = &self.galleys.galleys[i];
            if let Some(response) = ctx.read_response(galley.response.id) {
                let pos =
                    if let Some(pos) = response.interact_pointer_pos() { pos } else { continue };
                let location = Location::Pos(pos);
                let modifiers = ctx.input(|i| i.modifiers);

                // note: deliberate order; a double click is also a click
                let region = if response.triple_clicked() {
                    Region::BoundAt { bound: Bound::Paragraph, location, backwards: true }
                } else if response.double_clicked() {
                    Region::BoundAt { bound: Bound::Word, location, backwards: true }
                } else if response.clicked() && modifiers.shift {
                    Region::ToLocation(location)
                } else if response.clicked() {
                    Region::Location(location)
                } else if response.secondary_clicked() {
                    // todo: show context menu
                    continue;
                } else if response.dragged() && modifiers.shift {
                    Region::ToLocation(location)
                } else if response.dragged() {
                    let origin = if let Some(origin) = ctx.input(|i| i.pointer.press_origin()) {
                        origin
                    } else {
                        // unexpected
                        continue;
                    };

                    Region::BetweenLocations {
                        start: Location::Pos(origin),
                        end: Location::Pos(pos),
                    }
                } else {
                    // can't yet tell if drag
                    continue;
                };

                return vec![Event::Select { region }];
            }
        }
        Vec::new()
    }
}
