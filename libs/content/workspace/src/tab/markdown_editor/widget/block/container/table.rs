use comrak::nodes::AstNode;
use egui::{Pos2, Rect, Stroke, Ui, Vec2};
use lb_rs::model::text::offset_types::{RangeExt, RangeIterExt as _};

use crate::tab::markdown_editor::MdRender;

use crate::theme::palette_v2::ThemeExt as _;

impl<'ast> MdRender {
    pub fn height_table(&self, node: &'ast AstNode<'ast>) -> f32 {
        let width = self.width(node);
        let row_height = self.layout.row_height;
        if self.reveal_table(node) {
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

        if self.reveal_table(node) {
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

    /// Reveal table syntax when cursor is on the delimiter row or at
    /// the start of any line in the table. Reads `reveal_selection`
    /// rather than `buffer.current.selection` so unfocused frames see
    /// stable layout.
    fn reveal_table(&self, node: &'ast AstNode<'ast>) -> bool {
        let delimiter_row_line_idx = self.node_first_line_idx(node) + 1;
        for line_idx in self.node_lines(node).iter() {
            let line = self.bounds.source_lines[line_idx];
            let node_line = self.node_line(node, line);

            if line_idx == delimiter_row_line_idx && self.range_revealed(node_line, true) {
                return true;
            }
            if self
                .reveal_ranges()
                .any(|r| node_line.contains(r.start(), true, true))
            {
                return true;
            }
        }
        false
    }
}
