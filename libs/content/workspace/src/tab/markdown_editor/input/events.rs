use std::mem;

use crate::tab::markdown_editor::bounds::{BoundExt as _, RangesExt as _};
use crate::tab::{self, ClipContent, ExtendedInput as _, ExtendedOutput as _, markdown_editor};
use crate::theme::icons::Icon;
use crate::widgets::IconButton;
use comrak::nodes::AstNode;
use egui::{Context, EventFilter, Pos2, Stroke, ViewportCommand};
use lb_rs::model::text::buffer;
use lb_rs::model::text::offset_types::{DocCharOffset, RangeExt as _, RangeIterExt as _};
use markdown_editor::Editor;
use markdown_editor::input::{Event, Region};

use super::{Bound, Location, mutation};

impl<'ast> Editor {
    pub fn process_events(&mut self, ctx: &Context, root: &'ast AstNode<'ast>) -> bool {
        let mut ops = Vec::new();
        let mut response = buffer::Response::default();
        for event in mem::take(&mut self.event.internal_events) {
            response |= self.calc_operations(ctx, root, event, &mut ops);
        }
        for event in self.get_workspace_events(ctx) {
            response |= self.calc_operations(ctx, root, event, &mut ops);
        }
        for event in self.get_key_events(ctx) {
            response |= self.calc_operations(ctx, root, event, &mut ops);
        }
        for event in self.get_pointer_events(ctx) {
            response |= self.calc_operations(ctx, root, event, &mut ops);
        }
        self.buffer.queue(ops);
        response |= self.buffer.update();
        response.into()
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
                                let parent = self.core.get_file_by_id(self.file_id).unwrap().parent;

                                let rel_path =
                                    tab::core_get_relative_path(&self.core, parent, file.id);
                                let markdown_image_link = format!("![{}]({})", file.name, rel_path);

                                result.push(Event::Replace {
                                    region: Region::Selection, // todo: more thoughtful location
                                    text: markdown_image_link,
                                    advance_cursor: true,
                                });
                            }
                            ClipContent::Files(..) => {
                                // todo: support file drop & paste
                                println!("unimplemented: editor file drop & paste");
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        result
    }

    fn get_key_events(&self, ctx: &Context) -> Vec<Event> {
        if self.focused(ctx) {
            ctx.input(|r| {
                r.filtered_events(&EventFilter {
                    tab: true,
                    horizontal_arrows: true,
                    vertical_arrows: true,
                    escape: false,
                })
            })
            .into_iter()
            .filter_map(|e| self.translate_egui_keyboard_event(e))
            .collect::<Vec<_>>()
        } else {
            Vec::new()
        }
    }

    fn get_pointer_events(&mut self, ctx: &Context) -> Vec<Event> {
        let modifiers = ctx.input(|i| i.modifiers);

        if let Some(response) = ctx.read_response(self.id()) {
            ctx.style_mut(|s| s.spacing.menu_margin = egui::vec2(10., 5.).into());
            ctx.style_mut(|s| s.visuals.menu_rounding = (2.).into());
            ctx.style_mut(|s| s.visuals.window_fill = s.visuals.extreme_bg_color);
            ctx.style_mut(|s| s.visuals.window_stroke = Stroke::NONE);
            if !cfg!(target_os = "ios") && !cfg!(target_os = "android") {
                let mut context_menu_events = Vec::new();
                response.context_menu(|ui| {
                    ui.horizontal(|ui| {
                        ui.set_min_height(30.);
                        ui.style_mut().spacing.button_padding = egui::vec2(5.0, 5.0);

                        if IconButton::new(&Icon::CONTENT_CUT)
                            .tooltip("Cut")
                            .show(ui)
                            .clicked()
                        {
                            context_menu_events.push(Event::Cut);
                            ui.close_menu();
                        }
                        ui.add_space(5.);
                        if IconButton::new(&Icon::CONTENT_COPY)
                            .tooltip("Copy")
                            .show(ui)
                            .clicked()
                        {
                            context_menu_events.push(Event::Copy);
                            ui.close_menu();
                        }
                        ui.add_space(5.);
                        if IconButton::new(&Icon::CONTENT_PASTE)
                            .tooltip("Paste")
                            .show(ui)
                            .clicked()
                        {
                            // paste must go through the window because we don't yet have the clipboard content
                            ui.ctx().send_viewport_cmd(ViewportCommand::RequestPaste);
                            ui.close_menu();
                        }
                    });
                });
                if !context_menu_events.is_empty() {
                    return context_menu_events;
                }
            }

            // note: early return here unless response has a pointer interaction
            let pos = if let Some(pos) = response.interact_pointer_pos() {
                pos
            } else {
                return Vec::new();
            };
            let location = Location::Pos(pos);

            // note: deliberate order; a double click is also a click
            let region = if response.double_clicked() || response.triple_clicked() {
                // egui triple click detection is not that good and can report triple clicks without reporting double clicks
                if cfg!(target_os = "android") {
                    // android native context menu: multi-tapped for selection
                    // position based on text range of word that will be selected
                    let offset = self.location_to_char_offset(location);
                    let range = offset
                        .range_bound(Bound::Word, true, true, &self.bounds)
                        .unwrap_or((offset, offset));
                    ctx.set_context_menu(self.context_menu_pos(range).unwrap_or(pos));

                    Region::BoundAt { bound: Bound::Word, location, backwards: true }
                } else if self.buffer.current.selection.is_empty() {
                    // double click behavior
                    Region::BoundAt { bound: Bound::Word, location, backwards: true }
                } else {
                    // triple click behavior
                    Region::BoundAt { bound: Bound::Paragraph, location, backwards: true }
                }
            } else if response.clicked() && modifiers.shift {
                Region::ToLocation(location)
            } else if response.clicked() {
                // android native context menu: tapped selection
                if cfg!(target_os = "android") {
                    let offset = mutation::pos_to_char_offset(pos, &self.galleys);
                    if self.buffer.current.selection.contains(offset, true, true) {
                        ctx.set_context_menu(
                            self.context_menu_pos(self.buffer.current.selection)
                                .unwrap_or(pos),
                        );
                        return Vec::new();
                    }
                }

                Region::Location(location)
            } else if response.secondary_clicked() {
                ctx.set_context_menu(pos);
                return Vec::new();
            } else if response.drag_stopped() {
                if let Some(in_progress_selection) = mem::take(&mut self.in_progress_selection) {
                    Region::from(in_progress_selection)
                } else {
                    return Vec::new();
                }
            } else if response.dragged() && modifiers.shift {
                self.in_progress_selection =
                    Some(self.region_to_range(Region::ToLocation(location)));
                return Vec::new();
            } else if response.dragged() {
                let drag_origin = ctx.input(|i| i.pointer.press_origin()).unwrap_or_default();
                let region =
                    Region::BetweenLocations { start: Location::Pos(drag_origin), end: location };
                self.in_progress_selection = Some(self.region_to_range(region));
                return Vec::new();
            } else {
                // can't yet tell if drag
                return Vec::new();
            };

            if cfg!(target_os = "ios") {
                // iOS handles cursor placement using virtual keyboard FFI fn's
                return Vec::new();
            }

            ctx.memory_mut(|m| m.request_focus(self.id()));

            return vec![Event::Select { region }];
        }

        Vec::new()
    }

    fn context_menu_pos(&self, range: (DocCharOffset, DocCharOffset)) -> Option<Pos2> {
        // find the first line of the selection
        let lines = self.bounds.wrap_lines.find_intersecting(range, false);
        let first_line = lines.iter().next()?;
        let mut line = self.bounds.wrap_lines[first_line];
        if line.0 < range.start() {
            line.0 = range.start();
        }
        if line.1 > range.end() {
            line.1 = range.end();
        }

        // open the context menu in the center of the top of the rect containing the line
        let start_line = self.cursor_line(line.0);
        let end_line = self.cursor_line(line.1);
        Some(Pos2 { x: (start_line[1].x + end_line[1].x) / 2., y: start_line[0].y })
    }
}
