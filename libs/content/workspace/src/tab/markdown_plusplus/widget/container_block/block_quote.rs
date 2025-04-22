use comrak::nodes::AstNode;
use egui::{Pos2, Rect, Stroke, TextFormat, Ui, Vec2};
use lb_rs::model::text::offset_types::{
    DocCharOffset, RangeExt as _, RangeIterExt as _, RelCharOffset,
};

use crate::tab::markdown_plusplus::{
    widget::{Wrap, BLOCK_SPACING, INDENT, ROW_SPACING},
    MarkdownPlusPlus,
};

impl<'ast> MarkdownPlusPlus {
    pub fn text_format_block_quote(&self, parent: &AstNode<'_>) -> TextFormat {
        let parent_text_format = self.text_format(parent);
        TextFormat { color: self.theme.fg().neutral_tertiary, ..parent_text_format }
    }

    pub fn height_block_quote(&self, node: &'ast AstNode<'ast>) -> f32 {
        self.height_item(node)
    }

    pub fn show_block_quote(&mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2) {
        let range = self.node_range(node); // wip

        let height = self.height_block_quote(node);
        let annotation_size = Vec2 { x: INDENT, y: height };
        let annotation_space = Rect::from_min_size(top_left, annotation_size);

        let mut annotation_top_left = top_left;
        for line_idx in self.node_lines(node).iter() {
            let mut line = self.bounds.source_lines[line_idx];
            line.0 += self.line_prefix_len(node.parent().unwrap(), line);

            // When the cursor is inside the syntax for a particular line,
            // reveal that line's syntax. This is a more subtle design than
            // revealing the syntax for the line when the cursor is anywhere
            // in the line, which itself is a more subtle design than
            // revealing the syntax for all lines when the cursor is
            // anywhere in the block quote. All designs seem viable.
            //
            // Revealing block quote syntax without revealing all syntax in the
            // block quote introduces an interesting question: where do we show
            // the `>` for the line? It depends on the heights of the
            // descendant's lines. Consider the following example:
            //
            // > Block-Quoted Setext Heading 1
            // > ===
            // > ...and a paragraph to follow!
            //
            // In any design where we want to allow a user to edit a block quote
            // without revealing the syntax for the whole block quote and
            // everything inside (which itself would present interesting
            // questions so it's not like there's a shortcut-workaround), we
            // have to account for the cumulative heights of the preceding
            // lines, which themselves could be affected by syntax reveal. In
            // the example, the setext heading's underline is only revealed when
            // the heading's range intersects the selection. Even the syntax
            // reveal state of descendant nodes must be accounted for.
        }

        // if self.node_intersects_selection(node) {
        //     for line_idx in self.node_lines(node).iter() {
        //         let mut line = self.bounds.source_lines[line_idx];
        //         line.0 += self.line_prefix_len(node.parent().unwrap(), line);
        //     }
        // } else {
        // ui.painter().vline(
        //     annotation_space.center().x,
        //     annotation_space.y_range(),
        //     Stroke::new(3., self.theme.bg().neutral_tertiary),
        // );
        // }

        // debug
        // ui.painter()
        //     .rect_stroke(annotation_space, 2., egui::Stroke::new(1., self.theme.fg().blue));
        // }

        let mut children_top_left = top_left;
        children_top_left.x += annotation_space.width();
        self.show_block_children(ui, node, children_top_left);
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

    pub fn show_line_prefix_block_quote(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, line: (DocCharOffset, DocCharOffset),
        top_left: Pos2, height: f32, _row_height: f32,
    ) {
        let annotation_size = Vec2 { x: INDENT, y: height };
        let annotation_space = Rect::from_min_size(top_left, annotation_size);
        let text_format = self.text_format_syntax(node);

        let line_prefix_len = self.line_prefix_len(node, line);
        let parent_line_prefix_len = self.line_prefix_len(node.parent().unwrap(), line);
        let prefix_range = (line.start() + parent_line_prefix_len, line.start() + line_prefix_len);

        if prefix_range.intersects(&self.buffer.current.selection, false)
            || self
                .buffer
                .current
                .selection
                .contains(prefix_range.start(), true, true)
        {
            let mut wrap = Wrap::new(INDENT);
            self.show_text_line(ui, top_left, &mut wrap, prefix_range, text_format, false);
        } else {
            ui.painter().vline(
                annotation_space.center().x,
                annotation_space
                    .y_range()
                    .expand(ROW_SPACING.max(BLOCK_SPACING) / 2.), // lazy af hack
                Stroke::new(3., self.theme.bg().neutral_tertiary),
            );
        }

        if !prefix_range.is_empty() {
            self.bounds.paragraphs.push(prefix_range);
        }
    }
}
