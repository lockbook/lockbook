use std::f32;

use comrak::nodes::{AstNode, NodeValue};
use egui::{self, Vec2};
use lb_rs::model::text::offset_types::{Grapheme, RangeExt as _};
use lb_rs::model::text::operation_types::Operation;

use crate::tab::markdown_editor::input::{Event, Location, Region};
use crate::tab::markdown_editor::widget::utils::NodeValueExt as _;
use crate::tab::markdown_editor::widget::utils::wrap_layout::{ImageSpec, Layout};
use crate::tab::markdown_editor::{MdEdit, MdRender};
use crate::tab::{ContextMenuTarget, ExtendedOutput as _};

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

    /// Logical-point size an `Image` node will render at, constrained
    /// by its block's width and any Obsidian `|WxH` hint. `None` if
    /// `node` isn't an `Image`, has no leaf-block ancestor, or
    /// collapses to zero on the first frame.
    pub fn image_logical_size(&self, node: &'ast AstNode<'ast>) -> Option<Vec2> {
        let url = match &node.data.borrow().value {
            comrak::nodes::NodeValue::Image(link) => link.url.clone(),
            _ => return None,
        };
        let block_ancestor = node
            .ancestors()
            .skip(1)
            .find(|a| a.data.borrow().value.is_leaf_block())?;
        let width = self.width(block_ancestor);
        let texture_size = self.embeds.size(&url);
        let size = self.image_size(texture_size, width, self.image_dims(node));
        (size.x > 0.0 && size.y > 0.0).then_some(size)
    }

    /// Collapsed → emit one `InlineItem::Image` covering the syntax.
    /// Revealed / `disable_images` / multi-line / unsizable → fall
    /// through to `layout_link` so the source bytes render for editing.
    pub fn layout_image(
        &self, layout: &mut Layout, node: &'ast AstNode<'ast>, range: (Grapheme, Grapheme),
    ) {
        let node_range = self.node_range(node);
        let single_line = range.contains_range(&node_range, true, true);
        let revealed = self.range_revealed_interior(node_range);
        let collapsed_size = (!self.disable_images && !revealed && single_line)
            .then(|| self.image_logical_size(node))
            .flatten();
        let Some(size) = collapsed_size else {
            self.layout_link(layout, node, range);
            return;
        };
        let url = match &node.data.borrow().value {
            comrak::nodes::NodeValue::Image(link) => link.url.clone(),
            _ => return,
        };
        // Always interactive: a plain tap selects, a cmd/keyboard-hidden tap opens.
        layout.interaction_open(Self::image_interaction_id_salt(node_range), egui::Sense::click());
        layout.push_image(ImageSpec {
            advance: size.x,
            ascent: size.y,
            descent: 0.0,
            source_range: node_range,
            url,
        });
        layout.interaction_close();
    }

    /// Interaction-response salt for a rendered image; distinct from
    /// [`Self::link_interaction_id_salt`] so the collapsed-image and
    /// revealed-link handlers don't collide.
    pub fn image_interaction_id_salt(node_range: (Grapheme, Grapheme)) -> egui::Id {
        egui::Id::new(("md_image", node_range))
    }

    /// Recompute [`super::super::super::bounds::Bounds::images`] — the source
    /// range of every inline image. Empty when images render raw (disabled),
    /// since then the source is plain editable text. Depends on text only.
    pub fn calc_image_bounds<'a>(&mut self, root: &'a AstNode<'a>) {
        let mut images = Vec::new();
        if !self.disable_images {
            for node in root.descendants() {
                if matches!(node.data.borrow().value, NodeValue::Image(_)) {
                    images.push(self.node_range(node));
                }
            }
        }
        images.sort_unstable();
        self.bounds.images = images;
    }
}

impl<'ast> MdEdit {
    /// Open or select an image clicked this frame: a cmd-click (desktop) or
    /// keyboard-hidden tap (mobile) opens its target, a plain tap selects it.
    /// Adds the rect to `touch_consuming_rects` so iOS routes the tap here.
    pub fn handle_image_interactions(
        &mut self, root: &'ast AstNode<'ast>, ui: &egui::Ui, id: egui::Id, keyboard_visible: bool,
        ops: &mut Vec<Operation>,
    ) {
        let open_image = if self.renderer.touch_mode {
            !keyboard_visible
        } else {
            ui.ctx().input(|i| i.modifiers.command)
        };
        for node in root.descendants() {
            let url = match &node.data.borrow().value {
                NodeValue::Image(link) => link.url.clone(),
                _ => continue,
            };
            let node_range = self.renderer.node_range(node);
            let salt = MdRender::image_interaction_id_salt(node_range);
            let response = match self.renderer.interaction_responses.get(&ui.id().with(salt)) {
                Some(r) => r.clone(), // clone to release the `renderer` borrow
                None => continue,
            };

            self.renderer.touch_consuming_rects.push(response.rect);

            if open_image && response.hovered() {
                ui.ctx()
                    .output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
            }
            if response.clicked() {
                if open_image {
                    self.renderer.open_resolved_link(&url, ui.ctx());
                } else {
                    ui.memory_mut(|m| m.request_focus(id));
                    let region = Region::BetweenLocations {
                        start: Location::Grapheme(node_range.start()),
                        end: Location::Grapheme(node_range.end()),
                    };
                    self.calc_operations(ui.ctx(), root, Event::Select { region }, ops);
                    // Touch: pop the edit menu (copy/edit/paste) above the image.
                    if self.renderer.touch_mode {
                        ui.ctx()
                            .set_context_menu(response.rect.center_top(), ContextMenuTarget::Atom);
                    }
                }
            }
        }
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
