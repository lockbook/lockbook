use comrak::nodes::AstNode;
use egui::{Pos2, Rect, Stroke, Ui, Vec2};
use lb_rs::model::text::offset_types::{RangeExt, RangeIterExt as _};

use crate::resolvers::{EmbedResolver, LinkResolver};
use crate::tab::markdown_editor::MdLabel;

use crate::theme::palette_v2::ThemeExt as _;

impl<'ast, E: EmbedResolver, L: LinkResolver> MdLabel<E, L> {
    pub fn height_table(&self, node: &'ast AstNode<'ast>) -> f32 {
        if self.reveal_table(node) {
            let mut height = 0.;

            for line_idx in self.node_lines(node).iter() {
                let line = self.bounds.source_lines[line_idx];
                let node_line = self.node_line(node, line);

                height += self.height_section(
                    &mut self.new_wrap(self.width(node)),
                    node_line,
                    self.text_format_syntax(),
                );
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

        if self.reveal_table(node) {
            for line_idx in self.node_lines(node).iter() {
                let line = self.bounds.source_lines[line_idx];
                let node_line = self.node_line(node, line);

                let mut wrap = self.new_wrap(self.width(node));
                self.show_section(ui, top_left, &mut wrap, node_line, self.text_format_syntax());

                top_left.y += wrap.height();
                top_left.y += self.layout.block_spacing;
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
                Stroke { width: 1., color: self.ctx.get_lb_theme().neutral_bg_tertiary() },
                egui::epaint::StrokeKind::Inside,
            );
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
