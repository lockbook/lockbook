use crate::tab::markdown_editor;
use egui::{Modifiers, Pos2, Vec2};
use lb_rs::text::offset_types::DocCharOffset;
use lb_rs::text::offset_types::RangeExt as _;
use markdown_editor::appearance::Appearance;
use markdown_editor::bounds::Text;
use markdown_editor::galleys::{self, Galleys};
use std::time::{Duration, Instant};

use super::advance::AdvanceExt as _;

// drag for longer than this amount of time or further than this distance to count as a drag
const DRAG_DURATION: Duration = Duration::from_millis(300);
const DRAG_DISTANCE: f32 = 10.0;

#[derive(Debug, Default)]
pub struct CursorState {
    /// When navigating using up/down keys, x_target stores the original *absolute* x coordinate of
    /// the cursor, which helps us keep the cursor in a consistent x position even navigating past
    /// lines that are shorter, empty, annotated, etc.
    pub x_target: Option<f32>,
}

pub fn line(
    offset: DocCharOffset, galleys: &Galleys, text: &Text, appearance: &Appearance,
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
