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
        let cur_frag = &self.renderer.fragments[cur_idx];

        // Climb one visual row at a time until the cursor reaches a
        // distinct offset. A soft-wrap boundary shares one offset between a
        // row's end and the next row's start; landing back on `offset`
        // wouldn't move the cursor, so the scan skips it and climbs on when
        // a row offers nothing else.
        let mut row_cutoff = if backwards { cur_frag.rect.top() } else { cur_frag.rect.bottom() };
        loop {
            let (found, row_edge) = self
                .closest_distinct_offset_on_row(cur_idx, row_cutoff, offset, x_target, backwards);
            if let Some(found) = found {
                return found;
            }
            match row_edge {
                Some(edge) => row_cutoff = edge, // row had only the self-match; keep climbing
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

    /// Scans the visual row adjacent to `row_cutoff` (the row just above it
    /// when `backwards`, just below otherwise) for the offset closest to
    /// `x_target`, ignoring matches equal to `from` (the soft-wrap self-
    /// match). Returns the chosen offset and the scanned row's far edge —
    /// its top when climbing up, bottom when climbing down — so the caller
    /// can keep climbing past a row that yields nothing distinct.
    fn closest_distinct_offset_on_row(
        &self, cur_idx: usize, row_cutoff: f32, from: Grapheme, x_target: f32, backwards: bool,
    ) -> (Option<Grapheme>, Option<f32>) {
        let fragments = &self.renderer.fragments;
        let mut closest_offset: Option<Grapheme> = None;
        let mut closest_distance = f32::INFINITY;
        let mut row_edge: Option<f32> = None;

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
            let new_frag = &fragments[idx];

            // Adjacent in the travel direction, and not yet onto the row
            // past the first one we land on.
            let adjacent = if backwards {
                new_frag.rect.bottom() < row_cutoff
            } else {
                new_frag.rect.top() > row_cutoff
            };
            let past_row = row_edge.is_some_and(|edge| {
                if backwards { new_frag.rect.bottom() < edge } else { new_frag.rect.top() > edge }
            });

            if past_row {
                break;
            } else if adjacent {
                row_edge =
                    Some(if backwards { new_frag.rect.top() } else { new_frag.rect.bottom() });

                let new_offset = self.renderer.fragment_offset(new_frag, x_target);
                if new_offset == from {
                    continue; // wrap-boundary self-match; would not move the cursor
                }
                let new_x = self.renderer.fragment_x(new_frag, new_offset);
                let distance = (new_x - x_target).abs(); // closest as in closest to target

                // prefer empty fragments which are placed deliberately to affect such behavior
                if distance < closest_distance
                    || (distance == closest_distance
                        && new_frag.source_range.start() == new_frag.source_range.end())
                {
                    closest_offset = Some(new_offset);
                    closest_distance = distance;
                }
            }
        }

        (closest_offset, row_edge)
    }

    /// returns the x coordinate of the absolute position of `self` in `fragment`
    fn x(&self, offset: Grapheme) -> Option<f32> {
        let frag = self.renderer.fragment_at_offset(offset)?;
        Some(self.renderer.fragment_x(frag, offset))
    }
}
