use comrak::nodes::{AstNode, NodeCodeBlock};
use egui::{Pos2, Ui};

use crate::resolvers::{EmbedResolver, LinkResolver};
use crate::tab::markdown_editor::MdLabel;
use crate::tab::markdown_editor::widget::utils::wrap_layout::Format;

impl<'ast, E: EmbedResolver, L: LinkResolver> MdLabel<E, L> {
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
