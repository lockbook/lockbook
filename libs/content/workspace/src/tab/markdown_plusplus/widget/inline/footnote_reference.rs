use comrak::nodes::AstNode;
use egui::{Pos2, TextFormat, Ui};

use crate::tab::markdown_plusplus::{widget::WrapContext, MarkdownPlusPlus};

impl<'ast> MarkdownPlusPlus {
    pub fn text_format_footnote_reference(&self, parent: &AstNode<'_>) -> TextFormat {
        TextFormat {
            color: self.theme.fg().neutral_tertiary,
            ..self.text_format_superscript(parent)
        }
    }

    pub fn span_footnote_reference(
        &self, node: &'ast AstNode<'ast>, wrap: &WrapContext, ix: u32,
    ) -> f32 {
        let text = format!("{}", ix);
        self.span_node_text_line(node, wrap, &text)
    }

    pub fn show_footnote_reference(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2, wrap: &mut WrapContext,
    ) {
        // [^footnotereference]
        let mut sourcepos = node.data.borrow().sourcepos;
        sourcepos.start.column += 2;
        sourcepos.end.column -= 1;

        self.show_node_text_line(ui, node, top_left, wrap, self.sourcepos_to_range(sourcepos));
    }
}
