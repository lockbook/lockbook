use comrak::nodes::AstNode;
use lb_rs::model::text::offset_types::Grapheme;

use crate::tab::markdown_editor::MdRender;
use crate::tab::markdown_editor::widget::utils::wrap_layout::Layout;

impl<'ast> MdRender {
    pub fn layout_soft_break(
        &self, layout: &mut Layout, node: &'ast AstNode<'ast>, range: (Grapheme, Grapheme),
    ) {
        self.layout_line_break(layout, node, range);
    }
}
