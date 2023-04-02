use crate::appearance::Appearance;
use crate::ast::Ast;
use crate::buffer::{Buffer, Modification, SubBuffer, SubModification};
use crate::cursor::Cursor;
use crate::debug::DebugInfo;
use crate::element::{Element, ItemType};
use crate::galleys::Galleys;
use crate::layouts::{Annotation, Layouts};
use crate::offset_types::DocCharOffset;
use crate::unicode_segs::UnicodeSegs;
use egui::{Event, Key, PointerButton, Pos2, Vec2};
use std::cmp::Ordering;
use std::time::{Duration, Instant};

/// text location
#[derive(Clone, Copy, Debug)]
pub enum Location {
    CurrentCursor, // start or end of current selection depending on context
    DocCharOffset(DocCharOffset),
    Pos(Pos2),
}

/// text unit that has a start and end location
#[derive(Clone, Copy, Debug)]
pub enum Bound {
    Word,
    Line,
    Doc,
}

/// text unit you can increment or decrement a location by
#[derive(Clone, Copy, Debug)]
pub enum Increment {
    Char,
    Line,
}

/// text location relative to some absolute text location
#[derive(Clone, Copy, Debug)]
pub enum Offset {
    To(Bound),
    By(Increment),
}

/// text region specified in some manner
#[derive(Clone, Copy, Debug)]
pub enum Region {
    Range { start: Location, end: Location }, // text between two locations
    Bound { location: Location, bound: Bound }, // word/line/doc at a location
    Increment { offset: Offset, backwards: bool, extend_selection: bool }, // text from current cursor to one char/line before/after or to start/end of word/line/doc
}

/// Standardized edits to any editor state e.g. buffer, clipboard, debug state.
/// May depend on render state e.g. galley positions, line wrap.
#[derive(Clone, Debug)]
pub enum EditorModification {
    Select { region: Region },
    Mark { start: DocCharOffset, end: DocCharOffset },
    Replace { region: Region, text: String },
    Indent { deindent: bool },
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    OpenUrl { location: Location },
    ToggleDebug,
}

/// Represents state required for parsing single/double/triple clicks/taps and drags
pub struct PointerState {
    /// Whether the primary pointer button was pressed last frame; used to detect click and drag
    pressed: bool,

    /// Time of release of last three presses, used for double & triple click detection
    last_click_times: (Option<Instant>, Option<Instant>, Option<Instant>),
}

static DOUBLE_CLICK_PERIOD: Duration = Duration::from_millis(300);

pub enum ClickType {
    Single,
    Double,
    Triple,
    Drag,
}

impl PointerState {
    pub fn press(&mut self, t: Instant) -> ClickType {
        self.pressed = true;
        self.last_click_times.2 = self.last_click_times.1;
        self.last_click_times.1 = self.last_click_times.0;
        self.last_click_times.0 = Some(t);

        if self.pressed {
            return ClickType::Drag;
        }

        if let (Some(one_click_ago), Some(three_clicks_ago)) =
            (self.last_click_times.0, self.last_click_times.2)
        {
            if one_click_ago - three_clicks_ago < DOUBLE_CLICK_PERIOD * 2 {
                return ClickType::Triple;
            }
        }

        if let (Some(one_click_ago), Some(two_clicks_ago)) =
            (self.last_click_times.0, self.last_click_times.1)
        {
            if one_click_ago - two_clicks_ago < DOUBLE_CLICK_PERIOD {
                return ClickType::Double;
            }
        }

        ClickType::Single
    }

    pub fn release(&mut self) {
        self.pressed = false;
    }
}

// todo: revisit params
pub fn calc(
    event: &Event, ast: &Ast, layouts: &Layouts, galleys: &Galleys, appearance: &Appearance,
    buffer: &mut Buffer, debug: &mut DebugInfo, ui_size: Vec2,
) -> Option<EditorModification> {
    Some(match event {
        // up/down navigation
        Event::Key { key, pressed: true, modifiers }
            if matches!(key, Key::ArrowUp | Key::ArrowDown) =>
        {
            EditorModification::Select {
                region: Region::Increment {
                    offset: if modifiers.command {
                        Offset::By(Increment::Line)
                    } else {
                        Offset::By(Increment::Char)
                    },
                    backwards: key == &Key::ArrowUp,
                    extend_selection: modifiers.shift,
                },
            }
        }
        // left/right/home/end navigation
        Event::Key { key, pressed: true, modifiers }
            if matches!(key, Key::ArrowRight | Key::ArrowLeft | Key::Home | Key::End) =>
        {
            EditorModification::Select {
                region: Region::Increment {
                    offset: if matches!(key, Key::Home | Key::End) || modifiers.command {
                        Offset::To(Bound::Line)
                    } else if modifiers.alt {
                        Offset::To(Bound::Word)
                    } else {
                        Offset::By(Increment::Char)
                    },
                    backwards: matches!(key, Key::ArrowLeft | Key::Home),
                    extend_selection: modifiers.shift,
                },
            }
        }
        _ => {
            return None;
        }
    })
}

pub fn pos_to_char_offset(
    pos: Pos2, galleys: &Galleys, segs: &UnicodeSegs,
) -> Option<DocCharOffset> {
    if !galleys.is_empty() {
        if pos.y < galleys[0].galley_location.min.y {
            // click position is above first galley
            Some(DocCharOffset(0))
        } else if pos.y >= galleys[galleys.len() - 1].galley_location.max.y {
            // click position is below last galley
            Some(segs.last_cursor_position())
        } else {
            let mut result = None;
            for galley_idx in 0..galleys.len() {
                let galley = &galleys[galley_idx];
                if galley.galley_location.contains(pos) {
                    // click position is in a galley
                    let relative_pos = pos - galley.text_location;
                    let new_cursor = galley.galley.cursor_from_pos(relative_pos);
                    result = Some(galleys.char_offset_by_galley_and_cursor(
                        galley_idx,
                        &new_cursor,
                        segs,
                    ));
                }
            }
            result
        }
    } else {
        None
    }
}

#[allow(clippy::too_many_arguments)]
fn increment_numbered_list_items(
    starting_layout_idx: usize, indent_level: u8, amount: usize, decrement: bool,
    segs: &UnicodeSegs, layouts: &Layouts, buffer: &SubBuffer, cursor: Cursor,
) -> Modification {
    let mut modifications = Vec::new();

    let mut layout_idx = starting_layout_idx;
    loop {
        layout_idx += 1;
        if layout_idx == layouts.len() {
            break;
        }
        let layout = &layouts[layout_idx];
        if let Some(Annotation::Item(item_type, cur_indent_level)) = &layout.annotation {
            match cur_indent_level.cmp(&indent_level) {
                Ordering::Greater => {
                    continue; // skip nested list items
                }
                Ordering::Less => {
                    break; // end of nested list
                }
                Ordering::Equal => {
                    if let ItemType::Numbered(cur_number) = item_type {
                        // replace cur_number with next_number in head
                        modifications.push(SubModification::Cursor {
                            cursor: Cursor {
                                pos: segs
                                    .byte_offset_to_char(layout.range.start + layout.head_size),
                                selection_origin: Some(
                                    segs.byte_offset_to_char(layout.range.start),
                                ),
                                ..Default::default()
                            },
                        });
                        let head = layout.head(buffer);
                        let text = head[0..head.len() - (cur_number).to_string().len() - 2]
                            .to_string()
                            + &(if !decrement {
                                cur_number.saturating_add(amount)
                            } else {
                                cur_number.saturating_sub(amount)
                            })
                            .to_string()
                            + ". ";
                        modifications.push(SubModification::Insert { text });
                        modifications.push(SubModification::Cursor { cursor });
                    }
                }
            }
        } else {
            break;
        }
    }

    modifications
}
