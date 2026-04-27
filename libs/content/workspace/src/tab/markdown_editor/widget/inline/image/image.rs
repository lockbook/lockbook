use std::f32;

use comrak::nodes::{AstNode, NodeLink};
use egui::{self, Pos2, Rect, Ui, Vec2};
use lb_rs::model::text::offset_types::Grapheme;

use crate::tab::markdown_editor::MdRender;

use crate::tab::markdown_editor::widget::inline::Response;
use crate::tab::markdown_editor::widget::utils::wrap_layout::Wrap;

impl MdRender {
    pub fn warm_images<'a>(&self, node: &'a comrak::nodes::AstNode<'a>) {
        for descendant in node.descendants() {
            let url = match &descendant.data.borrow().value {
                comrak::nodes::NodeValue::Image(link) => link.url.clone(),
                _ => continue,
            };
            self.embeds.warm(&url);
        }
    }
}

impl<'ast> MdRender {
    pub fn span_image(
        &self, node: &'ast AstNode<'ast>, wrap: &Wrap, range: (Grapheme, Grapheme),
    ) -> f32 {
        self.circumfix_span(node, wrap, range)
    }

    pub fn show_image(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2, wrap: &mut Wrap,
        node_link: &NodeLink, range: (Grapheme, Grapheme),
    ) -> Response {
        self.show_link(ui, node, top_left, wrap, node_link, range)
    }

    pub fn height_image(&self, node: &'ast AstNode<'ast>, url: &str) -> f32 {
        let width = self.width(node);
        let dims = self.embeds.size(url);
        self.image_size(dims, width).y
    }

    pub fn show_image_block(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2, url: &str,
    ) {
        let width = self.width(node);
        let dims = self.embeds.size(url);
        let size = self.image_size(dims, width);
        let padding = (width - size.x) / 2.0;
        let image_top_left = top_left + Vec2::new(padding, 0.);
        let rect = Rect::from_min_size(image_top_left, size);

        self.embeds.show(ui, url, rect);
    }

    pub fn image_size(&self, texture_size: Vec2, width: f32) -> Vec2 {
        // Constrain the image to fit the renderer with a margin of breathing
        // room. Clamp to non-negative — when the renderer is too small to
        // satisfy the margin (initial frames before viewport is known,
        // zero-height containers), the image collapses to 0 rather than
        // letting negative dimensions corrupt the surrounding block layout.
        let image_max_size = (Vec2::new(self.width, self.viewport_height)
            - Vec2::splat(self.layout.margin))
        .max(Vec2::ZERO);

        // only shrink images, never stretch beyond their natural size
        let width = width.min(texture_size.x).min(image_max_size.x);
        let height = (texture_size.y * width / texture_size.x).min(image_max_size.y);

        // if height was the binding constraint, recompute width to preserve aspect ratio
        let width = texture_size.x * height / texture_size.y;

        Vec2::new(width, height)
    }
}
