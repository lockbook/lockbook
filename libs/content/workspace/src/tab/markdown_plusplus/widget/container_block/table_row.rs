use std::sync::Arc;

use comrak::nodes::AstNode;
use egui::{FontFamily, FontId, Pos2, Rangef, Rect, Stroke, TextFormat, Ui, Vec2};

use crate::tab::markdown_plusplus::MarkdownPlusPlus;

impl<'ast> MarkdownPlusPlus {
    pub fn text_format_table_row(&self, parent: &AstNode<'_>, is_header_row: bool) -> TextFormat {
        let parent_text_format = self.text_format(parent);
        TextFormat {
            font_id: FontId {
                family: if is_header_row {
                    FontFamily::Name(Arc::from("Bold"))
                } else {
                    FontFamily::Proportional
                },
                ..parent_text_format.font_id
            },
            ..parent_text_format
        }
    }

    pub fn height_table_row(&self, node: &'ast AstNode<'ast>, width: f32) -> f32 {
        // the height of the row is the height of the tallest cell
        let child_width = width / node.children().count() as f32;
        let mut cell_height_max = 0.0f32;
        for table_cell in node.children() {
            cell_height_max = cell_height_max.max(self.height(table_cell, child_width));
        }

        cell_height_max
    }

    pub fn show_table_row(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2, width: f32,
        is_header_row: bool,
    ) {
        let height = self.height_table_row(node, width);

        // draw row backgrounds
        let row_rect =
            Rect::from_min_size(top_left, Vec2::new(width, self.height_table_row(node, width)));
        if is_header_row {
            ui.painter()
                .rect_filled(row_rect, 0., self.theme.bg().neutral_secondary);
        }

        // draw cell contents
        let mut child_top_left = top_left;
        let child_width = width / node.children().count() as f32;
        for table_cell in node.children() {
            self.show_block(ui, table_cell, child_top_left, child_width);
            child_top_left.x += child_width;
        }

        // draw interior decorations
        let stroke = Stroke { width: 1., color: self.theme.bg().neutral_tertiary };
        if !is_header_row {
            ui.painter()
                .hline(Rangef::new(top_left.x, top_left.x + width), top_left.y, stroke);
        }
        for child_idx in 1..node.children().count() {
            ui.painter().vline(
                top_left.x + child_idx as f32 * child_width,
                Rangef::new(top_left.y, top_left.y + height),
                stroke,
            );
        }
    }
}
