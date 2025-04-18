use comrak::nodes::AstNode;
use egui::{Pos2, Rect, Stroke, TextFormat, Ui, Vec2};
use lb_rs::model::text::offset_types::{DocCharOffset, RangeExt as _, RelCharOffset};

use crate::tab::markdown_plusplus::{widget::INDENT, MarkdownPlusPlus};

impl<'ast> MarkdownPlusPlus {
    pub fn text_format_block_quote(&self, parent: &AstNode<'_>) -> TextFormat {
        let parent_text_format = self.text_format(parent);
        TextFormat { color: self.theme.fg().neutral_tertiary, ..parent_text_format }
    }

    pub fn height_block_quote(&self, node: &'ast AstNode<'ast>, width: f32) -> f32 {
        self.height_item(node, width)
    }

    pub fn show_block_quote(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, mut top_left: Pos2, mut width: f32,
    ) {
        let height = self.height_block_quote(node, width);
        let annotation_size = Vec2 { x: INDENT, y: height };
        let annotation_space = Rect::from_min_size(top_left, annotation_size);

        ui.painter().vline(
            annotation_space.center().x,
            annotation_space.y_range(),
            Stroke::new(3., self.theme.bg().neutral_tertiary),
        );

        // debug
        // ui.painter()
        //     .rect_stroke(annotation_space, 2., egui::Stroke::new(1., self.theme.fg().blue));
        // }

        top_left.x += annotation_space.width();
        width -= annotation_space.width();
        self.show_block_children(ui, node, top_left, width);
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
        let mut result = self.line_prefix_len(parent, line);

        // "A block quote marker consists of 0-3 spaces of initial indent, plus
        // (a) the character > together with a following space, or (b) a single
        // character > not followed by a space."
        //
        // "If a string of lines Ls constitute a sequence of blocks Bs, then the
        // result of prepending a block quote marker to the beginning of each
        // line in Ls is a block quote containing Bs."
        let text = &self.buffer[(line.start() + self.line_prefix_len(parent, line), line.end())];
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
