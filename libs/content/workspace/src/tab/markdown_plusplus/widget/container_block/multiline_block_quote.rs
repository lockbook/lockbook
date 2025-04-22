use comrak::nodes::AstNode;
use egui::{Pos2, TextFormat, Ui};
use lb_rs::model::text::offset_types::{DocCharOffset, RangeExt as _, RelCharOffset};

use crate::tab::markdown_plusplus::{widget::Wrap, MarkdownPlusPlus};

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
    //
    // Cleverly, however, we do not need to support calls on the code fence
    // lines. This is because never will a nested block have text on those
    // lines.
    pub fn line_prefix_len_multiline_block_quote(
        &self, node: &'ast AstNode<'ast>, line: (DocCharOffset, DocCharOffset),
    ) -> RelCharOffset {
        let parent = node.parent().unwrap();
        let mut result = self.line_prefix_len(parent, line);

        let text = &self.buffer[(line.start() + self.line_prefix_len(parent, line), line.end())];
        if text.starts_with("   ") {
            result += 3;
        } else if text.starts_with("  ") {
            result += 2;
        } else if text.starts_with(" ") {
            result += 1;
        }

        result.min(line.len())
    }

    pub fn show_line_prefix_multiline_block_quote(
        &self, ui: &mut Ui, node: &'ast AstNode<'ast>, line: (DocCharOffset, DocCharOffset),
        top_left: Pos2, height: f32, row_height: f32,
    ) {
        todo!()
    }
}
