//! [`DocScrollContent`] — id-keyed [`Rows`] adapter for the markdown
//! editor.
//!
//! `DocRowId::Block(idx)` walks the document's top-level blocks. Each
//! block is one row, rendered atomically via `show_block`. Container
//! chrome (block-quote bars, alert bars, list markers, code-block
//! borders, …) is painted by the container's own `show_*` function,
//! the same path `MdLabel` and tests use.
//!
//! Plus a virtual `Leading` row at the start with `approx = precise =
//! leading_precise` (real top padding — counted toward the scrollbar)
//! and a virtual `Trailing` row at the end with `approx = 0`, `precise
//! = trailing_precise` (overscroll room — not counted toward the
//! scrollbar) so the last block can scroll into the middle of the
//! viewport.
//!
//! In plaintext mode (`renderer.plaintext == true`) the rows are source
//! lines indexed by `DocRowId::Line(idx)` instead of blocks.

use comrak::nodes::AstNode;
use egui::{Pos2, Ui};
use lb_rs::model::text::offset_types::Grapheme;

use crate::tab::markdown_editor::MdRender;
use crate::widgets::affine_scroll::Rows;

/// Stable identity of a row in the document. `Leading` and `Trailing`
/// are the always-present virtual padding rows; `Block(i)` is a top-
/// level block index, `Line(i)` a source-line index in plaintext mode.
///
/// `Send + Sync + 'static` so [`crate::widgets::affine_scroll::ScrollArea<DocRowId>`]
/// can be persisted in egui memory.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum DocRowId {
    Leading,
    Block(usize),
    Line(usize),
    Trailing,
}

pub struct DocScrollContent<'a, 'ast> {
    pub renderer: &'a MdRender,
    pub root: &'ast AstNode<'ast>,
    /// Cached top-level child blocks for this frame. Empty in plaintext
    /// mode (the [`Rows`] impl uses `Line(idx)` instead).
    blocks: Vec<&'ast AstNode<'ast>>,
    /// Source-line count, sampled at construction. Used in plaintext
    /// mode for `Line(idx)` bounds.
    line_count: usize,
    plaintext: bool,
    /// Precise + approx height of the virtual leading pad row. Counts
    /// toward the scrollbar — unlike `trailing_precise`, which is
    /// approx-zero — so the pad behaves like real top padding rather
    /// than overscroll.
    pub leading_precise: f32,
    /// Precise height of the virtual trailing pad row (e.g. vh/2).
    /// Approx height is 0 — overscroll, not content.
    pub trailing_precise: f32,
}

impl<'a, 'ast> DocScrollContent<'a, 'ast> {
    pub fn new(renderer: &'a MdRender, root: &'ast AstNode<'ast>, trailing_precise: f32) -> Self {
        let plaintext = renderer.plaintext;
        let blocks: Vec<_> = if plaintext { Vec::new() } else { root.children().collect() };
        let line_count = renderer.bounds.source_lines.len();
        Self {
            renderer,
            root,
            blocks,
            line_count,
            plaintext,
            leading_precise: 0.0,
            trailing_precise,
        }
    }

    /// Set `leading_precise` to the default top-padding for production
    /// rendering. Larger on Android to clear the system status bar /
    /// safe area. All production walks (render + persistence + scroll-
    /// to) must agree on this value or anchor offsets drift by the
    /// difference.
    pub fn with_default_leading(mut self) -> Self {
        self.leading_precise = if cfg!(target_os = "android") { 60.0 } else { 15.0 };
        self
    }

    /// First row whose source range contains `target` (inclusive on both
    /// ends so an end-of-line cursor matches its row).
    pub fn find_text_row(&self, target: Grapheme) -> Option<DocRowId> {
        if self.plaintext {
            for i in 0..self.line_count {
                let (s, e) = self.renderer.bounds.source_lines[i];
                if target >= s && target <= e {
                    return Some(DocRowId::Line(i));
                }
            }
        } else {
            for (i, node) in self.blocks.iter().enumerate() {
                let (s, e) = self.renderer.node_range(node);
                if target >= s && target <= e {
                    return Some(DocRowId::Block(i));
                }
            }
        }
        None
    }
}

impl<'a, 'ast> Rows for DocScrollContent<'a, 'ast> {
    type RowId = DocRowId;

    fn first(&self) -> Option<DocRowId> {
        Some(DocRowId::Leading)
    }

    fn last(&self) -> Option<DocRowId> {
        Some(DocRowId::Trailing)
    }

    fn next(&self, id: &DocRowId) -> Option<DocRowId> {
        match id {
            DocRowId::Leading => {
                if self.plaintext {
                    if self.line_count > 0 {
                        Some(DocRowId::Line(0))
                    } else {
                        Some(DocRowId::Trailing)
                    }
                } else if self.blocks.is_empty() {
                    Some(DocRowId::Trailing)
                } else {
                    Some(DocRowId::Block(0))
                }
            }
            DocRowId::Block(i) => {
                if self.plaintext || *i >= self.blocks.len() {
                    return None;
                }
                if i + 1 < self.blocks.len() {
                    Some(DocRowId::Block(i + 1))
                } else {
                    Some(DocRowId::Trailing)
                }
            }
            DocRowId::Line(i) => {
                if !self.plaintext || *i >= self.line_count {
                    return None;
                }
                if i + 1 < self.line_count {
                    Some(DocRowId::Line(i + 1))
                } else {
                    Some(DocRowId::Trailing)
                }
            }
            DocRowId::Trailing => None,
        }
    }

    fn prev(&self, id: &DocRowId) -> Option<DocRowId> {
        match id {
            DocRowId::Leading => None,
            DocRowId::Block(i) => {
                if self.plaintext || *i >= self.blocks.len() {
                    return None;
                }
                if *i == 0 { Some(DocRowId::Leading) } else { Some(DocRowId::Block(i - 1)) }
            }
            DocRowId::Line(i) => {
                if !self.plaintext || *i >= self.line_count {
                    return None;
                }
                if *i == 0 { Some(DocRowId::Leading) } else { Some(DocRowId::Line(i - 1)) }
            }
            DocRowId::Trailing => {
                if self.plaintext {
                    if self.line_count > 0 {
                        Some(DocRowId::Line(self.line_count - 1))
                    } else {
                        Some(DocRowId::Leading)
                    }
                } else if !self.blocks.is_empty() {
                    Some(DocRowId::Block(self.blocks.len() - 1))
                } else {
                    Some(DocRowId::Leading)
                }
            }
        }
    }

    fn approx(&self, id: &DocRowId) -> f32 {
        match id {
            DocRowId::Leading => self.leading_precise,
            DocRowId::Block(i) => {
                let n = self.blocks[*i];
                self.renderer.block_pre_spacing_height_approx(n)
                    + self.renderer.height_approx(n)
                    + self.renderer.block_post_spacing_height_approx(n)
            }
            // Per-line plaintext rows: monospace char-count estimate
            // (no shaping); precise reflects actual cosmic-text wrap.
            DocRowId::Line(i) => {
                let width = self.renderer.width(self.root);
                self.renderer.height_approx_source_line(*i, width)
                    + self.renderer.layout.row_spacing
            }
            DocRowId::Trailing => 0.0,
        }
    }

    fn precise(&self, id: &DocRowId) -> f32 {
        match id {
            DocRowId::Leading => self.leading_precise,
            DocRowId::Block(i) => {
                let n = self.blocks[*i];
                self.renderer.block_pre_spacing_height(n)
                    + self.renderer.height(n)
                    + self.renderer.block_post_spacing_height(n)
            }
            DocRowId::Line(i) => {
                let width = self.renderer.width(self.root);
                self.renderer.height_source_line(*i, width) + self.renderer.layout.row_spacing
            }
            DocRowId::Trailing => self.trailing_precise,
        }
    }

    fn warm(&self, id: &DocRowId) {
        if let DocRowId::Block(i) = id {
            self.renderer.warm_images(self.blocks[*i]);
        }
    }
}

/// Paint a single row at `top_left` (screen coords). Companion to the
/// pull-style [`crate::widgets::affine_scroll::AffineScrollArea::show`]
/// loop: the caller iterates `visible` and invokes this per row.
///
/// Takes `&mut MdRender` because `show_*` functions push galleys and
/// text areas; takes `blocks` separately so the caller can drop the
/// `&MdRender` borrow held by [`DocScrollContent`] before painting.
pub fn paint_row<'ast>(
    ui: &mut Ui, renderer: &mut MdRender, root: &'ast AstNode<'ast>,
    blocks: &[&'ast AstNode<'ast>], id: &DocRowId, top_left: Pos2, content_x: f32,
) {
    match id {
        DocRowId::Leading | DocRowId::Trailing => {} // virtual padding
        DocRowId::Block(i) => {
            let node = blocks[*i];
            let mut tl = Pos2::new(content_x, top_left.y);
            renderer.show_block_pre_spacing(ui, node, tl);
            tl.y += renderer.block_pre_spacing_height(node);
            renderer.show_block(ui, node, tl);
            tl.y += renderer.height(node);
            renderer.show_block_post_spacing(ui, node, tl);
        }
        DocRowId::Line(line_idx) => {
            let tl = Pos2::new(content_x, top_left.y);
            let width = renderer.width(root);
            renderer.show_source_line(ui, tl, *line_idx, width);
        }
    }
}
