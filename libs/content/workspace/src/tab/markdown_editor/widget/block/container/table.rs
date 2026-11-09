use std::rc::Rc;

use comrak::nodes::AstNode;
use egui::{Pos2, Rangef, Rect, Ui, UiBuilder, Vec2};
use lb_rs::model::text::offset_types::{RangeExt as _, RangeIterExt as _};

use crate::tab::markdown_editor::input::{Event, Location, Region};
use crate::tab::markdown_editor::row_scroll::RowScroller;
use crate::tab::markdown_editor::table::TableSelection;
use crate::tab::markdown_editor::{MdRender, TouchTarget};

use crate::theme::palette_v2::ThemeExt as _;

impl<'ast> MdRender {
    pub fn height_table(&self, node: &'ast AstNode<'ast>) -> f32 {
        let width = self.width(node);
        let row_height = self.layout.row_height;
        if self.table_revealed(node) {
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

        if self.table_revealed(node) {
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
            let first_cell_box = self.table_cell_boxes.len();
            let columns = self.table_column_widths(node);
            let content: f32 = columns.iter().sum();
            let size = Vec2::new(content.min(width), self.block_children_height(node));
            let table = Rect::from_min_size(top_left, size);
            let theme = self.ctx.get_lb_theme();

            if content <= width + 0.5 {
                // it may have scrolled while it was wider
                let key = node.first_child().filter(|_| !self.row_scrolls.is_empty());
                let key = key.map(|header| self.table_row_key(header));
                if let Some(table) = key.and_then(|key| self.table_ordinal(key)) {
                    self.row_scrolls.remove(&RowScroller::Table(table));
                }
                self.show_block_children(ui, node, top_left);
            } else {
                // wider than the note: the table scrolls along its rows
                let scroll = self.table_scroll(ui, node, &columns, table);
                // text scrolls inside the border, which stays put over it
                let clip = table
                    .shrink(1.0)
                    .x_range()
                    .intersection(ui.clip_rect().x_range());
                let clip = Rect::from_x_y_ranges(clip, ui.clip_rect().y_range());
                let touch_targets = self.touch_targets.len();

                let mut clipped = ui.new_child(UiBuilder::new().max_rect(table));
                clipped.set_clip_rect(clip);
                self.show_block_children(&mut clipped, node, top_left - Vec2::new(scroll, 0.0));

                // rows fade into the page, the header's ground too, so an edge
                // with more beyond it shows; the border stays put over the fades
                let inside = table.shrink(1.0);
                let bands = [(inside.top(), inside.bottom(), theme.neutral_bg())];
                let key = node.first_child().map(|header| self.table_row_key(header));
                if let Some(ordinal) = key.and_then(|key| self.table_ordinal(key)) {
                    let key = RowScroller::Table(ordinal);
                    let content = content + inside.width() - table.width();
                    self.row_scroll_chrome(ui, key, inside, clip, content, &bands);
                }
                // the rules between rows run edge to edge over the fades, as
                // the border stays put; a drag float paints no fades
                if !self.painting_drag_float {
                    let stroke = egui::Stroke::new(1., theme.neutral_bg_tertiary());
                    let span = inside.x_range().intersection(clip.x_range());
                    let mut y = top_left.y;
                    for row in node.children() {
                        if y > top_left.y {
                            self.over_text.push(egui::Shape::hline(span, y, stroke));
                        }
                        y += self.height_table_row(row);
                    }
                }

                // what the table hides of a cell is not there to touch
                let shown = self.touch_targets.split_off(touch_targets).into_iter();
                let shown = shown.map(|(rect, target)| (rect.intersect(clip), target));
                self.touch_targets
                    .extend(shown.filter(|(rect, _)| rect.is_positive()));
            }

            // draw exterior decoration
            crate::style::paint_doc_border(ui.painter(), table, 2, &theme);
            self.show_table_handles(ui, first_cell_box);

            // below a table that ends the note a tap edits (`MdEdit::tap`):
            // a target, so the platform brings the tap to the editor
            let last = self.buffer.current.segs.last_cursor_position();
            let trailing = self
                .bounds
                .table_at(self.node_range(node).start())
                .is_some_and(|t| self.bounds.source_lines[t.lines().1 - 1].end() == last);
            if trailing && !self.readonly {
                let x = ui.max_rect().x_range();
                let below = Rect::from_x_y_ranges(x, table.bottom()..=table.bottom() + 100_000.0);
                let region = Region::Location(Location::Pos(below.left_top() + Vec2::splat(1.0)));
                self.touch_targets
                    .push((below, TouchTarget::Tap(Event::Select { region })));
            }
        }
    }

    /// How far a table wider than the note is scrolled along its rows (see
    /// [`MdRender::row_scroll`]); a moved selection that has whole rows
    /// leaves it where it is.
    fn table_scroll(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, columns: &[f32], viewport: Rect,
    ) -> f32 {
        let key = node.first_child().map(|header| self.table_row_key(header));
        let Some(ordinal) = key.and_then(|key| self.table_ordinal(key)) else { return 0.0 };
        if !self.painting_drag_float {
            self.row_scrollers
                .push((viewport.intersect(ui.clip_rect()), RowScroller::Table(ordinal)));
        }
        let content: f32 = columns.iter().sum();
        self.row_scroll(ui, RowScroller::Table(ordinal), viewport, content, |r, at, caret| {
            let selection = r.buffer.current.selection;
            if caret && matches!(r.table_selection(selection), Some(TableSelection::Rows { .. })) {
                return None;
            }
            let table = &r.bounds.tables[ordinal];
            let (_, column) = table
                .cell_at(at)
                .filter(|_| r.bounds.table_at(at) == Some(table))?;
            let left: f32 = columns[..column].iter().sum();
            Some(Rangef::new(left, left + columns[column]))
        })
    }

    /// The width of each of the table's columns, padding included: as
    /// wide as its widest cell's content on one row, between a column's
    /// narrowest and widest. Past the widest, text wraps; a table wider
    /// than the note scrolls.
    pub fn table_column_widths(&self, table: &'ast AstNode<'ast>) -> Rc<[f32]> {
        if let Some(columns) = self.get_cached_table_columns(table) {
            return columns;
        }
        let count = table
            .first_child()
            .map_or(0, |header| header.children().count());
        let padding = 2.0 * self.layout.block_padding;
        let (narrowest, widest) = (self.layout.table_column_width, self.layout.table_column_max);
        let mut widths = vec![narrowest; count];
        self.layout_cache.table_cell_measuring.set(true);
        for row in table.children() {
            for (column, cell) in row.children().enumerate().take(count) {
                if self.table_cell_autocompleted(cell) {
                    break;
                }
                // a point to spare, or rounding wraps a cell that fits
                let fit = padding + self.table_cell_fit(cell, widest - padding).ceil() + 1.0;
                widths[column] = widths[column].max(fit.min(widest));
            }
        }
        self.layout_cache.table_cell_measuring.set(false);
        let columns: Rc<[f32]> = widths.into();
        self.set_cached_table_columns(table, columns.clone());
        columns
    }

    /// How wide `cell`'s content is laid out within `room`: its widest row,
    /// so what wraps there, as text or a stack of embeds, is as wide as it
    /// is wrapped.
    fn table_cell_fit(&self, cell: &'ast AstNode<'ast>, room: f32) -> f32 {
        let key = self
            .table_cell_stamp(cell)
            .map(|stamp| (stamp, room.to_bits(), true));
        self.table_cell_cached(key, || {
            let layout = self.layout_table_cell(cell);
            let layout = self.compute_layout_from(layout, room, self.layout.row_height);
            // a row holding a background, as a chip, keeps its padding's room, as wrapping does
            let room = 2.0 * self.layout.inline_padding;
            let row_width = |row: &crate::tab::markdown_editor::widget::utils::wrap_layout::Row| {
                let right = row
                    .fragments
                    .iter()
                    .map(|f| f.rect.right())
                    .fold(0.0, f32::max);
                let background = row.fragments.iter().any(|f| {
                    f.style_stack
                        .iter()
                        .any(|s| s.format.background != egui::Color32::TRANSPARENT)
                });
                if background { right + room } else { right }
            };
            layout.rows.iter().map(row_width).fold(0.0, f32::max)
        })
    }
}
