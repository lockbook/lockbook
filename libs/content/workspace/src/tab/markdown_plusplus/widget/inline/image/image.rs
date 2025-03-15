use std::f32;

use comrak::nodes::AstNode;
use egui::{self, Pos2, TextFormat, Ui, Vec2};

use crate::tab::markdown_plusplus::{widget::WrapContext, MarkdownPlusPlus};

impl<'ast> MarkdownPlusPlus {
    pub fn text_format_image(&self, parent: &AstNode<'_>) -> TextFormat {
        self.text_format_link(parent)
    }

    pub fn height_image(&self, width: f32, url: &str) -> f32 {
        let image = image_from_link(url);
        let image_size = image
            .load_for_size(&self.ctx, Vec2::new(width, f32::INFINITY))
            .unwrap()
            .size()
            .unwrap();
        image_size.y * width / image_size.x
    }

    pub fn show_image_block(&self, ui: &mut Ui, top_left: Pos2, width: f32, url: &str) {
        let image = image_from_link(url);
        let height = self.height_image(width, url);
        let rect = egui::Rect::from_min_size(top_left, Vec2::new(width, height));
        ui.allocate_ui_at_rect(rect, |ui| {
            ui.add(image.max_width(width).rounding(2.));
        });
    }

    pub fn inline_span_image(&self, node: &AstNode<'_>, wrap: &WrapContext, title: &str) -> f32 {
        self.inline_span_link(node, wrap, title)
    }

    pub fn show_image(
        &self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2, wrap: &mut WrapContext,
        title: &str,
    ) {
        self.show_link(ui, node, top_left, wrap, title)
    }
}

// todo: resolve image links
pub(crate) fn image_from_link(link: &str) -> egui::Image {
    match link {
        "https://www.image.com/parth" => {
            egui::Image::new(egui::include_image!("../../../assets/parth.jpg"))
        }
        "https://www.image.com/travis" => {
            egui::Image::new(egui::include_image!("../../../assets/travis.jpg"))
        }
        "https://www.image.com/smail" => {
            egui::Image::new(egui::include_image!("../../../assets/smail.jpg"))
        }
        _ => egui::Image::new(egui::include_image!("../../../assets/adam.jpg")),
    }
}
