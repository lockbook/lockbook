use comrak::nodes::AstNode;
use egui::{Pos2, Rect, Stroke, Ui, Vec2};
use lb_rs::model::text::offset_types::RangeExt;

use crate::tab::markdown_plusplus::{
    widget::{WrapContext, ROW_HEIGHT},
    MarkdownPlusPlus,
};

impl<'ast> MarkdownPlusPlus {
    pub fn height_thematic_break(&self) -> f32 {
        ROW_HEIGHT
    }

    pub fn show_thematic_break(&mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2) {
        let width = self.width(node);

        // for some reason, thematic break ranges contain the trailing newline,
        // so we trim the range to the first line
        let mut range = self.bounds.source_lines[self.node_lines(node).start()];

        // when the thematic break is nested in a container block with per-line
        // syntax, like a block quote, the range needs to be stripped of that
        // syntax
        range.0 += self.line_prefix_len(node.parent().unwrap(), range);

        if self.node_intersects_selection(node) {
            let mut wrap = WrapContext::new(width);
            self.show_text_line(
                ui,
                top_left,
                &mut wrap,
                range,
                self.text_format_syntax(node),
                false,
            );
        } else {
            let rect = Rect::from_min_size(top_left, Vec2::new(width, ROW_HEIGHT));
            ui.painter().hline(
                rect.x_range(),
                rect.center().y,
                Stroke { width: 1.0, color: self.theme.bg().neutral_tertiary },
            );
        }

        self.bounds.paragraphs.push(range);
    }
}
