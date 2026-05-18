use comrak::nodes::AstNode;
use lb_rs::model::text::offset_types::Grapheme;

use crate::tab::markdown_editor::MdRender;
use crate::tab::markdown_editor::widget::utils::wrap_layout::{Format, Layout};

impl<'ast> MdRender {
    pub fn text_format_escaped_tag(&self, parent: &AstNode<'_>) -> Format {
        self.text_format(parent)
    }

    pub fn layout_escaped_tag(
        &self, layout: &mut Layout, node: &'ast AstNode<'ast>, range: (Grapheme, Grapheme),
    ) {
        let fmt = self.text_format_escaped_tag(node.parent().unwrap());
        self.layout_circumfix(layout, node, range, fmt);
    }
}
