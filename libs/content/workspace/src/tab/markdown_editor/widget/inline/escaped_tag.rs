use comrak::nodes::AstNode;
use lb_rs::model::text::offset_types::{Grapheme, RangeExt as _};

use crate::tab::markdown_editor::MdRender;
use crate::tab::markdown_editor::widget::utils::wrap_layout::{Format, Layout};

impl<'ast> MdRender {
    pub fn text_format_escaped_tag(&self, parent: &AstNode<'_>) -> Format {
        self.text_format(parent)
    }

    /// A tag that is not one: its delimiters are text, and show. A pair
    /// of single pipes is one of these.
    pub fn layout_escaped_tag(
        &self, layout: &mut Layout, node: &'ast AstNode<'ast>, range: (Grapheme, Grapheme),
    ) {
        let fmt = self.text_format_escaped_tag(node.parent().unwrap());
        let tag = self.node_range(node);
        let inner = match (node.first_child(), node.last_child()) {
            (Some(first), Some(last)) => {
                (self.node_range(first).start(), self.node_range(last).end())
            }
            _ => (tag.end(), tag.end()),
        };
        let prefix = (tag.start(), inner.start()).trim(&range);
        let postfix = (inner.end(), tag.end()).trim(&range);
        self.layout_source(layout, node, prefix, fmt.clone(), true);
        self.layout_inline_children(layout, node, range);
        self.layout_source(layout, node, postfix, fmt, true);
    }
}
