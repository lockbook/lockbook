use comrak::nodes::AstNode;
use egui::{Pos2, Rect, Stroke, Ui, Vec2};
use lb_rs::model::text::offset_types::{RangeExt, RangeIterExt as _};

use crate::tab::markdown_editor::widget::utils::text_layout::Wrap;
use crate::tab::markdown_editor::widget::BLOCK_SPACING;
use crate::tab::markdown_editor::Editor;

impl<'ast> Editor {
    pub fn height_table(&self, node: &'ast AstNode<'ast>) -> f32 {
        if self.reveal_table(node) {
            let mut height = 0.;

            for line_idx in self.node_lines(node).iter() {
                let line = self.bounds.source_lines[line_idx];
                let node_line = self.node_line(node, line);

                height += self.height_text_line(
                    &mut Wrap::new(self.width(node)),
                    node_line,
                    self.text_format_syntax(node),
                );
                height += BLOCK_SPACING;
            }
            if height > 0. {
                height -= BLOCK_SPACING;
            }

            height
        } else {
            self.block_children_height(node)
        }
    }

    pub fn show_table(&mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, mut top_left: Pos2) {
        let width = self.width(node);

        if self.reveal_table(node) {
            for line_idx in self.node_lines(node).iter() {
                let line = self.bounds.source_lines[line_idx];
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

                top_left.y += wrap.height();
                top_left.y += BLOCK_SPACING;
                self.bounds.wrap_lines.extend(wrap.row_ranges);
            }
        } else {
            self.show_block_children(ui, node, top_left);

            // draw exterior decoration
            let table =
                Rect::from_min_size(top_left, Vec2::new(width, self.block_children_height(node)));
            ui.painter().rect_stroke(
                table,
                2.,
                Stroke { width: 1., color: self.theme.bg().neutral_tertiary },
            );
        }
    }

    pub fn compute_bounds_table(&mut self, node: &'ast AstNode<'ast>) {
        if self.reveal_table(node) {
            for line_idx in self.node_lines(node).iter() {
                let line = self.bounds.source_lines[line_idx];
                let node_line = self.node_line(node, line);
                self.bounds.paragraphs.push(node_line);
            }
        } else {
            let delimiter_row_line_idx = self.node_first_line_idx(node) + 1;
            let delimiter_row_line = self.bounds.source_lines[delimiter_row_line_idx];
            let delimiter_row_node_line = self.node_line(node, delimiter_row_line);
            self.bounds.paragraphs.push(delimiter_row_node_line);

            self.compute_bounds_block_children(node);
        }
    }

    fn reveal_table(&self, node: &'ast AstNode<'ast>) -> bool {
        let selection = self.buffer.current.selection;
        let delimiter_row_line_idx = self.node_first_line_idx(node) + 1;
        for line_idx in self.node_lines(node).iter() {
            let line = self.bounds.source_lines[line_idx];
            let node_line = self.node_line(node, line);

            if line_idx == delimiter_row_line_idx && selection.intersects(&node_line, true) {
                return true;
            }
            if selection.contains(node_line.start(), true, true) {
                return true;
            }
        }

        false
    }
}
