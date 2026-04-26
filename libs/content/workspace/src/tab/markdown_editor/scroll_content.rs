//! [`ScrollContent`] adapter for the markdown editor.
//!
//! `DocScrollContent` is the cursor itself — no flat leaf list, no
//! pre-collected `Vec` of nodes. `next` / `prev` walk the AST on
//! demand.
//!
//! Today every top-level AST block (paragraph, heading, code block,
//! list, blockquote, etc.) is one row. R2 will change this so the
//! cursor descends into containers and per-line leaves become rows.
//!
//! Plus one **virtual trailing pad** row at the end with `approx = 0`
//! and `precise = trailing_precise`, giving the user vh/2 of empty
//! space below the doc end so the bottom of the last real row can be
//! scrolled into view.

use comrak::nodes::AstNode;
use egui::{Pos2, Ui};
use lb_rs::model::text::offset_types::Grapheme;

use crate::tab::markdown_editor::MdRender;
use crate::widgets::affine_scroll::Rows;

pub struct DocScrollContent<'a, 'ast> {
    pub renderer: &'a mut MdRender,
    pub root: &'ast AstNode<'ast>,
    /// Precise height of the virtual trailing pad row (e.g. vh/2).
    pub trailing_precise: f32,
    cursor: Cursor<'ast>,
}

#[derive(Clone, Copy)]
enum Cursor<'ast> {
    Start,
    At { node: &'ast AstNode<'ast> },
    Trailing,
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

    /// Source-text range covered by the row at the cursor's current
    /// position, or `None` if the cursor is off a real row.
    pub fn text_range(&self) -> Option<(Grapheme, Grapheme)> {
        match self.cursor {
            Cursor::At { node } => Some(self.renderer.node_range(node)),
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
                Some(first) => self.cursor = Cursor::At { node: first },
                None => self.cursor = Cursor::Trailing,
            },
            Cursor::At { node } => match node.next_sibling() {
                Some(next) => self.cursor = Cursor::At { node: next },
                None => self.cursor = Cursor::Trailing,
            },
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
                Some(last) => self.cursor = Cursor::At { node: last },
                None => {
                    self.cursor = Cursor::Start;
                    return false;
                }
            },
            Cursor::At { node } => match node.previous_sibling() {
                Some(prev) => self.cursor = Cursor::At { node: prev },
                None => {
                    self.cursor = Cursor::Start;
                    return false;
                }
            },
            Cursor::Start => return false,
        }
        true
    }

    fn approx(&self) -> f32 {
        match self.cursor {
            Cursor::At { node } => {
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
            Cursor::At { node } => {
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
            Cursor::At { node } => {
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
