use comrak::nodes::AstNode;
use egui::{Pos2, Rect, Stroke, Ui};

use crate::tab::markdown_editor::MdRender;
use crate::tab::markdown_editor::widget::utils::wrap_layout::{EmbedKind, EmbedSpec, Layout};

use crate::theme::palette_v2::ThemeExt as _;

impl<'ast> MdRender {
    pub fn height_thematic_break(&self) -> f32 {
        self.layout.row_height
    }

    pub fn show_thematic_break(&mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2) {
        let width = self.width(node);
        let line = self.node_first_line(node);
        let node_line = self.node_line(node, line);
        let row_height = self.layout.row_height;

        // Reveal on the node's *line* range, not `node_range`: comrak
        // reports a thematic break's sourcepos as a single column. Like an
        // image, only a caret inside the syntax reveals it; one at either
        // edge sits beside the drawn rule.
        if self.range_revealed_interior(node_line) {
            let result = self.compute_section_layout_new(
                node_line,
                width,
                row_height,
                self.text_format_syntax(),
            );
            self.show_wrap_layout(ui, top_left, &result);
        } else {
            let mut layout = Layout::new(node_line);
            layout.push_embed(EmbedSpec {
                advance: width,
                ascent: row_height,
                descent: 0.0,
                source_range: node_line,
                url: String::new(),
                kind: EmbedKind::Rule,
            });
            let result = self.compute_layout_from(layout, width, row_height);
            self.show_wrap_layout(ui, top_left, &result);
        }
        self.show_block_line_prefixes(ui, node, line, top_left, row_height);
    }

    /// A divider drawn as a rule across `rect`.
    pub fn paint_rule(&self, ui: &Ui, rect: Rect) {
        let stroke = Stroke { width: 1.0, color: self.ctx.get_lb_theme().neutral() };
        ui.painter().hline(rect.x_range(), rect.center().y, stroke);
    }
}
