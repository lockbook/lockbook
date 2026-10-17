use comrak::nodes::AstNode;
use egui::{Pos2, Ui, Vec2};
use lb_rs::model::text::offset_types::RangeExt as _;

use crate::tab::markdown_editor::MdRender;
use crate::tab::markdown_editor::widget::utils::wrap_layout::Layout;

impl<'ast> MdRender {
    /// Lays out a cell's content. Its padding takes no room, but every
    /// position in it has a place — at the content's near edge — so a caret
    /// an edit leaves there still shows.
    fn layout_table_cell(&self, node: &'ast AstNode<'ast>) -> Layout {
        let span = self.node_range(node); // table cells are always single-line
        let content = self.table_cell_content(span);
        let format = self.text_format(node);
        let mut layout = Layout::new(span);
        if content.is_empty() {
            layout.push_override(span, "", format);
            return layout;
        }

        if span.start() < content.start() {
            layout.push_override((span.start(), content.start()), "", format.clone());
        }
        self.layout_inline_children(&mut layout, node, span);

        // a space typed at the end of a cell is padding once typed; it shows
        // while the caret is still after it
        let typed = self
            .reveal_selection
            .into_iter()
            .flat_map(|selection| [selection.0, selection.1])
            .filter(|&end| content.end() < end && end <= span.end())
            .max()
            .unwrap_or(content.end());
        if content.end() < typed {
            let trailing = (content.end(), typed);
            layout.push_source(trailing, &self.buffer[trailing], format.clone());
        }
        if typed < span.end() {
            layout.push_override((typed, span.end()), "", format);
        }
        layout
    }

    pub fn height_table_cell(&self, node: &'ast AstNode<'ast>) -> f32 {
        let content_height = if self.table_cell_autocompleted(node) {
            self.layout.row_height
        } else {
            let width = self.width(node) - 2.0 * self.layout.block_padding;
            self.compute_layout_from(self.layout_table_cell(node), width, self.layout.row_height)
                .height
        };
        self.layout.block_padding + content_height + self.layout.block_padding
    }

    pub fn width_table_cell(&self, node: &'ast AstNode<'ast>) -> f32 {
        let row = node.parent().unwrap();
        self.width(row) / row.children().count() as f32
    }

    pub fn show_table_cell(&mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, mut top_left: Pos2) {
        // a cell the row does not have is blank and holds no caret
        if self.table_cell_autocompleted(node) {
            return;
        }

        top_left += Vec2::splat(self.layout.block_padding);
        let width = self.width(node) - 2.0 * self.layout.block_padding;

        let result =
            self.compute_layout_from(self.layout_table_cell(node), width, self.layout.row_height);
        self.show_wrap_layout(ui, top_left, &result);
    }

    pub fn compute_bounds_table_cell(&mut self, node: &'ast AstNode<'ast>) {
        if self.table_cell_autocompleted(node) {
            return;
        }
        let content = self.table_cell_content(self.node_range(node));
        self.bounds.inline_paragraphs.push(content);
    }
}
