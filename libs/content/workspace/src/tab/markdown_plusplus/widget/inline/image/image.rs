use std::{f32, ops::Deref as _};

use comrak::nodes::AstNode;
use egui::{self, Align2, Color32, FontId, Pos2, Rect, Stroke, TextFormat, Ui, Vec2};
use epaint::RectShape;

use crate::tab::markdown_plusplus::{widget::WrapContext, MarkdownPlusPlus};

use super::cache::ImageState;

impl<'ast> MarkdownPlusPlus {
    pub fn text_format_image(&self, parent: &AstNode<'_>) -> TextFormat {
        self.text_format_link(parent)
    }

    pub fn span_image(&self, node: &'ast AstNode<'ast>, wrap: &WrapContext) -> f32 {
        self.circumfix_span(node, wrap)
    }

    pub fn show_image(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2, wrap: &mut WrapContext,
    ) {
        self.show_circumfix(ui, node, top_left, wrap);
    }

    pub fn height_image(&self, width: f32, url: &str) -> f32 {
        if let Some(image_state) = self.images.map.get(url) {
            let image_state = image_state.lock().unwrap().deref().clone();
            match image_state {
                ImageState::Loading => self.image_size(Vec2::splat(200.), width).y,
                ImageState::Loaded(texture_id) => {
                    let [image_width, image_height] =
                        self.ctx.tex_manager().read().meta(texture_id).unwrap().size;
                    let size =
                        self.image_size(Vec2::new(image_width as _, image_height as _), width);

                    size.y
                }
                ImageState::Failed(_) => self.image_size(Vec2::splat(200.), width).y,
            }
        } else {
            0.
        }
    }

    pub fn show_image_block(&mut self, ui: &mut Ui, top_left: Pos2, width: f32, url: &str) {
        if let Some(image_state) = self.images.map.get(url) {
            let image_state = image_state.lock().unwrap().deref().clone();
            match image_state {
                ImageState::Loading => {
                    let icon = "\u{e410}";
                    let caption = "Loading image...";

                    let size = self.image_size(Vec2::splat(200.), width);
                    let rect = Rect::from_min_size(top_left, Vec2::new(width, size.y));

                    ui.allocate_ui_at_rect(rect, |ui| {
                        let rect = ui.max_rect();
                        ui.painter().text(
                            rect.center(),
                            Align2::CENTER_CENTER,
                            icon,
                            FontId { size: 48.0, family: egui::FontFamily::Monospace },
                            self.theme.fg().neutral_tertiary,
                        );
                        ui.painter().text(
                            rect.center_bottom() + Vec2 { x: 0.0, y: -50.0 },
                            Align2::CENTER_BOTTOM,
                            caption,
                            FontId::default(),
                            self.theme.fg().neutral_tertiary,
                        );
                        ui.painter().rect_stroke(
                            rect,
                            2.,
                            Stroke { width: 1., color: self.theme.bg().neutral_tertiary },
                        );
                    });
                }
                ImageState::Loaded(texture_id) => {
                    let [image_width, image_height] =
                        self.ctx.tex_manager().read().meta(texture_id).unwrap().size;

                    let size =
                        self.image_size(Vec2::new(image_width as _, image_height as _), width);
                    let padding = (width - size.x) / 2.0;
                    let image_top_left = top_left + Vec2::new(padding, 0.);
                    let rect = Rect::from_min_size(image_top_left, size);

                    ui.allocate_ui_at_rect(rect, |ui| {
                        ui.add(egui::Image::from_texture((texture_id, size)).rounding(2.));

                        ui.painter().add(RectShape {
                            rect,
                            rounding: (2.).into(),
                            fill: Color32::WHITE,
                            stroke: Stroke { width: 1., color: self.theme.bg().neutral_tertiary },
                            blur_width: 0.0,
                            fill_texture_id: texture_id,
                            uv: Rect { min: Pos2 { x: 0.0, y: 0.0 }, max: Pos2 { x: 1.0, y: 1.0 } },
                        });
                        // ui.painter().image(
                        //     texture_id,
                        //     rect,
                        //     Rect { min: Pos2 { x: 0.0, y: 0.0 }, max: Pos2 { x: 1.0, y: 1.0 } },
                        //     Default::default(),
                        // );
                    });
                }
                ImageState::Failed(_) => {
                    let icon = "\u{f116}";
                    let caption = "Could not show image";

                    let size = self.image_size(Vec2::splat(200.), width);
                    let rect = Rect::from_min_size(top_left, Vec2::new(width, size.y));

                    ui.allocate_ui_at_rect(rect, |ui| {
                        let rect = ui.max_rect();
                        ui.painter().text(
                            rect.center(),
                            Align2::CENTER_CENTER,
                            icon,
                            FontId { size: 48.0, family: egui::FontFamily::Monospace },
                            self.theme.fg().neutral_tertiary,
                        );
                        ui.painter().text(
                            rect.center_bottom() + Vec2 { x: 0.0, y: -50.0 },
                            Align2::CENTER_BOTTOM,
                            caption,
                            FontId::default(),
                            self.theme.fg().neutral_tertiary,
                        );
                        ui.painter().rect_stroke(
                            rect,
                            2.,
                            Stroke { width: 1., color: self.theme.bg().neutral_tertiary },
                        );
                    });
                }
            }
        }
    }

    pub fn image_size(&self, texture_size: Vec2, width: f32) -> Vec2 {
        let width_capped_size = Vec2::new(width, texture_size.y * width / texture_size.x);
        let height_capped_size = Vec2::new(
            texture_size.x * self.image_max_size.y / texture_size.y,
            self.image_max_size.y,
        );

        if width_capped_size.length() < height_capped_size.length() {
            width_capped_size
        } else {
            height_capped_size
        }
    }
}
