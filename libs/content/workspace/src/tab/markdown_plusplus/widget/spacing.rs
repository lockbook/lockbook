use std::f32;

use comrak::nodes::{AstNode, NodeValue};
use egui::{Pos2, Ui};
use lb_rs::model::text::offset_types::IntoRangeExt as _;

use crate::tab::markdown_plusplus::MarkdownPlusPlus;

use super::{inline::text, WrapContext, ROW_HEIGHT, ROW_SPACING};

impl<'ast> MarkdownPlusPlus {
    pub(crate) fn block_pre_spacing_height(&self, node: &'ast AstNode<'ast>) -> f32 {
        let (lines, spacing) = self.pre_spacing(node);
        spacing + lines as f32 * (ROW_HEIGHT + ROW_SPACING)
    }

    pub(crate) fn show_block_pre_spacing(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, mut top_left: Pos2, width: f32,
    ) {
        let (lines, spacing) = self.pre_spacing(node);

        top_left.y += spacing;

        let mut line_column = node.data.borrow().sourcepos.start;
        line_column.line -= lines;
        let mut offset = self.line_column_to_offset(line_column);
        for _ in 0..lines {
            let range = offset.into_range();
            self.show_text_line(
                ui,
                node,
                top_left,
                &mut WrapContext::new(width),
                range,
                Some(self.theme.fg().neutral_quarternary),
            );
            self.bounds.paragraphs.push(range);

            top_left.y += ROW_HEIGHT;
            top_left.y += ROW_SPACING;
            offset += 1;
        }
    }

    pub(crate) fn block_post_spacing_height(&self, node: &'ast AstNode<'ast>) -> f32 {
        let lines = self.post_spacing(node);
        lines as f32 * (ROW_SPACING + ROW_HEIGHT)
    }

    pub(crate) fn show_block_post_spacing(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, mut top_left: Pos2, width: f32,
    ) {
        let lines = self.post_spacing(node);

        let line_column = node.data.borrow().sourcepos.end;
        let mut offset = self.line_column_to_offset(line_column) + 2; // +1 to convert to exclusive, +1 to skip a newline
        for _ in 0..lines {
            top_left.y += ROW_SPACING;

            let range = offset.into_range();
            self.bounds.paragraphs.push(range);
            self.show_text_line(
                ui,
                node,
                top_left,
                &mut WrapContext::new(width),
                range,
                Some(self.theme.fg().neutral_quarternary),
            );

            top_left.y += ROW_HEIGHT;
            offset += 1;
        }
    }

    fn pre_spacing(&self, node: &'ast AstNode<'ast>) -> (usize, f32) {
        let sourcepos = node.data.borrow().sourcepos;
        let value = &node.data.borrow().value;

        let mut spacing = 0.;

        // spacing based on lines between this block and the previous block
        let siblings = self.sorted_siblings(node);
        let this_sibling_index = siblings
            .iter()
            .position(|sibling| sibling.data.borrow().sourcepos == sourcepos)
            .unwrap();

        let empty_line_count = if this_sibling_index > 0 {
            let prev_sibling = siblings[this_sibling_index - 1];

            // space between two siblings
            spacing = ROW_SPACING;

            let prev_sibling_sourcepos = prev_sibling.data.borrow().sourcepos;
            let line_count = sourcepos.start.line - prev_sibling_sourcepos.end.line;
            line_count.saturating_sub(1) // one of these is just the line break
        } else if let Some(parent) = node.parent() {
            // space between the start of the parent and the first sibling
            let parent_sourcepos = parent.data.borrow().sourcepos;
            sourcepos.start.line - parent_sourcepos.start.line
        } else {
            unreachable!("spacing not evaluated for document")
        };

        if let NodeValue::TableRow(_) = value {
            (0, 0.) // no spacing before (or after) table rows
        } else {
            (empty_line_count, spacing)
        }
    }

    fn post_spacing(&self, node: &'ast AstNode<'ast>) -> usize {
        let sourcepos = node.data.borrow().sourcepos;

        // spacing based on lines between this block and the next block
        let siblings = self.sorted_siblings(node);
        let this_sibling_index = siblings
            .iter()
            .position(|sibling| sibling.data.borrow().sourcepos == sourcepos)
            .unwrap();

        if this_sibling_index != siblings.len() - 1 {
            0 // space between two siblings rendered as pre-spacing of the next sibling
        } else if let Some(parent) = node.parent() {
            // space between the last sibling and the end of the parent
            let parent_is_document = parent.parent().is_none();
            let parent_sourcepos = parent.data.borrow().sourcepos;
            let mut empty_line_count = parent_sourcepos.end.line - sourcepos.end.line;
            if parent_is_document && text::ends_with_newline(&self.buffer.current.text) {
                empty_line_count += 1;
            }
            empty_line_count
        } else {
            unreachable!("spacing not evaluated for document")
        }
    }

    fn sorted_siblings(&self, node: &'ast AstNode<'ast>) -> Vec<&'ast AstNode<'ast>> {
        let mut preceding_siblings = node.preceding_siblings();
        preceding_siblings.next().unwrap(); // "Call .next().unwrap() once on the iterator to skip the node itself."

        let mut following_siblings = node.following_siblings();
        following_siblings.next().unwrap(); // "Call .next().unwrap() once on the iterator to skip the node itself."

        let mut siblings = Vec::new();
        siblings.extend(preceding_siblings);
        siblings.push(node);
        siblings.extend(following_siblings);
        siblings.sort_by(|a, b| {
            a.data
                .borrow()
                .sourcepos
                .start
                .line
                .cmp(&b.data.borrow().sourcepos.start.line)
        });
        siblings
    }
}
