use comrak::nodes::AstNode;
use lb_rs::model::text::offset_types::Grapheme;

use crate::tab::markdown_editor::MdRender;
use crate::tab::markdown_editor::widget::utils::wrap_layout::{Format, Layout};

impl<'ast> MdRender {
    pub fn text_format_subscript(&self, parent: &AstNode<'_>) -> Format {
        let parent_text_format = self.text_format(parent);
        Format { subscript: true, ..parent_text_format }
    }

    pub fn layout_subscript(
        &self, layout: &mut Layout, node: &'ast AstNode<'ast>, range: (Grapheme, Grapheme),
    ) {
        let fmt = self.text_format_subscript(node.parent().unwrap());
        self.layout_circumfix(layout, node, range, fmt);
    }
}
