use crate::input::click_checker::ClickChecker;
use crate::input::cursor::{ClickType, PointerState};
use crate::offset_types::{DocCharOffset, RelCharOffset};
use crate::style::{InlineNode, MarkdownNode};
use crate::{CTextPosition, CTextRange};
use egui::{Event, Key, Modifiers, PointerButton, Pos2};
use pulldown_cmark::LinkType;
use std::time::Instant;

/// text location
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Location {
    CurrentCursor,
    DocCharOffset(DocCharOffset),
    Pos(Pos2),
}

/// text unit that has a start and end location
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Bound {
    Char,
    Word,
    Line,
    Paragraph,
    Doc,
}

/// text unit you can increment or decrement a location by
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Increment {
    Line,
}

/// text location relative to some absolute text location
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Offset {
    /// text location at a bound; if you're already there, this leaves you there (e.g. cmd+left/right)
    To(Bound),

    /// text location at the next bound; if you're already there, this goes to the next one (e.g. option+left/right)
    Next(Bound),

    /// text location some increment away
    By(Increment),
}

/// text region specified in some manner
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Region {
    /// 0-length region starting and ending at location
    Location(Location),

    /// Text from secondary cursor to location. preserves selection.
    ToLocation(Location),

    /// Text from one location to another
    BetweenLocations { start: Location, end: Location },

    /// Currently selected text
    Selection,

    /// Currently selected text, or if the selection is empty, text from the primary cursor to one char/line
    /// before/after or to start/end of word/line/doc
    SelectionOrOffset { offset: Offset, backwards: bool },

    /// Text from primary cursor to one char/line before/after or to start/end of word/line/paragraph/doc. In some
    /// situations this instead represents the start of selection (if `backwards`) or end of selection, based on what
    /// feels intuitive when using arrow keys to navigate a document.
    ToOffset { offset: Offset, backwards: bool, extend_selection: bool },

    /// Current word/line/paragraph/doc, preferring previous word if `backwards`
    Bound { bound: Bound, backwards: bool },

    /// Word/line/paragraph/doc at a location, preferring previous word if `backwards`
    BoundAt { bound: Bound, location: Location, backwards: bool },
}

/// Standardized edits to any editor state e.g. buffer, clipboard, debug state.
/// May depend on render state e.g. galley positions, line wrap.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Modification {
    Select { region: Region },
    StageMarked { highlighted: (RelCharOffset, RelCharOffset), text: String },
    CommitMarked,
    Replace { region: Region, text: String },
    ToggleStyle { region: Region, style: MarkdownNode },
    Newline { advance_cursor: bool }, // distinct from replace because it triggers auto-bullet, etc
    Indent { deindent: bool },
    Undo,
    Redo,
    Cut,
    Copy,
    ToggleDebug,
    ToggleCheckbox(usize),
    OpenUrl(String),
    Heading(u32),
    BulletListItem,
    NumberListItem,
    TodoListItem,
}

impl From<&Modifiers> for Offset {
    fn from(modifiers: &Modifiers) -> Self {
        let should_jump_line = modifiers.mac_cmd;

        let is_apple = cfg!(target_vendor = "apple");
        let is_apple_alt = is_apple && modifiers.alt;
        let is_non_apple_ctrl = !is_apple && modifiers.ctrl;
        let should_jump_word = is_apple_alt || is_non_apple_ctrl;

        if should_jump_line {
            Offset::To(Bound::Line)
        } else if should_jump_word {
            Offset::Next(Bound::Word)
        } else {
            Offset::Next(Bound::Char)
        }
    }
}

pub fn calc(
    event: &Event, click_checker: impl ClickChecker, pointer_state: &mut PointerState,
    now: Instant, touch_mode: bool,
) -> Option<Modification> {
    match event {
        Event::Key { key, pressed: true, modifiers, .. }
            if matches!(key, Key::ArrowUp | Key::ArrowDown) =>
        {
            Some(Modification::Select {
                region: Region::ToOffset {
                    offset: if modifiers.mac_cmd {
                        Offset::To(Bound::Doc)
                    } else {
                        Offset::By(Increment::Line)
                    },
                    backwards: key == &Key::ArrowUp,
                    extend_selection: modifiers.shift,
                },
            })
        }
        Event::Key { key, pressed: true, modifiers, .. }
            if matches!(key, Key::ArrowRight | Key::ArrowLeft | Key::Home | Key::End) =>
        {
            Some(Modification::Select {
                region: Region::ToOffset {
                    offset: if matches!(key, Key::Home | Key::End) {
                        Offset::To(Bound::Line)
                    } else {
                        Offset::from(modifiers)
                    },
                    backwards: matches!(key, Key::ArrowLeft | Key::Home),
                    extend_selection: modifiers.shift,
                },
            })
        }
        Event::Text(text) | Event::Paste(text) => {
            Some(Modification::Replace { region: Region::Selection, text: text.clone() })
        }
        Event::Key { key, pressed: true, modifiers, .. }
            if matches!(key, Key::Backspace | Key::Delete) =>
        {
            Some(Modification::Replace {
                region: Region::SelectionOrOffset {
                    offset: Offset::from(modifiers),
                    backwards: key == &Key::Backspace,
                },
                text: "".to_string(),
            })
        }
        Event::Key { key: Key::Enter, pressed: true, modifiers, .. } => {
            Some(Modification::Newline { advance_cursor: !modifiers.shift })
        }
        Event::Key { key: Key::Tab, pressed: true, modifiers, .. } if !modifiers.alt => {
            Some(Modification::Indent { deindent: modifiers.shift })
        }
        Event::Key { key: Key::A, pressed: true, modifiers, .. } if modifiers.command => {
            Some(Modification::Select {
                region: Region::Bound { bound: Bound::Doc, backwards: true },
            })
        }
        Event::Key { key: Key::X, pressed: true, modifiers, .. }
            if modifiers.command && !modifiers.shift =>
        {
            Some(Modification::Cut)
        }
        Event::Key { key: Key::C, pressed: true, modifiers, .. }
            if modifiers.command && !modifiers.shift =>
        {
            Some(Modification::Copy)
        }
        Event::Key { key: Key::Z, pressed: true, modifiers, .. } if modifiers.command => {
            if !modifiers.shift {
                Some(Modification::Undo)
            } else {
                Some(Modification::Redo)
            }
        }
        Event::Key { key: Key::B, pressed: true, modifiers, .. } if modifiers.command => {
            Some(Modification::ToggleStyle {
                region: Region::Selection,
                style: MarkdownNode::Inline(InlineNode::Bold),
            })
        }
        Event::Key { key: Key::I, pressed: true, modifiers, .. } if modifiers.command => {
            Some(Modification::ToggleStyle {
                region: Region::Selection,
                style: MarkdownNode::Inline(InlineNode::Italic),
            })
        }
        Event::Key { key: Key::C, pressed: true, modifiers, .. }
            if modifiers.command && modifiers.shift =>
        {
            Some(Modification::ToggleStyle {
                region: Region::Selection,
                style: MarkdownNode::Inline(InlineNode::Code),
            })
        }
        Event::Key { key: Key::X, pressed: true, modifiers, .. }
            if modifiers.command && modifiers.shift =>
        {
            Some(Modification::ToggleStyle {
                region: Region::Selection,
                style: MarkdownNode::Inline(InlineNode::Strikethrough),
            })
        }
        Event::Key { key: Key::K, pressed: true, modifiers, .. } if modifiers.command => {
            Some(Modification::ToggleStyle {
                region: Region::Selection,
                style: MarkdownNode::Inline(InlineNode::Link(
                    LinkType::Inline,
                    "".into(),
                    "".into(),
                )),
            })
        }
        Event::Key { key: Key::Num7, pressed: true, modifiers, .. }
            if modifiers.command && modifiers.shift =>
        {
            Some(Modification::NumberListItem)
        }
        Event::Key { key: Key::Num8, pressed: true, modifiers, .. }
            if modifiers.command && modifiers.shift =>
        {
            Some(Modification::BulletListItem)
        }
        Event::Key { key: Key::Num9, pressed: true, modifiers, .. }
            if modifiers.command && modifiers.shift =>
        {
            Some(Modification::TodoListItem)
        }
        Event::PointerButton { pos, button: PointerButton::Primary, pressed: true, modifiers }
            if click_checker.ui(*pos) =>
        {
            pointer_state.press(now, *pos, *modifiers);
            None
        }
        Event::PointerMoved(pos) if click_checker.ui(*pos) => {
            pointer_state.drag(now, *pos);
            if pointer_state.click_dragged.unwrap_or_default() && !touch_mode {
                if pointer_state.click_mods.unwrap_or_default().shift {
                    Some(Modification::Select { region: Region::ToLocation(Location::Pos(*pos)) })
                } else if let Some(click_pos) = pointer_state.click_pos {
                    Some(Modification::Select {
                        region: Region::BetweenLocations {
                            start: Location::Pos(click_pos),
                            end: Location::Pos(*pos),
                        },
                    })
                } else {
                    Some(Modification::Select { region: Region::ToLocation(Location::Pos(*pos)) })
                }
            } else {
                None
            }
        }
        Event::PointerButton { pos, button: PointerButton::Primary, pressed: false, .. } => {
            let click_type = pointer_state.click_type.unwrap_or_default();
            let click_pos = pointer_state.click_pos.unwrap_or_default();
            let click_mods = pointer_state.click_mods.unwrap_or_default();
            let click_dragged = pointer_state.click_dragged.unwrap_or_default();
            pointer_state.release();
            let location = Location::Pos(*pos);

            if let Some(galley_idx) = click_checker.checkbox(*pos, touch_mode) {
                Some(Modification::ToggleCheckbox(galley_idx))
            } else if let Some(url) = click_checker.link(*pos) {
                if touch_mode || click_mods.command {
                    Some(Modification::OpenUrl(url))
                } else {
                    None
                }
            } else {
                None
            }
            .or_else(|| {
                if click_checker.ui(*pos) {
                    Some(Modification::Select {
                        region: if click_mods.shift {
                            Region::ToLocation(location)
                        } else {
                            match click_type {
                                ClickType::Single => {
                                    if touch_mode {
                                        if !click_dragged {
                                            Region::Location(location)
                                        } else {
                                            return None;
                                        }
                                    } else {
                                        Region::BetweenLocations {
                                            start: Location::Pos(click_pos),
                                            end: location,
                                        }
                                    }
                                }
                                ClickType::Double => Region::BoundAt {
                                    bound: Bound::Word,
                                    location,
                                    backwards: true,
                                },
                                ClickType::Triple => Region::BoundAt {
                                    bound: Bound::Line,
                                    location,
                                    backwards: true,
                                },
                                ClickType::Quadruple => {
                                    Region::BoundAt { bound: Bound::Doc, location, backwards: true }
                                }
                            }
                        },
                    })
                } else {
                    None
                }
            })
        }
        Event::Key { key: Key::F2, pressed: true, .. } => Some(Modification::ToggleDebug),
        _ => None,
    }
}

impl From<CTextRange> for (DocCharOffset, DocCharOffset) {
    fn from(value: CTextRange) -> Self {
        (value.start.pos.into(), value.end.pos.into())
    }
}

impl From<CTextRange> for (RelCharOffset, RelCharOffset) {
    fn from(value: CTextRange) -> Self {
        (value.start.pos.into(), value.end.pos.into())
    }
}

impl From<CTextRange> for Region {
    fn from(value: CTextRange) -> Self {
        Region::BetweenLocations { start: value.start.into(), end: value.end.into() }
    }
}

impl From<CTextPosition> for Location {
    fn from(value: CTextPosition) -> Self {
        Self::DocCharOffset(value.pos.into())
    }
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
        text: Option<usize>,
        checkbox: Option<usize>,
        link: Option<String>,
    }

    impl ClickChecker for TestClickChecker {
        fn ui(&self, _pos: Pos2) -> bool {
            self.ui
        }

        fn text(&self, _pos: Pos2) -> Option<usize> {
            self.text
        }

        fn checkbox(&self, _pos: Pos2, _touch_mode: bool) -> Option<usize> {
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
                Instant::now(),
                false
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
                    modifiers: Modifiers { mac_cmd: true, ..Default::default() },
                },
                TestClickChecker::default(),
                &mut Default::default(),
                Instant::now(),
                false
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
                Instant::now(),
                false
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
                    modifiers: Modifiers { mac_cmd: true, shift: true, ..Default::default() },
                },
                TestClickChecker::default(),
                &mut Default::default(),
                Instant::now(),
                false
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
                Instant::now(),
                false
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
                    modifiers: Modifiers { mac_cmd: true, ..Default::default() },
                },
                TestClickChecker::default(),
                &mut Default::default(),
                Instant::now(),
                false
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
                Instant::now(),
                false
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
                    modifiers: Modifiers { mac_cmd: true, shift: true, ..Default::default() },
                },
                TestClickChecker::default(),
                &mut Default::default(),
                Instant::now(),
                false
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
                Instant::now(),
                false
            ),
            Some(Modification::Select {
                region: Region::ToOffset {
                    offset: Offset::Next(Bound::Char),
                    backwards: false,
                    extend_selection: false,
                },
            })
        ));
    }

    #[test]
    #[cfg(target_vendor = "Apple")]
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
                Instant::now(),
                false
            ),
            Some(Modification::Select {
                region: Region::ToOffset {
                    offset: Offset::Next(Bound::Word),
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
                    modifiers: Modifiers { mac_cmd: true, ..Default::default() },
                },
                TestClickChecker::default(),
                &mut Default::default(),
                Instant::now(),
                false
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
                Instant::now(),
                false
            ),
            Some(Modification::Select {
                region: Region::ToOffset {
                    offset: Offset::Next(Bound::Char),
                    backwards: false,
                    extend_selection: true,
                },
            })
        ));
    }

    #[test]
    #[cfg(target_vendor = "Apple")]
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
                Instant::now(),
                false
            ),
            Some(Modification::Select {
                region: Region::ToOffset {
                    offset: Offset::Next(Bound::Word),
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
                    modifiers: Modifiers { mac_cmd: true, shift: true, ..Default::default() },
                },
                TestClickChecker::default(),
                &mut Default::default(),
                Instant::now(),
                false
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
                Instant::now(),
                false
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
                Instant::now(),
                false
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
                Instant::now(),
                false
            ),
            Some(Modification::Select {
                region: Region::ToOffset {
                    offset: Offset::Next(Bound::Char),
                    backwards: true,
                    extend_selection: false,
                },
            })
        ));
    }

    #[test]
    #[cfg(target_vendor = "Apple")]
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
                Instant::now(),
                false
            ),
            Some(Modification::Select {
                region: Region::ToOffset {
                    offset: Offset::Next(Bound::Word),
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
                    modifiers: Modifiers { mac_cmd: true, ..Default::default() },
                },
                TestClickChecker::default(),
                &mut Default::default(),
                Instant::now(),
                false
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
                Instant::now(),
                false
            ),
            Some(Modification::Select {
                region: Region::ToOffset {
                    offset: Offset::Next(Bound::Char),
                    backwards: true,
                    extend_selection: true,
                },
            })
        ));
    }

    #[test]
    #[cfg(target_vendor = "Apple")]
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
                Instant::now(),
                false
            ),
            Some(Modification::Select {
                region: Region::ToOffset {
                    offset: Offset::Next(Bound::Word),
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
                    modifiers: Modifiers { mac_cmd: true, shift: true, ..Default::default() },
                },
                TestClickChecker::default(),
                &mut Default::default(),
                Instant::now(),
                false
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
                Instant::now(),
                false
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
                Instant::now(),
                false
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
