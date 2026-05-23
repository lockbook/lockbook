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
        if backwards {
            // jump to the closest fragment above that's not above another fragment that's above
            let mut closest_offset: Option<Grapheme> = None;
            let mut closest_distance = f32::INFINITY;
            let mut row_above_top: Option<f32> = None;
            for new_idx in (0..cur_idx).rev() {
                let new_frag = &self.renderer.fragments[new_idx];
                let new_is_above = new_frag.rect.bottom() < cur_frag.rect.top();
                let new_too_above = if let Some(row_above_top) = row_above_top {
                    new_frag.rect.bottom() < row_above_top
                } else {
                    false
                };

                if new_too_above {
                    break;
                } else if new_is_above {
                    row_above_top = Some(new_frag.rect.top());

                    let new_offset = self.renderer.fragment_offset(new_frag, x_target);
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
                } else {
                    continue; // keep going until we're on the prev row (if there is one)
                }
            }

            closest_offset.unwrap_or(offset)
        } else {
            // jump to the closest fragment below that's not below another fragment that's below
            let mut closest_offset: Option<Grapheme> = None;
            let mut closest_distance = f32::INFINITY;
            let mut row_below_bottom: Option<f32> = None;
            for new_idx in cur_idx + 1..self.renderer.fragments.len() {
                let new_frag = &self.renderer.fragments[new_idx];
                let new_is_below = new_frag.rect.top() > cur_frag.rect.bottom();
                let new_too_below = if let Some(row_below_bottom) = row_below_bottom {
                    new_frag.rect.top() > row_below_bottom
                } else {
                    false
                };

                if new_too_below {
                    break;
                } else if new_is_below {
                    row_below_bottom = Some(new_frag.rect.bottom());

                    let new_offset = self.renderer.fragment_offset(new_frag, x_target);
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
                } else {
                    continue; // keep going until we're on the next row (if there is one)
                }
            }

            if let Some(closest_offset) = closest_offset {
                closest_offset
            } else if !self
                .renderer
                .bounds
                .source_lines
                .find_containing(offset, true, true)
                .contains(self.renderer.bounds.source_lines.len() - 1, true, false)
            {
                // if we're in the last fragment but not the last source line it's
                // because the last fragment is hidden (perhaps by a folded node)
                self.renderer.buffer.current.segs.last_cursor_position()
            } else {
                offset
            }
        }
    }

    /// returns the x coordinate of the absolute position of `self` in `fragment`
    fn x(&self, offset: Grapheme) -> Option<f32> {
        let frag = self.renderer.fragment_at_offset(offset)?;
        Some(self.renderer.fragment_x(frag, offset))
    }
}
