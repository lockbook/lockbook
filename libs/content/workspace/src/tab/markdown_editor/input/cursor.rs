use crate::tab::markdown_editor::appearance::Appearance;
use crate::tab::markdown_editor::bounds::{Bounds, Text};
use crate::tab::markdown_editor::buffer::SubBuffer;
use crate::tab::markdown_editor::galleys::{self, Galleys};
use crate::tab::markdown_editor::input::canonical::Offset;
use crate::tab::markdown_editor::offset_types::*;
use crate::tab::markdown_editor::unicode_segs::UnicodeSegs;
use egui::{Modifiers, Pos2, Vec2};
use std::ops::Range;
use std::time::{Duration, Instant};

// drag for longer than this amount of time or further than this distance to count as a drag
const DRAG_DURATION: Duration = Duration::from_millis(300);
const DRAG_DISTANCE: f32 = 10.0;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Cursor {
    /// Selected text. When selection is empty, elements are equal. First element represents start
    /// of selection and second element represents end of selection, which is the primary cursor
    /// position - elements are not ordered by value.
    pub selection: Range<DocCharOffset>,

    /// When navigating using up/down keys, x_target stores the original *absolute* x coordinate of
    /// the cursor, which helps us keep the cursor in a consistent x position even navigating past
    /// lines that are shorter, empty, annotated, etc.
    pub x_target: Option<f32>,

    /// Marked text indicates prospective input by smart keyboards, rendered inline
    pub mark: Option<Range<DocCharOffset>>,

    /// Highlighted region within marked text to indicate keyboard suggestion target
    pub mark_highlight: Option<Range<DocCharOffset>>,
}

impl From<usize> for Cursor {
    fn from(pos: usize) -> Self {
        Self { selection: pos.into()..pos.into(), ..Default::default() }
    }
}

impl From<DocCharOffset> for Cursor {
    fn from(pos: DocCharOffset) -> Self {
        pos.0.into()
    }
}

impl From<Range<usize>> for Cursor {
    fn from(value: Range<usize>) -> Self {
        Self { selection: value.start.into()..value.end.into(), ..Default::default() }
    }
}

impl From<Range<DocCharOffset>> for Cursor {
    fn from(value: Range<DocCharOffset>) -> Self {
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

    pub fn mark_highlight(&self) -> Option<Range<DocCharOffset>> {
        match self.mark_highlight {
            Some(ref mark_highlight) if !mark_highlight.is_empty() => Some(mark_highlight.clone()),
            _ => None,
        }
    }

    pub fn advance(
        &mut self, offset: Offset, backwards: bool, buffer: &SubBuffer, galleys: &Galleys,
        bounds: &Bounds,
    ) {
        self.selection.start = self.selection.end.advance(
            &mut self.x_target,
            offset,
            backwards,
            buffer,
            galleys,
            bounds,
        );
    }

    pub fn start_line(&self, galleys: &Galleys, text: &Text, appearance: &Appearance) -> [Pos2; 2] {
        self.line(galleys, self.selection.start, text, appearance)
    }

    pub fn end_line(&self, galleys: &Galleys, text: &Text, appearance: &Appearance) -> [Pos2; 2] {
        self.line(galleys, self.selection.end, text, appearance)
    }

    fn line(
        &self, galleys: &Galleys, offset: DocCharOffset, text: &Text, appearance: &Appearance,
    ) -> [Pos2; 2] {
        let (galley_idx, cursor) = galleys.galley_and_cursor_by_char_offset(offset, text);
        let galley = &galleys[galley_idx];

        let max = DocCharOffset::cursor_to_pos_abs(galley, cursor);
        let min = max - Vec2 { x: 0.0, y: galley.cursor_height() };

        if offset < galley.text_range().start() {
            // draw cursor before offset if that's where it is
            let annotation_offset = galleys::annotation_offset(&galley.annotation, appearance);
            [min - annotation_offset, max - annotation_offset]
        } else {
            [min, max]
        }
    }

    fn empty(&self) -> bool {
        self.selection.is_empty()
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
    pub pointer_pos: Option<Pos2>,

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
        self.pointer_pos = Some(pos)
    }

    pub fn drag(&mut self, t: Instant, pos: Pos2) {
        if let Some(click_pos) = self.click_pos {
            if pos.distance(click_pos) > DRAG_DISTANCE {
                self.click_dragged = Some(true);
            }
            if let Some(click_time) = self.last_click_times.0 {
                if t - click_time > DRAG_DURATION {
                    self.click_dragged = Some(true);
                }
            }
        }
        self.pointer_pos = Some(pos)
    }

    pub fn release(&mut self) {
        self.click_type = None;
        self.click_pos = None;
        self.click_mods = None;
        self.click_dragged = None;
    }
}
