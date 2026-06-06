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
    pub fn height_image(&self, node: &'ast AstNode<'ast>, url: &str, requested: ImageDims) -> f32 {
        let width = self.width(node);
        let dims = self.embeds.size(url);
        self.image_size(dims, width, requested).y
    }

    pub fn show_image_block(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2, url: &str,
        requested: ImageDims,
    ) {
        let width = self.width(node);
        let dims = self.embeds.size(url);
        let size = self.image_size(dims, width, requested);
        let padding = (width - size.x) / 2.0;
        let image_top_left = top_left + Vec2::new(padding, 0.);
        let rect = Rect::from_min_size(image_top_left, size);

        self.embeds.show(ui, url, rect);
    }

    pub fn image_size(&self, texture_size: Vec2, width: f32, requested: ImageDims) -> Vec2 {
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
        let aspect = if natural.x > 0.0 { natural.y / natural.x } else { 1.0 };

        // Obsidian-style explicit dims (`![alt|WxH](url)`) are logical points,
        // not device pixels. They set the box — upscaling past natural size is
        // allowed — but still shrink to fit the viewport, preserving the
        // requested aspect ratio. Width-only keeps the natural aspect.
        if requested.width.is_some() || requested.height.is_some() {
            let target = match (requested.width, requested.height) {
                (Some(w), Some(h)) => Vec2::new(w, h),
                (Some(w), None) => Vec2::new(w, w * aspect),
                (None, Some(h)) if aspect > 0.0 => Vec2::new(h / aspect, h),
                (None, Some(h)) => Vec2::new(h, h),
                (None, None) => unreachable!(),
            };
            let scale = (image_max_size.x / target.x)
                .min(image_max_size.y / target.y)
                .min(1.0);
            return (target * scale).max(Vec2::ZERO);
        }

        // only shrink images, never stretch beyond their natural size
        let width = width.min(natural.x).min(image_max_size.x);
        let height = (natural.y * width / natural.x).min(image_max_size.y);

        // if height was the binding constraint, recompute width to preserve aspect ratio
        let width = natural.x * height / natural.y;

        Vec2::new(width, height)
    }

    /// Obsidian-style explicit dimensions for an image, parsed from the
    /// trailing `|W` / `|WxH` of its alt text (`![alt|100x100](url)`).
    pub fn image_dims(&self, node: &'ast AstNode<'ast>) -> ImageDims {
        match self.infix_range(node) {
            Some(range) => parse_image_dims(&self.buffer[range]),
            None => ImageDims::default(),
        }
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

/// Explicit image dimensions in logical points; `None` means "use the
/// natural / fit-to-width size for that axis."
#[derive(Clone, Copy, Default)]
pub struct ImageDims {
    pub width: Option<f32>,
    pub height: Option<f32>,
}

/// Parse Obsidian's `|W` / `|WxH` size suffix off an image's alt text. Only a
/// trailing segment of positive integers (optionally split by a single `x`)
/// counts — anything else leaves a literal `|` in the alt and yields no dims.
fn parse_image_dims(alt: &str) -> ImageDims {
    let Some(bar) = alt.rfind('|') else {
        return ImageDims::default();
    };
    let mut parts = alt[bar + 1..].split('x');
    let w = parts.next().unwrap_or("");
    let h = parts.next();
    if parts.next().is_some() {
        return ImageDims::default(); // more than one `x`
    }

    let dim = |s: &str| -> Option<f32> {
        (!s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
            .then(|| s.parse::<u32>().ok())
            .flatten()
            .filter(|n| *n > 0)
            .map(|n| n as f32)
    };

    match h {
        None => match dim(w) {
            Some(width) => ImageDims { width: Some(width), height: None },
            None => ImageDims::default(),
        },
        // `WxH`: both must be valid, else the `|…` isn't a size spec
        Some(h) => match (dim(w), dim(h)) {
            (Some(width), Some(height)) => ImageDims { width: Some(width), height: Some(height) },
            _ => ImageDims::default(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::parse_image_dims;

    #[test]
    fn obsidian_image_dims() {
        let wh = |s: &str| {
            let d = parse_image_dims(s);
            (d.width, d.height)
        };
        // valid specs
        assert_eq!(wh("alt|100x40"), (Some(100.0), Some(40.0)));
        assert_eq!(wh("alt|100"), (Some(100.0), None));
        assert_eq!(wh("|100x40"), (Some(100.0), Some(40.0))); // empty alt
        assert_eq!(wh("a|b|100"), (Some(100.0), None)); // only the last `|` segment
        // not a size: the `|` stays part of the alt
        assert_eq!(wh("alt"), (None, None));
        assert_eq!(wh("alt|caption"), (None, None));
        assert_eq!(wh("alt|100x"), (None, None)); // missing height
        assert_eq!(wh("alt|100xx40"), (None, None)); // two `x`
        assert_eq!(wh("alt|100X40"), (None, None)); // uppercase X isn't the separator
        assert_eq!(wh("alt|0"), (None, None)); // zero rejected
        assert_eq!(wh("alt|100x0"), (None, None)); // zero dim rejected
    }
}
