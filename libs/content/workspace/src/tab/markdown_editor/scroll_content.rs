//! [`ScrollContent`] adapter for the markdown editor.
//!
//! `DocScrollContent` is the cursor itself — no flat leaf list, no
//! pre-collected `Vec` of nodes. `next` / `prev` walk the AST on
//! demand. The cursor identifies a leaf as either:
//!
//! - an **atomic** top-level AST block (paragraph, heading, list,
//!   blockquote, etc.), or
//! - one **line** within a line-based top-level block (code block,
//!   HTML block, front matter — and the whole document when in
//!   plaintext mode).
//!
//! Plus one **virtual trailing pad** leaf at the end with `approx = 0`
//! and `precise = trailing_precise`, giving the user vh/2 of empty
//! space below the doc end so the bottom of the last real leaf can be
//! scrolled into view.

use comrak::nodes::{AstNode, NodeValue};
use egui::{Pos2, Ui};
use lb_rs::model::text::offset_types::Grapheme;

use crate::tab::markdown_editor::MdRender;
use crate::widgets::affine_scroll::Rows;

pub struct DocScrollContent<'a, 'ast> {
    pub renderer: &'a mut MdRender,
    pub root: &'ast AstNode<'ast>,
    /// Precise height of the virtual trailing pad leaf (e.g. vh/2).
    pub trailing_precise: f32,
    cursor: Cursor<'ast>,
}

/// Cursor position within the doc's leaf sequence.
#[derive(Clone, Copy)]
enum Cursor<'ast> {
    /// Before the first leaf — fresh state after `reset()`.
    Start,
    /// At a top-level leaf. `line` is `Some` for line-based leaves,
    /// `None` for atomic.
    At { node: &'ast AstNode<'ast>, line: Option<usize> },
    /// At the virtual trailing pad leaf.
    Trailing,
    /// After the trailing pad — past the end.
    End,
}

impl<'a, 'ast> DocScrollContent<'a, 'ast> {
    pub fn new(
        renderer: &'a mut MdRender, root: &'ast AstNode<'ast>, trailing_precise: f32,
    ) -> Self {
        Self { renderer, root, trailing_precise, cursor: Cursor::Start }
    }

    /// Top-level siblings — collected per call (cheap; AST root usually
    /// has dozens of children) so spacing helpers can answer
    /// "first/last/only" questions about `node`.
    fn siblings(&self) -> Vec<&'ast AstNode<'ast>> {
        self.root.children().collect()
    }
}

/// Number of source lines in a line-based top-level block (code,
/// html, front matter), or `None` for atomic blocks.
fn line_count<'ast>(renderer: &MdRender, node: &'ast AstNode<'ast>) -> Option<usize> {
    let value = &node.data.borrow().value;
    let line_based = matches!(
        value,
        NodeValue::CodeBlock(_) | NodeValue::HtmlBlock(_) | NodeValue::FrontMatter(_)
    );
    if !line_based {
        return None;
    }
    let first = renderer.node_first_line_idx(node);
    let last = renderer.node_last_line_idx(node);
    Some(last - first + 1)
}

impl<'a, 'ast> DocScrollContent<'a, 'ast> {
    /// Source-text range covered by the row at the cursor's current
    /// position, or `None` if the cursor is off a real row (trailing
    /// pad, before start, past end).
    pub fn text_range(&self) -> Option<(Grapheme, Grapheme)> {
        match self.cursor {
            Cursor::At { node, line: None } => Some(self.renderer.node_range(node)),
            Cursor::At { node, line: Some(idx) } => {
                let first = self.renderer.node_first_line_idx(node);
                Some(self.renderer.bounds.source_lines[first + idx])
            }
            Cursor::Trailing | Cursor::Start | Cursor::End => None,
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
        match self.cursor {
            Cursor::Start => match self.root.children().next() {
                Some(first) => {
                    let line = line_count(self.renderer, first).map(|_| 0);
                    self.cursor = Cursor::At { node: first, line };
                }
                None => self.cursor = Cursor::Trailing,
            },
            Cursor::At { node, line } => {
                let advanced_line = line.and_then(|idx| {
                    let total = line_count(self.renderer, node).unwrap_or(1);
                    (idx + 1 < total).then_some(idx + 1)
                });
                if let Some(idx) = advanced_line {
                    self.cursor = Cursor::At { node, line: Some(idx) };
                } else if let Some(next) = node.next_sibling() {
                    let line = line_count(self.renderer, next).map(|_| 0);
                    self.cursor = Cursor::At { node: next, line };
                } else {
                    self.cursor = Cursor::Trailing;
                }
            }
            Cursor::Trailing => {
                self.cursor = Cursor::End;
                return false;
            }
            Cursor::End => return false,
        }
        true
    }

    fn prev(&mut self) -> bool {
        match self.cursor {
            Cursor::End => self.cursor = Cursor::Trailing,
            Cursor::Trailing => match self.root.children().last() {
                Some(last) => {
                    let line = line_count(self.renderer, last).map(|n| n.saturating_sub(1));
                    self.cursor = Cursor::At { node: last, line };
                }
                None => {
                    self.cursor = Cursor::Start;
                    return false;
                }
            },
            Cursor::At { node, line } => {
                let stepped_line = line.and_then(|idx| (idx > 0).then(|| idx - 1));
                if let Some(idx) = stepped_line {
                    self.cursor = Cursor::At { node, line: Some(idx) };
                } else if let Some(prev) = node.previous_sibling() {
                    let line = line_count(self.renderer, prev).map(|n| n.saturating_sub(1));
                    self.cursor = Cursor::At { node: prev, line };
                } else {
                    self.cursor = Cursor::Start;
                    return false;
                }
            }
            Cursor::Start => return false,
        }
        true
    }

    fn approx(&self) -> f32 {
        match self.cursor {
            Cursor::At { node, line: _ } => {
                let siblings = self.siblings();
                self.renderer.block_pre_spacing_height_approx(node, &siblings)
                    + self.renderer.height_approx(node, &siblings)
                    + self.renderer.block_post_spacing_height_approx(node, &siblings)
            }
            Cursor::Trailing => 0.0,
            Cursor::Start | Cursor::End => panic!("approx() with cursor off a row"),
        }
    }

    fn precise(&mut self) -> f32 {
        match self.cursor {
            Cursor::At { node, line: _ } => {
                let siblings = self.siblings();
                self.renderer.block_pre_spacing_height(node, &siblings)
                    + self.renderer.height(node, &siblings)
                    + self.renderer.block_post_spacing_height(node, &siblings)
            }
            Cursor::Trailing => self.trailing_precise,
            Cursor::Start | Cursor::End => panic!("precise() with cursor off a row"),
        }
    }

    fn render(&mut self, ui: &mut Ui, top_left: Pos2) {
        match self.cursor {
            Cursor::At { node, line: _ } => {
                // x from renderer's centered content column; y from
                // scroll area's screen-space placement.
                let mut tl = Pos2::new(self.renderer.top_left.x, top_left.y);
                let siblings = self.siblings();
                self.renderer.show_block_pre_spacing(ui, node, tl, &siblings);
                tl.y += self.renderer.block_pre_spacing_height(node, &siblings);
                self.renderer.show_block(ui, node, tl, &siblings);
                tl.y += self.renderer.height(node, &siblings);
                self.renderer.show_block_post_spacing(ui, node, tl, &siblings);
            }
            Cursor::Trailing => {} // virtual padding — paints nothing
            Cursor::Start | Cursor::End => panic!("render() with cursor off a row"),
        }
    }
}
