use crate::bounds::Paragraphs;
use crate::buffer::SubBuffer;
use crate::galleys::{GalleyInfo, Galleys};
use crate::input::canonical::{Bound, Increment, Offset};
use crate::offset_types::{DocCharOffset, RangeExt, RelByteOffset};
use crate::unicode_segs::UnicodeSegs;
use egui::epaint::text::cursor::Cursor as EguiCursor;
use egui::{Pos2, Vec2};
use std::iter;
use unicode_segmentation::UnicodeSegmentation;

impl DocCharOffset {
    pub fn advance(
        self, maybe_x_target: &mut Option<f32>, offset: Offset, backwards: bool,
        buffer: &SubBuffer, galleys: &Galleys, paragraphs: &Paragraphs,
    ) -> Self {
        match offset {
            Offset::To(Bound::Char) => self,
            Offset::To(Bound::Word) => self
                .advance_to_word_bound(backwards, buffer, galleys)
                .fix(backwards, galleys),
            Offset::To(Bound::Line) => self.advance_to_line_bound(backwards, galleys),
            Offset::To(Bound::Paragraph) => self.advance_to_paragraph_bound(backwards, paragraphs),
            Offset::To(Bound::Doc) => self.advance_to_doc_bound(backwards, &buffer.segs),
            Offset::By(Increment::Char) => self
                .advance_by_char(backwards, &buffer.segs)
                .fix(backwards, galleys),
            Offset::By(Increment::Line) => {
                let x_target = maybe_x_target.unwrap_or(self.x(galleys));
                let result = self.advance_by_line(x_target, backwards, galleys);
                if self != 0 && self != buffer.segs.last_cursor_position() {
                    *maybe_x_target = Some(x_target);
                }
                result
            }
            .fix(backwards, galleys),
        }
    }

    fn advance_by_char(mut self, backwards: bool, segs: &UnicodeSegs) -> Self {
        if !backwards && self < segs.last_cursor_position() {
            self += 1;
        }
        if backwards && self > DocCharOffset(0) {
            self -= 1;
        }

        self
    }

    fn advance_by_line(self, x_target: f32, backwards: bool, galleys: &Galleys) -> Self {
        let (cur_galley_idx, cur_ecursor) = galleys.galley_and_cursor_by_char_offset(self);
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

            galleys.char_offset_by_galley_and_cursor(new_galley_idx, &new_ecursor)
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

            galleys.char_offset_by_galley_and_cursor(new_galley_idx, &new_ecursor)
        }
    }

    fn advance_to_word_bound(
        mut self, backwards: bool, buffer: &SubBuffer, galleys: &Galleys,
    ) -> Self {
        let segs = &buffer.segs;
        let galley_idx = galleys.galley_at_char(self);
        let galley = &galleys[galley_idx];
        let galley_text_range = galley.text_range();
        let galley_text = &buffer[galley_text_range];
        let galley_byte_range = buffer.segs.range_to_byte(galley_text_range);

        let word_bound_indices_with_words = galley_text.split_word_bound_indices();
        let word_bound_indices = word_bound_indices_with_words
            .clone()
            .map(|(idx, _)| segs.offset_to_char(galley_byte_range.start() + RelByteOffset(idx)))
            .chain(iter::once(galley_text_range.end())) // add last word boundary (note: no corresponding word)
            .collect::<Vec<_>>();
        let words = word_bound_indices_with_words
            .map(|(_, word)| word)
            .collect::<Vec<_>>();
        let i = match (word_bound_indices.binary_search(&self), backwards) {
            (Ok(i), _) | (Err(i), true) => i,
            (Err(i), false) => i.saturating_sub(1), // when moving forward from middle of word, behave as if from start of word
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
                    self = galleys[galley_idx + 1].text_range().start();
                }
                // ...or advance to the end of this galley
                else {
                    self = galley_text_range.end();
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
                    self = galleys[galley_idx - 1].text_range().end();
                }
                // ...or advance to the start of this galley
                else {
                    self = galley_text_range.start();
                }
            }
        }

        self
    }

    fn advance_to_line_bound(self, backwards: bool, galleys: &Galleys) -> Self {
        let (galley_idx, ecursor) = galleys.galley_and_cursor_by_char_offset(self);
        let galley = &galleys[galley_idx];
        let ecursor = if backwards {
            galley.galley.cursor_begin_of_row(&ecursor)
        } else {
            galley.galley.cursor_end_of_row(&ecursor)
        };
        galleys.char_offset_by_galley_and_cursor(galley_idx, &ecursor)
    }

    fn advance_to_paragraph_bound(self, backwards: bool, paragraphs: &Paragraphs) -> Self {
        // in a paragraph -> go to start or end
        for &paragraph in &paragraphs.paragraphs {
            if paragraph.contains(self) {
                return if backwards { paragraph.start() } else { paragraph.end() };
            }
        }

        // not in a paragraph -> go to end or start of previous or next paragraph
        if backwards {
            for &paragraph in paragraphs.paragraphs.iter().rev() {
                if paragraph.end() < self {
                    return paragraph.end();
                }
            }
        } else {
            for &paragraph in &paragraphs.paragraphs {
                if paragraph.start() > self {
                    return paragraph.start();
                }
            }
        }

        self
    }

    fn advance_to_doc_bound(self, backwards: bool, segs: &UnicodeSegs) -> Self {
        if backwards {
            0.into()
        } else {
            segs.last_cursor_position()
        }
    }

    fn fix(self, prefer_backwards: bool, galleys: &Galleys) -> Self {
        let galley_idx = galleys.galley_at_char(self);
        let galley = &galleys[galley_idx];
        let galley_text_range = galley.text_range();

        if self < galley_text_range.start() {
            if !prefer_backwards || galley_idx == 0 {
                // move cursor forwards into galley text
                galley_text_range.start()
            } else {
                // move cursor backwards into text of preceding galley
                galleys[galley_idx - 1].text_range().end()
            }
        } else if self > galley_text_range.end() {
            if prefer_backwards || galley_idx == galleys.len() - 1 {
                // move cursor backwards into galley text
                galley_text_range.end()
            } else {
                // move cursor forwards into text of next galley
                galleys[galley_idx + 1].text_range().start()
            }
        } else {
            self
        }
    }

    /// returns the x coordinate of the absolute position of `self` in `galley`
    fn x(self, galleys: &Galleys) -> f32 {
        let (cur_galley_idx, cur_cursor) = galleys.galley_and_cursor_by_char_offset(self);
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
