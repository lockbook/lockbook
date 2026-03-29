use comrak::nodes::{AstNode, NodeCodeBlock};
use egui::{Pos2, Ui};

use crate::tab::markdown_editor::Editor;

impl<'ast> Editor {
    fn frontmatter_as_code_block() -> NodeCodeBlock {
        NodeCodeBlock {
            fenced: true,
            fence_char: b'-',
            fence_length: 3,
            fence_offset: 0,
            info: "yaml".to_string(),
            literal: String::new(),
            closed: true,
        }
    }

    pub fn height_front_matter(&self, node: &'ast AstNode<'ast>) -> f32 {
        self.height_fenced_code_block(node, &Self::frontmatter_as_code_block())
    }

    pub fn show_front_matter(&mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2) {
        self.show_fenced_code_block(ui, node, top_left, &Self::frontmatter_as_code_block());
    }
}
