use egui::{FontId, TextFormat};

use crate::tab::markdown_plusplus::{widget::ROW_HEIGHT, MarkdownPlusPlus};

impl MarkdownPlusPlus {
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
}
