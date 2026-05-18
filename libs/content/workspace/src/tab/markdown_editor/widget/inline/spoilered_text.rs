use comrak::nodes::AstNode;
use lb_rs::model::text::offset_types::Grapheme;

use crate::tab::markdown_editor::MdRender;
use crate::tab::markdown_editor::widget::utils::wrap_layout::{Format, Layout};
use crate::theme::palette_v2::ThemeExt as _;

impl<'ast> MdRender {
    pub fn text_format_spoilered_text(&self, parent: &AstNode<'_>) -> Format {
        let parent_text_format = self.text_format(parent);
        Format {
            background: self.ctx.get_lb_theme().neutral_bg_tertiary(),
            spoiler: true,
            ..parent_text_format
        }
    }

    pub fn layout_spoilered_text(
        &self, layout: &mut Layout, node: &'ast AstNode<'ast>, range: (Grapheme, Grapheme),
    ) {
        let fmt = self.text_format_spoilered_text(node.parent().unwrap());
        self.layout_circumfix(layout, node, range, fmt);
    }
}
