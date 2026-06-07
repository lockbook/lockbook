use comrak::nodes::AstNode;
use egui::{Pos2, Rect, Stroke, Ui, Vec2};
use lb_rs::model::text::offset_types::{IntoRangeExt as _, RangeExt as _};

use crate::tab::markdown_editor::MdRender;
use crate::tab::markdown_editor::widget::utils::wrap_layout::Layout;

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
        // reports a thematic break's sourcepos as a single column, so
        // inside a container the cursor on a later `*`/`-` wouldn't
        // intersect it and the row's interior offsets would have no
        // fragment.
        if self.range_revealed(node_line, true) {
            let result = self.compute_section_layout_new(
                node_line,
                width,
                row_height,
                self.text_format_syntax(),
            );
            self.show_wrap_layout(ui, top_left, &result);
            self.show_block_line_prefixes(node, line, top_left, row_height);
        } else {
            // Painted as a horizontal rule, with zero-visible anchors
            // at each endpoint so cursor placement at the row's edges
            // resolves to the thematic-break's source range.
            let rect = Rect::from_min_size(top_left, Vec2::new(width, self.layout.row_height));
            ui.painter().hline(
                rect.x_range(),
                rect.center().y,
                Stroke { width: 1.0, color: self.ctx.get_lb_theme().neutral() },
            );

            // Build a layout with two zero-visible anchors at the
            // line's endpoints. Each becomes a row Anchor (cursor-only,
            // no glyph) via the row builder's empty-source-segment
            // handling.
            let mut layout = Layout::new(node_line);
            layout.push_override(node_line.start().into_range(), "", self.text_format_syntax());
            layout.push_override(node_line.end().into_range(), "", self.text_format_syntax());
            let result = self.compute_layout_from(layout, width, row_height);
            self.show_wrap_layout(ui, top_left, &result);
            self.show_block_line_prefixes(node, line, top_left, row_height);
        }
    }
}
