use comrak::nodes::AstNode;
use egui::Color32;
use lb_rs::model::text::offset_types::Grapheme;

use crate::tab::markdown_editor::MdRender;
use crate::tab::markdown_editor::widget::utils::wrap_layout::{Format, Layout};
use crate::theme::palette_v2::ThemeExt;

impl<'ast> MdRender {
    pub fn background_color_highlight(&self) -> Color32 {
        self.ctx.get_lb_theme().bg().yellow.gamma_multiply(0.35)
    }

    pub fn text_format_highlight(&self, parent: &AstNode<'_>) -> Format {
        Format { background: self.background_color_highlight(), ..self.text_format(parent) }
    }

    pub fn layout_highlight(
        &self, layout: &mut Layout, node: &'ast AstNode<'ast>, range: (Grapheme, Grapheme),
    ) {
        let fmt = self.text_format_highlight(node.parent().unwrap());
        self.layout_circumfix(layout, node, range, fmt);
    }
}
