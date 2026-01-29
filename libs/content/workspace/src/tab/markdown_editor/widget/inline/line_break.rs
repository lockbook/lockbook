use comrak::nodes::AstNode;
use lb_rs::model::text::offset_types::{DocCharOffset, RangeExt};

use crate::tab::markdown_editor::Editor;
use crate::tab::markdown_editor::widget::inline::Response;
use crate::tab::markdown_editor::widget::utils::wrap_layout::Wrap;

impl<'ast> Editor {
    pub fn span_line_break(
        &self, node: &'ast AstNode<'ast>, wrap: &Wrap, range: (DocCharOffset, DocCharOffset),
    ) -> f32 {
        let node_range = self.node_range(node);
        if range.contains_range(&node_range, true, true) { wrap.row_remaining() } else { 0. }
    }

    pub fn show_line_break(
        &mut self, node: &'ast AstNode<'ast>, wrap: &mut Wrap,
        range: (DocCharOffset, DocCharOffset),
    ) -> Response {
        let node_range = self.node_range(node);
        if range.contains_range(&node_range, true, true) {
            wrap.offset = wrap.row_end();
        }
        Default::default()
    }
}
