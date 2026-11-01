use std::mem;

use crate::tab::markdown_editor::MdEdit;
use crate::tab::markdown_editor::View;
use crate::tab::markdown_editor::bounds::BoundExt as _;
use crate::tab::markdown_editor::input::{Advance, Bound, Increment};
use crate::tab::markdown_editor::table::{CellBounds, TableCellBox};
use crate::tab::markdown_editor::widget::utils::wrap_layout::Fragment;
use lb_rs::model::text::offset_types::{Grapheme, RangeExt as _};

impl MdEdit {
    pub fn advance(&mut self, offset: Grapheme, advance: Advance, backwards: bool) -> Grapheme {
        let maybe_x_target_value = mem::take(&mut self.cursor.x_target);
        let result = match advance {
            Advance::To(bound) => offset.advance_to_bound(bound, backwards, &self.renderer.bounds),
            Advance::Next(bound) => {
                offset.advance_to_next_bound(bound, backwards, &self.renderer.bounds)
            }
            Advance::By(Increment::Char) => {
                let mut result = offset;
                if backwards {
                    if result.0 > 0 {
                        result -= 1;
                    }
                } else {
                    result += 1;
                    result = result.min(self.renderer.buffer.current.segs.last_cursor_position());
                }
                result
            }
            Advance::By(Increment::Lines(n)) => {
                let mut result = offset;
                for _ in 0..n {
                    let Some(result_x) = self.x(result) else {
                        match self.table_advance_from_hidden_cell(result, backwards) {
                            Some(next) => result = next,
                            None => break,
                        }
                        continue;
                    };
                    let x_target = maybe_x_target_value.unwrap_or(result_x);
                    result = self.advance_by_line(result, x_target, backwards);
                    if result != 0
                        && result != self.renderer.buffer.current.segs.last_cursor_position()
                    {
                        self.cursor.x_target = Some(x_target);
                    }
                }
                result
            }
        };
        let result = self.renderer.snap_offset_out_of_folds(result, backwards);
        // in a cell a line is the cell's own: its ends are the content's,
        // and from an end the next cell's is the line
        let cell = self.renderer.bounds.table_at(offset).and_then(|table| {
            let (row, column) = table.cell_at(offset)?;
            Some((&table.rows[row], column))
        });
        if let Some((row, column)) = cell.filter(|_| advance == Advance::To(Bound::Line)) {
            let content = row.cells[column].content;
            let next = match backwards {
                true if offset == content.start() => column.checked_sub(1),
                false if offset == content.end() => Some(column + 1),
                _ => None,
            };
            if let Some(next) = next.and_then(|next| row.cells.get(next)) {
                return if backwards { next.content.start() } else { next.content.end() };
            }
            return result.max(content.start()).min(content.end());
        }
        self.renderer
            .snap_offset_into_table_cell(offset, result, backwards)
    }

    /// `n` visual lines from `offset`, holding its column. Pure: a platform
    /// text system keeps the column by always moving from the same start.
    pub fn lines_from(&self, offset: Grapheme, n: usize, backwards: bool) -> Grapheme {
        let Some(x_target) = self.x(offset) else { return offset };
        let result =
            (0..n).fold(offset, |result, _| self.advance_by_line(result, x_target, backwards));
        let result = self.renderer.snap_offset_out_of_folds(result, backwards);
        self.renderer
            .snap_offset_into_table_cell(offset, result, backwards)
    }

    /// One visual row up or down from `offset`, toward `x_target`. In a
    /// table that is a row of the cell's own text, else the cell above or
    /// below, else the row beyond the table's edge.
    pub(crate) fn advance_by_line(
        &self, offset: Grapheme, x_target: f32, backwards: bool,
    ) -> Grapheme {
        if let Some(result) = self.table_advance_by_line(offset, x_target, backwards) {
            return result;
        }
        let result = self
            .advance_by_row(offset, x_target, backwards, &|_| true)
            .unwrap_or_else(|| self.document_edge_past(backwards));
        self.table_enter_by_line(offset, result, x_target, backwards)
    }

    /// [`Self::advance_by_line`] from a table cell; `None` from elsewhere.
    fn table_advance_by_line(
        &self, offset: Grapheme, x_target: f32, backwards: bool,
    ) -> Option<Grapheme> {
        if self.renderer.view == View::Source {
            return None;
        }
        let table = self.renderer.bounds.table_at(offset)?;
        let (row, column) = table.cell_at(offset)?;
        let cell = table.rows[row].cells[column];
        let in_cell = |f: &Fragment| cell.span.contains_range(&f.source_range, true, true);
        if !self.renderer.fragments.iter().any(in_cell) {
            return None; // shown as source
        }

        if let Some(result) = self.advance_by_row(offset, x_target, backwards, &in_cell) {
            return Some(result.max(cell.content.start()).min(cell.content.end()));
        }
        if let Some(neighbor) = self.table_row_past(offset, backwards) {
            let row = &table.rows[neighbor];
            let cell = row.cells.get(column).or(row.cells.last())?;
            return self.table_cell_offset_near_x(cell, x_target, backwards);
        }

        let lines = &self.renderer.bounds.source_lines;
        let (first, end) = table.lines();
        let table_lines = (lines[first].start(), lines[end - 1].end());
        let outside = |f: &Fragment| !table_lines.contains_range(&f.source_range, true, true);
        // with nothing beyond the table's edge, the caret stays in its cell
        Some(
            self.advance_by_row(offset, x_target, backwards, &outside)
                .unwrap_or(offset),
        )
    }

    /// One row up or down from a cell that has no place on screen because
    /// its table, too wide for the note, is scrolled past it: the same
    /// column of the next row, or the line past the table. `None` from
    /// anywhere else.
    fn table_advance_from_hidden_cell(
        &self, offset: Grapheme, backwards: bool,
    ) -> Option<Grapheme> {
        if self.renderer.view == View::Source {
            return None;
        }
        let r = &self.renderer;
        let table = r.bounds.table_at(offset)?;
        let (_, column) = table.cell_at(offset)?;
        let key = Some(table.rows[0].line.start());
        let painted = |cell_box: &TableCellBox| r.table_key(cell_box.row) == key;
        if self.x(offset).is_some() || !r.table_cell_boxes.iter().any(painted) {
            return None;
        }
        let (lines, (first, end)) = (&r.bounds.source_lines, table.lines());
        Some(match self.table_row_past(offset, backwards) {
            Some(neighbor) => {
                let row = &table.rows[neighbor];
                row.cells.get(column).or(row.cells.last())?.content.start()
            }
            None if backwards => first
                .checked_sub(1)
                .map_or(offset, |line| lines[line].end()),
            None => lines.get(end).map_or(offset, |line| line.start()),
        })
    }

    /// The row above or below the one holding `offset`, in the same table.
    pub(crate) fn table_row_past(&self, offset: Grapheme, backwards: bool) -> Option<usize> {
        let table = self.renderer.bounds.table_at(offset)?;
        let (row, _) = table.cell_at(offset)?;
        if backwards { row.checked_sub(1) } else { (row + 1 < table.rows.len()).then_some(row + 1) }
    }

    /// Whether `offset` is on its cell's first (`backwards`) or last visual
    /// row, so that a vertical move from it leaves the cell.
    pub(crate) fn table_cell_edge_row(&self, offset: Grapheme, backwards: bool) -> bool {
        let cell = self.renderer.bounds.table_at(offset).and_then(|table| {
            let (row, column) = table.cell_at(offset)?;
            Some(table.rows[row].cells[column])
        });
        let Some(cell) = cell else { return false };
        let in_cell = |f: &Fragment| cell.span.contains_range(&f.source_range, true, true);
        let Some(x) = self.x(offset) else { return false };
        self.renderer.fragments.iter().any(in_cell)
            && self
                .advance_by_row(offset, x, backwards, &in_cell)
                .is_none()
    }

    /// A vertical move from outside a table lands in the cell under
    /// `x_target`, not the one whose text happens to be nearest.
    fn table_enter_by_line(
        &self, origin: Grapheme, result: Grapheme, x_target: f32, backwards: bool,
    ) -> Grapheme {
        if self.renderer.view == View::Source {
            return result;
        }
        let r = &self.renderer;
        let Some(table) = r.bounds.table_at(result) else { return result };
        if r.bounds.table_at(origin) == Some(table) {
            return result;
        }
        let Some(row) = table.row_at(result).map(|row| &table.rows[row]) else { return result };
        let column = r
            .table_cell_boxes
            .iter()
            .filter(|cell_box| cell_box.row == row.line.start())
            .min_by(|a, b| {
                let distance = |rect: egui::Rect| {
                    (x_target - rect.left().max(x_target.min(rect.right()))).abs()
                };
                distance(a.rect).total_cmp(&distance(b.rect))
            })
            .map(|cell_box| cell_box.column);
        column
            .and_then(|column| row.cells.get(column).or(row.cells.last()))
            .and_then(|cell| self.table_cell_offset_near_x(cell, x_target, backwards))
            .unwrap_or(result)
    }

    /// The position in a cell's content nearest `x_target`, on the cell's
    /// last visual row (`bottom`) or its first.
    fn table_cell_offset_near_x(
        &self, cell: &CellBounds, x_target: f32, bottom: bool,
    ) -> Option<Grapheme> {
        let r = &self.renderer;
        let fragments = || {
            r.fragments
                .iter()
                .filter(|f| !f.far && cell.span.contains_range(&f.source_range, true, true))
        };
        let edge = if bottom {
            fragments().max_by(|a, b| a.rect.bottom().total_cmp(&b.rect.bottom()))
        } else {
            fragments().min_by(|a, b| a.rect.top().total_cmp(&b.rect.top()))
        }?;
        let on_row =
            |f: &&Fragment| f.rect.top() < edge.rect.bottom() && edge.rect.top() < f.rect.bottom();
        fragments()
            .filter(|f| on_row(f) || f.rect.top() == edge.rect.top())
            .map(|f| {
                let offset = r.fragment_offset(f, x_target);
                (offset, (r.fragment_x(f, offset) - x_target).abs())
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(offset, _)| offset.max(cell.content.start()).min(cell.content.end()))
    }

    /// One visual row up or down from `offset` among the fragments `allow`
    /// admits; `None` if they hold no row in that direction.
    fn advance_by_row(
        &self, offset: Grapheme, x_target: f32, backwards: bool, allow: &dyn Fn(&Fragment) -> bool,
    ) -> Option<Grapheme> {
        // Mirror `fragment_at_offset`'s last-match semantics so navigation
        // agrees with cursor rendering at wrap boundaries.
        let Some(cur_idx) = self
            .renderer
            .fragments
            .iter()
            .enumerate()
            .rev()
            .find_map(|(i, f)| {
                let (s, e) = f.source_range;
                (s <= offset && offset <= e).then_some(i)
            })
        else {
            return Some(offset);
        };
        // A far row has no laid-out neighbors to move to.
        if self.renderer.fragments[cur_idx].far {
            return Some(offset);
        }
        let cur_top = self.renderer.fragments[cur_idx].rect.top();

        // Walk one visual row at a time in the travel direction, nearest
        // first, taking the first row with a candidate that actually
        // *renders* past the current row. Climbing past a row with no such
        // candidate steps over wrap seams whose only offsets render back
        // onto the current row.
        let mut row_cutoff =
            if backwards { cur_top } else { self.renderer.fragments[cur_idx].rect.bottom() };
        loop {
            let (best, row_edge) =
                self.best_offset_on_row(cur_idx, row_cutoff, cur_top, x_target, backwards, allow);
            if let Some(best) = best {
                return Some(best);
            }
            // Nothing on that row rendered past us: keep climbing, unless
            // there is no further row in this direction.
            row_cutoff = row_edge?;
        }
    }

    /// Where a vertical move goes with no row left to reach: the
    /// document's edge.
    fn document_edge_past(&self, backwards: bool) -> Grapheme {
        if backwards {
            Grapheme(0)
        } else {
            self.renderer.buffer.current.segs.last_cursor_position()
        }
    }

    /// Picks the destination on the visual row adjacent to `row_cutoff` (just
    /// above it when `backwards`, just below otherwise): the candidate closest
    /// to `x_target` that *renders* past `cur_top` in the travel direction.
    /// Also returns the row's far edge (top going up, bottom going down) so the
    /// caller can climb past a row with no such candidate.
    ///
    /// Candidates are each fragment's closest point to `x_target`, plus the
    /// grapheme just inside the row's far edge (`row_end - 1` up, `row_start +
    /// 1` down) for nonempty rows — all ordered only by distance to `x_target`.
    /// The inner grapheme matters at a soft-wrap seam: the row's end offset is
    /// closest to a right-leaning `x_target` but renders on the row below (so it
    /// fails the test), leaving the inner grapheme as the nearest candidate that
    /// renders on the row itself. Down's `row_start + 1` never wins — last-match
    /// resolves a seam to the lower row, already correct for down — but is kept
    /// for symmetry.
    fn best_offset_on_row(
        &self, cur_idx: usize, row_cutoff: f32, cur_top: f32, x_target: f32, backwards: bool,
        allow: &dyn Fn(&Fragment) -> bool,
    ) -> (Option<Grapheme>, Option<f32>) {
        let fragments = &self.renderer.fragments;

        // Gather the adjacent row's fragments: each contributes its closest
        // point to `x_target`, while we track the row's source extent and
        // the fragments holding its far edges for the inner-grapheme step.
        let mut row_edge: Option<f32> = None;
        let mut candidates: Vec<(Grapheme, f32, bool)> = Vec::new(); // (offset, gen_x, empty_frag)
        let (mut lo, mut hi) = (Grapheme(usize::MAX), Grapheme(0));
        let (mut lo_idx, mut hi_idx) = (None, None);

        let mut idx = cur_idx;
        loop {
            if backwards {
                if idx == 0 {
                    break;
                }
                idx -= 1;
            } else {
                idx += 1;
                if idx >= fragments.len() {
                    break;
                }
            }
            let frag = &fragments[idx];
            if frag.far || !allow(frag) {
                continue;
            }

            // Adjacent in the travel direction, and not yet onto the row
            // past the first one we land on.
            let adjacent = if backwards {
                frag.rect.bottom() < row_cutoff
            } else {
                frag.rect.top() > row_cutoff
            };
            let past_row = row_edge.is_some_and(|edge| {
                if backwards { frag.rect.bottom() < edge } else { frag.rect.top() > edge }
            });

            if past_row {
                break;
            } else if adjacent {
                row_edge = Some(if backwards { frag.rect.top() } else { frag.rect.bottom() });

                let (s, e) = (frag.source_range.start(), frag.source_range.end());
                if s < lo {
                    (lo, lo_idx) = (s, Some(idx));
                }
                if e > hi {
                    (hi, hi_idx) = (e, Some(idx));
                }

                let off = self.renderer.fragment_offset(frag, x_target);
                candidates.push((off, self.renderer.fragment_x(frag, off), s == e));
            }
        }

        // The grapheme just inside the row's far edge — lets an up-press
        // land on a soft-wrapped row rather than on its seam (see above).
        if lo < hi {
            if backwards {
                if let Some(i) = hi_idx {
                    let off = Grapheme(hi.0 - 1);
                    candidates.push((off, self.renderer.fragment_x(&fragments[i], off), false));
                }
            } else if let Some(i) = lo_idx {
                let off = Grapheme(lo.0 + 1);
                candidates.push((off, self.renderer.fragment_x(&fragments[i], off), false));
            }
        }

        // Closest candidate to `x_target` that renders past the current row
        // in the travel direction. Ties prefer empty fragments, placed
        // deliberately to steer this navigation.
        let mut best: Option<Grapheme> = None;
        let mut best_distance = f32::INFINITY;
        for (off, gen_x, empty) in candidates {
            let renders_past = self.renderer.fragment_at_offset(off).is_some_and(|f| {
                if backwards { f.rect.top() < cur_top } else { f.rect.top() > cur_top }
            });
            if !renders_past {
                continue;
            }
            let distance = (gen_x - x_target).abs();
            if distance < best_distance || (distance == best_distance && empty) {
                (best, best_distance) = (Some(off), distance);
            }
        }

        (best, row_edge)
    }

    /// returns the x coordinate of the absolute position of `self` in `fragment`
    fn x(&self, offset: Grapheme) -> Option<f32> {
        let frag = self.renderer.fragment_at_offset(offset)?;
        Some(self.renderer.fragment_x(frag, offset))
    }
}
