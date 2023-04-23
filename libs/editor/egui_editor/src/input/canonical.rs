use crate::input::click_checker::ClickChecker;
use crate::input::cursor::{ClickType, PointerState};
use crate::offset_types::DocCharOffset;
use egui::{Event, Key, Modifiers, PointerButton, Pos2};
use std::time::Instant;

/// text location
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Location {
    CurrentCursor, // start or end of current selection depending on context
    DocCharOffset(DocCharOffset),
    Pos(Pos2),
}

/// text unit that has a start and end location
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Bound {
    Word,
    Line,
    Doc,
}

/// text unit you can increment or decrement a location by
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Increment {
    Char,
    Line,
}

/// text location relative to some absolute text location
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Offset {
    To(Bound),
    By(Increment),
}

/// text region specified in some manner
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Region {
    /// 0-length region starting and ending at location
    Location(Location),

    /// text from secondary cursor to location. preserves selection.
    ToLocation(Location),

    /// currently selected text
    Selection,

    /// currently selected text, or if the selection is empty, text from the primary cursor
    /// to one char/line before/after or to start/end of word/line/doc
    SelectionOrOffset { offset: Offset, backwards: bool },

    /// text from primary cursor to one char/line before/after or to start/end of word/line/doc.
    ToOffset { offset: Offset, backwards: bool, extend_selection: bool },

    /// current word/line/doc
    Bound { bound: Bound },

    /// word/line/doc at a location
    BoundAt { bound: Bound, location: Location },
}

/// Standardized edits to any editor state e.g. buffer, clipboard, debug state.
/// May depend on render state e.g. galley positions, line wrap.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Modification {
    Select { region: Region },
    Mark { start: DocCharOffset, end: DocCharOffset },
    Replace { region: Region, text: String },
    Newline, // distinct from replace because it triggers auto-bullet, etc
    Indent { deindent: bool },
    Undo,
    Redo,
    Cut,
    Copy,
    ToggleDebug,
    ToggleCheckbox(usize),
    OpenUrl(String),
}

impl From<&Modifiers> for Offset {
    fn from(modifiers: &Modifiers) -> Self {
        if modifiers.command {
            Offset::To(Bound::Line)
        } else if modifiers.alt {
            Offset::To(Bound::Word)
        } else {
            Offset::By(Increment::Char)
        }
    }
}

pub fn calc(
    event: &Event, click_checker: impl ClickChecker, pointer_state: &mut PointerState, now: Instant,
) -> Option<Modification> {
    Some(match event {
        Event::Key { key, pressed: true, modifiers, .. }
            if matches!(key, Key::ArrowUp | Key::ArrowDown) =>
        {
            Modification::Select {
                region: Region::ToOffset {
                    offset: if modifiers.command {
                        Offset::To(Bound::Doc)
                    } else {
                        Offset::By(Increment::Line)
                    },
                    backwards: key == &Key::ArrowUp,
                    extend_selection: modifiers.shift,
                },
            }
        }
        Event::Key { key, pressed: true, modifiers, .. }
            if matches!(key, Key::ArrowRight | Key::ArrowLeft | Key::Home | Key::End) =>
        {
            Modification::Select {
                region: Region::ToOffset {
                    offset: if matches!(key, Key::Home | Key::End) {
                        Offset::To(Bound::Line)
                    } else {
                        Offset::from(modifiers)
                    },
                    backwards: matches!(key, Key::ArrowLeft | Key::Home),
                    extend_selection: modifiers.shift,
                },
            }
        }
        Event::Text(text) | Event::Paste(text) => {
            Modification::Replace { region: Region::Selection, text: text.clone() }
        }
        Event::Key { key, pressed: true, modifiers, .. }
            if matches!(key, Key::Backspace | Key::Delete) =>
        {
            Modification::Replace {
                region: Region::SelectionOrOffset {
                    offset: Offset::from(modifiers),
                    backwards: key == &Key::Backspace,
                },
                text: "".to_string(),
            }
        }
        Event::Key { key: Key::Enter, pressed: true, .. } => Modification::Newline,
        Event::Key { key: Key::Tab, pressed: true, modifiers, .. } => {
            Modification::Indent { deindent: modifiers.shift }
        }
        Event::Key { key: Key::A, pressed: true, modifiers, .. } if modifiers.command => {
            Modification::Select { region: Region::Bound { bound: Bound::Doc } }
        }
        Event::Key { key: Key::X, pressed: true, modifiers, .. } if modifiers.command => {
            Modification::Cut
        }
        Event::Key { key: Key::C, pressed: true, modifiers, .. } if modifiers.command => {
            Modification::Copy
        }
        Event::Key { key: Key::Z, pressed: true, modifiers, .. } if modifiers.command => {
            if !modifiers.shift {
                Modification::Undo
            } else {
                Modification::Redo
            }
        }
        Event::PointerButton { pos, button: PointerButton::Primary, pressed: true, modifiers }
            if click_checker.ui(*pos) =>
        {
            if let Some(galley_idx) = click_checker.checkbox(*pos) {
                return Some(Modification::ToggleCheckbox(galley_idx));
            }
            if modifiers.command {
                if let Some(url) = click_checker.link(*pos) {
                    return Some(Modification::OpenUrl(url));
                }
            }

            let click_type = pointer_state.press(now);
            let location = Location::Pos(*pos);
            Modification::Select {
                region: if modifiers.shift {
                    Region::ToLocation(location)
                } else {
                    match click_type {
                        ClickType::Single => Region::Location(location),
                        ClickType::Double => Region::BoundAt { bound: Bound::Word, location },
                        ClickType::Triple => Region::BoundAt { bound: Bound::Line, location },
                        ClickType::Quadruple => Region::BoundAt { bound: Bound::Doc, location },
                    }
                },
            }
        }
        Event::PointerMoved(pos) if click_checker.ui(*pos) => {
            if !pointer_state.pressed || pointer_state.last_click_type != ClickType::Single {
                return None;
            } else {
                Modification::Select { region: Region::ToLocation(Location::Pos(*pos)) }
            }
        }
        Event::PointerButton { pos, button: PointerButton::Primary, pressed: false, .. }
            if click_checker.ui(*pos) =>
        {
            pointer_state.release();
            return None;
        }
        Event::Key { key: Key::F2, pressed: true, .. } => Modification::ToggleDebug,
        _ => {
            return None;
        }
    })
}

#[cfg(test)]
mod test {
    use super::calc;
    use crate::input::canonical::{Bound, Increment, Modification, Offset, Region};
    use crate::input::click_checker::ClickChecker;
    use egui::{Event, Key, Modifiers, Pos2};
    use std::time::Instant;

    #[derive(Default)]
    struct TestClickChecker {
        ui: bool,
        checkbox: Option<usize>,
        link: Option<String>,
    }

    impl ClickChecker for TestClickChecker {
        fn ui(&self, _pos: Pos2) -> bool {
            self.ui
        }

        fn checkbox(&self, _pos: Pos2) -> Option<usize> {
            self.checkbox
        }

        fn link(&self, _pos: Pos2) -> Option<String> {
            self.link.clone()
        }
    }

    #[test]
    fn calc_down() {
        assert!(matches!(
            calc(
                &Event::Key {
                    key: Key::ArrowDown,
                    pressed: true,
                    repeat: false,
                    modifiers: Default::default()
                },
                TestClickChecker::default(),
                &mut Default::default(),
                Instant::now()
            ),
            Some(Modification::Select {
                region: Region::ToOffset {
                    offset: Offset::By(Increment::Line),
                    backwards: false,
                    extend_selection: false,
                },
            })
        ));
    }

    #[test]
    fn calc_cmd_down() {
        assert!(matches!(
            calc(
                &Event::Key {
                    key: Key::ArrowDown,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers { command: true, ..Default::default() },
                },
                TestClickChecker::default(),
                &mut Default::default(),
                Instant::now()
            ),
            Some(Modification::Select {
                region: Region::ToOffset {
                    offset: Offset::To(Bound::Doc),
                    backwards: false,
                    extend_selection: false,
                },
            })
        ));
    }

    #[test]
    fn calc_shift_down() {
        assert!(matches!(
            calc(
                &Event::Key {
                    key: Key::ArrowDown,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers { shift: true, ..Default::default() },
                },
                TestClickChecker::default(),
                &mut Default::default(),
                Instant::now()
            ),
            Some(Modification::Select {
                region: Region::ToOffset {
                    offset: Offset::By(Increment::Line),
                    backwards: false,
                    extend_selection: true,
                },
            })
        ));
    }

    #[test]
    fn calc_cmd_shift_down() {
        assert!(matches!(
            calc(
                &Event::Key {
                    key: Key::ArrowDown,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers { command: true, shift: true, ..Default::default() },
                },
                TestClickChecker::default(),
                &mut Default::default(),
                Instant::now()
            ),
            Some(Modification::Select {
                region: Region::ToOffset {
                    offset: Offset::To(Bound::Doc),
                    backwards: false,
                    extend_selection: true,
                },
            })
        ));
    }

    #[test]
    fn calc_up() {
        assert!(matches!(
            calc(
                &Event::Key {
                    key: Key::ArrowUp,
                    pressed: true,
                    repeat: false,
                    modifiers: Default::default()
                },
                TestClickChecker::default(),
                &mut Default::default(),
                Instant::now()
            ),
            Some(Modification::Select {
                region: Region::ToOffset {
                    offset: Offset::By(Increment::Line),
                    backwards: true,
                    extend_selection: false,
                },
            })
        ));
    }

    #[test]
    fn calc_cmd_up() {
        assert!(matches!(
            calc(
                &Event::Key {
                    key: Key::ArrowUp,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers { command: true, ..Default::default() },
                },
                TestClickChecker::default(),
                &mut Default::default(),
                Instant::now()
            ),
            Some(Modification::Select {
                region: Region::ToOffset {
                    offset: Offset::To(Bound::Doc),
                    backwards: true,
                    extend_selection: false,
                },
            })
        ));
    }

    #[test]
    fn calc_shift_up() {
        assert!(matches!(
            calc(
                &Event::Key {
                    key: Key::ArrowUp,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers { shift: true, ..Default::default() },
                },
                TestClickChecker::default(),
                &mut Default::default(),
                Instant::now()
            ),
            Some(Modification::Select {
                region: Region::ToOffset {
                    offset: Offset::By(Increment::Line),
                    backwards: true,
                    extend_selection: true,
                },
            })
        ));
    }

    #[test]
    fn calc_cmd_shift_up() {
        assert!(matches!(
            calc(
                &Event::Key {
                    key: Key::ArrowUp,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers { command: true, shift: true, ..Default::default() },
                },
                TestClickChecker::default(),
                &mut Default::default(),
                Instant::now()
            ),
            Some(Modification::Select {
                region: Region::ToOffset {
                    offset: Offset::To(Bound::Doc),
                    backwards: true,
                    extend_selection: true,
                },
            })
        ));
    }

    #[test]
    fn calc_right() {
        assert!(matches!(
            calc(
                &Event::Key {
                    key: Key::ArrowRight,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers { ..Default::default() },
                },
                TestClickChecker::default(),
                &mut Default::default(),
                Instant::now()
            ),
            Some(Modification::Select {
                region: Region::ToOffset {
                    offset: Offset::By(Increment::Char),
                    backwards: false,
                    extend_selection: false,
                },
            })
        ));
    }

    #[test]
    fn calc_alt_right() {
        assert!(matches!(
            calc(
                &Event::Key {
                    key: Key::ArrowRight,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers { alt: true, ..Default::default() },
                },
                TestClickChecker::default(),
                &mut Default::default(),
                Instant::now()
            ),
            Some(Modification::Select {
                region: Region::ToOffset {
                    offset: Offset::To(Bound::Word),
                    backwards: false,
                    extend_selection: false,
                },
            })
        ));
    }

    #[test]
    fn calc_cmd_right() {
        assert!(matches!(
            calc(
                &Event::Key {
                    key: Key::ArrowRight,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers { command: true, ..Default::default() },
                },
                TestClickChecker::default(),
                &mut Default::default(),
                Instant::now()
            ),
            Some(Modification::Select {
                region: Region::ToOffset {
                    offset: Offset::To(Bound::Line),
                    backwards: false,
                    extend_selection: false,
                },
            })
        ));
    }

    #[test]
    fn calc_shift_right() {
        assert!(matches!(
            calc(
                &Event::Key {
                    key: Key::ArrowRight,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers { shift: true, ..Default::default() },
                },
                TestClickChecker::default(),
                &mut Default::default(),
                Instant::now()
            ),
            Some(Modification::Select {
                region: Region::ToOffset {
                    offset: Offset::By(Increment::Char),
                    backwards: false,
                    extend_selection: true,
                },
            })
        ));
    }

    #[test]
    fn calc_alt_shift_right() {
        assert!(matches!(
            calc(
                &Event::Key {
                    key: Key::ArrowRight,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers { alt: true, shift: true, ..Default::default() },
                },
                TestClickChecker::default(),
                &mut Default::default(),
                Instant::now()
            ),
            Some(Modification::Select {
                region: Region::ToOffset {
                    offset: Offset::To(Bound::Word),
                    backwards: false,
                    extend_selection: true,
                },
            })
        ));
    }

    #[test]
    fn calc_cmd_shift_right() {
        assert!(matches!(
            calc(
                &Event::Key {
                    key: Key::ArrowRight,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers { command: true, shift: true, ..Default::default() },
                },
                TestClickChecker::default(),
                &mut Default::default(),
                Instant::now()
            ),
            Some(Modification::Select {
                region: Region::ToOffset {
                    offset: Offset::To(Bound::Line),
                    backwards: false,
                    extend_selection: true,
                },
            })
        ));
    }

    #[test]
    fn calc_end() {
        assert!(matches!(
            calc(
                &Event::Key {
                    key: Key::End,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers { ..Default::default() },
                },
                TestClickChecker::default(),
                &mut Default::default(),
                Instant::now()
            ),
            Some(Modification::Select {
                region: Region::ToOffset {
                    offset: Offset::To(Bound::Line),
                    backwards: false,
                    extend_selection: false,
                },
            })
        ));
    }

    #[test]
    fn calc_shift_end() {
        assert!(matches!(
            calc(
                &Event::Key {
                    key: Key::End,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers { shift: true, ..Default::default() },
                },
                TestClickChecker::default(),
                &mut Default::default(),
                Instant::now()
            ),
            Some(Modification::Select {
                region: Region::ToOffset {
                    offset: Offset::To(Bound::Line),
                    backwards: false,
                    extend_selection: true,
                },
            })
        ));
    }

    #[test]
    fn calc_left() {
        assert!(matches!(
            calc(
                &Event::Key {
                    key: Key::ArrowLeft,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers { ..Default::default() },
                },
                TestClickChecker::default(),
                &mut Default::default(),
                Instant::now()
            ),
            Some(Modification::Select {
                region: Region::ToOffset {
                    offset: Offset::By(Increment::Char),
                    backwards: true,
                    extend_selection: false,
                },
            })
        ));
    }

    #[test]
    fn calc_alt_left() {
        assert!(matches!(
            calc(
                &Event::Key {
                    key: Key::ArrowLeft,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers { alt: true, ..Default::default() },
                },
                TestClickChecker::default(),
                &mut Default::default(),
                Instant::now()
            ),
            Some(Modification::Select {
                region: Region::ToOffset {
                    offset: Offset::To(Bound::Word),
                    backwards: true,
                    extend_selection: false,
                },
            })
        ));
    }

    #[test]
    fn calc_cmd_left() {
        assert!(matches!(
            calc(
                &Event::Key {
                    key: Key::ArrowLeft,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers { command: true, ..Default::default() },
                },
                TestClickChecker::default(),
                &mut Default::default(),
                Instant::now()
            ),
            Some(Modification::Select {
                region: Region::ToOffset {
                    offset: Offset::To(Bound::Line),
                    backwards: true,
                    extend_selection: false,
                },
            })
        ));
    }

    #[test]
    fn calc_shift_left() {
        assert!(matches!(
            calc(
                &Event::Key {
                    key: Key::ArrowLeft,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers { shift: true, ..Default::default() },
                },
                TestClickChecker::default(),
                &mut Default::default(),
                Instant::now()
            ),
            Some(Modification::Select {
                region: Region::ToOffset {
                    offset: Offset::By(Increment::Char),
                    backwards: true,
                    extend_selection: true,
                },
            })
        ));
    }

    #[test]
    fn calc_alt_shift_left() {
        assert!(matches!(
            calc(
                &Event::Key {
                    key: Key::ArrowLeft,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers { alt: true, shift: true, ..Default::default() },
                },
                TestClickChecker::default(),
                &mut Default::default(),
                Instant::now()
            ),
            Some(Modification::Select {
                region: Region::ToOffset {
                    offset: Offset::To(Bound::Word),
                    backwards: true,
                    extend_selection: true,
                },
            })
        ));
    }

    #[test]
    fn calc_cmd_shift_left() {
        assert!(matches!(
            calc(
                &Event::Key {
                    key: Key::ArrowLeft,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers { command: true, shift: true, ..Default::default() },
                },
                TestClickChecker::default(),
                &mut Default::default(),
                Instant::now()
            ),
            Some(Modification::Select {
                region: Region::ToOffset {
                    offset: Offset::To(Bound::Line),
                    backwards: true,
                    extend_selection: true,
                },
            })
        ));
    }

    #[test]
    fn calc_home() {
        assert!(matches!(
            calc(
                &Event::Key {
                    key: Key::Home,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers { ..Default::default() },
                },
                TestClickChecker::default(),
                &mut Default::default(),
                Instant::now()
            ),
            Some(Modification::Select {
                region: Region::ToOffset {
                    offset: Offset::To(Bound::Line),
                    backwards: true,
                    extend_selection: false,
                },
            })
        ));
    }

    #[test]
    fn calc_shift_home() {
        assert!(matches!(
            calc(
                &Event::Key {
                    key: Key::Home,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers { shift: true, ..Default::default() },
                },
                TestClickChecker::default(),
                &mut Default::default(),
                Instant::now()
            ),
            Some(Modification::Select {
                region: Region::ToOffset {
                    offset: Offset::To(Bound::Line),
                    backwards: true,
                    extend_selection: true,
                },
            })
        ));
    }
}
