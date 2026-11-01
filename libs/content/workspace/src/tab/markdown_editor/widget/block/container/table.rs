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
                let header_slots = self.table_header_slots.len();
                self.show_block_children(&mut clipped, node, top_left - Vec2::new(scroll, 0.0));

                // the header fades in its own color, inside the border, which
                // stays put over the fades
                let inside = table.shrink(1.0);
                let header = self
                    .table_header_slots
                    .get(header_slots)
                    .map(|(_, rect)| *rect);
                let header_bottom = header.map_or(inside.top(), |h| h.bottom());
                let bands = [
                    (inside.top(), header_bottom, theme.neutral_bg_secondary()),
                    (header_bottom, inside.bottom(), theme.neutral_bg()),
                ];
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

    /// The width of each of the table's columns, padding included. A
    /// column is as wide as its widest cell on one row where the table's
    /// width allows that; else the columns share the width in proportion,
    /// none narrower than its longest unbreakable run, and cells wrap. If
    /// not even those runs fit, each column is as wide as its own and the
    /// table is wider than the note, in which it scrolls.
    pub fn table_column_widths(&self, table: &'ast AstNode<'ast>) -> Rc<[f32]> {
        if let Some(columns) = self.get_cached_table_columns(table) {
            return columns;
        }

        let available = self.width(table);
        let padding = 2.0 * self.layout.block_padding;
        let row_height = self.layout.row_height;
        // an image's size here is what it will shrink from, never grow to
        let embed_min = 3.0 * row_height;

        let column_count = table
            .first_child()
            .map_or(0, |header| header.children().count());
        let mut narrowest = vec![0f32; column_count];
        let mut widest = vec![0f32; column_count];
        self.layout_cache
            .table_cell_measure
            .set(Some((available - padding).max(0.0)));
        for row in table.children() {
            for (column, cell) in row.children().enumerate().take(column_count) {
                if self.table_cell_autocompleted(cell) {
                    break;
                }
                // an embed is measured at the width it is clamped to
                let measuring = (available - padding).max(0.0).to_bits();
                let key = self.table_cell_stamp(cell);
                let (run, row) = self
                    .table_cell_cached(key.map(|stamp| (stamp, measuring, true)), || {
                        self.measure_layout(self.layout_table_cell(cell), row_height, embed_min)
                    });
                if row > 0.0 {
                    narrowest[column] = narrowest[column].max(run.ceil());
                    // a pixel to spare, or rounding wraps a cell that fits
                    widest[column] = widest[column].max(row.ceil() + 1.0);
                }
            }
        }
        self.layout_cache.table_cell_measure.set(None);
        for column in 0..column_count {
            // room to click into a column, more if it holds nothing yet
            let room = if widest[column] == 0.0 { 3.0 * row_height } else { row_height };
            // and no column wider than the note: a run that long breaks
            let note = (available - padding).max(room);
            narrowest[column] = narrowest[column].max(room).min(note);
            widest[column] = widest[column].max(room);
        }

        let total = |widths: &[f32]| widths.iter().sum::<f32>() + padding * column_count as f32;
        let (least, most) = (total(&narrowest), total(&widest));
        let columns: Rc<[f32]> = (0..column_count)
            .map(|column| {
                let (narrowest, widest) = (narrowest[column], widest[column]);
                padding
                    + if most <= available {
                        widest
                    } else if least <= available {
                        let spare = (available - least) / (most - least);
                        narrowest + (widest - narrowest) * spare
                    } else {
                        narrowest
                    }
            })
            .collect();
        self.set_cached_table_columns(table, columns.clone());
        columns
    }
}
