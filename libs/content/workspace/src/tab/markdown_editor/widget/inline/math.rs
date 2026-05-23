use comrak::nodes::AstNode;
use lb_rs::model::text::offset_types::Grapheme;

use crate::tab::markdown_editor::MdRender;
use crate::tab::markdown_editor::widget::utils::wrap_layout::{Format, Layout};

impl<'ast> MdRender {
    pub fn text_format_math(&self, parent: &AstNode<'_>) -> Format {
        self.text_format_code(parent)
    }

    pub fn layout_math(
        &self, layout: &mut Layout, node: &'ast AstNode<'ast>, range: (Grapheme, Grapheme),
    ) {
        // Math has same prefix/postfix shape as `Code` — single
        // delimiter char each side, content as plain text. Reuses
        // `layout_code` directly (text_format_math == text_format_code).
        self.layout_code(layout, node, range);
    }
}
