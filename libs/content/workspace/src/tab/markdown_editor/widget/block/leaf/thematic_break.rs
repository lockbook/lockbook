use comrak::nodes::AstNode;
use egui::{Pos2, Rect, Stroke, Ui, Vec2};
use lb_rs::model::text::offset_types::{IntoRangeExt as _, RangeExt as _};

use crate::tab::markdown_editor::Editor;
use crate::tab::markdown_editor::widget::ROW_HEIGHT;
use crate::tab::markdown_editor::widget::utils::wrap_layout::Wrap;

impl<'ast> Editor {
    pub fn height_thematic_break(&self) -> f32 {
        ROW_HEIGHT
    }

    pub fn show_thematic_break(&mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2) {
        let width = self.width(node);
        let node_line = self.node_line(node, self.node_first_line(node));

        if self.node_intersects_selection(node) {
            let mut wrap = Wrap::new(width);
            self.show_section(
                ui,
                top_left,
                &mut wrap,
                node_line,
                self.text_format_syntax(node),
                false,
            );
            self.bounds.wrap_lines.extend(wrap.row_ranges);
        } else {
            let rect = Rect::from_min_size(top_left, Vec2::new(width, ROW_HEIGHT));
            ui.painter().hline(
                rect.x_range(),
                rect.center().y,
                Stroke { width: 1.0, color: self.theme.bg().neutral_tertiary },
            );

            // show empty row with mapped text range
            let mut wrap = Wrap::new(width);
            self.show_section(
                ui,
                top_left,
                &mut wrap,
                node_line.end().into_range(),
                self.text_format_syntax(node),
                false,
            );
            self.bounds.wrap_lines.extend(wrap.row_ranges);
        }
    }

    pub fn compute_bounds_thematic_break(&mut self, node: &'ast AstNode<'ast>) {
        let node_line = self.node_line(node, self.node_first_line(node));
        self.bounds.paragraphs.push(node_line);
    }
}
