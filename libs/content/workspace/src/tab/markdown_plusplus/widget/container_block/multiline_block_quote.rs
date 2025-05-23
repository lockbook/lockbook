use comrak::nodes::{AstNode, NodeMultilineBlockQuote};
use egui::{Pos2, TextFormat, Ui};
use lb_rs::model::text::offset_types::{DocCharOffset, RangeExt as _, RelCharOffset};

use crate::tab::markdown_plusplus::MarkdownPlusPlus;

impl<'ast> MarkdownPlusPlus {
    pub fn text_format_multiline_block_quote(&self, parent: &AstNode<'_>) -> TextFormat {
        self.text_format_block_quote(parent)
    }

    pub fn height_multiline_block_quote(&self, node: &'ast AstNode<'ast>) -> f32 {
        self.height_block_quote(node)
    }

    pub fn show_multiline_block_quote(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2,
    ) {
        self.show_block_quote(ui, node, top_left);
    }

    // This routine only slightly more complex than regular block quotes beacuse
    // closing fences must be at least as long as opening fences. A clean spec
    // is not available because these are a GFM extension, so assumptions are
    // made with experimental verification - 0-3 spaces indentation, with fence
    // mechanics like fenced code blocks.
    pub fn line_prefix_len_multiline_block_quote(
        &self, node: &'ast AstNode<'ast>, node_multiline_block_quote: &NodeMultilineBlockQuote,
        line: (DocCharOffset, DocCharOffset),
    ) -> RelCharOffset {
        // todo: not last line, but closing fence line (is the quote closed?)
        let NodeMultilineBlockQuote { fence_length, .. } = *node_multiline_block_quote;

        // "The content of the code block consists of all subsequent lines,
        // until a closing code fence of the same type as the code block began
        // with (backticks or tildes), and with at least as many backticks or
        // tildes as the opening code fence."
        let node_line = self.node_line(node, line);
        let fence_str = ">".repeat(fence_length);

        let text = &self.buffer[self.node_line(node, line)];
        let is_opening_fence = line == self.node_first_line(node);
        let is_closing_fence = line == self.node_last_line(node) && text.starts_with(&fence_str);

        if is_opening_fence || is_closing_fence {
            return line.len();
        }

        let prefix_len = if text.starts_with("   ") {
            3
        } else if text.starts_with("  ") {
            2
        } else if text.starts_with(" ") {
            1
        } else {
            0
        };

        // (parent_prefix_len + prefix_len).min(line.len())
        todo!()
    }
}
