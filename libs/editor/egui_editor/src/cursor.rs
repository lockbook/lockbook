use crate::cursor_types::*;
use crate::editor::Editor;
use crate::galley::GalleyInfo;
use egui::epaint::text::cursor::Cursor as EguiCursor;
use egui::text::CCursor;
use egui::{Color32, Pos2, Rect, Rounding, Stroke, Ui, Vec2};
use std::cmp::{max, min};
use std::iter;
use std::ops::Range;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Default)]
pub struct Cursor {
    pub loc: DocCharOffset,

    /// When navigating using up/down keys, x_target stores the original *absolute* x coordinate of
    /// the cursor, which helps us keep the cursor in a consistent x position even navigating past
    /// lines that are shorter, empty, annotated, etc.
    pub x_target: Option<f32>,

    /// When selecting text, this is the location of the cursor when text selection began. This may
    /// be before or after the cursor location and, once set, generally doesn't move until cleared
    pub selection_origin: Option<DocCharOffset>,

    /// When clicking and dragging, this is the time and location of the initial click
    pub click_and_drag_origin: Option<DocCharOffset>,
}

impl Cursor {
    /// sets `x_target` to match the position of `cursor` in `galley` if there isn't already an
    /// `x_target`
    pub fn set_x_target(&mut self, galley: &GalleyInfo, cursor: EguiCursor) -> f32 {
        if let Some(x_target) = self.x_target {
            x_target
        } else {
            let pos_abs = Self::cursor_to_pos_abs(galley, cursor);
            self.x_target = Some(pos_abs.x);
            pos_abs.x
        }
    }

    /// sets `selection_origin` to match `loc` if there isn't already a `selection_origin`
    pub fn set_selection_origin(&mut self) -> DocCharOffset {
        if let Some(selection_origin) = self.selection_origin {
            selection_origin
        } else {
            self.selection_origin = Some(self.loc);
            self.loc
        }
    }

    /// sets `click_and_drag_origin` to match `loc` if there isn't already a `click_and_drag_origin`
    pub fn set_click_and_drag_origin(&mut self) -> DocCharOffset {
        if let Some(click_and_drag_origin) = self.click_and_drag_origin {
            click_and_drag_origin
        } else {
            self.click_and_drag_origin = Some(self.loc);
            self.loc
        }
    }

    /// returns the (nonempty) range of selected text, if any
    pub fn selection_range(&self) -> Option<Range<DocCharOffset>> {
        if let Some(selection_origin) = self.selection_origin {
            if selection_origin != self.loc {
                Some(Range {
                    start: min(self.loc, selection_origin),
                    end: max(self.loc, selection_origin),
                })
            } else {
                None
            }
        } else {
            None
        }
    }

    /// adjusts cursor so that its absolute x coordinate matches the target (if there is one)
    pub fn move_to_x_target(galley: &GalleyInfo, cursor: EguiCursor, x_target: f32) -> EguiCursor {
        let mut pos_abs = Self::cursor_to_pos_abs(galley, cursor);
        pos_abs.x = x_target;
        Self::pos_abs_to_cursor(galley, pos_abs)
    }

    /// returns the absolute position of `cursor` in `galley`
    fn cursor_to_pos_abs(galley: &GalleyInfo, cursor: EguiCursor) -> Pos2 {
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

impl Editor {
    pub fn calc_unicode_segs(&mut self) {
        self.gr_ind.clear();
        if !self.raw.is_empty() {
            self.gr_ind.extend(
                UnicodeSegmentation::grapheme_indices(self.raw.as_str(), true)
                    .map(|t| DocByteOffset(t.0)),
            );
        }
        self.gr_ind.push(DocByteOffset(self.raw.len()));
    }

    pub fn replace(&mut self, range: Range<DocCharOffset>, replacement: &str) {
        if range.contains(&self.cursor.loc) || self.cursor.loc == range.end {
            self.cursor.loc = range.start;
        }
        self.raw.replace_range(
            Range {
                start: self.char_offset_to_byte(range.start).0,
                end: self.char_offset_to_byte(range.end).0,
            },
            replacement,
        );
    }

    pub fn insert_at_cursor(&mut self, insertion: &str) {
        self.raw
            .insert_str(self.char_offset_to_byte(self.cursor.loc).0, insertion);
        self.cursor.loc += UnicodeSegmentation::grapheme_indices(insertion, true).count();
    }

    pub fn last_cursor_position(&self) -> DocCharOffset {
        DocCharOffset(self.gr_ind.len() - 1)
    }

    pub fn char_offset_to_byte(&self, i: DocCharOffset) -> DocByteOffset {
        if self.gr_ind.is_empty() && i.0 == 0 {
            return DocByteOffset(0);
        }
        self.gr_ind[i.0]
    }

    pub fn byte_offset_to_char(&self, i: DocByteOffset) -> DocCharOffset {
        if self.gr_ind.is_empty() && i.0 == 0 {
            return DocCharOffset(0);
        }

        DocCharOffset(self.gr_ind.binary_search(&i).unwrap())
    }

    pub fn galley_at_char(&self, char_index: DocCharOffset) -> usize {
        let raw_index = self.char_offset_to_byte(char_index);
        for i in 0..self.galleys.len() {
            let galley = &self.galleys[i];
            if galley.range.start <= raw_index && raw_index < galley.range.end {
                return i;
            }
        }

        self.galleys.len() - 1
    }

    pub fn rel_char_offset(&self, char_idx: DocCharOffset, galley: &GalleyInfo) -> RelCharOffset {
        let start_position = self.byte_offset_to_char(galley.range.start);
        char_idx - start_position
    }

    pub fn set_galley_and_cursor(&mut self, galley_idx: usize, cursor: &EguiCursor) {
        self.cursor.loc = self.char_offset_by_galley_and_cursor(galley_idx, cursor)
    }

    pub fn char_offset_by_galley_and_cursor(
        &self, galley_idx: usize, cursor: &EguiCursor,
    ) -> DocCharOffset {
        let galley = &self.galleys[galley_idx];
        let galley_text_range = galley.text_range(self);
        let mut result = galley_text_range.start + galley.head_modification + cursor.ccursor.index;

        // correct for prefer_next_row behavior
        let read_cursor = galley.galley.from_ccursor(CCursor {
            index: (result - galley_text_range.start).0,
            prefer_next_row: true,
        });
        if read_cursor.rcursor.row > cursor.rcursor.row {
            result -= 1;
        }

        result
    }

    pub fn galley_and_cursor(&self) -> (usize, EguiCursor) {
        self.galley_and_cursor_by_char_offset(self.cursor.loc)
    }

    pub fn galley_and_cursor_selection_origin(&self) -> Option<(usize, EguiCursor)> {
        self.cursor
            .selection_origin
            .map(|so| self.galley_and_cursor_by_char_offset(so))
    }

    fn galley_and_cursor_by_char_offset(&self, char_offset: DocCharOffset) -> (usize, EguiCursor) {
        let byte_offset = self.char_offset_to_byte(char_offset);

        let mut galley_index = self.galleys.len() - 1;
        for i in 0..self.galleys.len() {
            let galley = &self.galleys[i];
            if galley.range.start <= byte_offset && byte_offset < galley.range.end {
                galley_index = i;
                break;
            }
        }

        let galley = &self.galleys[galley_index];
        let galley_text_range = galley.text_range(self);
        let cursor = galley.galley.from_ccursor(CCursor {
            index: (char_offset - galley_text_range.start).0,
            prefer_next_row: true,
        });

        (galley_index, cursor)
    }

    pub fn selection_range_bytes(&self) -> Option<Range<DocByteOffset>> {
        let selection_range_chars = self.cursor.selection_range();
        selection_range_chars.map(|sr| Range {
            start: self.char_offset_to_byte(sr.start),
            end: self.char_offset_to_byte(sr.end),
        })
    }

    pub fn fix_cursor(&mut self, galley_idx: usize, prefer_backwards: bool) {
        let galley = &self.galleys[galley_idx];
        let galley_text_range = galley.text_range(self);

        if self.cursor.loc < galley_text_range.start {
            if !prefer_backwards || galley_idx == 0 {
                // move cursor forwards into galley text
                self.cursor.loc = galley_text_range.start;
            } else {
                // move cursor backwards into text of preceding galley
                let galley_idx = galley_idx - 1;
                let galley = &self.galleys[galley_idx];
                let galley_text_range = galley.text_range(self);
                self.cursor.loc = galley_text_range.end;
            }
        }
        if self.cursor.loc > galley_text_range.end {
            if prefer_backwards || galley_idx == self.galleys.len() - 1 {
                // move cursor backwards into galley text
                self.cursor.loc = galley_text_range.end;
            } else {
                // move cursor forwards into text of next galley
                let galley_idx = galley_idx + 1;
                let galley = &self.galleys[galley_idx];
                let galley_text_range = galley.text_range(self);
                self.cursor.loc = galley_text_range.start;
            }
        }
    }

    pub fn cursor_to_next_char(&mut self, galley_idx: usize, backwards: bool) {
        if !backwards && self.cursor.loc < self.last_cursor_position() {
            self.cursor.loc += 1;
        }
        if backwards && self.cursor.loc > DocCharOffset(0) {
            self.cursor.loc -= 1;
        }

        self.fix_cursor(galley_idx, backwards);
    }

    pub fn cursor_to_next_word_boundary(
        &self, mut galley_idx: usize, mut cursor: EguiCursor, backwards: bool,
    ) -> (usize, EguiCursor) {
        let galley = &self.galleys[galley_idx];
        let galley_text = galley.text(&self.raw);
        let galley_text_range = galley.text_range(self);

        let word_bound_indices_with_words = galley_text.split_word_bound_indices();
        let word_bound_indices = word_bound_indices_with_words
            .clone()
            .map(|(idx, _)| idx)
            .chain(iter::once((galley_text_range.end - galley_text_range.start).0)) // add last word boundary (note: no corresponding word)
            .collect::<Vec<_>>();
        let words = word_bound_indices_with_words
            .map(|(_, word)| word)
            .collect::<Vec<_>>();

        let i = match (word_bound_indices.binary_search(&cursor.ccursor.index), backwards) {
            (Ok(i), _) | (Err(i), true) => i,
            (Err(i), false) => i - 1, // when moving forward from middle of word, behave as if from start of word
        };

        if !backwards {
            // advance to the end of the next non-whitespace word if there is one...
            let mut found = false;
            for i in i..words.len() {
                if !words[i].trim().is_empty() {
                    found = true;
                    cursor.ccursor.index = word_bound_indices[i + 1];
                    break;
                }
            }

            // ...otherwise...
            if !found {
                // ...advance to the start of the next galley if there is one...
                if galley_idx + 1 < self.galleys.len() {
                    galley_idx += 1;
                    cursor.ccursor.index = 0;
                }
                // ...or advance to the end of this galley
                else {
                    cursor.ccursor.index = (galley_text_range.end - galley_text_range.start).0;
                }
            }
        }
        if backwards {
            // advance to the start of the previous non-whitespace word if there is one...
            let mut found = false;
            for i in (0..i).rev() {
                if !words[i].trim().is_empty() {
                    found = true;
                    cursor.ccursor.index = word_bound_indices[i];
                    break;
                }
            }

            // ...otherwise...
            if !found {
                // ...advance to the end of the previous galley if there is one...
                if galley_idx > 0 {
                    galley_idx -= 1;
                    let galley = &self.galleys[galley_idx];
                    let galley_text_range = galley.text_range(self);
                    cursor.ccursor.index = (galley_text_range.end - galley_text_range.start).0;
                }
                // ...or advance to the start of this galley
                else {
                    cursor.ccursor.index = 0;
                }
            }
        }

        (galley_idx, cursor)
    }

    pub fn draw_cursor(&mut self, ui: &mut Ui) {
        let (galley_idx, cursor) = self.galley_and_cursor();
        let galley = &self.galleys[galley_idx];
        let cursor_size = Vec2 { x: 1.0, y: galley.cursor_height() };

        let max = Cursor::cursor_to_pos_abs(galley, cursor);
        let min = max - cursor_size;
        let cursor_rect = Rect { min, max };

        ui.painter().rect(
            cursor_rect,
            Rounding::none(),
            Color32::TRANSPARENT,
            Stroke { width: 1.0, color: self.visual_appearance.text() },
        );
    }
}
