use comrak::nodes::AstNode;
use egui::{Pos2, Ui};
use lb_rs::model::text::offset_types::{IntoRangeExt as _, RangeExt as _};

use crate::tab::markdown_plusplus::MarkdownPlusPlus;

use super::{Wrap, BLOCK_SPACING, ROW_HEIGHT};

impl<'ast> MarkdownPlusPlus {
    fn sibling_index(
        &self, node: &'ast AstNode<'ast>, sorted_siblings: &[&'ast AstNode<'ast>],
    ) -> usize {
        let range = self.node_range(node);
        let this_sibling_index = sorted_siblings
            .iter()
            .position(|sibling| self.node_range(sibling) == range)
            .unwrap();

        this_sibling_index
    }

    pub fn block_pre_spacing_height(&self, node: &'ast AstNode<'ast>) -> f32 {
        let Some(parent) = node.parent() else {
            // document never spaced
            return 0.;
        };

        let siblings = self.sorted_siblings(node);
        let sibling_index = self.sibling_index(node, &siblings);
        let is_first_sibling = sibling_index == 0;
        let node_first_line = self.node_first_line_idx(node);
        if is_first_sibling {
            // parent top -> (empty row -> spacing)* -> first sibling top
            let parent_first_line = self.node_first_line_idx(parent);
            let empty_lines = node_first_line - parent_first_line;
            empty_lines as f32 * (ROW_HEIGHT + BLOCK_SPACING)
        } else {
            // prev sibling bottom -> spacing -> (empty row -> spacing)* -> first sibling top
            let prev_sibling = siblings[sibling_index - 1];
            let prev_sibling_last_line = self.node_last_line_idx(prev_sibling);
            let empty_lines = node_first_line - (prev_sibling_last_line + 1);
            BLOCK_SPACING + empty_lines as f32 * (ROW_HEIGHT + BLOCK_SPACING)
        }
    }

    pub fn show_block_pre_spacing(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, mut top_left: Pos2,
    ) {
        let Some(parent) = node.parent() else {
            // document never spaced
            return;
        };
        let width = self.width(node);

        let siblings = self.sorted_siblings(node);
        let sibling_index = self.sibling_index(node, &siblings);
        let is_first_sibling = sibling_index == 0;
        let node_first_line = self.node_first_line_idx(node);
        let spacing_first_line = if is_first_sibling {
            // parent top -> (empty row -> spacing)* -> first sibling top
            self.node_first_line_idx(parent)
        } else {
            // prev sibling bottom -> spacing -> (empty row -> spacing)* -> first sibling top
            top_left.y += BLOCK_SPACING;

            let prev_sibling = siblings[sibling_index - 1];
            self.node_last_line_idx(prev_sibling) + 1
        };

        // show each empty row with mapped text range
        for line_idx in spacing_first_line..node_first_line {
            let line = self.bounds.source_lines[line_idx];

            let node_line = (line.start() + self.line_prefix_len(parent, line), line.end());
            let node_line_start = node_line.start().into_range();

            self.show_text_line(
                ui,
                top_left,
                &mut Wrap::new(width),
                node_line_start,
                self.text_format_document(),
                false,
            );
            self.bounds.paragraphs.push(node_line_start);

            top_left.y += ROW_HEIGHT;
            top_left.y += BLOCK_SPACING;
        }
    }

    pub(crate) fn block_post_spacing_height(&self, node: &'ast AstNode<'ast>) -> f32 {
        let Some(parent) = node.parent() else {
            // document never spaced
            return 0.;
        };

        let siblings = self.sorted_siblings(node);
        let sibling_index = self.sibling_index(node, &siblings);
        let is_last_sibling = sibling_index == siblings.len() - 1;
        let node_last_line = self.node_last_line_idx(node);
        if !is_last_sibling {
            // lines between blocks rendered as pre-spacing
            return 0.;
        }

        // last sibling bottom -> (empty row -> spacing)* -> parent bottom
        let parent_last_line = self.node_last_line_idx(parent);
        let empty_lines = parent_last_line - node_last_line;
        empty_lines as f32 * (ROW_HEIGHT + BLOCK_SPACING)
    }

    pub(crate) fn show_block_post_spacing(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, mut top_left: Pos2,
    ) {
        let Some(parent) = node.parent() else {
            // document never spaced
            return;
        };
        let node_last_line = self.node_last_line_idx(node);
        let width = self.width(node);

        let siblings = self.sorted_siblings(node);
        let sibling_index = self.sibling_index(node, &siblings);
        let is_last_sibling = sibling_index == siblings.len() - 1;
        if !is_last_sibling {
            // lines between blocks rendered as pre-spacing
            return;
        }

        // show each empty row with mapped text range
        let parent_last_line = self.node_last_line_idx(parent);
        for line_idx in (node_last_line + 1)..=parent_last_line {
            let line = self.bounds.source_lines[line_idx];

            let node_line = (line.start() + self.line_prefix_len(parent, line), line.end());
            let node_line_start = node_line.start().into_range();

            top_left.y += BLOCK_SPACING;

            self.show_text_line(
                ui,
                top_left,
                &mut Wrap::new(width),
                node_line_start,
                self.text_format_document(),
                false,
            );
            self.bounds.paragraphs.push(node_line_start);

            top_left.y += ROW_HEIGHT;
        }
    }
}
