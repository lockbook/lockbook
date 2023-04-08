use crate::buffer::SubBuffer;
use crate::galleys::{GalleyInfo, Galleys};
use crate::offset_types::*;
use crate::unicode_segs::UnicodeSegs;
use egui::epaint::text::cursor::Cursor as EguiCursor;
use egui::{Pos2, Rect, Vec2};
use std::cmp::{max, min};
use std::iter;
use std::ops::Range;
use std::time::{Duration, Instant};
use unicode_segmentation::UnicodeSegmentation;

static DOUBLE_CLICK_PERIOD: Duration = Duration::from_millis(300);

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Cursor {
    pub pos: DocCharOffset,

    /// When navigating using up/down keys, x_target stores the original *absolute* x coordinate of
    /// the cursor, which helps us keep the cursor in a consistent x position even navigating past
    /// lines that are shorter, empty, annotated, etc.
    pub x_target: Option<f32>,

    /// When selecting text, this is the location of the cursor when text selection began. This may
    /// be before or after the cursor location and, once set, generally doesn't move until cleared
    pub selection_origin: Option<DocCharOffset>,

    /// When clicking and dragging, this is the location of the initial click
    pub click_and_drag_origin: Option<DocCharOffset>,

    /// Time of release of last three clicks, used for double & triple click detection
    pub last_click_times: (Option<Instant>, Option<Instant>, Option<Instant>),
}

impl From<usize> for Cursor {
    fn from(value: usize) -> Self {
        Self { pos: DocCharOffset(value), ..Default::default() }
    }
}

impl From<(usize, usize)> for Cursor {
    fn from(value: (usize, usize)) -> Self {
        Self {
            pos: DocCharOffset(value.1),
            selection_origin: Some(DocCharOffset(value.0)),
            ..Default::default()
        }
    }
}

impl From<(DocCharOffset, DocCharOffset)> for Cursor {
    fn from(value: (DocCharOffset, DocCharOffset)) -> Self {
        Self { pos: value.1, selection_origin: Some(value.0), ..Default::default() }
    }
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

    /// sets `selection_origin` to match `pos` if there isn't already a `selection_origin`
    pub fn set_selection_origin(&mut self) -> DocCharOffset {
        if let Some(selection_origin) = self.selection_origin {
            selection_origin
        } else {
            self.selection_origin = Some(self.pos);
            self.pos
        }
    }

    /// sets `click_and_drag_origin` to match `pos` if there isn't already a `click_and_drag_origin`
    pub fn set_click_and_drag_origin(&mut self) -> DocCharOffset {
        if let Some(click_and_drag_origin) = self.click_and_drag_origin {
            click_and_drag_origin
        } else {
            self.click_and_drag_origin = Some(self.pos);
            self.pos
        }
    }

    /// returns the (nonempty) range of selected text, if any
    pub fn selection(&self) -> Option<Range<DocCharOffset>> {
        if let Some(selection_origin) = self.selection_origin {
            if selection_origin != self.pos {
                Some(Range {
                    start: min(self.pos, selection_origin),
                    end: max(self.pos, selection_origin),
                })
            } else {
                None
            }
        } else {
            None
        }
    }

    /// returns the (nonempty) byte range of selected text, if any
    pub fn selection_bytes(&self, segs: &UnicodeSegs) -> Option<Range<DocByteOffset>> {
        let selection_chars = self.selection();
        selection_chars.map(|sr| Range {
            start: segs.char_offset_to_byte(sr.start),
            end: segs.char_offset_to_byte(sr.end),
        })
    }

    /// returns the (possibly empty) selected text
    pub fn selection_text<'b>(&self, buffer: &'b SubBuffer, segs: &UnicodeSegs) -> &'b str {
        if let Some(selection_bytes) = self.selection_bytes(segs) {
            &buffer.text[selection_bytes.start.0..selection_bytes.end.0]
        } else {
            ""
        }
    }

    /// adjusts cursor so that its absolute x coordinate matches the target (if there is one)
    pub fn move_to_x_target(galley: &GalleyInfo, cursor: EguiCursor, x_target: f32) -> EguiCursor {
        let mut pos_abs = Self::cursor_to_pos_abs(galley, cursor);
        pos_abs.x = x_target;
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

    pub fn fix(&mut self, prefer_backwards: bool, segs: &UnicodeSegs, galleys: &Galleys) {
        let galley_idx = galleys.galley_at_char(self.pos, segs);
        let galley = &galleys[galley_idx];
        let galley_text_range = galley.text_range(segs);

        if self.pos < galley_text_range.start {
            if !prefer_backwards || galley_idx == 0 {
                // move cursor forwards into galley text
                self.pos = galley_text_range.start;
            } else {
                // move cursor backwards into text of preceding galley
                let galley_idx = galley_idx - 1;
                let galley = &galleys[galley_idx];
                let galley_text_range = galley.text_range(segs);
                self.pos = galley_text_range.end;
            }
        }
        if self.pos > galley_text_range.end {
            if prefer_backwards || galley_idx == galleys.len() - 1 {
                // move cursor backwards into galley text
                self.pos = galley_text_range.end;
            } else {
                // move cursor forwards into text of next galley
                let galley_idx = galley_idx + 1;
                let galley = &galleys[galley_idx];
                let galley_text_range = galley.text_range(segs);
                self.pos = galley_text_range.start;
            }
        }
    }

    pub fn advance_char(&mut self, backwards: bool, segs: &UnicodeSegs, galleys: &Galleys) {
        if !backwards && self.pos < segs.last_cursor_position() {
            self.pos += 1;
        }
        if backwards && self.pos > DocCharOffset(0) {
            self.pos -= 1;
        }

        self.fix(backwards, segs, galleys);
    }

    pub fn advance_word(
        &mut self, backwards: bool, buffer: &SubBuffer, segs: &UnicodeSegs, galleys: &Galleys,
    ) {
        let mut galley_idx = galleys.galley_at_char(self.pos, segs);
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
        let i = match (word_bound_indices.binary_search(&self.pos), backwards) {
            (Ok(i), _) | (Err(i), true) => i,
            (Err(i), false) => i - 1, // when moving forward from middle of word, behave as if from start of word
        };

        if !backwards {
            // advance to the end of the next non-whitespace word if there is one...
            let mut found = false;
            for i in i..words.len() {
                if !words[i].trim().is_empty() {
                    found = true;
                    self.pos = word_bound_indices[i + 1];
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
                    self.pos = galley_text_range.start;
                }
                // ...or advance to the end of this galley
                else {
                    self.pos = galley_text_range.end;
                }
            }
        }
        if backwards {
            // advance to the start of the previous non-whitespace word if there is one...
            let mut found = false;
            for i in (0..i).rev() {
                if !words[i].trim().is_empty() {
                    found = true;
                    self.pos = word_bound_indices[i];
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
                    self.pos = galley_text_range.end;
                }
                // ...or advance to the start of this galley
                else {
                    self.pos = galley_text_range.start;
                }
            }
        }
    }

    pub fn process_click_instant(&mut self, t: Instant) {
        self.last_click_times.2 = self.last_click_times.1;
        self.last_click_times.1 = self.last_click_times.0;
        self.last_click_times.0 = Some(t);
    }

    pub fn triple_click(&mut self) -> bool {
        if let (Some(one_click_ago), Some(three_clicks_ago)) =
            (self.last_click_times.0, self.last_click_times.2)
        {
            one_click_ago - three_clicks_ago < DOUBLE_CLICK_PERIOD * 2
        } else {
            false
        }
    }

    pub fn double_click(&mut self) -> bool {
        if let (Some(one_click_ago), Some(two_clicks_ago)) =
            (self.last_click_times.0, self.last_click_times.1)
        {
            one_click_ago - two_clicks_ago < DOUBLE_CLICK_PERIOD
        } else {
            false
        }
    }

    pub fn rect(&self, segs: &UnicodeSegs, galleys: &Galleys) -> Rect {
        let (galley_idx, cursor) = galleys.galley_and_cursor_by_char_offset(self.pos, segs);
        let galley = &galleys[galley_idx];
        let cursor_size = Vec2 { x: 1.0, y: galley.cursor_height() };

        let max = Cursor::cursor_to_pos_abs(galley, cursor);
        let min = max - cursor_size;
        Rect { min, max }
    }
}
