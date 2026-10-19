use std::rc::Rc;

use comrak::nodes::AstNode;
use egui::{Color32, Pos2, Rect, Stroke, Ui, UiBuilder, Vec2};
use lb_rs::model::text::offset_types::{RangeExt as _, RangeIterExt as _};

use crate::tab::markdown_editor::MdRender;
use crate::tab::markdown_editor::table::TableSelection;

use crate::style::Space;
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
                let key = node
                    .first_child()
                    .filter(|_| !self.table_scrolls.is_empty());
                let key = key.map(|header| self.table_row_key(header));
                if let Some(table) = key.and_then(|key| self.table_ordinal(key)) {
                    self.table_scrolls.remove(&table);
                }
                self.show_block_children(ui, node, top_left);
            } else {
                // wider than the note: the table scrolls along its rows
                let scroll = self.table_scroll(ui, node, &columns, table);
                let clip = table.x_range().intersection(ui.clip_rect().x_range());
                let clip = Rect::from_x_y_ranges(clip, ui.clip_rect().y_range());
                let touch_targets = self.touch_targets.len();

                let mut clipped = ui.new_child(UiBuilder::new().max_rect(table));
                clipped.set_clip_rect(clip);
                let header_slots = self.table_header_slots.len();
                self.show_block_children(&mut clipped, node, top_left - Vec2::new(scroll, 0.0));

                // the rows fade out at an edge with more beyond it — painted
                // over the text, once it is — the header in its own color
                let header = self
                    .table_header_slots
                    .get(header_slots)
                    .map(|(_, rect)| *rect);
                let (header_bottom, fade) =
                    (header.map_or(table.top(), |h| h.bottom()), Space::Lg.pts());
                let bands = [
                    (table.top(), header_bottom, theme.neutral_bg_secondary()),
                    (header_bottom, table.bottom(), theme.neutral_bg()),
                ];
                let edges = [
                    (scroll > 0.5, table.left(), table.left() + fade),
                    (scroll + width < content - 0.5, table.right() - fade, table.right()),
                ];
                for (more, x0, x1) in edges {
                    for (y0, y1, color) in bands.iter().filter(|_| more) {
                        let rect = Rect::from_x_y_ranges(x0..=x1, *y0..=*y1);
                        let (left, right) = if x0 == table.left() {
                            (*color, Color32::TRANSPARENT)
                        } else {
                            (Color32::TRANSPARENT, *color)
                        };
                        self.table_fades.push((rect.intersect(clip), left, right));
                    }
                }

                // what the table hides of a cell is not there to touch
                let shown = self.touch_targets.split_off(touch_targets).into_iter();
                let shown = shown.map(|(rect, target)| (rect.intersect(clip), target));
                self.touch_targets
                    .extend(shown.filter(|(rect, _)| rect.is_positive()));

                // a thumb along the bottom border says there is more
                let track = table.shrink2(Vec2::new(2.0, 1.0));
                let thumb = Vec2::new(track.width() * width / content, 3.0);
                let travel = (track.width() - thumb.x) * scroll / (content - width);
                let thumb =
                    Rect::from_min_size(track.left_bottom() + Vec2::new(travel, -thumb.y), thumb);
                ui.painter().rect_filled(thumb, 1.5, theme.neutral());
            }

            // draw exterior decoration
            ui.painter().rect_stroke(
                table,
                2.,
                Stroke { width: 1., color: theme.neutral_bg_tertiary() },
                egui::epaint::StrokeKind::Inside,
            );
            self.show_table_handles(ui, first_cell_box);
        }
    }

    /// How far a table wider than the note is scrolled along its rows:
    /// where a wheel over it or a touch drag (`MdEdit::pan_table`) has left
    /// it, or where the end of the selection, when the user moves it, has
    /// its column in view.
    fn table_scroll(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, columns: &[f32], viewport: Rect,
    ) -> f32 {
        let key = node.first_child().map(|header| self.table_row_key(header));
        let Some(ordinal) = key.and_then(|key| self.table_ordinal(key)) else { return 0.0 };
        let scrolled = self.table_scrolls.get(&ordinal).copied();
        let mut scroll = scrolled.unwrap_or_default();

        if ui.rect_contains_pointer(viewport) {
            let wheel = ui.input_mut(|i| std::mem::take(&mut i.smooth_scroll_delta.x));
            scroll.offset -= wheel;
            if wheel != 0.0 {
                scroll.velocity = 0.0;
            }
        }
        // coasting after a pan, slowing as the page does
        if scroll.velocity.abs() > 1.0 {
            const DECAY_PER_SEC: f32 = 4.0;
            let dt = ui.input(|i| i.stable_dt).max(0.0001);
            scroll.offset += scroll.velocity * dt;
            scroll.velocity *= (-DECAY_PER_SEC * dt).exp();
            ui.ctx().request_repaint();
        } else {
            scroll.velocity = 0.0;
        }

        // a match the reader is shown, or the end of a selection the user
        // moved unless it has whole rows: the table scrolls to its column
        let selection = self.buffer.current.selection;
        let shown = [self.find_current_match, self.preview_match];
        let moved = if shown != scroll.shown {
            shown.iter().flatten().next().map(|shown| shown.start())
        } else if self.caret_moved || scrolled.is_none() {
            let rows = matches!(self.table_selection(selection), Some(TableSelection::Rows { .. }));
            (!rows).then_some(selection.1)
        } else {
            None
        };
        scroll.shown = shown;
        let table = &self.bounds.tables[ordinal];
        let in_table = moved.filter(|moved| self.bounds.table_at(*moved) == Some(table));
        if let Some((_, column)) = in_table.and_then(|moved| table.cell_at(moved)) {
            let left: f32 = columns[..column].iter().sum();
            let right = left + columns[column];
            if left < scroll.offset {
                scroll.offset = left;
            } else if right > scroll.offset + viewport.width() {
                scroll.offset = right - viewport.width();
            }
            scroll.velocity = 0.0;
        }

        let content: f32 = columns.iter().sum();
        let clamped = scroll
            .offset
            .clamp(0.0, (content - viewport.width()).max(0.0));
        // an edge stops a coast
        if clamped != scroll.offset {
            scroll.velocity = 0.0;
        }
        scroll.offset = clamped;
        self.table_scrolls.insert(ordinal, scroll);
        scroll.offset
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
