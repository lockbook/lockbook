use std::collections::HashMap;

use crate::tab::markdown_editor::appearance::{GRAY, YELLOW};
use crate::tab::markdown_editor::bounds::RangesExt;
use crate::tab::markdown_editor::images::ImageState;
use crate::tab::markdown_editor::input::{Event, Location, Region};
use crate::tab::markdown_editor::layouts::Annotation;
use crate::tab::markdown_editor::style::{
    BlockNode, InlineNode, ListItem, MarkdownNode, RenderStyle,
};
use crate::tab::markdown_editor::Editor;
use crate::tab::ExtendedInput;
use crate::theme::icons::Icon;
use egui::text::LayoutJob;
use egui::{
    Align2, Color32, CursorIcon, FontId, PlatformOutput, Pos2, Rect, Rounding, Sense, Stroke,
    TextFormat, TextStyle, TextWrapMode, Ui, Vec2, WidgetText,
};
use lb_rs::text::offset_types::RangeExt;
use pulldown_cmark::HeadingLevel;

use super::input::cursor;

impl Editor {
    pub fn draw_text(&self, ui: &mut Ui) {
        let bullet_radius = self.appearance.bullet_radius();
        let mut current_code_block = None;
        let mut code_block_copy_buttons = HashMap::new();
        let mut code_block_language_badges = HashMap::new();
        for galley_idx in 0..self.galleys.galleys.len() {
            let galley = &self.galleys.galleys[galley_idx];

            // draw annotations
            if let Some(annotation) = &galley.annotation {
                match annotation {
                    Annotation::Item(item_type, indent_level) => match item_type {
                        ListItem::Bulleted => {
                            let bullet_point = galley.bullet_center();
                            match indent_level {
                                0 => ui.painter().circle_filled(
                                    bullet_point,
                                    bullet_radius,
                                    self.appearance.text(),
                                ),
                                _ => ui.painter().circle_stroke(
                                    bullet_point,
                                    bullet_radius,
                                    Stroke::new(1.0, self.appearance.text()),
                                ),
                            };
                        }
                        ListItem::Numbered(num) => {
                            let mut job = LayoutJob::default();

                            let mut text_format = galley.annotation_text_format.clone();
                            let style =
                                RenderStyle::Markdown(MarkdownNode::Inline(InlineNode::Bold));
                            style.apply_style(&mut text_format, &self.appearance, ui.visuals());

                            job.append(&(num.to_string() + "."), 0.0, text_format);
                            let pos = galley.bullet_bounds(&self.appearance);

                            let galley = ui.ctx().fonts(|f| f.layout_job(job));
                            let rect = Align2::CENTER_TOP
                                .anchor_rect(Rect::from_min_size(pos.center(), galley.size()));
                            ui.painter().galley(rect.min, galley, Color32::TRANSPARENT);
                        }
                        ListItem::Todo(checked) => {
                            let bounds = galley.checkbox_bounds(&self.appearance);
                            let resp = ui.allocate_rect(bounds, Sense::click());

                            let hovered = resp.hovered();
                            let pointer_down = resp.is_pointer_button_down_on() || resp.clicked();
                            let bg_color = if hovered {
                                ui.output_mut(|o: &mut PlatformOutput| {
                                    o.cursor_icon = CursorIcon::PointingHand
                                });
                                ui.visuals().code_bg_color
                            } else {
                                Color32::TRANSPARENT
                            };
                            let icon_color = if pointer_down {
                                ui.visuals().widgets.active.bg_fill
                            } else if *checked {
                                ui.visuals().text_color()
                            } else {
                                Color32::TRANSPARENT
                            };

                            ui.painter().rect(
                                bounds,
                                self.appearance.checkbox_rounding(),
                                bg_color,
                                Stroke { width: 1., color: self.appearance.checkbox_bg() },
                            );

                            let icon = &Icon::CHECK.size(16.);
                            let icon_text: WidgetText = icon.into();
                            let galley = icon_text.into_galley(
                                ui,
                                Some(TextWrapMode::Extend),
                                ui.available_width(),
                                TextStyle::Body,
                            );
                            let draw_pos = resp.rect.center() - egui::Vec2::splat(icon.size) / 2.
                                + egui::vec2(0., 1.5);
                            ui.painter().galley(draw_pos, galley, icon_color);

                            if resp.clicked() {
                                ui.ctx()
                                    .push_markdown_event(Event::ToggleCheckbox(galley_idx));
                                ui.ctx().request_repaint();
                            }
                        }
                    },
                    Annotation::HeadingRule => {
                        let y = (galley.rect.max.y + galley.response.rect.max.y) / 2.;
                        let min = Pos2 { x: galley.rect.min.x, y };
                        let max = Pos2 { x: galley.rect.max.x, y };

                        ui.painter()
                            .line_segment([min, max], Stroke::new(0.3, self.appearance.rule()));
                    }
                    Annotation::Rule => {
                        let y = galley.rect.min.y + galley.rect.height() / 2.0;
                        let min = Pos2 { x: galley.rect.min.x, y };
                        let max = Pos2 { x: galley.rect.max.x, y };

                        ui.painter()
                            .line_segment([min, max], Stroke::new(0.3, self.appearance.rule()));
                    }
                    Annotation::Image(_, _, _) => {} // todo: draw image here
                    Annotation::BlockQuote => {
                        ui.painter().vline(
                            galley.rect.min.x - 15.,
                            galley.response.rect.y_range(),
                            Stroke { width: 3., color: self.appearance.checkbox_bg() },
                        );
                    }
                    Annotation::CodeBlock { text_range, language, captured, .. } => {
                        let code_block_galley_idx = if let Some((
                            current_code_block_range,
                            current_code_block_galley_idx,
                        )) = current_code_block.take()
                        {
                            if text_range == current_code_block_range {
                                // extend existing code block
                                current_code_block_galley_idx
                            } else {
                                // create a new code block: bordering the previous
                                galley_idx
                            }
                        } else {
                            // create a new code block: standalone
                            galley_idx
                        };

                        // language badge
                        if *captured
                            && !language.is_empty()
                            && !code_block_language_badges.contains_key(text_range)
                        {
                            let code_block_galley = &self.galleys.galleys[code_block_galley_idx];
                            let top_left =
                                self.galleys.galleys[code_block_galley_idx].rect.left_top()
                                    - egui::vec2(15., 15. - 10. / 2.);
                            let padding = 10.;
                            let badge_height = code_block_galley.cursor_height() * 2.;
                            let top_left = top_left + egui::vec2(padding, padding);
                            let badge_rect =
                                Rect::from_min_size(top_left, egui::vec2(0., badge_height));
                            let badge_response = ui.allocate_rect(
                                badge_rect,
                                Sense { click: true, drag: false, focusable: false },
                            );
                            code_block_language_badges
                                .insert(*text_range, (badge_response, language));
                        }

                        // copy button
                        if !code_block_copy_buttons.contains_key(text_range) {
                            let code_block_galley = &self.galleys.galleys[code_block_galley_idx];
                            let mut top_right =
                                self.galleys.galleys[code_block_galley_idx].rect.right_top()
                                    + egui::vec2(15., 0.);
                            if !language.is_empty() {
                                top_right.y -= 10.;
                            }
                            let padding = 5.;
                            let button_height = (code_block_galley.cursor_height() - padding) * 2.;
                            let top_left =
                                top_right + egui::vec2(-button_height - padding, padding);
                            let button_rect = Rect::from_min_size(
                                top_left,
                                egui::vec2(button_height, button_height),
                            );
                            let copy_button_response = ui.allocate_rect(
                                button_rect,
                                Sense { click: true, drag: false, focusable: false },
                            );

                            let galley_text_hovered = galley
                                .response
                                .hover_pos()
                                .map(|pos| galley.response.rect.contains(pos))
                                .unwrap_or_default();
                            let show_code_block_button = galley_text_hovered
                                || copy_button_response.hovered()
                                || cfg!(target_os = "ios")
                                || cfg!(target_os = "android");
                            if show_code_block_button {
                                code_block_copy_buttons.insert(*text_range, copy_button_response);
                            }
                        }

                        // when extending, this covers the smaller already-drawn portion of the code block
                        let mut top_left =
                            self.galleys.galleys[code_block_galley_idx].rect.left_top();
                        if !language.is_empty() {
                            top_left.y -= 10.;
                        }
                        ui.painter().rect(
                            Rect { min: top_left, max: galley.rect.max }
                                .expand2(egui::vec2(15. - 1., 0. - 1.)),
                            2.,
                            ui.style().visuals.code_bg_color,
                            Stroke::NONE,
                        );

                        current_code_block = Some((text_range, code_block_galley_idx));
                    }
                }
            }

            // draw images
            if let Some(image) = &galley.image {
                match &image.image_state {
                    ImageState::Loading => {
                        let photo = "\u{e410}";
                        self.draw_image_placeholder(ui, image.location, photo, "Loading image...");
                    }
                    ImageState::Loaded(texture_id) => {
                        let uv =
                            Rect { min: Pos2 { x: 0.0, y: 0.0 }, max: Pos2 { x: 1.0, y: 1.0 } };
                        ui.painter()
                            .image(*texture_id, image.location, uv, Color32::WHITE);
                    }
                    ImageState::Failed(_) => {
                        let image_not_supported = "\u{f116}";
                        self.draw_image_placeholder(
                            ui,
                            image.location,
                            image_not_supported,
                            "Failed to load image.",
                        );
                    }
                }
            }
        }

        // draw text
        for galley in &self.galleys.galleys {
            ui.painter()
                .galley(galley.text_location, galley.galley.clone(), Color32::TRANSPARENT);
        }

        // draw code block language badges
        for (.., (badge_response, language)) in code_block_language_badges {
            let mut job = LayoutJob::default();
            let mut text_format = TextFormat::default();
            RenderStyle::Syntax.apply_style(&mut text_format, &self.appearance, ui.visuals());
            job.append(language, 0., text_format);
            let pos = badge_response.rect.left_top();

            let galley = ui.ctx().fonts(|f| f.layout_job(job));
            let space = 5.;
            let rect = Align2::LEFT_TOP
                .anchor_rect(Rect::from_min_size(pos, galley.size() + egui::vec2(space * 2., 0.)));
            ui.painter()
                .galley(rect.min + egui::vec2(space, 0.), galley, Color32::TRANSPARENT);
            ui.painter().rect(
                rect,
                2.,
                Color32::TRANSPARENT,
                Stroke::new(1., self.appearance.syntax()),
            );
        }

        // draw code block copy buttons
        for (text_range, response) in code_block_copy_buttons {
            if response.hovered() {
                ui.painter().rect(
                    response.rect,
                    2.,
                    ui.style().visuals.extreme_bg_color,
                    Stroke::NONE,
                );
                ui.output_mut(|o: &mut PlatformOutput| o.cursor_icon = CursorIcon::PointingHand);
            }
            if response.clicked() {
                ui.output_mut(|o| o.copied_text = self.buffer[text_range].to_string())
            }

            let x_icon = Icon::CONTENT_COPY.size(16.0);
            let icon_draw_pos = egui::pos2(
                response.rect.center().x - x_icon.size / 2.,
                response.rect.center().y - x_icon.size / 2.2,
            );
            let icon: WidgetText = (&x_icon).into();
            let icon = icon.into_galley(
                ui,
                Some(TextWrapMode::Extend),
                response.rect.width(),
                TextStyle::Body,
            );
            let icon_color = if response.is_pointer_button_down_on() {
                ui.visuals().widgets.active.bg_fill
            } else {
                ui.visuals().text_color()
            };
            ui.painter().galley(icon_draw_pos, icon, icon_color);
        }
    }

    pub fn draw_image_placeholder(
        &self, ui: &mut Ui, location: Rect, icon: &'static str, caption: &'static str,
    ) {
        ui.painter().text(
            location.center(),
            Align2::CENTER_CENTER,
            icon,
            FontId { size: 48.0, family: egui::FontFamily::Monospace },
            GRAY.get(self.appearance.current_theme),
        );
        ui.painter().text(
            location.center_bottom() + Vec2 { x: 0.0, y: -50.0 },
            Align2::CENTER_BOTTOM,
            caption,
            FontId::default(),
            GRAY.get(self.appearance.current_theme),
        );
    }

    pub fn draw_cursor(&self, ui: &mut Ui, touch_mode: bool) {
        // determine cursor style
        let selection_start_line = cursor::line(
            self.buffer.current.selection.0,
            &self.galleys,
            &self.bounds.text,
            &self.appearance,
        );
        let selection_end_line = cursor::line(
            self.buffer.current.selection.1,
            &self.galleys,
            &self.bounds.text,
            &self.appearance,
        );

        let color = if touch_mode { self.appearance.cursor() } else { self.appearance.text() };
        let stroke = Stroke { width: 1.0, color };

        let (selection_end_line, stroke) = if self.buffer.current.selection.is_empty() {
            let mut selection_end_line = selection_end_line;
            let mut stroke = stroke;

            for style in self
                .ast
                .styles_at_offset(self.buffer.current.selection.1, &self.bounds.ast)
            {
                match style {
                    MarkdownNode::Inline(InlineNode::Bold)
                    | MarkdownNode::Block(BlockNode::Heading(HeadingLevel::H1)) => {
                        stroke.width = 2.0;
                    }
                    MarkdownNode::Inline(InlineNode::Italic) => {
                        if !touch_mode {
                            // iOS draws its own cursor based on a rectangle we return
                            // a slanted line cannot be represented as a rectangle
                            // todo: don't double-draw cursor
                            selection_end_line[0].x += 4.0;
                        }
                    }
                    MarkdownNode::Inline(InlineNode::Code)
                    | MarkdownNode::Block(BlockNode::Code(..)) => {
                        stroke.color = self.appearance.code();
                    }
                    MarkdownNode::Inline(InlineNode::Link(..))
                    | MarkdownNode::Inline(InlineNode::Image(..)) => {
                        stroke.color = self.appearance.link();
                    }
                    _ => {}
                }
            }

            if !self
                .bounds
                .links
                .find_containing(self.buffer.current.selection.1, true, true)
                .is_empty()
            {
                stroke.color = self.appearance.link();
            }

            (selection_end_line, stroke)
        } else {
            (selection_end_line, stroke)
        };

        // draw cursor for selection end
        ui.painter().line_segment(selection_end_line, stroke);

        if touch_mode {
            // draw cursor for selection start
            ui.painter().line_segment(selection_start_line, stroke);

            // draw selection handles
            // handles invisible but still draggable when selection is empty
            // we must allocate handles to check if they were dragged last frame
            if !self.buffer.current.selection.is_empty() {
                let selection_start_center =
                    Pos2 { x: selection_start_line[0].x, y: selection_start_line[0].y - 5.0 };
                ui.painter()
                    .circle_filled(selection_start_center, 5.0, color);
                let selection_end_center =
                    Pos2 { x: selection_end_line[1].x, y: selection_end_line[1].y + 5.0 };
                ui.painter().circle_filled(selection_end_center, 5.0, color);
            }

            // allocate rects to capture selection handle drag
            let selection_start_handle_rect = Rect {
                min: Pos2 {
                    x: selection_start_line[0].x - 5.0,
                    y: selection_start_line[0].y - 10.0,
                },
                max: Pos2 { x: selection_start_line[0].x + 5.0, y: selection_start_line[0].y },
            };
            let start_response = ui.allocate_rect(selection_start_handle_rect, Sense::drag());
            let selection_end_handle_rect = Rect {
                min: Pos2 { x: selection_end_line[1].x - 5.0, y: selection_end_line[1].y },
                max: Pos2 { x: selection_end_line[1].x + 5.0, y: selection_end_line[1].y + 10.0 },
            };
            let end_response = ui.allocate_rect(selection_end_handle_rect, Sense::drag());

            // adjust cursor based on selection handle drag
            if start_response.dragged() {
                ui.ctx().push_markdown_event(Event::Select {
                    region: Region::BetweenLocations {
                        start: Location::Pos(ui.input(|i| {
                            i.pointer.interact_pos().unwrap_or_default() + Vec2 { x: 0.0, y: 10.0 }
                        })),
                        end: Location::DocCharOffset(self.buffer.current.selection.1),
                    },
                });
            }
            if end_response.dragged() {
                ui.ctx().push_markdown_event(Event::Select {
                    region: Region::BetweenLocations {
                        start: Location::DocCharOffset(self.buffer.current.selection.0),
                        end: Location::Pos(ui.input(|i| {
                            i.pointer.interact_pos().unwrap_or_default() - Vec2 { x: 0.0, y: 10.0 }
                        })),
                    },
                });
            }
        }
    }

    pub fn draw_debug(&self, ui: &mut Ui) {
        for galley in &self.galleys.galleys {
            let galley_rect = galley.galley.rect.translate(galley.text_location.to_vec2());
            ui.painter().rect(
                galley_rect,
                Rounding::ZERO,
                Color32::TRANSPARENT,
                Stroke { width: 0.5, color: YELLOW.light },
            );
            let line_pt_1 =
                Pos2::new(galley_rect.max.x, (galley_rect.max.y + galley_rect.min.y) / 2.0);
            let line_pt_2 =
                Pos2::new(galley_rect.max.x + 40.0, (galley_rect.max.y + galley_rect.min.y) / 2.0);
            ui.painter()
                .line_segment([line_pt_1, line_pt_2], Stroke { width: 0.5, color: YELLOW.light });

            ui.painter().text(
                line_pt_2,
                Align2::LEFT_CENTER,
                format!("{:?}-{:?}", galley.range.start(), galley.range.end()),
                FontId::default(),
                YELLOW.light,
            );
        }

        let screen_size = format!(
            "screen: {} x {}",
            ui.ctx().input(|i| i.screen_rect.width()),
            ui.ctx().input(|i| i.screen_rect.height())
        );

        let doc_info =
            format!("last_cursor_position: {:?}", self.buffer.current.segs.last_cursor_position());

        let cursor_info = format!(
            "selection: ({:?}, {:?}), byte: {:?}, x_target: {}",
            self.buffer.current.selection.0,
            self.buffer.current.selection.1,
            self.buffer
                .current
                .segs
                .offset_to_byte(self.buffer.current.selection.1),
            self.cursor
                .x_target
                .map(|x| x.to_string())
                .unwrap_or_else(|| "None".to_string()),
        );

        let frames = format!("frame #{}, {}ms", self.debug.frame_count, self.debug.ms_elapsed());

        let output = format!("{}\n{}\n{}\n{}", doc_info, cursor_info, screen_size, frames);

        let loc = ui.input(|i| i.screen_rect.max);
        ui.painter()
            .text(loc, Align2::RIGHT_BOTTOM, output, FontId::default(), YELLOW.light);
    }
}
