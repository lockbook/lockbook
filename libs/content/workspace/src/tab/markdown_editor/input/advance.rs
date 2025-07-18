use std::mem;

use crate::tab::markdown_editor::bounds::{BoundExt as _, Bounds};
use crate::tab::markdown_editor::galleys::Galleys;
use crate::tab::markdown_editor::input::{Increment, Offset};
use egui::Vec2;
use lb_rs::model::text::offset_types::{DocCharOffset, RangeExt};
use lb_rs::model::text::unicode_segs::UnicodeSegs;

use super::cursor;

pub trait AdvanceExt {
    fn advance(
        self, maybe_x_target: &mut Option<f32>, offset: Offset, backwards: bool,
        segs: &UnicodeSegs, galleys: &Galleys, bounds: &Bounds,
    ) -> Self;
    fn advance_by_line(self, x_target: f32, backwards: bool, galleys: &Galleys) -> Self;
    fn x(self, galleys: &Galleys) -> f32;
}

impl AdvanceExt for DocCharOffset {
    fn advance(
        self, maybe_x_target: &mut Option<f32>, offset: Offset, backwards: bool,
        segs: &UnicodeSegs, galleys: &Galleys, bounds: &Bounds,
    ) -> Self {
        let maybe_x_target_value = mem::take(maybe_x_target);
        match offset {
            Offset::To(bound) => self.advance_to_bound(bound, backwards, bounds),
            Offset::Next(bound) => self.advance_to_next_bound(bound, backwards, bounds),
            Offset::By(Increment::Line) => {
                let x_target = maybe_x_target_value.unwrap_or(self.x(galleys));
                let result = self.advance_by_line(x_target, backwards, galleys);
                if self != 0 && self != segs.last_cursor_position() {
                    *maybe_x_target = Some(x_target);
                }

                result
            }
        }
    }

    fn advance_by_line(self, x_target: f32, backwards: bool, galleys: &Galleys) -> Self {
        let (cur_galley_idx, cur_ecursor) = galleys.galley_and_cursor_by_offset(self);
        let cur_galley = &galleys[cur_galley_idx];
        if backwards {
            let at_top_of_cur_galley = cur_ecursor.rcursor.row == 0;
            if !at_top_of_cur_galley {
                // within a galley: just move up one row
                let new_cursor = cur_galley.galley.cursor_up_one_row(&cur_ecursor);
                return galleys.offset_by_galley_and_cursor(cur_galley, new_cursor);
            }

            // jump to the closest galley above that's not above another galley that's above
            let mut closest_offset: Option<Self> = None;
            let mut closest_distance = f32::INFINITY;
            let mut row_above_top: Option<f32> = None;
            for new_galley_idx in (0..cur_galley_idx).rev() {
                let new_galley = &galleys[new_galley_idx];
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
                    new_cursor = cursor::from_x(x_target, &galleys[new_galley_idx], new_cursor);

                    let pos = cursor::cursor_to_pos_abs(new_galley, new_cursor);
                    let distance = (pos.x - x_target).abs(); // closest as in closest to target

                    // prefer empty galleys which are placed deliberately to affect such behavior
                    if distance < closest_distance
                        || (distance == closest_distance && new_galley.range.is_empty())
                    {
                        closest_offset =
                            Some(galleys.offset_by_galley_and_cursor(new_galley, new_cursor));
                        closest_distance = distance;
                    }
                } else {
                    continue; // keep going until we're on the prev row (if there is one)
                }
            }

            closest_offset.unwrap_or(self)
        } else {
            let at_bottom_of_cur_galley =
                cur_ecursor.rcursor.row == cur_galley.galley.rows.len() - 1;
            if !at_bottom_of_cur_galley {
                // within a galley: just move down one row
                let new_cursor = cur_galley.galley.cursor_down_one_row(&cur_ecursor);
                return galleys.offset_by_galley_and_cursor(cur_galley, new_cursor);
            }

            // jump to the closest galley below that's not below another galley that's below
            let mut closest_offset: Option<Self> = None;
            let mut closest_distance = f32::INFINITY;
            let mut row_below_bottom: Option<f32> = None;
            for new_galley_idx in cur_galley_idx + 1..galleys.len() {
                let new_galley = &galleys[new_galley_idx];
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
                    new_cursor = cursor::from_x(x_target, &galleys[new_galley_idx], new_cursor);

                    let pos = cursor::cursor_to_pos_abs(new_galley, new_cursor);
                    let distance = (pos.x - x_target).abs(); // closest as in closest to target

                    // prefer empty galleys which are placed deliberately to affect such behavior
                    if distance < closest_distance
                        || (distance == closest_distance && new_galley.range.is_empty())
                    {
                        closest_offset =
                            Some(galleys.offset_by_galley_and_cursor(new_galley, new_cursor));
                        closest_distance = distance;
                    }
                } else {
                    continue; // keep going until we're on the next row (if there is one)
                }
            }

            closest_offset.unwrap_or(self)
        }
    }

    /// returns the x coordinate of the absolute position of `self` in `galley`
    fn x(self, galleys: &Galleys) -> f32 {
        let (cur_galley_idx, cur_cursor) = galleys.galley_and_cursor_by_offset(self);
        let cur_galley = &galleys[cur_galley_idx];
        cursor::x_impl(cur_galley, cur_cursor)
    }
}
