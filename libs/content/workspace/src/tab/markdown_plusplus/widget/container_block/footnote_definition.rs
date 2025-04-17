use comrak::nodes::{AstNode, ListType, NodeValue};
use egui::{Pos2, TextFormat, Ui};

use crate::tab::markdown_plusplus::MarkdownPlusPlus;

impl<'ast> MarkdownPlusPlus {
    pub fn text_format_footnote_definition(&self, parent: &AstNode<'_>) -> TextFormat {
        let parent_text_format = self.text_format(parent);
        TextFormat { color: self.theme.fg().neutral_tertiary, ..parent_text_format }
    }

    pub fn height_footnote_definition(&self, node: &'ast AstNode<'ast>, width: f32) -> f32 {
        self.height_item(node, width)
    }

    pub fn show_footnote_definition(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2, width: f32,
    ) {
        self.show_item(ui, node, top_left, width, ListType::Ordered, self.definition_number(node));
    }

    /// Footnote definitions are usually rendered in the order in which they're
    /// referenced, rather than the order in which they're written ('source
    /// order'). From where I'm sitting, I just don't see how that works in an
    /// interactive editor, even though I'm quite committed to rendering to
    /// spec, so we render them in source order.
    ///
    /// One aspect of footnote rendering we can support, though, is rendering
    /// footnote references and the labels for definitions as a number
    /// representing the order that they're referenced. So, the first
    /// _reference_ will always be rendered with a superscript '1', the next
    /// with a '2', etc, regardless the order the _definitions_ are written.
    /// This creates a unique editing experience because the rendered reference
    /// number is not from the source text; it changes to the reference text
    /// only when you select it.
    ///
    /// In the AST, it turns out, the nodes are not presented in source order
    /// when it comes to footnote definitions. Instead, they're presented in
    /// reference order. At first, I
    /// [thought](https://github.com/kivikakk/comrak/issues/554) this a bug, but
    /// it's intended behavior. This is why [`Self::sorted_siblings`] exists.
    /// Anyway, we leverage this behavior to determine the number we should
    /// render for the definition, since the node itself does not contain it.
    fn definition_number(&self, node: &'ast AstNode<'ast>) -> usize {
        let mut result = 0;
        let document = node.ancestors().last().expect("There is always a document");
        for descendant in document.descendants() {
            if matches!(descendant.data.borrow().value, NodeValue::FootnoteDefinition(_)) {
                result += 1;
            }
            if descendant.data.borrow().sourcepos == node.data.borrow().sourcepos {
                return result;
            }
        }
        unreachable!("All nodes are somewhere in the document");
    }
}
