use crate::tab::markdown_editor::appearance::{GRAY, YELLOW};
use crate::tab::markdown_editor::bounds::RangesExt;
use crate::tab::markdown_editor::images::ImageState;
use crate::tab::markdown_editor::input::canonical::{Location, Modification, Region};
use crate::tab::markdown_editor::layouts::Annotation;
use crate::tab::markdown_editor::offset_types::RangeExt;
use crate::tab::markdown_editor::style::{
    BlockNode, InlineNode, ListItem, MarkdownNode, RenderStyle,
};
use crate::tab::markdown_editor::Editor;
use crate::tab::EventManager;
use egui::text::LayoutJob;
use egui::{Align2, Color32, FontId, Pos2, Rect, Rounding, Sense, Stroke, Ui, Vec2};
use pulldown_cmark::HeadingLevel;

impl Editor {
    pub fn draw_text(&mut self, mut ui_size: Vec2, ui: &mut Ui, touch_mode: bool) {
        let bullet_radius = self.appearance.bullet_radius();
        for galley in &self.galleys.galleys {
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
                            style.apply_style(&mut text_format, &self.appearance);

                            job.append(&(num.to_string() + "."), 0.0, text_format);
                            let pos = galley.bullet_bounds(&self.appearance);

                            let galley = ui.ctx().fonts(|f| f.layout_job(job));
                            let rect = Align2::RIGHT_TOP
                                .anchor_rect(Rect::from_min_size(pos.max, galley.size()));
                            ui.painter().galley(rect.min, galley, Color32::TRANSPARENT);
                        }
                        ListItem::Todo(checked) => {
                            ui.painter().rect_filled(
                                galley.checkbox_bounds(touch_mode, &self.appearance),
                                self.appearance.checkbox_rounding(),
                                self.appearance.checkbox_bg(),
                            );
                            if *checked {
                                ui.painter().line_segment(
                                    galley.checkbox_slash(touch_mode, &self.appearance),
                                    Stroke {
                                        width: self.appearance.checkbox_slash_width(),
                                        color: self.appearance.text(),
                                    },
                                );
                            }
                        }
                    },
                    Annotation::HeadingRule => {
                        let y = galley.galley_location.max.y - 7.0;
                        let min = Pos2 { x: galley.galley_location.min.x, y };
                        let max = Pos2 { x: galley.galley_location.max.x, y };

                        ui.painter().line_segment(
                            [min, max],
                            Stroke::new(0.3, self.appearance.heading_line()),
                        );
                    }
                    Annotation::Rule => {
                        let y =
                            galley.galley_location.min.y + galley.galley_location.height() / 2.0;
                        let min = Pos2 { x: galley.galley_location.min.x, y };
                        let max = Pos2 { x: galley.galley_location.max.x, y };

                        ui.painter().line_segment(
                            [min, max],
                            Stroke::new(0.3, self.appearance.heading_line()),
                        );
                    }
                    _ => {}
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

            // draw text
            ui.painter()
                .galley(galley.text_location, galley.galley.clone(), Color32::TRANSPARENT);
        }

        // draw end-of-text padding
        ui_size.y -= self.galleys.galleys[self.galleys.len() - 1]
            .galley
            .rect
            .size()
            .y;
        ui.allocate_exact_size(ui_size, Sense::hover());
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

    pub fn draw_cursor(&mut self, ui: &mut Ui, touch_mode: bool) {
        // determine cursor style
        let cursor = self.buffer.current.cursor;
        let selection_start_line =
            cursor.start_line(&self.galleys, &self.bounds.text, &self.appearance);
        let selection_end_line =
            cursor.end_line(&self.galleys, &self.bounds.text, &self.appearance);

        let color = if touch_mode { self.appearance.cursor() } else { self.appearance.text() };
        let stroke = Stroke { width: 1.0, color };

        let (selection_end_line, stroke) = if cursor.selection.is_empty() {
            let mut selection_end_line = selection_end_line;
            let mut stroke = stroke;

            for style in self
                .ast
                .styles_at_offset(cursor.selection.1, &self.bounds.ast)
            {
                match style {
                    MarkdownNode::Inline(InlineNode::Bold)
                    | MarkdownNode::Block(BlockNode::Heading(HeadingLevel::H1)) => {
                        stroke.width = 2.0;
                    }
                    MarkdownNode::Inline(InlineNode::Italic)
                    | MarkdownNode::Block(BlockNode::Quote) => {
                        if !touch_mode {
                            // iOS draws its own cursor based on a rectangle we return
                            // a slanted line cannot be represented as a rectangle
                            // todo: don't double-draw cursor
                            selection_end_line[0].x += 5.0;
                        }
                    }
                    MarkdownNode::Inline(InlineNode::Code)
                    | MarkdownNode::Block(BlockNode::Code) => {
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
                .find_containing(cursor.selection.1, true, true)
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
            if !cursor.selection.is_empty() {
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
                ui.ctx().push_markdown_event(Modification::Select {
                    region: Region::BetweenLocations {
                        start: Location::Pos(ui.input(|i| {
                            i.pointer.interact_pos().unwrap_or_default() + Vec2 { x: 0.0, y: 10.0 }
                        })),
                        end: Location::DocCharOffset(cursor.selection.1),
                    },
                });
            }
            if end_response.dragged() {
                ui.ctx().push_markdown_event(Modification::Select {
                    region: Region::BetweenLocations {
                        start: Location::DocCharOffset(cursor.selection.0),
                        end: Location::Pos(ui.input(|i| {
                            i.pointer.interact_pos().unwrap_or_default() - Vec2 { x: 0.0, y: 10.0 }
                        })),
                    },
                });
            }
        }
    }

    pub fn draw_debug(&mut self, ui: &mut Ui) {
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
            self.buffer.current.cursor.selection.0,
            self.buffer.current.cursor.selection.1,
            self.buffer
                .current
                .segs
                .offset_to_byte(self.buffer.current.cursor.selection.1),
            self.buffer
                .current
                .cursor
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
