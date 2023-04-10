use crate::buffer::SubBuffer;
use crate::galleys::Galleys;
use crate::input::canonical::Offset;
use crate::offset_types::*;
use crate::unicode_segs::UnicodeSegs;
use egui::{Rect, Vec2};
use std::cmp::{max, min};
use std::ops::Range;
use std::time::{Duration, Instant};

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
}

impl From<usize> for Cursor {
    fn from(pos: usize) -> Self {
        Self { pos: DocCharOffset(pos), ..Default::default() }
    }
}

impl From<DocCharOffset> for Cursor {
    fn from(pos: DocCharOffset) -> Self {
        Self { pos, ..Default::default() }
    }
}

impl From<(usize, usize)> for Cursor {
    fn from(value: (usize, usize)) -> Self {
        Self { pos: value.1.into(), selection_origin: Some(value.0.into()), ..Default::default() }
    }
}

impl From<(DocCharOffset, DocCharOffset)> for Cursor {
    fn from(value: (DocCharOffset, DocCharOffset)) -> Self {
        Self { pos: value.1, selection_origin: Some(value.0), ..Default::default() }
    }
}

impl Cursor {
    /// returns selection origin if there is one or current position if there isn't
    pub fn selection_origin(&self) -> DocCharOffset {
        self.selection_origin.unwrap_or(self.pos)
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

    pub fn advance(
        &mut self, offset: Offset, backwards: bool, buffer: &SubBuffer, segs: &UnicodeSegs,
        galleys: &Galleys,
    ) {
        self.pos = self
            .pos
            .advance(&mut self.x_target, offset, backwards, buffer, segs, galleys);
    }

    pub fn rect(&self, segs: &UnicodeSegs, galleys: &Galleys) -> Rect {
        let (galley_idx, cursor) = galleys.galley_and_cursor_by_char_offset(self.pos, segs);
        let galley = &galleys[galley_idx];
        let cursor_size = Vec2 { x: 1.0, y: galley.cursor_height() };

        let max = DocCharOffset::cursor_to_pos_abs(galley, cursor);
        let min = max - cursor_size;
        Rect { min, max }
    }
}

/// Represents state required for parsing single/double/triple clicks/taps and drags
#[derive(Default)]
pub struct PointerState {
    /// Whether the primary pointer button was pressed last frame; used to detect click and drag
    pub pressed: bool,

    pub last_click_type: ClickType,

    /// Time of release of last few presses, used for double & triple click detection
    pub last_click_times: (Option<Instant>, Option<Instant>, Option<Instant>, Option<Instant>),
}

static DOUBLE_CLICK_PERIOD: Duration = Duration::from_millis(300);

#[derive(Clone, Copy, Default, PartialEq)]
pub enum ClickType {
    #[default]
    Single,
    Double,
    Triple,
    Quadruple,
}

impl PointerState {
    pub fn press(&mut self, t: Instant) -> ClickType {
        self.pressed = true;

        self.last_click_times.3 = self.last_click_times.2;
        self.last_click_times.2 = self.last_click_times.1;
        self.last_click_times.1 = self.last_click_times.0;
        self.last_click_times.0 = Some(t);

        self.last_click_type = match self.last_click_times {
            (_, None, _, _) => ClickType::Single,
            (Some(one), Some(two), _, _) if one - two > DOUBLE_CLICK_PERIOD => ClickType::Single,
            (_, _, None, _) => ClickType::Double,
            (_, Some(two), Some(three), _) if two - three > DOUBLE_CLICK_PERIOD => {
                ClickType::Double
            }
            (_, _, _, None) => ClickType::Triple,
            (_, _, Some(three), Some(four)) if three - four > DOUBLE_CLICK_PERIOD => {
                ClickType::Triple
            }
            _ => ClickType::Quadruple,
        };
        self.last_click_type
    }

    pub fn release(&mut self) {
        self.pressed = false;
    }
}
