//! [`ScrollContent`] adapter for the markdown editor.
//!
//! `DocScrollContent` walks the document's top-level blocks. Each
//! block is one row, rendered atomically via `show_block`. Container
//! chrome (block-quote bars, alert bars, list markers, code-block
//! borders, etc.) is painted by the container's own `show_*` function,
//! the same path `MdLabel` and tests use.
//!
//! Plus a virtual leading pad row at the start with `approx = precise
//! = leading_precise` (real top padding — counted toward the
//! scrollbar) and a virtual trailing pad at the end with `approx =
//! 0`, `precise = trailing_precise` (overscroll room — not counted
//! toward the scrollbar — so the last block can scroll into the
//! middle of the viewport).

use comrak::nodes::AstNode;
use egui::{Pos2, Ui};
use lb_rs::model::text::offset_types::Grapheme;

use crate::tab::markdown_editor::MdRender;
use crate::widgets::affine_scroll::Rows;

pub struct DocScrollContent<'a, 'ast> {
    pub renderer: &'a mut MdRender,
    pub root: &'ast AstNode<'ast>,
    /// Precise + approx height of the virtual leading pad row. Counts
    /// toward the scrollbar — unlike `trailing_precise` which is
    /// approx-zero — so the pad behaves like real top padding rather
    /// than overscroll.
    pub leading_precise: f32,
    /// Precise height of the virtual trailing pad row (e.g. vh/2).
    /// Approx height is 0 — overscroll, not content.
    pub trailing_precise: f32,
    /// X position to render content at. Lets the content sit centered
    /// inside the canvas while the scroll widget's body (and scrollbar)
    /// span the full canvas width.
    pub content_x: f32,
    cursor: Cursor<'ast>,
}

#[derive(Clone, Copy)]
enum Cursor<'ast> {
    Start,
    Leading,
    /// Markdown-mode row: one top-level AST block.
    At {
        node: &'ast AstNode<'ast>,
    },
    /// Plaintext-mode row: one source line.
    AtLine {
        line_idx: usize,
    },
    Trailing,
    End,
}

impl<'a, 'ast> DocScrollContent<'a, 'ast> {
    pub fn new(
        renderer: &'a mut MdRender, root: &'ast AstNode<'ast>, content_x: f32,
        trailing_precise: f32,
    ) -> Self {
        Self {
            renderer,
            root,
            leading_precise: 0.0,
            trailing_precise,
            content_x,
            cursor: Cursor::Start,
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

    /// Source-text range covered by the row at the cursor's current
    /// position, or `None` if the cursor is off a real row.
    pub fn text_range(&self) -> Option<(Grapheme, Grapheme)> {
        match self.cursor {
            Cursor::At { node } => Some(self.renderer.node_range(node)),
            Cursor::AtLine { line_idx } => Some(self.renderer.bounds.source_lines[line_idx]),
            Cursor::Leading | Cursor::Trailing | Cursor::Start | Cursor::End => None,
        }
    }
}

impl<'a, 'ast> Rows for DocScrollContent<'a, 'ast> {
    fn reset(&mut self) {
        self.cursor = Cursor::Start;
    }

    fn reset_back(&mut self) {
        self.cursor = Cursor::End;
    }

    fn next(&mut self) -> bool {
        let plaintext = self.renderer.plaintext;
        let line_count = self.renderer.bounds.source_lines.len();
        match self.cursor {
            Cursor::Start => self.cursor = Cursor::Leading,
            Cursor::Leading if plaintext && line_count > 0 => {
                self.cursor = Cursor::AtLine { line_idx: 0 }
            }
            Cursor::Leading => match self.root.children().next() {
                Some(first) => self.cursor = Cursor::At { node: first },
                // Childless doc — yield the root itself as a single
                // row so `show_document`'s fallback (iterate source
                // lines) runs and the cursor has a galley to land on.
                None => self.cursor = Cursor::At { node: self.root },
            },
            Cursor::At { node } => match node.next_sibling() {
                Some(next) => self.cursor = Cursor::At { node: next },
                None => self.cursor = Cursor::Trailing,
            },
            Cursor::AtLine { line_idx } if line_idx + 1 < line_count => {
                self.cursor = Cursor::AtLine { line_idx: line_idx + 1 }
            }
            Cursor::AtLine { .. } => self.cursor = Cursor::Trailing,
            Cursor::Trailing => {
                self.cursor = Cursor::End;
                return false;
            }
            Cursor::End => return false,
        }
        true
    }

    fn prev(&mut self) -> bool {
        let plaintext = self.renderer.plaintext;
        let line_count = self.renderer.bounds.source_lines.len();
        match self.cursor {
            Cursor::End => self.cursor = Cursor::Trailing,
            Cursor::Trailing if plaintext && line_count > 0 => {
                self.cursor = Cursor::AtLine { line_idx: line_count - 1 }
            }
            Cursor::Trailing => match self.root.children().last() {
                Some(last) => self.cursor = Cursor::At { node: last },
                None => self.cursor = Cursor::At { node: self.root },
            },
            Cursor::At { node } => match node.previous_sibling() {
                Some(prev) => self.cursor = Cursor::At { node: prev },
                None => self.cursor = Cursor::Leading,
            },
            Cursor::AtLine { line_idx } if line_idx > 0 => {
                self.cursor = Cursor::AtLine { line_idx: line_idx - 1 }
            }
            Cursor::AtLine { .. } => self.cursor = Cursor::Leading,
            Cursor::Leading => {
                self.cursor = Cursor::Start;
                return false;
            }
            Cursor::Start => return false,
        }
        true
    }

    fn approx(&self) -> f32 {
        match self.cursor {
            Cursor::At { node } => {
                self.renderer.block_pre_spacing_height_approx(node)
                    + self.renderer.height_approx(node)
                    + self.renderer.block_post_spacing_height_approx(node)
            }
            // Per-line plaintext rows: monospace char-count estimate
            // (no shaping); precise reflects actual cosmic-text wrap.
            Cursor::AtLine { line_idx } => {
                let width = self.renderer.width(self.root);
                self.renderer.height_approx_source_line(line_idx, width)
                    + self.renderer.layout.row_spacing
            }
            Cursor::Leading => self.leading_precise,
            Cursor::Trailing => 0.0,
            Cursor::Start | Cursor::End => panic!("approx() with cursor off a row"),
        }
    }

    fn precise(&mut self) -> f32 {
        match self.cursor {
            Cursor::At { node } => {
                self.renderer.block_pre_spacing_height(node)
                    + self.renderer.height(node)
                    + self.renderer.block_post_spacing_height(node)
            }
            Cursor::AtLine { line_idx } => {
                let width = self.renderer.width(self.root);
                self.renderer.height_source_line(line_idx, width) + self.renderer.layout.row_spacing
            }
            Cursor::Leading => self.leading_precise,
            Cursor::Trailing => self.trailing_precise,
            Cursor::Start | Cursor::End => panic!("precise() with cursor off a row"),
        }
    }

    fn warm(&mut self) {
        if let Cursor::At { node } = self.cursor {
            self.renderer.warm_images(node);
        }
    }

    fn render(&mut self, ui: &mut Ui, top_left: Pos2) {
        match self.cursor {
            Cursor::At { node } => {
                let mut tl = Pos2::new(self.content_x, top_left.y);
                self.renderer.show_block_pre_spacing(ui, node, tl);
                tl.y += self.renderer.block_pre_spacing_height(node);
                self.renderer.show_block(ui, node, tl);
                tl.y += self.renderer.height(node);
                self.renderer.show_block_post_spacing(ui, node, tl);
            }
            Cursor::AtLine { line_idx } => {
                let tl = Pos2::new(self.content_x, top_left.y);
                let width = self.renderer.width(self.root);
                self.renderer.show_source_line(ui, tl, line_idx, width);
            }
            Cursor::Leading | Cursor::Trailing => {} // virtual padding — paints nothing
            Cursor::Start | Cursor::End => panic!("render() with cursor off a row"),
        }
    }
}
