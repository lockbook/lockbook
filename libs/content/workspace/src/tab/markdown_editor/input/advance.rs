use std::mem;

use crate::tab::markdown_editor::bounds::{Bounds, Text};
use crate::tab::markdown_editor::buffer::SubBuffer;
use crate::tab::markdown_editor::galleys::{GalleyInfo, Galleys};
use crate::tab::markdown_editor::input::canonical::{Increment, Offset};
use crate::tab::markdown_editor::offset_types::DocCharOffset;
use egui::epaint::text::cursor::Cursor as EguiCursor;
use egui::{Pos2, Vec2};

impl DocCharOffset {
    pub fn advance(
        self, maybe_x_target: &mut Option<f32>, offset: Offset, backwards: bool,
        buffer: &SubBuffer, galleys: &Galleys, bounds: &Bounds,
    ) -> Self {
        let maybe_x_target_value = mem::take(maybe_x_target);
        match offset {
            Offset::To(bound) => self.advance_to_bound(bound, backwards, bounds),
            Offset::Next(bound) => self.advance_to_next_bound(bound, backwards, bounds),
            Offset::By(Increment::Line) => {
                let x_target = maybe_x_target_value.unwrap_or(self.x(galleys, &bounds.text));
                let result = self.advance_by_line(x_target, backwards, galleys, &bounds.text);
                if self != 0 && self != buffer.segs.last_cursor_position() {
                    *maybe_x_target = Some(x_target);
                }
                result
            }
        }
    }

    fn advance_by_line(
        self, x_target: f32, backwards: bool, galleys: &Galleys, text: &Text,
    ) -> Self {
        let (cur_galley_idx, cur_ecursor) = galleys.galley_and_cursor_by_char_offset(self, text);
        let cur_galley = &galleys[cur_galley_idx];
        if backwards {
            let at_top_of_cur_galley = cur_ecursor.rcursor.row == 0;
            let in_first_galley = cur_galley_idx == 0;
            let (mut new_ecursor, new_galley_idx) = if at_top_of_cur_galley && !in_first_galley {
                // move to the last row of the previous galley
                let new_galley_idx = cur_galley_idx - 1;
                let new_galley = &galleys[new_galley_idx];
                let new_cursor = new_galley.galley.cursor_from_pos(Vec2 {
                    x: 0.0,                          // overwritten below
                    y: new_galley.galley.rect.max.y, // bottom of new galley
                });
                (new_cursor, new_galley_idx)
            } else {
                // move up one row in the current galley
                let new_cursor = cur_galley.galley.cursor_up_one_row(&cur_ecursor);
                (new_cursor, cur_galley_idx)
            };

            if !(at_top_of_cur_galley && in_first_galley) {
                // move to the x_target in the new row/galley
                new_ecursor = Self::from_x(x_target, &galleys[new_galley_idx], new_ecursor);
            }

            galleys.char_offset_by_galley_and_cursor(new_galley_idx, &new_ecursor, text)
        } else {
            let at_bottom_of_cur_galley =
                cur_ecursor.rcursor.row == cur_galley.galley.rows.len() - 1;
            let in_last_galley = cur_galley_idx == galleys.len() - 1;
            let (mut new_ecursor, new_galley_idx) = if at_bottom_of_cur_galley && !in_last_galley {
                // move to the first row of the next galley
                let new_galley_idx = cur_galley_idx + 1;
                let new_galley = &galleys[new_galley_idx];
                let new_cursor = new_galley.galley.cursor_from_pos(Vec2 {
                    x: 0.0, // overwritten below
                    y: 0.0, // top of new galley
                });
                (new_cursor, new_galley_idx)
            } else {
                // move down one row in the current galley
                let new_cursor = cur_galley.galley.cursor_down_one_row(&cur_ecursor);
                (new_cursor, cur_galley_idx)
            };

            if !(at_bottom_of_cur_galley && in_last_galley) {
                // move to the x_target in the new row/galley
                new_ecursor = Self::from_x(x_target, &galleys[new_galley_idx], new_ecursor);
            }

            galleys.char_offset_by_galley_and_cursor(new_galley_idx, &new_ecursor, text)
        }
    }

    /// returns the x coordinate of the absolute position of `self` in `galley`
    fn x(self, galleys: &Galleys, text: &Text) -> f32 {
        let (cur_galley_idx, cur_cursor) = galleys.galley_and_cursor_by_char_offset(self, text);
        let cur_galley = &galleys[cur_galley_idx];
        Self::x_impl(cur_galley, cur_cursor)
    }

    /// returns the x coordinate of the absolute position of `cursor` in `galley`
    fn x_impl(galley: &GalleyInfo, cursor: EguiCursor) -> f32 {
        Self::cursor_to_pos_abs(galley, cursor).x
    }

    /// adjusts cursor so that its absolute x coordinate matches the target (if there is one)
    fn from_x(x: f32, galley: &GalleyInfo, cursor: EguiCursor) -> EguiCursor {
        let mut pos_abs = Self::cursor_to_pos_abs(galley, cursor);
        pos_abs.x = x;
        Self::pos_abs_to_cursor(galley, pos_abs)
    }

    /// returns the absolute position of `cursor` in `galley`
    pub fn cursor_to_pos_abs(galley: &GalleyInfo, cursor: EguiCursor) -> Pos2 {
        // experimentally, max.y gives us the y that will put us in the correct row
        galley.text_location + galley.galley.pos_from_cursor(&cursor).max.to_vec2()
    }

    /// returns a cursor which has the absolute position `pos_abs` in `galley`
    fn pos_abs_to_cursor(galley: &GalleyInfo, pos_abs: Pos2) -> EguiCursor {
        galley
            .galley
            .cursor_from_pos(pos_abs - galley.text_location)
    }
}
