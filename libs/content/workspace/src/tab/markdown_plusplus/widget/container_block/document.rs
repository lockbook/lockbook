use comrak::nodes::AstNode;
use egui::{FontId, Pos2, TextFormat};
use lb_rs::model::text::offset_types::{DocCharOffset, IntoRangeExt as _, RangeIterExt as _};

use crate::tab::markdown_plusplus::{
    widget::{WrapContext, ROW_HEIGHT, ROW_SPACING},
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

        if node.children().count() == 0 {
            for offset in
                (DocCharOffset(0), self.buffer.current.segs.last_cursor_position() + 1).iter()
            {
                let range = offset.into_range();
                self.show_node_text_line(ui, node, top_left, &mut WrapContext::new(width), range);
                self.bounds.paragraphs.push(range);

                top_left.y += ROW_HEIGHT;
                top_left.y += ROW_SPACING;
            }
        } else {
            self.show_block_children(ui, node, top_left)
        }
    }
}
