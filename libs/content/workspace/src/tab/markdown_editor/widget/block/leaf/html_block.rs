use comrak::nodes::{AstNode, NodeCodeBlock};
use egui::{Pos2, Ui};

use crate::tab::markdown_editor::MdRender;
use crate::tab::markdown_editor::widget::utils::wrap_layout::Format;

impl<'ast> MdRender {
    pub fn text_format_html_block(&self, parent: &AstNode<'_>) -> Format {
        self.text_format_code_block(parent)
    }

    pub fn height_html_block(&self, node: &'ast AstNode<'ast>) -> f32 {
        self.height_indented_code_block(
            node,
            &NodeCodeBlock { info: "html".into(), ..Default::default() },
            true,
        )
    }

    pub(crate) fn height_auto_html_block(&self, node: &'ast AstNode<'ast>, top_left: Pos2) -> f32 {
        self.height_auto_indented_code_block(
            node,
            &NodeCodeBlock { info: "html".into(), ..Default::default() },
            true,
            top_left,
        )
    }

    pub fn show_html_block(&mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2) {
        self.show_indented_code_block(
            ui,
            node,
            top_left,
            &NodeCodeBlock { info: "html".into(), ..Default::default() },
            true,
        );
    }
}
