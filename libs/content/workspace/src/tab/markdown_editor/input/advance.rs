use std::mem;

use crate::tab::markdown_editor::MdEdit;
use crate::tab::markdown_editor::bounds::{BoundExt as _, RangesExt as _};
use crate::tab::markdown_editor::input::{Advance, Increment};
use lb_rs::model::text::offset_types::{Grapheme, RangeExt as _};

impl MdEdit {
    pub fn advance(&mut self, offset: Grapheme, advance: Advance, backwards: bool) -> Grapheme {
        let maybe_x_target_value = mem::take(&mut self.cursor.x_target);
        match advance {
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
                        break;
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
        }
    }

    fn advance_by_line(&self, offset: Grapheme, x_target: f32, backwards: bool) -> Grapheme {
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
            return offset;
        };
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
                self.best_offset_on_row(cur_idx, row_cutoff, cur_top, x_target, backwards);
            if let Some(best) = best {
                return best;
            }
            match row_edge {
                Some(edge) => row_cutoff = edge, // nothing rendered past us; keep climbing
                None => break,                   // no further row in this direction
            }
        }

        // No reachable row remains. If we're not on the document's edge
        // source line, the adjacent line is hidden (e.g. by a fold), so
        // snap to that document edge; otherwise stay put.
        let source_lines = &self.renderer.bounds.source_lines;
        let edge_line = if backwards { 0 } else { source_lines.len() - 1 };
        if source_lines
            .find_containing(offset, true, true)
            .contains(edge_line, true, false)
        {
            offset
        } else if backwards {
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
