use comrak::nodes::AstNode;
use egui::{FontId, Pos2, Rect, Stroke, TextFormat, Ui, Vec2};
use lb_rs::model::text::offset_types::{RangeExt as _, RelByteOffset};

use crate::tab::markdown_plusplus::{
    bounds::RangesExt as _,
    widget::{WrapContext, ROW_HEIGHT},
    MarkdownPlusPlus,
};

impl<'ast> MarkdownPlusPlus {
    pub fn text_format_heading(&self, parent: &AstNode<'_>, level: u8) -> TextFormat {
        let parent_text_format = self.text_format(parent);
        TextFormat {
            font_id: FontId {
                size: match level {
                    6 => 16.,
                    5 => 19.,
                    4 => 22.,
                    3 => 25.,
                    2 => 28.,
                    _ => 32.,
                },
                ..parent_text_format.font_id
            },
            ..parent_text_format
        }
    }

    pub fn height_heading(&self, node: &'ast AstNode<'ast>, width: f32, level: u8) -> f32 {
        self.inline_children_height(node, width) + if level == 1 { ROW_HEIGHT } else { 0. }
    }

    pub fn show_heading(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, mut top_left: Pos2, width: f32,
        level: u8, setext: bool,
    ) {
        let mut wrap = WrapContext::new(width);

        self.show_inline_children(ui, node, top_left, &mut wrap);
        top_left.y += self.inline_children_height(node, width);

        if level == 1 {
            let line_break_rect = Rect::from_min_size(top_left, Vec2::new(width, ROW_HEIGHT));

            ui.painter().hline(
                line_break_rect.x_range(),
                line_break_rect.center().y,
                Stroke { width: 1.0, color: self.theme.bg().neutral_tertiary },
            );
        }

        // bounds
        let sourcepos = node.data.borrow().sourcepos;
        let range = self.sourcepos_to_range(sourcepos);

        if !setext {
            // https://github.github.com/gfm/#atx-headings

            // # ATX heading
            let text_start_byte = self.offset_to_byte(range.start());
            let prefix_length = RelByteOffset(level as usize + 1);
            let text_start_byte = text_start_byte + prefix_length;
            let text_start_char = self.offset_to_char(text_start_byte);

            self.bounds.paragraphs.push((text_start_char, range.end()));
        } else {
            // https://github.github.com/gfm/#setext-headings

            // Setext
            // heading
            // --------------
            let heading_end_line = self
                .bounds
                .source_lines
                .find_containing(range.end(), true, true)
                .end();
            let text_end_line = heading_end_line - 1;
            let text_end_char = self.bounds.source_lines[text_end_line].end();
            self.bounds.paragraphs.push((range.start(), text_end_char));
        }
    }
}
