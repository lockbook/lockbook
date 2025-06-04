use comrak::nodes::{AstNode, NodeValue};
use egui::{Pos2, Ui};
use lb_rs::model::text::offset_types::RangeExt as _;

use crate::tab::markdown_plusplus::widget::utils::text_layout::Wrap;
use crate::tab::markdown_plusplus::widget::BLOCK_SPACING;
use crate::tab::markdown_plusplus::MarkdownPlusPlus;

impl<'ast> MarkdownPlusPlus {
    pub fn block_pre_spacing_height(&self, node: &'ast AstNode<'ast>) -> f32 {
        let Some(parent) = node.parent() else {
            // document never spaced
            return 0.;
        };
        if matches!(node.data.borrow().value, NodeValue::TableRow(_)) {
            // table rows never spaced
            return 0.;
        }
        let width = self.width(node);

        let mut result = 0.;

        let siblings = self.sorted_siblings(node);
        let sibling_index = self.sibling_index(node, &siblings);
        let is_first_sibling = sibling_index == 0;

        let spacing_first_line = if is_first_sibling {
            // parent top -> (empty row -> spacing)* -> first sibling top
            let mut spacing_first_line = self.node_first_line_idx(parent);

            if matches!(&parent.data.borrow().value, NodeValue::Alert(_)) {
                // the first line of an alert is rendered by the alert
                spacing_first_line += 1;
            }

            spacing_first_line
        } else {
            // prev sibling bottom -> spacing -> (empty row -> spacing)* -> first sibling top
            result += BLOCK_SPACING;

            let prev_sibling = siblings[sibling_index - 1];
            self.node_last_line_idx(prev_sibling) + 1
        };
        let node_first_line = self.node_first_line_idx(node);

        // show each empty row with mapped text range
        for line_idx in spacing_first_line..node_first_line {
            let line = self.bounds.source_lines[line_idx];
            let node_line = (line.start() + self.line_prefix_len(parent, line), line.end());

            result += self.height_text_line(
                &mut Wrap::new(width),
                node_line,
                self.text_format_syntax(node),
            );

            result += BLOCK_SPACING;
        }

        result
    }

    pub fn show_block_pre_spacing(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, mut top_left: Pos2,
    ) {
        let Some(parent) = node.parent() else {
            // document never spaced
            return;
        };
        if matches!(node.data.borrow().value, NodeValue::TableRow(_)) {
            // table rows never spaced
            return;
        }
        let width = self.width(node);

        let siblings = self.sorted_siblings(node);
        let sibling_index = self.sibling_index(node, &siblings);
        let is_first_sibling = sibling_index == 0;
        let spacing_first_line = if is_first_sibling {
            // parent top -> (empty row -> spacing)* -> first sibling top
            let mut spacing_first_line = self.node_first_line_idx(parent);

            if matches!(&parent.data.borrow().value, NodeValue::Alert(_)) {
                // the first line of an alert is rendered by the alert
                spacing_first_line += 1;
            }

            spacing_first_line
        } else {
            // prev sibling bottom -> spacing -> (empty row -> spacing)* -> first sibling top
            top_left.y += BLOCK_SPACING;

            let prev_sibling = siblings[sibling_index - 1];
            self.node_last_line_idx(prev_sibling) + 1
        };
        let node_first_line = self.node_first_line_idx(node);

        // show each empty row with mapped text range
        for line_idx in spacing_first_line..node_first_line {
            let line = self.bounds.source_lines[line_idx];
            let node_line = (line.start() + self.line_prefix_len(parent, line), line.end());

            self.bounds.paragraphs.push(node_line);
            self.show_text_line(
                ui,
                top_left,
                &mut Wrap::new(width),
                node_line,
                self.text_format_syntax(node),
                false,
            );
            top_left.y += self.height_text_line(
                &mut Wrap::new(width),
                node_line,
                self.text_format_syntax(node),
            );

            top_left.y += BLOCK_SPACING;
        }
    }

    pub(crate) fn block_post_spacing_height(&self, node: &'ast AstNode<'ast>) -> f32 {
        let Some(parent) = node.parent() else {
            // document never spaced
            return 0.;
        };
        if matches!(node.data.borrow().value, NodeValue::TableRow(_)) {
            // table rows never spaced
            return 0.;
        }
        let width = self.width(node);

        let mut result = 0.;

        let siblings = self.sorted_siblings(node);
        let sibling_index = self.sibling_index(node, &siblings);
        let is_last_sibling = sibling_index == siblings.len() - 1;
        let node_last_line = self.node_last_line_idx(node);
        if !is_last_sibling {
            // lines between blocks rendered as pre-spacing
            return 0.;
        }

        // show each empty row with mapped text range
        let parent_last_line = self.node_last_line_idx(parent);
        for line_idx in (node_last_line + 1)..=parent_last_line {
            let line = self.bounds.source_lines[line_idx];
            let node_line = (line.start() + self.line_prefix_len(parent, line), line.end());

            result += BLOCK_SPACING;

            result += self.height_text_line(
                &mut Wrap::new(width),
                node_line,
                self.text_format_syntax(node),
            );
        }

        result
    }

    pub(crate) fn show_block_post_spacing(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, mut top_left: Pos2,
    ) {
        let Some(parent) = node.parent() else {
            // document never spaced
            return;
        };
        if matches!(node.data.borrow().value, NodeValue::TableRow(_)) {
            // table rows never spaced
            return;
        }
        let width = self.width(node);

        let siblings = self.sorted_siblings(node);
        let sibling_index = self.sibling_index(node, &siblings);
        let is_last_sibling = sibling_index == siblings.len() - 1;
        let node_last_line = self.node_last_line_idx(node);
        if !is_last_sibling {
            // lines between blocks rendered as pre-spacing
            return;
        }

        // show each empty row with mapped text range
        let parent_last_line = self.node_last_line_idx(parent);
        for line_idx in (node_last_line + 1)..=parent_last_line {
            let line = self.bounds.source_lines[line_idx];
            let node_line = (line.start() + self.line_prefix_len(parent, line), line.end());

            top_left.y += BLOCK_SPACING;

            self.bounds.paragraphs.push(node_line);
            self.show_text_line(
                ui,
                top_left,
                &mut Wrap::new(width),
                node_line,
                self.text_format_syntax(node),
                false,
            );
            top_left.y += self.height_text_line(
                &mut Wrap::new(width),
                node_line,
                self.text_format_syntax(node),
            );
        }
    }
}
