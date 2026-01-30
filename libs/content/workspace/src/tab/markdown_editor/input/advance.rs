use std::mem;

use crate::tab::markdown_editor::Editor;
use crate::tab::markdown_editor::bounds::{BoundExt as _, RangesExt};
use crate::tab::markdown_editor::input::{Advance, Increment};
use egui::Vec2;
use lb_rs::model::text::offset_types::{DocCharOffset, RangeExt};

use super::cursor;

impl Editor {
    pub fn advance(
        &mut self, offest: DocCharOffset, advance: Advance, backwards: bool,
    ) -> DocCharOffset {
        let maybe_x_target_value = mem::take(&mut self.cursor.x_target);
        match advance {
            Advance::To(bound) => offest.advance_to_bound(bound, backwards, &self.bounds),
            Advance::Next(bound) => offest.advance_to_next_bound(bound, backwards, &self.bounds),
            Advance::By(Increment::Lines(n)) => {
                let mut result = offest;
                for _ in 0..n {
                    let Some(result_x) = self.x(result) else {
                        break;
                    };
                    let x_target = maybe_x_target_value.unwrap_or(result_x);
                    result = self.advance_by_line(result, x_target, backwards);
                    if result != 0 && result != self.buffer.current.segs.last_cursor_position() {
                        self.cursor.x_target = Some(x_target);
                    }
                }
                result
            }
        }
    }

    fn advance_by_line(
        &self, offset: DocCharOffset, x_target: f32, backwards: bool,
    ) -> DocCharOffset {
        let Some((cur_galley_idx, cur_ecursor)) = self.galleys.galley_and_cursor_by_offset(offset)
        else {
            return offset;
        };
        let cur_galley = &self.galleys[cur_galley_idx];
        if backwards {
            let at_top_of_cur_galley = cur_ecursor.rcursor.row == 0;
            if !at_top_of_cur_galley {
                // within a galley: just move up one row
                let new_cursor = cur_galley.galley.cursor_up_one_row(&cur_ecursor);
                return self
                    .galleys
                    .offset_by_galley_and_cursor(cur_galley, new_cursor);
            }

            // jump to the closest galley above that's not above another galley that's above
            let mut closest_offset: Option<DocCharOffset> = None;
            let mut closest_distance = f32::INFINITY;
            let mut row_above_top: Option<f32> = None;
            for new_galley_idx in (0..cur_galley_idx).rev() {
                let new_galley = &self.galleys[new_galley_idx];
                let new_galley_is_above = new_galley.rect.bottom() < cur_galley.rect.top();
                let new_galley_too_above = if let Some(row_above_top) = row_above_top {
                    new_galley.rect.bottom() < row_above_top
                } else {
                    false
                };

                if new_galley_too_above {
                    break;
                } else if new_galley_is_above {
                    row_above_top = Some(new_galley.rect.top());

                    let mut new_cursor = new_galley.galley.cursor_from_pos(Vec2 {
                        x: 0.0, // overwritten next line
                        y: new_galley.rect.bottom(),
                    });
                    new_cursor =
                        cursor::from_x(x_target, &self.galleys[new_galley_idx], new_cursor);

                    let pos = cursor::cursor_to_pos_abs(new_galley, new_cursor);
                    let distance = (pos.x - x_target).abs(); // closest as in closest to target

                    // prefer empty galleys which are placed deliberately to affect such behavior
                    if distance < closest_distance
                        || (distance == closest_distance && new_galley.range.is_empty())
                    {
                        closest_offset = Some(
                            self.galleys
                                .offset_by_galley_and_cursor(new_galley, new_cursor),
                        );
                        closest_distance = distance;
                    }
                } else {
                    continue; // keep going until we're on the prev row (if there is one)
                }
            }

            closest_offset.unwrap_or(offset)
        } else {
            let at_bottom_of_cur_galley =
                cur_ecursor.rcursor.row == cur_galley.galley.rows.len() - 1;
            if !at_bottom_of_cur_galley {
                // within a galley: just move down one row
                let new_cursor = cur_galley.galley.cursor_down_one_row(&cur_ecursor);
                return self
                    .galleys
                    .offset_by_galley_and_cursor(cur_galley, new_cursor);
            }

            // jump to the closest galley below that's not below another galley that's below
            let mut closest_offset: Option<DocCharOffset> = None;
            let mut closest_distance = f32::INFINITY;
            let mut row_below_bottom: Option<f32> = None;
            for new_galley_idx in cur_galley_idx + 1..self.galleys.len() {
                let new_galley = &self.galleys[new_galley_idx];
                let new_galley_is_below = new_galley.rect.top() > cur_galley.rect.bottom();
                let new_galley_too_below = if let Some(row_below_bottom) = row_below_bottom {
                    new_galley.rect.top() > row_below_bottom
                } else {
                    false
                };

                if new_galley_too_below {
                    break;
                } else if new_galley_is_below {
                    row_below_bottom = Some(new_galley.rect.bottom());

                    let mut new_cursor = new_galley.galley.cursor_from_pos(Vec2 {
                        x: 0.0, // overwritten next line
                        y: new_galley.rect.top(),
                    });
                    new_cursor =
                        cursor::from_x(x_target, &self.galleys[new_galley_idx], new_cursor);

                    let pos = cursor::cursor_to_pos_abs(new_galley, new_cursor);
                    let distance = (pos.x - x_target).abs(); // closest as in closest to target

                    // prefer empty galleys which are placed deliberately to affect such behavior
                    if distance < closest_distance
                        || (distance == closest_distance && new_galley.range.is_empty())
                    {
                        closest_offset = Some(
                            self.galleys
                                .offset_by_galley_and_cursor(new_galley, new_cursor),
                        );
                        closest_distance = distance;
                    }
                } else {
                    continue; // keep going until we're on the next row (if there is one)
                }
            }

            if let Some(closest_offset) = closest_offset {
                closest_offset
            } else if !self
                .bounds
                .source_lines
                .find_containing(offset, true, true)
                .contains(self.bounds.source_lines.len() - 1, true, false)
            {
                // if we're in the last galley but not the last source line it's
                // because the lasy galley is hidden (perhaps by a folded node)
                self.buffer.current.segs.last_cursor_position()
            } else {
                offset
            }
        }
    }

    /// returns the x coordinate of the absolute position of `self` in `galley`
    fn x(&self, offset: DocCharOffset) -> Option<f32> {
        let (cur_galley_idx, cur_cursor) = self.galleys.galley_and_cursor_by_offset(offset)?;
        let cur_galley = &self.galleys[cur_galley_idx];
        Some(cursor::x_impl(cur_galley, cur_cursor))
    }
}
