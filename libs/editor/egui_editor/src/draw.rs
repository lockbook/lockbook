use crate::appearance::YELLOW;
use crate::element::{Element, ItemType};
use crate::layouts::Annotation;
use crate::Editor;
use egui::text::LayoutJob;
use egui::{Align2, Color32, FontId, Pos2, Rect, Rounding, Sense, Stroke, Ui, Vec2};

impl Editor {
    pub fn draw_text(&mut self, mut ui_size: Vec2, ui: &mut Ui) {
        let bullet_radius = self.appearance.bullet_radius();
        for galley in &self.galleys.galleys {
            // draw annotations
            if let Some(annotation) = &galley.annotation {
                match annotation {
                    Annotation::Item(item_type, indent_level) => match item_type {
                        ItemType::Bulleted => {
                            let bullet_point = galley.bullet_center();
                            match indent_level {
                                1 => ui.painter().circle_filled(
                                    bullet_point,
                                    bullet_radius,
                                    self.appearance.text(),
                                ),
                                _ => ui.painter().circle_stroke(
                                    bullet_point,
                                    bullet_radius,
                                    Stroke::new(1.0, self.appearance.text()),
                                ),
                            }
                        }
                        ItemType::Numbered(num) => {
                            let mut job = LayoutJob::default();

                            let mut text_format = galley.annotation_text_format.clone();
                            Element::Strong.apply_style(&mut text_format, &self.appearance);

                            job.append(&(num.to_string() + "."), 0.0, text_format);
                            let pos = galley.bullet_bounds(&self.appearance);

                            let galley = ui.ctx().fonts().layout_job(job);
                            let rect = Align2::RIGHT_TOP
                                .anchor_rect(Rect::from_min_size(pos.max, galley.size()));
                            ui.painter().galley(rect.min, galley);
                        }
                        ItemType::Todo(checked) => {
                            ui.painter().rect_filled(
                                galley.checkbox_bounds(&self.appearance),
                                self.appearance.checkbox_rounding(),
                                self.appearance.checkbox_bg(),
                            );
                            if *checked {
                                ui.painter().line_segment(
                                    galley.checkbox_slash(&self.appearance),
                                    Stroke {
                                        width: self.appearance.checkbox_slash_width(),
                                        color: self.appearance.text(),
                                    },
                                );
                            }
                        }
                    },
                    Annotation::Rule => {
                        let mut max = galley.galley_location.max;
                        max.y -= 7.0;

                        let mut min = galley.galley_location.max;
                        min.y -= 7.0;
                        min.x = galley.galley_location.min.x;

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
                let uv = Rect { min: Pos2 { x: 0.0, y: 0.0 }, max: Pos2 { x: 1.0, y: 1.0 } };
                ui.painter().image(
                    image.texture,
                    image.image_bounds(&self.appearance, ui),
                    uv,
                    Color32::WHITE,
                );
            }

            // draw text
            ui.painter()
                .galley(galley.text_location, galley.galley.clone());
        }

        // draw end-of-text padding
        ui_size.y -= self.galleys.galleys[self.galleys.len() - 1]
            .galley
            .rect
            .size()
            .y;
        let (padding_rect, _) = ui.allocate_exact_size(ui_size, Sense::click_and_drag());
        ui.painter()
            .rect(padding_rect, Rounding::none(), Color32::TRANSPARENT, Stroke::NONE);
    }

    pub fn draw_cursor(&mut self, ui: &mut Ui) {
        ui.painter().rect(
            self.buffer
                .current
                .cursor
                .rect(&self.buffer.current.segs, &self.galleys),
            Rounding::none(),
            Color32::TRANSPARENT,
            Stroke { width: 1.0, color: self.appearance.text() },
        );
    }

    pub fn draw_debug(&mut self, ui: &mut Ui) {
        for galley in &self.galleys.galleys {
            let galley_rect = galley.galley.rect.translate(galley.text_location.to_vec2());
            ui.painter().rect(
                galley_rect,
                Rounding::none(),
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
                format!("{}-{}", galley.range.start.0, galley.range.end.0),
                FontId::default(),
                YELLOW.light,
            );
        }

        let screen_size = format!(
            "screen: {} x {}",
            ui.ctx().input().screen_rect.width(),
            ui.ctx().input().screen_rect.height()
        );

        let doc_info =
            format!("last_cursor_position: {}", self.buffer.current.segs.last_cursor_position().0);

        let cursor_info = format!(
            "character: {}, byte: {}, x_target: {}, selection_origin: {}",
            self.buffer.current.cursor.pos.0,
            self.buffer
                .current
                .segs
                .char_offset_to_byte(self.buffer.current.cursor.pos)
                .0,
            self.buffer
                .current
                .cursor
                .x_target
                .map(|x| x.to_string())
                .unwrap_or_else(|| "None".to_string()),
            self.buffer
                .current
                .cursor
                .selection_origin
                .map(|x| x.0.to_string())
                .unwrap_or_else(|| "None".to_string()),
        );

        let frames = format!("frame #{}, {}ms", self.debug.frame_count, self.debug.ms_elapsed());

        let output = format!("{}\n{}\n{}\n{}", doc_info, cursor_info, screen_size, frames);

        let loc = ui.input().screen_rect.max;
        ui.painter()
            .text(loc, Align2::RIGHT_BOTTOM, output, FontId::default(), YELLOW.light);
    }
}
