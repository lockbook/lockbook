use crate::tab::markdown_editor::bounds::{BoundCase, BoundExt as _, RangesExt as _};
use crate::tab::markdown_editor::input::Location;
use crate::tab::{self, markdown_editor, ClipContent, ExtendedInput as _, ExtendedOutput as _};
use egui::{
    Button, Context, EventFilter, FontFamily, FontId, Pos2, Stroke, TextStyle, TextWrapMode,
};
use lb_rs::text::buffer;
use lb_rs::text::offset_types::{DocCharOffset, RangeExt as _, RangeIterExt as _};
use markdown_editor::input::{Event, Region};
use markdown_editor::Editor;

use super::canonical::translate_egui_keyboard_event;
use super::mutation::pos_to_char_offset;
use super::{cursor, mutation, Bound};

impl Editor {
    pub fn process_events(&mut self, ctx: &Context) -> bool {
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
                crate::Event::PredictedTouch { .. } => {}
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
            .filter_map(translate_egui_keyboard_event)
            .collect::<Vec<_>>()
        } else {
            Vec::new()
        }
    }

    fn get_pointer_events(&mut self, ctx: &Context) -> Vec<Event> {
        for i in 0..self.galleys.galleys.len() {
            let galley = &self.galleys.galleys[i];
            if let Some(response) = ctx.read_response(galley.response.id) {
                let modifiers = ctx.input(|i| i.modifiers);

                ctx.style_mut(|s| s.spacing.menu_margin = egui::vec2(10., 5.).into());
                ctx.style_mut(|s| s.visuals.menu_rounding = (2.).into());
                ctx.style_mut(|s| s.visuals.window_fill = s.visuals.extreme_bg_color);
                ctx.style_mut(|s| s.visuals.window_stroke = Stroke::NONE);
                ctx.style_mut(|s| {
                    s.text_styles
                        .insert(TextStyle::Button, FontId::new(16.0, FontFamily::Proportional));
                });

                if !cfg!(target_os = "ios") && !cfg!(target_os = "android") {
                    let mut context_menu_events = Vec::new();
                    response.context_menu(|ui| {
                        let offset = pos_to_char_offset(
                            self.event.prev_click_pos,
                            &self.galleys,
                            &self.buffer.current.segs,
                            &self.bounds.text,
                        );
                        if let Some(spelling_error_range) = self
                            .spelling
                            .errors
                            .iter()
                            .find(|error| error.contains(offset, true, true))
                        {
                            let spelling_error_range = *spelling_error_range;
                            let spelling_error_text = &self.buffer[spelling_error_range];

                            let max_suggestions = 5;
                            let mut suggestions = Vec::new();
                            self.spelling
                                .dict
                                .suggest(spelling_error_text, &mut suggestions);
                            for (i, suggestion) in suggestions.into_iter().enumerate() {
                                if i >= max_suggestions {
                                    break;
                                }
                                let msg = format!(
                                    "Replace ‘{}’ with ‘{}’",
                                    spelling_error_text, suggestion
                                );

                                if ui
                                    .add(Button::new(msg).wrap_mode(TextWrapMode::Extend))
                                    .clicked()
                                {
                                    context_menu_events.push(Event::Replace {
                                        region: Region::BetweenLocations {
                                            start: Location::DocCharOffset(
                                                spelling_error_range.start(),
                                            ),
                                            end: Location::DocCharOffset(
                                                spelling_error_range.end(),
                                            ),
                                        },
                                        text: suggestion,
                                    });
                                    ui.close_menu();
                                }
                            }
                        }
                    });
                    if !context_menu_events.is_empty() {
                        return context_menu_events;
                    }
                }

                // hover-based cursor icons
                let hovering_clickable = ctx
                    .input(|r| r.pointer.latest_pos())
                    .map(|pos| {
                        modifiers.command
                            && mutation::pos_to_link(
                                pos,
                                &self.galleys,
                                &self.buffer,
                                &self.bounds,
                                &self.ast,
                            )
                            .is_some()
                    })
                    .unwrap_or_default();
                if response.hovered() && hovering_clickable {
                    ctx.output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
                } else if response.hovered() {
                    ctx.output_mut(|o| o.cursor_icon = egui::CursorIcon::Text);
                }

                // note: early continue here unless response has a pointer interaction
                let pos =
                    if let Some(pos) = response.interact_pointer_pos() { pos } else { continue };
                self.event.prev_click_pos = pos;
                let location = Location::Pos(pos);

                let maybe_clicked_link = if modifiers.command
                    || cfg!(target_os = "ios")
                    || cfg!(target_os = "android")
                {
                    mutation::pos_to_link(pos, &self.galleys, &self.buffer, &self.bounds, &self.ast)
                } else {
                    None
                };

                // note: deliberate order; a double click is also a click
                let region = if response.clicked() && maybe_clicked_link.is_some() {
                    let url = maybe_clicked_link.unwrap();
                    let url = if !url.contains("://") { format!("https://{}", url) } else { url };
                    ctx.output_mut(|o| o.open_url = Some(egui::output::OpenUrl::new_tab(url)));
                    continue;
                } else if response.triple_clicked() {
                    Region::BoundAt { bound: Bound::Paragraph, location, backwards: true }
                } else if response.double_clicked() {
                    // android native context menu: double-tapped
                    if cfg!(target_os = "android") {
                        // context menu position based on text range of word that will be selected
                        let offset = self.location_to_char_offset(location);
                        let range = offset
                            .range_bound(Bound::Word, true, true, &self.bounds)
                            .unwrap_or((offset, offset));
                        ctx.set_context_menu(self.context_menu_pos(range).unwrap_or(pos));
                    }

                    Region::BoundAt { bound: Bound::Word, location, backwards: true }
                } else if response.clicked() && modifiers.shift {
                    Region::ToLocation(location)
                } else if response.clicked() {
                    // android native context menu: tapped selection
                    if cfg!(target_os = "android") {
                        let offset = pos_to_char_offset(
                            pos,
                            &self.galleys,
                            &self.buffer.current.segs,
                            &self.bounds.text,
                        );
                        if self.buffer.current.selection.contains(offset, true, true) {
                            ctx.set_context_menu(
                                self.context_menu_pos(self.buffer.current.selection)
                                    .unwrap_or(pos),
                            );
                            continue;
                        }
                    }

                    Region::Location(location)
                } else if response.secondary_clicked() {
                    ctx.set_context_menu(pos);
                    continue;
                } else if response.dragged() && modifiers.shift {
                    Region::ToLocation(location)
                } else if response.dragged() {
                    if response.drag_started() {
                        Region::Location(location)
                    } else {
                        Region::ToLocation(location)
                    }
                } else {
                    // can't yet tell if drag
                    continue;
                };

                if cfg!(target_os = "ios") {
                    // iOS handles cursor placement using virtual keyboard FFI fn's
                    continue;
                }

                return vec![Event::Select { region }];
            }
        }
        Vec::new()
    }

    fn context_menu_pos(&self, range: (DocCharOffset, DocCharOffset)) -> Option<Pos2> {
        // find the first line of the selection
        let lines = self.bounds.lines.find_intersecting(range, false);
        let first_line = lines.iter().next()?;
        let mut line = self.bounds.lines[first_line];
        if line.0 < range.start() {
            line.0 = range.start();
        }
        if line.1 > range.end() {
            line.1 = range.end();
        }

        // open the context menu in the center of the top of the rect containing the line
        let start_line = cursor::line(line.0, &self.galleys, &self.bounds.text, &self.appearance);
        let end_line = cursor::line(line.1, &self.galleys, &self.bounds.text, &self.appearance);
        Some(Pos2 { x: (start_line[1].x + end_line[1].x) / 2., y: start_line[0].y })
    }
}
