use comrak::nodes::AstNode;
use lb_rs::model::text::offset_types::Grapheme;

use crate::tab::markdown_editor::MdRender;
use crate::tab::markdown_editor::widget::utils::wrap_layout::{Format, Layout};

impl<'ast> MdRender {
    pub fn text_format_strong(&self, parent: &AstNode<'_>) -> Format {
        let parent_text_format = self.text_format(parent);
        Format { bold: true, ..parent_text_format }
    }

    pub fn layout_strong(
        &self, layout: &mut Layout, node: &'ast AstNode<'ast>, range: (Grapheme, Grapheme),
    ) {
        let fmt = self.text_format_strong(node.parent().unwrap());
        self.layout_circumfix(layout, node, range, fmt);
    }
}
