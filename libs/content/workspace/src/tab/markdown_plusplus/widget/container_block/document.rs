use comrak::nodes::AstNode;
use egui::{FontId, Pos2, TextFormat};
use lb_rs::model::text::offset_types::RangeIterExt as _;

use crate::tab::markdown_plusplus::{
    widget::{Wrap, ROW_HEIGHT, ROW_SPACING},
    MarkdownPlusPlus,
};

impl<'ast> MarkdownPlusPlus {
    pub fn text_format_document(&self) -> TextFormat {
        let parent_text_format = TextFormat::default();
        TextFormat {
            color: self.theme.fg().neutral_secondary,
            font_id: FontId {
                size: parent_text_format.font_id.size * ROW_HEIGHT
                    / self
                        .ctx
                        .fonts(|fonts| fonts.row_height(&parent_text_format.font_id)),
                ..parent_text_format.font_id
            },
            ..parent_text_format
        }
    }

    pub fn show_document(
        &mut self, ui: &mut egui::Ui, node: &'ast AstNode<'ast>, mut top_left: Pos2,
    ) {
        let width = self.width(node);

        // leading and trailing newlines are not parsed as part of the document
        let pre_spacing = self.block_pre_spacing_height(node);
        self.show_block_pre_spacing(ui, node, top_left);
        top_left.y += pre_spacing;

        let any_children = node.children().next().is_some();
        if any_children {
            self.show_block_children(ui, node, top_left);
            top_left.y += self.block_children_height(node);
        } else {
            for line_idx in self.node_lines(node).iter() {
                let line = self.bounds.source_lines[line_idx];

                self.show_text_line(
                    ui,
                    top_left,
                    &mut Wrap::new(width),
                    line,
                    self.text_format_syntax(node),
                    false,
                );
                self.bounds.paragraphs.push(line);

                top_left.y += ROW_HEIGHT;
                top_left.y += ROW_SPACING;
            }
        }

        self.show_block_post_spacing(ui, node, top_left);
    }
}
