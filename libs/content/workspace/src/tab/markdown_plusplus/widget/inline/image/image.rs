use std::f32;

use comrak::nodes::NodeLink;
use egui::{self, Context, Pos2, Stroke, TextFormat, Ui, Vec2};

use crate::tab::markdown_plusplus::{
    theme::Theme,
    widget::{Ast, Block, Inline, WrapContext},
};

pub struct Image<'a, 't, 'w> {
    ast: &'w Ast<'a, 't>,
    node: &'w NodeLink,
}

impl<'a, 't, 'w> Image<'a, 't, 'w> {
    pub fn new(ast: &'w Ast<'a, 't>, node: &'w NodeLink) -> Self {
        Self { ast, node }
    }

    pub fn text_format(theme: &Theme, parent_text_format: TextFormat, ctx: &Context) -> TextFormat {
        TextFormat {
            color: theme.fg().blue,
            underline: Stroke { width: 1., color: theme.fg().blue },
            ..parent_text_format
        }
    }
}

impl Inline for Image<'_, '_, '_> {
    fn show(&self, wrap: &mut WrapContext, mut top_left: Pos2, ui: &mut Ui) {
        self.ast.show_inline_children(wrap, &mut top_left, ui)
    }

    fn span(&self, wrap: &WrapContext, ctx: &Context) -> f32 {
        self.ast.inline_children_span(wrap, ctx)
    }
}

// the block implementation draws the image itself
impl Block for Image<'_, '_, '_> {
    fn show(&self, width: f32, top_left: Pos2, ui: &mut Ui) {
        let image = image_from_link(&self.node.url);
        let height = self.height(width, ui.ctx());
        let rect = egui::Rect::from_min_size(top_left, Vec2::new(width, height));
        ui.allocate_ui_at_rect(rect, |ui| {
            ui.add(image.max_width(width).rounding(2.));
        });
    }

    fn height(&self, width: f32, ctx: &Context) -> f32 {
        let image = image_from_link(&self.node.url);
        let image_size = image
            .load_for_size(ctx, Vec2::new(width, f32::INFINITY))
            .unwrap()
            .size()
            .unwrap();
        image_size.y * width / image_size.x
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
