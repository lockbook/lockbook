use comrak::nodes::{AstNode, NodeCodeBlock};
use egui::{Pos2, TextFormat, Ui};

use crate::tab::markdown_editor::Editor;

impl<'ast> Editor {
    pub fn text_format_html_block(&self, parent: &AstNode<'_>) -> TextFormat {
        self.text_format_code_block(parent)
    }

    pub fn height_html_block(&self, node: &'ast AstNode<'ast>) -> f32 {
        // html comments not rendered unless revealed
        let node_range = self.node_range(node);
        let node_text = &self.buffer[node_range];
        if !self.node_intersects_selection(node)
            && node_text.starts_with("<!--")
            && node_text.ends_with("-->")
        {
            return 0.;
        }

        self.height_indented_code_block(
            node,
            &NodeCodeBlock { info: "html".into(), ..Default::default() },
        )
    }

    pub fn show_html_block(&mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2) {
        // html comments not rendered unless revealed
        let node_range = self.node_range(node);
        let node_text = &self.buffer[node_range];
        if !self.node_intersects_selection(node)
            && node_text.starts_with("<!--")
            && node_text.ends_with("-->")
        {
            return Default::default();
        }

        self.show_indented_code_block(
            ui,
            node,
            top_left,
            &NodeCodeBlock { info: "html".into(), ..Default::default() },
        );
    }

    pub fn compute_bounds_html_block(&mut self, node: &'ast AstNode<'ast>) {
        self.compute_bounds_indented_code_block(node);
    }
}
