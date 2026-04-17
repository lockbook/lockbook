use comrak::nodes::AstNode;
use lb_rs::model::text::offset_types::DocCharOffset;

use crate::tab::markdown_editor::MdRender;
use crate::tab::markdown_editor::widget::inline::Response;
use crate::tab::markdown_editor::widget::utils::wrap_layout::Wrap;

impl<'ast> MdRender {
    pub fn span_soft_break(
        &self, node: &'ast AstNode<'ast>, wrap: &Wrap, range: (DocCharOffset, DocCharOffset),
    ) -> f32 {
        self.span_line_break(node, wrap, range)
    }

    pub fn show_soft_break(
        &mut self, node: &'ast AstNode<'ast>, wrap: &mut Wrap,
        range: (DocCharOffset, DocCharOffset),
    ) -> Response {
        self.show_line_break(node, wrap, range)
    }
}
