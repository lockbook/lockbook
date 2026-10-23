use std::hash::{Hash as _, Hasher as _};

use comrak::nodes::{AstNode, NodeValue, TableAlignment};
use egui::{Pos2, Ui, Vec2};
use lb_rs::model::text::offset_types::RangeExt as _;

use crate::tab::markdown_editor::MdRender;
use crate::tab::markdown_editor::widget::block::TableCellKey;
use crate::tab::markdown_editor::widget::utils::wrap_layout::Layout;

impl<'ast> MdRender {
    /// Lays out a cell's content. Its padding takes no room, but every
    /// position in it has a place — at the content's near edge — so a caret
    /// an edit leaves there still shows.
    pub fn layout_table_cell(&self, node: &'ast AstNode<'ast>) -> Layout {
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
        let measuring = self.layout_cache.table_cell_measure.get().is_some();
        let typed = self
            .reveal_selection
            .into_iter()
            .flat_map(|selection| [selection.0, selection.1])
            .filter(|&end| content.end() < end && end <= span.end() && !measuring)
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

    /// A hash of what the cell's layout is made from, its width aside: its
    /// source and how that parsed, its text's format, and how much of what
    /// it embeds and links to has loaded. `None` while something that
    /// reveals or marks syntax touches the cell: its layout then depends
    /// on where.
    pub fn table_cell_stamp(&self, node: &'ast AstNode<'ast>) -> Option<u64> {
        let span = self.node_range(node);
        let touches = |range: (_, _)| span.intersects(&range, true);
        let measuring = self.layout_cache.table_cell_measure.get().is_some();
        let marked = self.entered_atom.filter(|_| !measuring);
        let marked = marked.into_iter().chain(self.search_range);
        if self.reveal_ranges().chain(marked).any(touches) {
            return None;
        }
        let format = self.text_format(node);
        let loaded = (self.embeds.seq(), self.previews.seq());
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.buffer[span].hash(&mut hasher);
        // a pipe before the cell and definitions elsewhere change the parse
        let column = node.data.borrow().sourcepos.start.column;
        for inline in node.descendants() {
            let inline = inline.data.borrow();
            let (start, end) = (inline.sourcepos.start.column, inline.sourcepos.end.column);
            (start.wrapping_sub(column), end.wrapping_sub(column)).hash(&mut hasher);
            std::mem::discriminant(&inline.value).hash(&mut hasher);
            if let NodeValue::Link(link) | NodeValue::Image(link) = &inline.value {
                link.url.hash(&mut hasher);
            }
        }
        (format.family, format.bold, format.italic).hash(&mut hasher);
        (loaded, self.revealed_spoilers.len()).hash(&mut hasher);
        // an image is sized to the note and the viewport
        let room = [self.width, self.viewport_height].map(f32::to_bits);
        (room, self.disable_images, self.contact_linked_sites).hash(&mut hasher);
        (self.layout.row_height.to_bits(), self.ctx.pixels_per_point().to_bits()).hash(&mut hasher);
        Some(hasher.finish())
    }

    /// What `compute` gives for a cell, kept under `key` for the next
    /// time the cell, or one just like it, is asked the same.
    pub fn table_cell_cached(
        &self, key: Option<TableCellKey>, compute: impl FnOnce() -> (f32, f32),
    ) -> (f32, f32) {
        let Some(key) = key else { return compute() };
        let cached = self.layout_cache.table_cells.borrow().get(&key).copied();
        if let Some(cached) = cached {
            // under test, every hit is checked against what it stands in for
            #[cfg(test)]
            assert_eq!(cached, compute(), "a table cell's stamp misses something of its layout");
            return cached;
        }
        let computed = compute();
        let mut cells = self.layout_cache.table_cells.borrow_mut();
        if cells.len() >= 1 << 16 {
            cells.clear();
        }
        cells.insert(key, computed);
        computed
    }

    pub fn height_table_cell(&self, node: &'ast AstNode<'ast>) -> f32 {
        let content_height = if self.table_cell_autocompleted(node) {
            self.layout.row_height
        } else {
            let width = self.width(node) - 2.0 * self.layout.block_padding;
            let key = self.table_cell_stamp(node);
            let key = key.map(|stamp| (stamp, width.to_bits(), false));
            let height = || {
                let layout = self.layout_table_cell(node);
                (
                    self.compute_layout_from(layout, width, self.layout.row_height)
                        .height,
                    0.0,
                )
            };
            self.table_cell_cached(key, height).0
        };
        self.layout.block_padding + content_height + self.layout.block_padding
    }

    pub fn width_table_cell(&self, node: &'ast AstNode<'ast>) -> f32 {
        if let Some(measuring) = self.layout_cache.table_cell_measure.get() {
            return measuring + 2.0 * self.layout.block_padding;
        }
        let row = node.parent().unwrap();
        let column = row
            .children()
            .take_while(|cell| !cell.same_node(node))
            .count();
        let columns = self.table_column_widths(row.parent().unwrap());
        columns.get(column).copied().unwrap_or_default()
    }

    /// The alignment the delimiter row gives the cell's column.
    fn table_cell_alignment(&self, node: &'ast AstNode<'ast>) -> TableAlignment {
        let row = node.parent().unwrap();
        let column = row
            .children()
            .take_while(|cell| !cell.same_node(node))
            .count();
        match &row.parent().unwrap().data.borrow().value {
            NodeValue::Table(table) => table.alignments.get(column).copied(),
            _ => None,
        }
        .unwrap_or(TableAlignment::None)
    }

    pub fn show_table_cell(&mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, mut top_left: Pos2) {
        // a cell the row does not have is blank and holds no caret
        if self.table_cell_autocompleted(node) {
            return;
        }

        top_left += Vec2::splat(self.layout.block_padding);
        let width = self.width(node) - 2.0 * self.layout.block_padding;

        let mut result =
            self.compute_layout_from(self.layout_table_cell(node), width, self.layout.row_height);

        // each row of the cell's text goes to its column's side
        let spare = match self.table_cell_alignment(node) {
            TableAlignment::None | TableAlignment::Left => 0.0,
            TableAlignment::Center => 0.5,
            TableAlignment::Right => 1.0,
        };
        for row in &mut result.rows {
            let row_width = row
                .fragments
                .iter()
                .fold(0.0f32, |width, fragment| width.max(fragment.rect.right()));
            let shift = Vec2::new(((width - row_width) * spare).max(0.0), 0.0);
            for fragment in &mut row.fragments {
                fragment.rect = fragment.rect.translate(shift);
            }
            for anchor in &mut row.anchors {
                anchor.x += shift.x;
            }
        }
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
