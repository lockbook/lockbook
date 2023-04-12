use crate::buffer::SubBuffer;
use crate::galleys::{GalleyInfo, Galleys};
use crate::input::canonical::{Bound, Increment, Offset};
use crate::offset_types::{DocCharOffset, RelByteOffset};
use crate::unicode_segs::UnicodeSegs;
use egui::epaint::text::cursor::Cursor as EguiCursor;
use egui::{Pos2, Vec2};
use std::iter;
use unicode_segmentation::UnicodeSegmentation;

impl DocCharOffset {
    pub fn advance(
        self, maybe_x_target: &mut Option<f32>, offset: Offset, backwards: bool,
        buffer: &SubBuffer, segs: &UnicodeSegs, galleys: &Galleys,
    ) -> Self {
        match offset {
            Offset::To(Bound::Word) => self.advance_to_word_bound(backwards, buffer, segs, galleys),
            Offset::To(Bound::Line) => self.advance_to_line_bound(backwards, segs, galleys),
            Offset::To(Bound::Doc) => self.advance_to_doc_bound(backwards, segs),
            Offset::By(Increment::Char) => self.advance_by_char(backwards, segs, galleys),
            Offset::By(Increment::Line) => {
                let x_target = maybe_x_target.unwrap_or(self.x(galleys, segs));
                let result = self.advance_by_line(x_target, backwards, segs, galleys);
                if self.0 != 0 && self != segs.last_cursor_position() {
                    *maybe_x_target = Some(x_target);
                }
                result
            }
        }
    }

    fn advance_by_char(mut self, backwards: bool, segs: &UnicodeSegs, galleys: &Galleys) -> Self {
        if !backwards && self < segs.last_cursor_position() {
            self += 1;
        }
        if backwards && self > DocCharOffset(0) {
            self -= 1;
        }

        self.fix(backwards, segs, galleys)
    }

    fn advance_by_line(
        self, x_target: f32, backwards: bool, segs: &UnicodeSegs, galleys: &Galleys,
    ) -> Self {
        let (cur_galley_idx, cur_ecursor) = galleys.galley_and_cursor_by_char_offset(self, segs);
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

            galleys.char_offset_by_galley_and_cursor(new_galley_idx, &new_ecursor, segs)
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

            galleys.char_offset_by_galley_and_cursor(new_galley_idx, &new_ecursor, segs)
        }
    }

    fn advance_to_word_bound(
        mut self, backwards: bool, buffer: &SubBuffer, segs: &UnicodeSegs, galleys: &Galleys,
    ) -> Self {
        let mut galley_idx = galleys.galley_at_char(self, segs);
        let galley = &galleys[galley_idx];
        let galley_text = galley.text(buffer);
        let galley_text_range = galley.text_range(segs);

        let word_bound_indices_with_words = galley_text.split_word_bound_indices();
        let word_bound_indices = word_bound_indices_with_words
            .clone()
            .map(|(idx, _)| {
                segs.byte_offset_to_char(galley.byte_range().start + RelByteOffset(idx))
            })
            .chain(iter::once(galley_text_range.end)) // add last word boundary (note: no corresponding word)
            .collect::<Vec<_>>();
        let words = word_bound_indices_with_words
            .map(|(_, word)| word)
            .collect::<Vec<_>>();
        let i = match (word_bound_indices.binary_search(&self), backwards) {
            (Ok(i), _) | (Err(i), true) => i,
            (Err(i), false) => i - 1, // when moving forward from middle of word, behave as if from start of word
        };

        if !backwards {
            // advance to the end of the next non-whitespace word if there is one...
            let mut found = false;
            for i in i..words.len() {
                if !words[i].trim().is_empty() {
                    found = true;
                    self = word_bound_indices[i + 1];
                    break;
                }
            }

            // ...otherwise...
            if !found {
                // ...advance to the start of the next galley if there is one...
                if galley_idx + 1 < galleys.len() {
                    galley_idx += 1;
                    let galley = &galleys[galley_idx];
                    let galley_text_range = galley.text_range(segs);
                    self = galley_text_range.start;
                }
                // ...or advance to the end of this galley
                else {
                    self = galley_text_range.end;
                }
            }
        } else {
            // advance to the start of the previous non-whitespace word if there is one...
            let mut found = false;
            for i in (0..i).rev() {
                if !words[i].trim().is_empty() {
                    found = true;
                    self = word_bound_indices[i];
                    break;
                }
            }

            // ...otherwise...
            if !found {
                // ...advance to the end of the previous galley if there is one...
                if galley_idx > 0 {
                    galley_idx -= 1;
                    let galley = &galleys[galley_idx];
                    let galley_text_range = galley.text_range(segs);
                    self = galley_text_range.end;
                }
                // ...or advance to the start of this galley
                else {
                    self = galley_text_range.start;
                }
            }
        }

        self
    }

    fn advance_to_line_bound(self, backwards: bool, segs: &UnicodeSegs, galleys: &Galleys) -> Self {
        let (galley_idx, ecursor) = galleys.galley_and_cursor_by_char_offset(self, segs);
        let galley = &galleys[galley_idx];
        let ecursor = if backwards {
            galley.galley.cursor_begin_of_row(&ecursor)
        } else {
            galley.galley.cursor_end_of_row(&ecursor)
        };
        galleys.char_offset_by_galley_and_cursor(galley_idx, &ecursor, segs)
    }

    fn advance_to_doc_bound(self, backwards: bool, segs: &UnicodeSegs) -> Self {
        if backwards {
            0.into()
        } else {
            segs.last_cursor_position()
        }
    }

    pub fn fix(mut self, prefer_backwards: bool, segs: &UnicodeSegs, galleys: &Galleys) -> Self {
        let galley_idx = galleys.galley_at_char(self, segs);
        let galley = &galleys[galley_idx];
        let galley_text_range = galley.text_range(segs);

        if self < galley_text_range.start {
            if !prefer_backwards || galley_idx == 0 {
                // move cursor forwards into galley text
                self = galley_text_range.start;
            } else {
                // move cursor backwards into text of preceding galley
                let galley_idx = galley_idx - 1;
                let galley = &galleys[galley_idx];
                let galley_text_range = galley.text_range(segs);
                self = galley_text_range.end;
            }
        }
        if self > galley_text_range.end {
            if prefer_backwards || galley_idx == galleys.len() - 1 {
                // move cursor backwards into galley text
                self = galley_text_range.end;
            } else {
                // move cursor forwards into text of next galley
                let galley_idx = galley_idx + 1;
                let galley = &galleys[galley_idx];
                let galley_text_range = galley.text_range(segs);
                self = galley_text_range.start;
            }
        }
        self
    }

    /// returns the x coordinate of the absolute position of `self` in `galley`
    fn x(self, galleys: &Galleys, segs: &UnicodeSegs) -> f32 {
        let (cur_galley_idx, cur_cursor) = galleys.galley_and_cursor_by_char_offset(self, segs);
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
