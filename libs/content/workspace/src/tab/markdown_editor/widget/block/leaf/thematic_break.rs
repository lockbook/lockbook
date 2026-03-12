use comrak::nodes::AstNode;
use egui::{Pos2, Rect, Stroke, Ui, Vec2};
use lb_rs::model::text::offset_types::{IntoRangeExt as _, RangeExt as _};

use crate::tab::markdown_editor::Editor;

impl<'ast> Editor {
    pub fn height_thematic_break(&self) -> f32 {
        self.visuals.row_height
    }

    pub fn show_thematic_break(&mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2) {
        let width = self.width(node);
        let node_line = self.node_line(node, self.node_first_line(node));

        if self.node_intersects_selection(node) {
            let mut wrap = self.wrap(width);
            self.show_section(ui, top_left, &mut wrap, node_line, self.text_format_syntax());
            self.bounds.wrap_lines.extend(wrap.row_ranges);
        } else {
            let rect = Rect::from_min_size(top_left, Vec2::new(width, self.visuals.row_height));
            ui.painter().hline(
                rect.x_range(),
                rect.center().y,
                Stroke { width: 1.0, color: self.theme.bg().neutral_tertiary },
            );

            // show empty row with mapped text range
            let mut wrap = self.wrap(width);
            self.show_section(
                ui,
                top_left,
                &mut wrap,
                node_line.end().into_range(),
                self.text_format_syntax(),
            );
            self.bounds.wrap_lines.extend(wrap.row_ranges);
        }
    }
}
