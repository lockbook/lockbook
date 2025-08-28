use comrak::nodes::{AstNode, NodeValue};
use egui::{Pos2, Ui};

use crate::tab::markdown_editor::Editor;
use crate::tab::markdown_editor::widget::BLOCK_SPACING;
use crate::tab::markdown_editor::widget::utils::text_layout::Wrap;

impl<'ast> Editor {
    pub fn block_pre_spacing_height(
        &self, node: &'ast AstNode<'ast>, siblings: &[&'ast AstNode<'ast>],
    ) -> f32 {
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

        let sibling_index = self.sibling_index(node, siblings);
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
            let node_line = self.node_line(node, line);

            result += self.height_text_line(
                &mut Wrap::new(width),
                node_line,
                self.text_format_document(),
            );
            result += BLOCK_SPACING;
        }

        result
    }

    pub fn show_block_pre_spacing(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, mut top_left: Pos2,
        siblings: &[&'ast AstNode<'ast>],
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

        let sibling_index = self.sibling_index(node, siblings);
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
            let node_line = self.node_line(node, line);

            let mut wrap = Wrap::new(width);
            self.show_text_line(
                ui,
                top_left,
                &mut wrap,
                node_line,
                self.text_format_document(),
                false,
            );
            top_left.y += wrap.height();
            top_left.y += BLOCK_SPACING;
            self.bounds.wrap_lines.extend(wrap.row_ranges);
        }
    }

    pub(crate) fn block_post_spacing_height(
        &self, node: &'ast AstNode<'ast>, siblings: &[&'ast AstNode<'ast>],
    ) -> f32 {
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

        let sibling_index = self.sibling_index(node, siblings);
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
            let node_line = self.node_line(node, line);

            result += BLOCK_SPACING;

            result += self.height_text_line(
                &mut Wrap::new(width),
                node_line,
                self.text_format_document(),
            );
        }

        result
    }

    pub(crate) fn show_block_post_spacing(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, mut top_left: Pos2,
        siblings: &[&'ast AstNode<'ast>],
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

        let sibling_index = self.sibling_index(node, siblings);
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
            let node_line = self.node_line(node, line);

            top_left.y += BLOCK_SPACING;

            let mut wrap = Wrap::new(width);
            self.show_text_line(
                ui,
                top_left,
                &mut wrap,
                node_line,
                self.text_format_document(),
                false,
            );
            top_left.y += wrap.height();
            self.bounds.wrap_lines.extend(wrap.row_ranges);
        }
    }

    pub fn compute_bounds_block_pre_spacing(
        &mut self, node: &'ast AstNode<'ast>, siblings: &[&'ast AstNode<'ast>],
    ) {
        let Some(parent) = node.parent() else {
            // document never spaced
            return;
        };
        if matches!(node.data.borrow().value, NodeValue::TableRow(_)) {
            // table rows never spaced
            return;
        }

        let sibling_index = self.sibling_index(node, siblings);
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
            let prev_sibling = siblings[sibling_index - 1];
            self.node_last_line_idx(prev_sibling) + 1
        };
        let node_first_line = self.node_first_line_idx(node);

        // compute bounds for each empty row with mapped text range
        for line_idx in spacing_first_line..node_first_line {
            let line = self.bounds.source_lines[line_idx];
            let node_line = self.node_line(node, line);
            self.bounds.paragraphs.push(node_line);
            self.bounds.inline_paragraphs.push(node_line);
        }
    }

    pub(crate) fn compute_bounds_block_post_spacing(
        &mut self, node: &'ast AstNode<'ast>, siblings: &[&'ast AstNode<'ast>],
    ) {
        let Some(parent) = node.parent() else {
            // document never spaced
            return;
        };
        if matches!(node.data.borrow().value, NodeValue::TableRow(_)) {
            // table rows never spaced
            return;
        }

        let sibling_index = self.sibling_index(node, siblings);
        let is_last_sibling = sibling_index == siblings.len() - 1;
        let node_last_line = self.node_last_line_idx(node);
        if !is_last_sibling {
            // lines between blocks rendered as pre-spacing
            return;
        }

        // compute bounds for each empty row with mapped text range
        let parent_last_line = self.node_last_line_idx(parent);
        for line_idx in (node_last_line + 1)..=parent_last_line {
            let line = self.bounds.source_lines[line_idx];
            let node_line = self.node_line(node, line);
            self.bounds.paragraphs.push(node_line);
            self.bounds.inline_paragraphs.push(node_line);
        }
    }
}
