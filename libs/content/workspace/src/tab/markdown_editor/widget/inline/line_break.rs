use comrak::nodes::AstNode;
use lb_rs::model::text::offset_types::{Grapheme, RangeExt};

use crate::tab::markdown_editor::MdRender;
use crate::tab::markdown_editor::widget::utils::wrap_layout::Layout;

impl<'ast> MdRender {
    pub fn layout_line_break(
        &self, layout: &mut Layout, node: &'ast AstNode<'ast>, range: (Grapheme, Grapheme),
    ) {
        let node_range = self.node_range(node);
        if range.contains_range(&node_range, true, true) {
            layout.push_break(node_range);
        }
    }
}
