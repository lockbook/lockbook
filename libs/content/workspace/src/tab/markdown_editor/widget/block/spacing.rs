use std::ops::Range;

use comrak::nodes::{AstNode, NodeValue};
use egui::{Pos2, Ui};
use lb_rs::model::text::offset_types::{Grapheme, RangeExt as _};

use crate::tab::markdown_editor::MdRender;

impl<'ast> MdRender {
    /// Converts an optional source line index range to a doc char offset range.
    pub fn spacing_range(&self, line_range: &Option<Range<usize>>) -> (Grapheme, Grapheme) {
        match line_range {
            Some(r) if !r.is_empty() => {
                let start = self.bounds.source_lines[r.start].start();
                let end = self.bounds.source_lines[r.end - 1].end();
                (start, end)
            }
            _ => (Grapheme(usize::MAX), Grapheme(0)),
        }
    }

    /// Returns the source line index range for the pre-spacing of `node`, i.e.
    /// the empty lines between the previous sibling (or parent start) and this
    /// node. Returns `None` when there is no pre-spacing (document root, table
    /// rows, folded nodes).
    pub fn pre_spacing_lines(&self, node: &'ast AstNode<'ast>) -> Option<Range<usize>> {
        let parent = node.parent()?;
        if matches!(node.data.borrow().value, NodeValue::TableRow(_)) {
            return None;
        }
        if self.hidden_by_fold(node) {
            return None;
        }

        let first = match node.previous_sibling() {
            None => {
                let mut first = self.node_first_line_idx(parent);
                if matches!(&parent.data.borrow().value, NodeValue::Alert(_)) {
                    first += 1;
                }
                first
            }
            Some(prev) => self.node_last_line_idx(prev) + 1,
        };

        Some(first..self.node_first_line_idx(node))
    }

    /// Returns the source line index range for the post-spacing of `node`, i.e.
    /// the empty lines between this node and the parent's end. Only the last
    /// sibling has post-spacing; returns `None` otherwise.
    pub fn post_spacing_lines(&self, node: &'ast AstNode<'ast>) -> Option<Range<usize>> {
        let parent = node.parent()?;
        if matches!(node.data.borrow().value, NodeValue::TableRow(_)) {
            return None;
        }

        if node.next_sibling().is_some() {
            return None;
        }

        let first = self.node_last_line_idx(node) + 1;
        let last = self.node_last_line_idx(parent);
        Some(first..(last + 1))
    }

    /// Cheap pre-spacing height estimate. Each blank line is one
    /// `row_height`; plus inter-block `block_spacing`. Used by
    /// `height_approx` for off-screen content.
    pub fn block_pre_spacing_height_approx(&self, node: &'ast AstNode<'ast>) -> f32 {
        if self.hidden_by_fold(node) {
            return 0.;
        }
        let Some(line_range) = self.pre_spacing_lines(node) else {
            return 0.;
        };
        let mut result = 0.;
        if node.previous_sibling().is_some() {
            result += self.layout.block_spacing;
        }
        let n = line_range.end.saturating_sub(line_range.start) as f32;
        result += n * self.layout.row_height;
        result += n * self.layout.block_spacing;
        result
    }

    /// Cheap post-spacing height estimate.
    pub fn block_post_spacing_height_approx(&self, node: &'ast AstNode<'ast>) -> f32 {
        let Some(line_range) = self.post_spacing_lines(node) else {
            return 0.;
        };
        let n = line_range.end.saturating_sub(line_range.start) as f32;
        n * self.layout.row_height + n * self.layout.block_spacing
    }

    pub fn block_pre_spacing_height(&self, node: &'ast AstNode<'ast>) -> f32 {
        let Some(line_range) = self.pre_spacing_lines(node) else {
            return 0.;
        };

        let width = self.width(node);
        let mut result = 0.;

        if node.previous_sibling().is_some() {
            result += self.layout.block_spacing;
        }

        for line_idx in line_range {
            let line = self.bounds.source_lines[line_idx];
            let node_line = self.node_line(node, line);

            result += self.height_section(
                &mut self.new_wrap(width),
                node_line,
                self.text_format_document(),
            );
            result += self.layout.block_spacing;
        }

        result
    }

    pub fn show_block_pre_spacing(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, mut top_left: Pos2,
    ) {
        let Some(line_range) = self.pre_spacing_lines(node) else {
            return;
        };

        let width = self.width(node);

        if node.previous_sibling().is_some() {
            top_left.y += self.layout.block_spacing;
        }

        for line_idx in line_range {
            let line = self.bounds.source_lines[line_idx];
            let node_line = self.node_line(node, line);

            let mut wrap = self.new_wrap(width);
            self.show_section(ui, top_left, &mut wrap, node_line, self.text_format_document());
            top_left.y += wrap.height();
            top_left.y += self.layout.block_spacing;
            self.bounds.wrap_lines.extend(wrap.row_ranges);
        }
    }

    pub(crate) fn block_post_spacing_height(&self, node: &'ast AstNode<'ast>) -> f32 {
        let Some(line_range) = self.post_spacing_lines(node) else {
            return 0.;
        };

        let width = self.width(node);
        let mut result = 0.;

        for line_idx in line_range {
            let line = self.bounds.source_lines[line_idx];
            let node_line = self.node_line(node, line);

            result += self.layout.block_spacing;

            result += self.height_section(
                &mut self.new_wrap(width),
                node_line,
                self.text_format_document(),
            );
        }

        result
    }

    pub(crate) fn show_block_post_spacing(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, mut top_left: Pos2,
    ) {
        let Some(line_range) = self.post_spacing_lines(node) else {
            return;
        };

        let width = self.width(node);

        for line_idx in line_range {
            let line = self.bounds.source_lines[line_idx];
            let node_line = self.node_line(node, line);

            top_left.y += self.layout.block_spacing;

            let mut wrap = self.new_wrap(width);
            self.show_section(ui, top_left, &mut wrap, node_line, self.text_format_document());
            top_left.y += wrap.height();
            self.bounds.wrap_lines.extend(wrap.row_ranges);
        }
    }

    pub fn compute_bounds_block_pre_spacing(&mut self, node: &'ast AstNode<'ast>) {
        let Some(line_range) = self.pre_spacing_lines(node) else {
            return;
        };

        for line_idx in line_range {
            let line = self.bounds.source_lines[line_idx];
            let node_line = self.node_line(node, line);
            self.bounds.inline_paragraphs.push(node_line);
        }
    }

    pub(crate) fn compute_bounds_block_post_spacing(&mut self, node: &'ast AstNode<'ast>) {
        let Some(line_range) = self.post_spacing_lines(node) else {
            return;
        };

        for line_idx in line_range {
            let line = self.bounds.source_lines[line_idx];
            let node_line = self.node_line(node, line);
            self.bounds.inline_paragraphs.push(node_line);
        }
    }
}
