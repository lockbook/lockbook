use std::f32;

use comrak::nodes::AstNode;
use egui::{Pos2, Ui};
use lb_rs::model::text::offset_types::{IntoRangeExt as _, RangeExt as _};

use crate::tab::markdown_plusplus::MarkdownPlusPlus;

use super::{Wrap, BLOCK_SPACING, ROW_HEIGHT};

impl<'ast> MarkdownPlusPlus {
    pub(crate) fn block_pre_spacing_height(&self, node: &'ast AstNode<'ast>) -> f32 {
        let parent = node.parent().unwrap();
        let node_first_line = self.node_first_line_idx(node);

        let range = self.node_range(node);
        let siblings = self.sorted_siblings(node);
        let this_sibling_index = siblings
            .iter()
            .position(|sibling| self.node_range(sibling) == range)
            .unwrap();

        let spacing_first_line = if this_sibling_index > 0 {
            let prev_sibling = siblings[this_sibling_index - 1];
            self.node_last_line_idx(prev_sibling) + 1 // exclude sibling last line
        } else {
            self.node_first_line_idx(parent) // include parent first line
        };

        let num_empty_lines = node_first_line - spacing_first_line;
        let num_spacings = num_empty_lines + 1;

        num_empty_lines as f32 * ROW_HEIGHT + num_spacings as f32 * BLOCK_SPACING
    }

    pub(crate) fn show_block_pre_spacing(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, mut top_left: Pos2,
    ) {
        let parent = node.parent().unwrap();
        let node_first_line = self.node_first_line_idx(node);
        let width = self.width(node);

        let range = self.node_range(node);
        let siblings = self.sorted_siblings(node);
        let this_sibling_index = siblings
            .iter()
            .position(|sibling| self.node_range(sibling) == range)
            .unwrap();

        let spacing_first_line = if this_sibling_index > 0 {
            let prev_sibling = siblings[this_sibling_index - 1];
            self.node_last_line_idx(prev_sibling) + 1 // exclude sibling last line
        } else {
            self.node_first_line_idx(parent) // include parent first line
        };

        top_left.y += BLOCK_SPACING;

        for line_idx in spacing_first_line..node_first_line {
            let line = self.bounds.source_lines[line_idx];
            self.show_line_prefix(ui, parent, line, top_left, ROW_HEIGHT, ROW_HEIGHT);

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
        let parent = node.parent().unwrap();
        let node_last_line = self.node_last_line_idx(node);

        let range = self.node_range(node);
        let siblings = self.sorted_siblings(node);
        let this_sibling_index = siblings
            .iter()
            .position(|sibling| self.node_range(sibling) == range)
            .unwrap();

        let spacing_last_line = if this_sibling_index < siblings.len() - 1 {
            // lines between blocks rendered as pre-spacing
            return 0.;
        } else {
            self.node_last_line_idx(parent)
        };

        let num_empty_lines = spacing_last_line - node_last_line;
        let num_spacings = num_empty_lines + 1;

        num_empty_lines as f32 * ROW_HEIGHT + num_spacings as f32 * BLOCK_SPACING
    }

    pub(crate) fn show_block_post_spacing(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, mut top_left: Pos2,
    ) {
        let parent = node.parent().unwrap();
        let node_last_line = self.node_last_line_idx(node);
        let width = self.width(node);

        let range = self.node_range(node);
        let siblings = self.sorted_siblings(node);
        let this_sibling_index = siblings
            .iter()
            .position(|sibling| self.node_range(sibling) == range)
            .unwrap();

        let spacing_last_line = if this_sibling_index < siblings.len() - 1 {
            // lines between blocks rendered as pre-spacing
            return;
        } else {
            self.node_last_line_idx(parent)
        };

        top_left.y += BLOCK_SPACING;

        for line_idx in node_last_line + 1..=spacing_last_line {
            let line = self.bounds.source_lines[line_idx];
            self.show_line_prefix(ui, parent, line, top_left, ROW_HEIGHT, ROW_HEIGHT);

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
}
