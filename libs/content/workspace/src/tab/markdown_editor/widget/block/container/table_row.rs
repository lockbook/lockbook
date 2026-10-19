use comrak::nodes::AstNode;
use egui::{Pos2, Rangef, Rect, Stroke, Ui, Vec2};
use lb_rs::model::text::offset_types::RangeExt as _;

use crate::tab::markdown_editor::MdRender;
use crate::tab::markdown_editor::table::TableCellBox;
use crate::tab::markdown_editor::widget::utils::wrap_layout::Format;
use crate::theme::palette_v2::ThemeExt as _;

impl<'ast> MdRender {
    pub fn text_format_table_row(&self, parent: &AstNode<'_>, is_header_row: bool) -> Format {
        let parent_text_format = self.text_format(parent);
        Format { bold: is_header_row, ..parent_text_format }
    }

    pub fn height_table_row(&self, node: &'ast AstNode<'ast>) -> f32 {
        // the height of the row is the height of the tallest cell
        let mut cell_height_max = 0.0f32;
        for table_cell in node.children() {
            cell_height_max = cell_height_max.max(self.height(table_cell));
        }

        cell_height_max
    }

    pub fn show_table_row(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2, is_header_row: bool,
    ) {
        let height = self.height_table_row(node);
        let width = self.width(node);
        let child_width = width / node.children().count() as f32;

        // draw row backgrounds
        let row_rect = Rect::from_min_size(top_left, Vec2::new(width, height));
        if is_header_row {
            ui.painter()
                .rect_filled(row_rect, 1., self.ctx.get_lb_theme().neutral_bg_secondary());
        }

        // draw interior decorations
        let stroke = Stroke { width: 1., color: self.ctx.get_lb_theme().neutral_bg_tertiary() };
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

        // draw cell contents
        let row = self.node_line(node, self.node_first_line(node)).start();
        let mut child_top_left = top_left;
        for (column, table_cell) in node.children().enumerate() {
            self.show_table_cell(ui, table_cell, child_top_left);
            self.table_cell_boxes.push(TableCellBox {
                rect: Rect::from_min_size(child_top_left, Vec2::new(child_width, height)),
                row,
                column,
            });
            child_top_left.x += child_width;
        }
    }
}
