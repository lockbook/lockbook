use comrak::nodes::AstNode;
use egui::{Pos2, Rect, Stroke, TextFormat, Ui, Vec2};
use lb_rs::model::text::offset_types::{
    DocCharOffset, IntoRangeExt as _, RangeExt as _, RangeIterExt as _, RelCharOffset,
};

use crate::tab::markdown_plusplus::{
    widget::{Wrap, BLOCK_SPACING, INDENT, ROW_HEIGHT},
    MarkdownPlusPlus,
};

impl<'ast> MarkdownPlusPlus {
    pub fn text_format_block_quote(&self, parent: &AstNode<'_>) -> TextFormat {
        let parent_text_format = self.text_format(parent);
        TextFormat { color: self.theme.fg().neutral_tertiary, ..parent_text_format }
    }

    pub fn height_block_quote(&self, node: &'ast AstNode<'ast>) -> f32 {
        let any_children = node.children().next().is_some();
        if any_children {
            self.block_children_height(node)
        } else {
            let num_lines = self.node_lines(node).len();
            num_lines as f32 * ROW_HEIGHT + num_lines.saturating_sub(1) as f32 * BLOCK_SPACING
        }
    }

    pub fn show_block_quote(&mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, mut top_left: Pos2) {
        let height = self.height(node);
        let annotation_size = Vec2 { x: INDENT, y: height };
        let annotation_space = Rect::from_min_size(top_left, annotation_size);

        ui.painter().vline(
            annotation_space.center().x,
            annotation_space.y_range(),
            Stroke::new(3., self.theme.bg().neutral_tertiary),
        );
        for line_idx in self.node_lines(node).iter() {
            let line = self.bounds.source_lines[line_idx];

            let prefix_len = self.line_prefix_len(node, line);
            let parent_prefix_len = self.line_prefix_len(node.parent().unwrap(), line);
            let prefix = (line.start() + parent_prefix_len, line.start() + prefix_len);

            println!("paragraphs.push({:?})", prefix);
            self.bounds.paragraphs.push(prefix);
        }

        top_left.x += annotation_space.width();

        let any_children = node.children().next().is_some();
        if any_children {
            self.show_block_children(ui, node, top_left);
        } else {
            for line in self.node_lines(node).iter() {
                let line = self.bounds.source_lines[line];
                let node_line = (line.start() + self.line_prefix_len(node, line), line.end());
                let node_line_start = node_line.start().into_range();

                self.show_text_line(
                    ui,
                    top_left,
                    &mut Wrap::new(self.width(node)),
                    node_line_start,
                    self.text_format_document(),
                    false,
                );
                top_left.y += ROW_HEIGHT + BLOCK_SPACING;
            }
        }
    }

    // This routine is standard-/reference-complexity, as the prefix len is
    // line-by-line (unlike list items) and block quotes contain multiline text,
    // so they are their own client. Most of the fundamental behavior with line
    // prefix lengths can be observed with block quotes alone.
    //
    // This implementation does benefit from the simplicity of the node - there
    // are only 8 cases.
    pub fn line_prefix_len_block_quote(
        &self, node: &'ast AstNode<'ast>, line: (DocCharOffset, DocCharOffset),
    ) -> RelCharOffset {
        let parent = node.parent().unwrap();
        let parent_prefix_len = self.line_prefix_len(parent, line);
        let mut result = parent_prefix_len;

        // "A block quote marker consists of 0-3 spaces of initial indent, plus
        // (a) the character > together with a following space, or (b) a single
        // character > not followed by a space."
        //
        // "If a string of lines Ls constitute a sequence of blocks Bs, then the
        // result of prepending a block quote marker to the beginning of each
        // line in Ls is a block quote containing Bs."
        let text = &self.buffer[(line.start() + parent_prefix_len, line.end())];
        if text.starts_with("   > ") {
            result += 5;
        } else if text.starts_with("   >") || text.starts_with("  > ") {
            result += 4;
        } else if text.starts_with("  >") || text.starts_with(" > ") {
            result += 3;
        } else if text.starts_with(" >") || text.starts_with("> ") {
            result += 2;
        } else if text.starts_with(">") {
            result += 1;
        }

        // "If a string of lines Ls constitute a block quote with contents Bs,
        // then the result of deleting the initial block quote marker from one
        // or more lines in which the next non-whitespace character after the
        // block quote marker is paragraph continuation text is a block quote
        // with Bs as its content."
        result.min(line.len())
    }
}
