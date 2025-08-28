use std::sync::Arc;

use comrak::nodes::AstNode;
use egui::{FontFamily, FontId, Pos2, Rangef, Rect, Stroke, TextFormat, Ui, Vec2};
use lb_rs::model::text::offset_types::RangeExt;

use crate::tab::markdown_editor::Editor;
use crate::tab::markdown_editor::widget::BLOCK_PADDING;
use crate::tab::markdown_editor::widget::utils::text_layout::Wrap;

impl<'ast> Editor {
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

    pub fn height_table_row(&self, node: &'ast AstNode<'ast>) -> f32 {
        BLOCK_PADDING
            + if self.reveal_table_row(node) {
                let line = self.node_first_line(node);
                let node_line = self.node_line(node, line);

                self.height_text_line(
                    &mut Wrap::new(self.width(node)),
                    node_line,
                    self.text_format_syntax(node),
                )
            } else {
                // the height of the row is the height of the tallest cell
                let mut cell_height_max = 0.0f32;
                for table_cell in node.children() {
                    cell_height_max = cell_height_max.max(self.height(table_cell));
                }

                cell_height_max
            }
            + BLOCK_PADDING
    }

    pub fn show_table_row(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, mut top_left: Pos2, is_header_row: bool,
    ) {
        if self.reveal_table_row(node) {
            top_left.y += BLOCK_PADDING;

            let line = self.node_first_line(node);
            let node_line = self.node_line(node, line);

            let mut wrap = Wrap::new(self.width(node));
            self.show_text_line(
                ui,
                top_left,
                &mut wrap,
                node_line,
                self.text_format_syntax(node),
                false,
            );
            self.bounds.wrap_lines.extend(wrap.row_ranges);
        } else {
            let height = self.height_table_row(node);
            let width = self.width(node);
            let child_width = width / node.children().count() as f32;

            // draw row backgrounds
            let row_rect =
                Rect::from_min_size(top_left, Vec2::new(width, self.height_table_row(node)));
            if is_header_row {
                ui.painter()
                    .rect_filled(row_rect, 0., self.theme.bg().neutral_secondary);
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

            top_left.y += BLOCK_PADDING;

            // draw cell contents
            let mut child_top_left = top_left;
            for table_cell in node.children() {
                self.show_table_cell(ui, table_cell, child_top_left);
                child_top_left.x += child_width;
            }
        }
    }

    fn reveal_table_row(&self, node: &'ast AstNode<'ast>) -> bool {
        let selection = self.buffer.current.selection;
        let row_range = self.node_range(node);
        let children = self.sorted_children(node); // todo: these will always already be sorted

        let mut range_start = row_range.start();
        for cell in &children {
            let cell_range = self.node_range(cell);

            let between_range = (range_start, cell_range.start());
            if between_range.intersects(&selection, true)
                || between_range.contains(selection.end(), true, true)
            {
                return true;
            }

            range_start = cell_range.end();
        }
        if let Some(cell) = children.last() {
            let cell_range = self.node_range(cell);

            let between_range = (cell_range.end(), row_range.end());
            if between_range.intersects(&selection, true)
                || between_range.contains(selection.end(), true, true)
            {
                return true;
            }
        }

        false
    }

    pub fn compute_bounds_table_row(&mut self, node: &'ast AstNode<'ast>) {
        if self.reveal_table_row(node) {
            let line = self.node_first_line(node);
            let node_line = self.node_line(node, line);
            self.bounds.paragraphs.push(node_line);
        } else {
            // Push bounds for syntax between cells
            let row_range = self.node_range(node);
            let children = self.sorted_children(node);

            let mut range_start = row_range.start();
            for cell in &children {
                let cell_range = self.node_range(cell);

                let between_range = (range_start, cell_range.start());
                self.bounds.paragraphs.push(between_range);
                self.bounds.inline_paragraphs.push(between_range);

                range_start = cell_range.end();
            }
            if let Some(cell) = children.last() {
                let cell_range = self.node_range(cell);

                let between_range = (cell_range.end(), row_range.end());
                self.bounds.paragraphs.push(between_range);
                self.bounds.inline_paragraphs.push(between_range);
            }

            // Compute bounds for cell contents
            for table_cell in node.children() {
                self.compute_bounds(table_cell);
            }
        }
    }
}
