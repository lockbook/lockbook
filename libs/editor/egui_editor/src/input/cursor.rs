use crate::bounds::Paragraphs;
use crate::buffer::SubBuffer;
use crate::galleys::Galleys;
use crate::input::canonical::{Bound, Offset};
use crate::offset_types::*;
use crate::unicode_segs::UnicodeSegs;
use egui::{Modifiers, Pos2, Rect, Vec2};
use std::ops::Range;
use std::time::{Duration, Instant};

// drag for longer than this amount of time or further than this distance to count as a drag
const DRAG_DURATION: Duration = Duration::from_millis(300);
const DRAG_DISTANCE: f32 = 10.0;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Cursor {
    /// Selected text. When selection is empty, elements are equal. First element represents start
    /// of selection and second element represents end of selection, which is the primary cursor
    /// position - elements are not ordered by value.
    pub selection: (DocCharOffset, DocCharOffset),

    /// When navigating using up/down keys, x_target stores the original *absolute* x coordinate of
    /// the cursor, which helps us keep the cursor in a consistent x position even navigating past
    /// lines that are shorter, empty, annotated, etc.
    pub x_target: Option<f32>,

    /// Marked text indicates prospective input by smart keyboards, rendered inline
    pub mark: Option<(DocCharOffset, DocCharOffset)>,

    /// Highlighted region within marked text to indicate keyboard suggestion target
    pub mark_highlight: Option<(DocCharOffset, DocCharOffset)>,
}

impl From<usize> for Cursor {
    fn from(pos: usize) -> Self {
        Self { selection: (pos.into(), pos.into()), ..Default::default() }
    }
}

impl From<DocCharOffset> for Cursor {
    fn from(pos: DocCharOffset) -> Self {
        pos.0.into()
    }
}

impl From<(usize, usize)> for Cursor {
    fn from(value: (usize, usize)) -> Self {
        Self { selection: (value.0.into(), value.1.into()), ..Default::default() }
    }
}

impl From<(DocCharOffset, DocCharOffset)> for Cursor {
    fn from(value: (DocCharOffset, DocCharOffset)) -> Self {
        Self { selection: value, ..Default::default() }
    }
}

impl Cursor {
    /// returns the sorted range of selected text
    pub fn selection_or_position(&self) -> Range<DocCharOffset> {
        Range { start: self.selection.start(), end: self.selection.end() }
    }

    /// returns the nonempty, sorted range of selected text, if any
    pub fn selection(&self) -> Option<Range<DocCharOffset>> {
        if self.empty() {
            None
        } else {
            Some(self.selection_or_position())
        }
    }

    /// returns the (nonempty) byte range of selected text, if any
    fn selection_bytes(&self, segs: &UnicodeSegs) -> Option<Range<DocByteOffset>> {
        let selection_chars = self.selection();
        selection_chars.map(|sr| Range {
            start: segs.offset_to_byte(sr.start),
            end: segs.offset_to_byte(sr.end),
        })
    }

    /// returns the (possibly empty) selected text
    pub fn selection_text<'b>(&self, buffer: &'b SubBuffer) -> &'b str {
        if let Some(selection_bytes) = self.selection_bytes(&buffer.segs) {
            &buffer.text[selection_bytes.start.0..selection_bytes.end.0]
        } else {
            ""
        }
    }

    /// returns the nonempty, sorted range of selected text, if any
    pub fn mark_highlight(&self) -> Option<Range<DocCharOffset>> {
        match self.mark_highlight {
            Some(mark_highlight) if !mark_highlight.is_empty() => {
                Some(Range { start: mark_highlight.0, end: mark_highlight.1 })
            }
            _ => None,
        }
    }

    pub fn advance(
        &mut self, offset: Offset, backwards: bool, buffer: &SubBuffer, galleys: &Galleys,
        paragraphs: &Paragraphs,
    ) {
        self.selection.1 = self.selection.1.advance(
            &mut self.x_target,
            offset,
            backwards,
            true,
            buffer,
            galleys,
            paragraphs,
        );
    }

    /// use to put the cursor in a place that's invalid for it to be at the end of the frame e.g. inside captured characters
    /// use only if you're sure your modifications will leave the cursor in a valid place at the end of the frame
    pub fn advance_for_edit(
        &mut self, offset: Offset, backwards: bool, buffer: &SubBuffer, galleys: &Galleys,
        paragraphs: &Paragraphs,
    ) {
        self.selection.1 = self.selection.1.advance(
            &mut self.x_target,
            offset,
            backwards,
            false,
            buffer,
            galleys,
            paragraphs,
        );
    }

    pub fn start_rect(&self, galleys: &Galleys) -> Rect {
        self.rect(galleys, self.selection.0)
    }

    pub fn end_rect(&self, galleys: &Galleys) -> Rect {
        self.rect(galleys, self.selection.1)
    }

    fn rect(&self, galleys: &Galleys, offset: DocCharOffset) -> Rect {
        let (galley_idx, cursor) = galleys.galley_and_cursor_by_char_offset(offset);
        let galley = &galleys[galley_idx];
        let cursor_size = Vec2 { x: 1.0, y: galley.cursor_height() };

        let max = DocCharOffset::cursor_to_pos_abs(galley, cursor);
        let min = max - cursor_size;
        Rect { min, max }
    }

    fn empty(&self) -> bool {
        self.selection.0 == self.selection.1
    }

    pub fn at_line_bound(
        &self, buffer: &SubBuffer, galleys: &Galleys, paragraphs: &Paragraphs,
    ) -> bool {
        let mut line_start = *self;
        let mut line_end = *self;
        line_start.advance(Offset::To(Bound::Line), true, buffer, galleys, paragraphs);
        line_end.advance(Offset::To(Bound::Line), false, buffer, galleys, paragraphs);
        self.selection.1 == line_start.selection.1 || self.selection.1 == line_end.selection.1
    }
}

/// Represents state required for parsing single/double/triple clicks/taps and drags
#[derive(Default)]
pub struct PointerState {
    /// Type, position, modifiers, and drag status of current click, recorded on press and processed on release
    pub click_type: Option<ClickType>,
    pub click_pos: Option<Pos2>,
    pub click_mods: Option<Modifiers>,
    pub click_dragged: Option<bool>,

    /// Time of release of last few presses, used for double & triple click detection
    pub last_click_times: (Option<Instant>, Option<Instant>, Option<Instant>, Option<Instant>),
}

static DOUBLE_CLICK_PERIOD: Duration = Duration::from_millis(300);

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum ClickType {
    #[default]
    Single,
    Double,
    Triple,
    Quadruple,
}

impl PointerState {
    pub fn press(&mut self, t: Instant, pos: Pos2, modifiers: Modifiers) {
        self.last_click_times.3 = self.last_click_times.2;
        self.last_click_times.2 = self.last_click_times.1;
        self.last_click_times.1 = self.last_click_times.0;
        self.last_click_times.0 = Some(t);

        self.click_type = Some(match self.last_click_times {
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
        });
        self.click_pos = Some(pos);
        self.click_mods = Some(modifiers);
        self.click_dragged = Some(false);
    }

    pub fn drag(&mut self, t: Instant, pos: Pos2) {
        if let Some(click_pos) = self.click_pos {
            if pos.distance(click_pos) > DRAG_DISTANCE {
                self.click_dragged = Some(true);
            }
        }
        if let Some(click_time) = self.last_click_times.0 {
            if t - click_time > DRAG_DURATION {
                self.click_dragged = Some(true);
            }
        }
    }

    pub fn release(&mut self) {
        self.click_type = None;
        self.click_pos = None;
        self.click_mods = None;
        self.click_dragged = None;
    }
}
