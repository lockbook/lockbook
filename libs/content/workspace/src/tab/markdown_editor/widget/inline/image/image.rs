use std::f32;

use comrak::nodes::AstNode;
use egui::{self, Pos2, Rect, Ui, Vec2};
use lb_rs::model::text::offset_types::Grapheme;

use crate::tab::markdown_editor::MdRender;
use crate::tab::markdown_editor::widget::utils::wrap_layout::Layout;

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

        // Texture dims are device pixels; the layout works in logical points.
        // Convert so a Retina screenshot (ppp 2) isn't shown at 2x real size.
        let natural = texture_size / self.ctx.pixels_per_point();

        // only shrink images, never stretch beyond their natural size
        let width = width.min(natural.x).min(image_max_size.x);
        let height = (natural.y * width / natural.x).min(image_max_size.y);

        // if height was the binding constraint, recompute width to preserve aspect ratio
        let width = natural.x * height / natural.y;

        Vec2::new(width, height)
    }

    /// Inline image. Block-positioned image rendering (the actual
    /// image-rect) lives in `show_image_block` and runs above the
    /// paragraph line; this just contributes the inline syntax bytes
    /// (`![alt](url)`) to the wrap layout — same logic as `layout_link`.
    pub fn layout_image(
        &self, layout: &mut Layout, node: &'ast AstNode<'ast>, range: (Grapheme, Grapheme),
    ) {
        self.layout_link(layout, node, range);
    }
}
