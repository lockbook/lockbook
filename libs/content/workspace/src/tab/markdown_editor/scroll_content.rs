//! [`ScrollContent`] adapter for the markdown editor — exposes the
//! document's top-level blocks to the [`AffineScrollArea`].
//!
//! Top-level blocks (children of the doc root) are the units the scroll
//! area treats as atomic. Per-block heights come from the existing
//! `height_approx` and `height` cache, so cost is amortised. Rendering
//! delegates to `show_block`.

use comrak::nodes::AstNode;
use egui::{Pos2, Ui};

use crate::tab::markdown_editor::MdRender;
use crate::widgets::affine_scroll::ScrollContent;

/// Adapter that lets `MdRender` plug into [`AffineScrollArea`]. Built
/// fresh per frame from the parsed root.
pub struct DocScrollContent<'a, 'ast> {
    pub renderer: &'a mut MdRender,
    pub blocks: Vec<&'ast AstNode<'ast>>,
}

impl<'a, 'ast> DocScrollContent<'a, 'ast> {
    pub fn new(renderer: &'a mut MdRender, root: &'ast AstNode<'ast>) -> Self {
        let blocks: Vec<_> = root.children().collect();
        Self { renderer, blocks }
    }
}

impl<'a, 'ast> ScrollContent for DocScrollContent<'a, 'ast> {
    fn block_count(&self) -> usize {
        self.blocks.len()
    }

    fn approx_height(&self, i: usize) -> f32 {
        self.renderer.block_pre_spacing_height_approx(self.blocks[i], &self.blocks)
            + self.renderer.height_approx(self.blocks[i], &self.blocks)
            + self.renderer.block_post_spacing_height_approx(self.blocks[i], &self.blocks)
    }

    fn precise_height(&mut self, i: usize) -> f32 {
        self.renderer.block_pre_spacing_height(self.blocks[i], &self.blocks)
            + self.renderer.height(self.blocks[i], &self.blocks)
            + self.renderer.block_post_spacing_height(self.blocks[i], &self.blocks)
    }

    fn render_block(&mut self, ui: &mut Ui, i: usize, top_left: Pos2) {
        // Use the renderer's pre-set `top_left.x` (the centered
        // content column) for x; take y from the scroll area. This
        // lets the scroll area span the full canvas width while
        // content paints in its centered column.
        let mut top_left = Pos2::new(self.renderer.top_left.x, top_left.y);
        self.renderer
            .show_block_pre_spacing(ui, self.blocks[i], top_left, &self.blocks);
        top_left.y +=
            self.renderer.block_pre_spacing_height(self.blocks[i], &self.blocks);
        self.renderer.show_block(ui, self.blocks[i], top_left, &self.blocks);
        top_left.y += self.renderer.height(self.blocks[i], &self.blocks);
        self.renderer
            .show_block_post_spacing(ui, self.blocks[i], top_left, &self.blocks);
    }
}
