use std::rc::Rc;

use comrak::nodes::AstNode;
use egui::{Pos2, Rect, Stroke, Ui, Vec2};
use lb_rs::model::text::offset_types::RangeIterExt as _;

use crate::tab::markdown_editor::MdRender;

use crate::theme::palette_v2::ThemeExt as _;

impl<'ast> MdRender {
    pub fn height_table(&self, node: &'ast AstNode<'ast>) -> f32 {
        let width = self.width(node);
        let row_height = self.layout.row_height;
        if self.table_revealed(node) {
            let mut height = 0.;
            for line_idx in self.node_lines(node).iter() {
                let line = self.bounds.source_lines[line_idx];
                let node_line = self.node_line(node, line);
                let l = self.compute_section_layout_new(
                    node_line,
                    width,
                    row_height,
                    self.text_format_syntax(),
                );
                height += l.height;
                height += self.layout.block_spacing;
            }
            if height > 0. {
                height -= self.layout.block_spacing;
            }
            height
        } else {
            self.block_children_height(node)
        }
    }

    pub fn show_table(&mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, mut top_left: Pos2) {
        let width = self.width(node);
        let row_height = self.layout.row_height;

        if self.table_revealed(node) {
            for line_idx in self.node_lines(node).iter() {
                let line = self.bounds.source_lines[line_idx];
                let node_line = self.node_line(node, line);
                let result = self.compute_section_layout_new(
                    node_line,
                    width,
                    row_height,
                    self.text_format_syntax(),
                );
                let h = result.height;
                self.show_wrap_layout(ui, top_left, &result);
                top_left.y += h;
                top_left.y += self.layout.block_spacing;
            }
        } else {
            self.show_block_children(ui, node, top_left);

            // draw exterior decoration
            let width = self.table_column_widths(node).iter().sum();
            let table =
                Rect::from_min_size(top_left, Vec2::new(width, self.block_children_height(node)));
            ui.painter().rect_stroke(
                table,
                2.,
                Stroke { width: 1., color: self.ctx.get_lb_theme().neutral_bg_tertiary() },
                egui::epaint::StrokeKind::Inside,
            );
        }
    }

    /// The width of each of the table's columns, padding included. A
    /// column is as wide as its widest cell on one row where the table's
    /// width allows that; else the columns share the width in proportion,
    /// none narrower than its longest unbreakable run, and cells wrap. If
    /// not even those runs fit, every column is narrowed alike and they
    /// break.
    pub fn table_column_widths(&self, table: &'ast AstNode<'ast>) -> Rc<[f32]> {
        if let Some(columns) = self.get_cached_table_columns(table) {
            return columns;
        }

        let available = self.width(table);
        let padding = 2.0 * self.layout.block_padding;
        let row_height = self.layout.row_height;
        // an image's size here is what it will shrink from, never grow to
        let embed_min = 3.0 * row_height;

        let column_count = table
            .first_child()
            .map_or(0, |header| header.children().count());
        let mut narrowest = vec![0f32; column_count];
        let mut widest = vec![0f32; column_count];
        self.layout_cache
            .table_cell_measure
            .set(Some((available - padding).max(0.0)));
        for row in table.children() {
            for (column, cell) in row.children().enumerate().take(column_count) {
                if self.table_cell_autocompleted(cell) {
                    break;
                }
                let (run, row) =
                    self.measure_layout(self.layout_table_cell(cell), row_height, embed_min);
                if row > 0.0 {
                    narrowest[column] = narrowest[column].max(run.ceil());
                    // a pixel to spare, or rounding wraps a cell that fits
                    widest[column] = widest[column].max(row.ceil() + 1.0);
                }
            }
        }
        self.layout_cache.table_cell_measure.set(None);
        for column in 0..column_count {
            // room to click into a column, more if it holds nothing yet
            let room = if widest[column] == 0.0 { 3.0 * row_height } else { row_height };
            narrowest[column] = narrowest[column].max(room);
            widest[column] = widest[column].max(room);
        }

        let total = |widths: &[f32]| widths.iter().sum::<f32>() + padding * column_count as f32;
        let (least, most) = (total(&narrowest), total(&widest));
        let columns: Rc<[f32]> = (0..column_count)
            .map(|column| {
                let (narrowest, widest) = (narrowest[column], widest[column]);
                padding
                    + if most <= available {
                        widest
                    } else if least <= available {
                        let spare = (available - least) / (most - least);
                        narrowest + (widest - narrowest) * spare
                    } else {
                        let content = (available - padding * column_count as f32).max(0.0);
                        narrowest * content / (least - padding * column_count as f32)
                    }
            })
            .collect();
        self.set_cached_table_columns(table, columns.clone());
        columns
    }
}
