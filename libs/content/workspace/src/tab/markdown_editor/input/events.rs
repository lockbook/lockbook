use crate::tab::markdown_editor::bounds::{BoundCase, BoundExt as _};
use crate::tab::markdown_editor::input::Location;
use crate::tab::markdown_editor::layouts::Annotation;
use crate::tab::markdown_editor::style::ListItem;
use crate::tab::{self, markdown_editor, ClipContent, ExtendedInput as _};
use egui::{Context, EventFilter};
use lb_rs::text::buffer;
use lb_rs::text::offset_types::RangeExt as _;
use markdown_editor::input::{Event, Region};
use markdown_editor::Editor;

use super::canonical::translate_egui_keyboard_event;
use super::{mutation, Bound};

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

    fn get_pointer_events(&self, ctx: &Context) -> Vec<Event> {
        for i in 0..self.galleys.galleys.len() {
            let galley = &self.galleys.galleys[i];
            if let Some(response) = ctx.read_response(galley.response.id) {
                let modifiers = ctx.input(|i| i.modifiers);

                // hover-based cursor icons
                let hovering_clickable = ctx
                    .input(|r| r.pointer.latest_pos())
                    .map(|pos| {
                        if modifiers.command
                            && mutation::pos_to_link(
                                pos,
                                &self.galleys,
                                &self.buffer,
                                &self.bounds,
                                &self.ast,
                            )
                            .is_some()
                        {
                            return true;
                        }
                        if let Some(Annotation::Item(ListItem::Todo(_), ..)) = galley.annotation {
                            if galley.checkbox_bounds(&self.appearance).contains(pos) {
                                return true;
                            }
                        }
                        false
                    })
                    .unwrap_or_default();
                if response.hovered() && hovering_clickable {
                    ctx.output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
                } else if response.hovered() {
                    ctx.output_mut(|o| o.cursor_icon = egui::CursorIcon::Text);
                }

                let pos =
                    if let Some(pos) = response.interact_pointer_pos() { pos } else { continue };
                let location = Location::Pos(pos);

                let maybe_clicked_link = if modifiers.command
                    || cfg!(target_os = "ios")
                    || cfg!(target_os = "android")
                {
                    mutation::pos_to_link(pos, &self.galleys, &self.buffer, &self.bounds, &self.ast)
                } else {
                    None
                };
                let clicked_checkbox =
                    if let Some(Annotation::Item(ListItem::Todo(_), ..)) = galley.annotation {
                        let mut checkbox_bounds = galley.checkbox_bounds(&self.appearance);
                        if cfg!(target_os = "ios") || cfg!(target_os = "android") {
                            checkbox_bounds = checkbox_bounds.expand(16.);
                        }
                        checkbox_bounds.contains(pos)
                    } else {
                        false
                    };

                // note: deliberate order; a double click is also a click
                let region = if response.clicked() && maybe_clicked_link.is_some() {
                    let url = maybe_clicked_link.unwrap();
                    let url = if !url.contains("://") { format!("https://{}", url) } else { url };
                    ctx.output_mut(|o| o.open_url = Some(egui::output::OpenUrl::new_tab(url)));
                    continue;
                } else if response.clicked() && clicked_checkbox {
                    return vec![Event::ToggleCheckbox(i)];
                } else if response.triple_clicked() {
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
                    return Vec::new();
                }

                return vec![Event::Select { region }];
            }
        }
        Vec::new()
    }
}
