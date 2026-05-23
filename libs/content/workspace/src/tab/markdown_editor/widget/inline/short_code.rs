use comrak::nodes::{AstNode, NodeShortCode};
use lb_rs::model::text::offset_types::{Grapheme, RangeExt as _};

use crate::tab::markdown_editor::MdRender;
use crate::tab::markdown_editor::widget::utils::wrap_layout::{Format, Layout};

impl<'ast> MdRender {
    pub fn text_format_short_code(&self, parent: &AstNode<'_>) -> Format {
        self.text_format(parent)
    }

    pub fn layout_short_code(
        &self, layout: &mut Layout, node: &'ast AstNode<'ast>, range: (Grapheme, Grapheme),
        node_short_code: &NodeShortCode,
    ) {
        let node_range = self.node_range(node);
        if !range.contains_range(&node_range, true, true) {
            return;
        }
        if self.node_revealed(node) {
            // Reveal: emit raw `:smile:` source bytes.
            self.layout_circumfix(
                layout,
                node,
                range,
                self.text_format_short_code(node.parent().unwrap()),
            );
        } else {
            // Hide: replace source with the emoji glyph.
            layout.push_override(
                node_range,
                &node_short_code.emoji,
                self.text_format_short_code(node.parent().unwrap()),
            );
        }
    }
}
