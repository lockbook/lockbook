use crate::tab::markdown_editor::appearance;
use crate::tab::markdown_editor::input::click_checker::ClickChecker;
use crate::tab::markdown_editor::input::cursor::{ClickType, PointerState};
use crate::tab::markdown_editor::style::{BlockNode, InlineNode, ListItem, MarkdownNode};
use egui::{Event, Key, Modifiers, PointerButton};
use pulldown_cmark::{HeadingLevel, LinkType};
use std::time::Instant;

use super::{Bound, Increment, Location, Modification, Offset, Region};

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
    now: Instant, touch_mode: bool, appearance: &appearance::Appearance,
) -> Option<Modification> {
    match event {
        Event::Key { key, pressed: true, modifiers, .. }
            if matches!(key, Key::ArrowUp | Key::ArrowDown) && !cfg!(target_os = "ios") =>
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
            if matches!(key, Key::ArrowRight | Key::ArrowLeft | Key::Home | Key::End)
                && !cfg!(target_os = "ios") =>
        {
            Some(Modification::Select {
                region: Region::ToOffset {
                    offset: if matches!(key, Key::Home | Key::End) {
                        if modifiers.command {
                            Offset::To(Bound::Doc)
                        } else {
                            Offset::To(Bound::Line)
                        }
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
            Some(Modification::Delete {
                region: Region::SelectionOrOffset {
                    offset: Offset::from(modifiers),
                    backwards: key == &Key::Backspace,
                },
            })
        }
        Event::Key { key: Key::Enter, pressed: true, modifiers, .. }
            if !cfg!(target_os = "ios") =>
        {
            Some(Modification::Newline { advance_cursor: !modifiers.shift })
        }
        Event::Key { key: Key::Tab, pressed: true, modifiers, .. } if !modifiers.alt => {
            if !modifiers.shift && cfg!(target_os = "ios") {
                return None;
            }

            Some(Modification::Indent { deindent: modifiers.shift })
        }
        Event::Key { key: Key::A, pressed: true, modifiers, .. }
            if modifiers.command && !cfg!(target_os = "ios") =>
        {
            Some(Modification::Select {
                region: Region::Bound { bound: Bound::Doc, backwards: true },
            })
        }
        Event::Cut => Some(Modification::Cut),
        Event::Key { key: Key::X, pressed: true, modifiers, .. }
            if modifiers.command && !modifiers.shift && !cfg!(target_os = "ios") =>
        {
            Some(Modification::Cut)
        }
        Event::Copy => Some(Modification::Copy),
        Event::Key { key: Key::C, pressed: true, modifiers, .. }
            if modifiers.command && !modifiers.shift && !cfg!(target_os = "ios") =>
        {
            Some(Modification::Copy)
        }
        Event::Key { key: Key::Z, pressed: true, modifiers, .. }
            if modifiers.command && !cfg!(target_os = "ios") =>
        {
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
            if !modifiers.alt {
                Some(Modification::ToggleStyle {
                    region: Region::Selection,
                    style: MarkdownNode::Inline(InlineNode::Code),
                })
            } else {
                Some(Modification::toggle_block_style(BlockNode::Code))
            }
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
            Some(Modification::toggle_block_style(BlockNode::ListItem(ListItem::Numbered(1), 0)))
        }
        Event::Key { key: Key::Num8, pressed: true, modifiers, .. }
            if modifiers.command && modifiers.shift =>
        {
            Some(Modification::toggle_block_style(BlockNode::ListItem(ListItem::Bulleted, 0)))
        }
        Event::Key { key: Key::Num9, pressed: true, modifiers, .. }
            if modifiers.command && modifiers.shift =>
        {
            Some(Modification::toggle_block_style(BlockNode::ListItem(ListItem::Todo(false), 0)))
        }
        Event::Key { key: Key::Num1, pressed: true, modifiers, .. }
            if modifiers.command && modifiers.alt =>
        {
            Some(Modification::toggle_block_style(BlockNode::Heading(HeadingLevel::H1)))
        }
        Event::Key { key: Key::Num2, pressed: true, modifiers, .. }
            if modifiers.command && modifiers.alt =>
        {
            Some(Modification::toggle_block_style(BlockNode::Heading(HeadingLevel::H2)))
        }
        Event::Key { key: Key::Num3, pressed: true, modifiers, .. }
            if modifiers.command && modifiers.alt =>
        {
            Some(Modification::toggle_block_style(BlockNode::Heading(HeadingLevel::H3)))
        }
        Event::Key { key: Key::Num4, pressed: true, modifiers, .. }
            if modifiers.command && modifiers.alt =>
        {
            Some(Modification::toggle_block_style(BlockNode::Heading(HeadingLevel::H4)))
        }
        Event::Key { key: Key::Num5, pressed: true, modifiers, .. }
            if modifiers.command && modifiers.alt =>
        {
            Some(Modification::toggle_block_style(BlockNode::Heading(HeadingLevel::H5)))
        }
        Event::Key { key: Key::Num6, pressed: true, modifiers, .. }
            if modifiers.command && modifiers.alt =>
        {
            Some(Modification::toggle_block_style(BlockNode::Heading(HeadingLevel::H6)))
        }
        Event::Key { key: Key::Q, pressed: true, modifiers, .. }
            if modifiers.command && modifiers.alt =>
        {
            Some(Modification::toggle_block_style(BlockNode::Quote))
        }
        Event::Key { key: Key::R, pressed: true, modifiers, .. }
            if modifiers.command && modifiers.alt =>
        {
            Some(Modification::toggle_block_style(BlockNode::Rule))
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
                if (touch_mode && !click_dragged) || click_mods.command {
                    Some(Modification::OpenUrl(url))
                } else {
                    None
                }
            } else {
                None
            }
            .or_else(|| {
                if click_checker.ui(*pos) && !cfg!(target_os = "ios") {
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
                                    bound: Bound::Paragraph,
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
        Event::Key { key: Key::Equals, pressed: true, modifiers, .. } if modifiers.command => {
            Some(Modification::SetBaseFontSize(appearance.font_size() + 1.0))
        }
        Event::Key { key: Key::Minus, pressed: true, modifiers, .. } if modifiers.command => {
            Some(Modification::SetBaseFontSize(appearance.font_size() - 1.0))
        }
        _ => None,
    }
}

impl Modification {
    pub fn toggle_block_style(block: BlockNode) -> Self {
        Modification::ToggleStyle {
            region: Region::Bound { bound: Bound::Paragraph, backwards: false },
            style: MarkdownNode::Block(block),
        }
    }

    pub fn toggle_heading_style(level: usize) -> Self {
        Self::toggle_block_style(BlockNode::Heading(
            HeadingLevel::try_from(level).unwrap_or(HeadingLevel::H1),
        ))
    }
}

#[cfg(test)]
mod test {
    use super::calc;
    use crate::tab::markdown_editor::input::canonical::{
        Bound, Increment, Modification, Offset, Region,
    };
    use crate::tab::markdown_editor::input::click_checker::ClickChecker;
    use crate::tab::markdown_editor::offset_types::DocCharOffset;
    use egui::{Event, Key, Modifiers, Pos2};
    use std::time::Instant;

    #[derive(Default)]
    struct TestClickChecker {
        ui: bool,
        text: Option<usize>,
        checkbox: Option<usize>,
        link: Option<String>,
        offset: DocCharOffset,
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

        fn pos_to_char_offset(&self, _pos: Pos2) -> DocCharOffset {
            self.offset
        }
    }

    #[test]
    fn calc_down() {
        assert!(matches!(
            calc(
                &Event::Key {
                    physical_key: None,
                    key: Key::ArrowDown,
                    pressed: true,
                    repeat: false,
                    modifiers: Default::default()
                },
                TestClickChecker::default(),
                &mut Default::default(),
                Instant::now(),
                false,
                &Default::default()
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
                    physical_key: None,
                    key: Key::ArrowDown,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers { mac_cmd: true, ..Default::default() },
                },
                TestClickChecker::default(),
                &mut Default::default(),
                Instant::now(),
                false,
                &Default::default()
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
                    physical_key: None,
                    key: Key::ArrowDown,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers { shift: true, ..Default::default() },
                },
                TestClickChecker::default(),
                &mut Default::default(),
                Instant::now(),
                false,
                &Default::default()
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
                    physical_key: None,
                    key: Key::ArrowDown,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers { mac_cmd: true, shift: true, ..Default::default() },
                },
                TestClickChecker::default(),
                &mut Default::default(),
                Instant::now(),
                false,
                &Default::default()
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
                    physical_key: None,
                    key: Key::ArrowUp,
                    pressed: true,
                    repeat: false,
                    modifiers: Default::default()
                },
                TestClickChecker::default(),
                &mut Default::default(),
                Instant::now(),
                false,
                &Default::default()
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
                    physical_key: None,
                    key: Key::ArrowUp,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers { mac_cmd: true, ..Default::default() },
                },
                TestClickChecker::default(),
                &mut Default::default(),
                Instant::now(),
                false,
                &Default::default()
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
                    physical_key: None,
                    key: Key::ArrowUp,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers { shift: true, ..Default::default() },
                },
                TestClickChecker::default(),
                &mut Default::default(),
                Instant::now(),
                false,
                &Default::default()
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
                    physical_key: None,
                    key: Key::ArrowUp,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers { mac_cmd: true, shift: true, ..Default::default() },
                },
                TestClickChecker::default(),
                &mut Default::default(),
                Instant::now(),
                false,
                &Default::default()
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
                    physical_key: None,
                    key: Key::ArrowRight,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers { ..Default::default() },
                },
                TestClickChecker::default(),
                &mut Default::default(),
                Instant::now(),
                false,
                &Default::default()
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
                    physical_key: None,
                    key: Key::ArrowRight,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers { alt: true, ..Default::default() },
                },
                TestClickChecker::default(),
                &mut Default::default(),
                Instant::now(),
                false,
                &Default::default()
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
                    physical_key: None,
                    key: Key::ArrowRight,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers { mac_cmd: true, ..Default::default() },
                },
                TestClickChecker::default(),
                &mut Default::default(),
                Instant::now(),
                false,
                &Default::default()
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
                    physical_key: None,
                    key: Key::ArrowRight,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers { shift: true, ..Default::default() },
                },
                TestClickChecker::default(),
                &mut Default::default(),
                Instant::now(),
                false,
                &Default::default()
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
                    physical_key: None,
                    key: Key::ArrowRight,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers { alt: true, shift: true, ..Default::default() },
                },
                TestClickChecker::default(),
                &mut Default::default(),
                Instant::now(),
                false,
                &Default::default()
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
                    physical_key: None,
                    key: Key::ArrowRight,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers { mac_cmd: true, shift: true, ..Default::default() },
                },
                TestClickChecker::default(),
                &mut Default::default(),
                Instant::now(),
                false,
                &Default::default()
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
                    physical_key: None,
                    key: Key::End,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers { ..Default::default() },
                },
                TestClickChecker::default(),
                &mut Default::default(),
                Instant::now(),
                false,
                &Default::default()
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
                    physical_key: None,
                    key: Key::End,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers { shift: true, ..Default::default() },
                },
                TestClickChecker::default(),
                &mut Default::default(),
                Instant::now(),
                false,
                &Default::default()
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
                    physical_key: None,
                    key: Key::ArrowLeft,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers { ..Default::default() },
                },
                TestClickChecker::default(),
                &mut Default::default(),
                Instant::now(),
                false,
                &Default::default()
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
                    physical_key: None,
                    key: Key::ArrowLeft,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers { alt: true, ..Default::default() },
                },
                TestClickChecker::default(),
                &mut Default::default(),
                Instant::now(),
                false,
                &Default::default()
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
                    physical_key: None,
                    key: Key::ArrowLeft,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers { mac_cmd: true, ..Default::default() },
                },
                TestClickChecker::default(),
                &mut Default::default(),
                Instant::now(),
                false,
                &Default::default()
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
                    physical_key: None,
                    key: Key::ArrowLeft,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers { shift: true, ..Default::default() },
                },
                TestClickChecker::default(),
                &mut Default::default(),
                Instant::now(),
                false,
                &Default::default()
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
                    physical_key: None,
                    key: Key::ArrowLeft,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers { alt: true, shift: true, ..Default::default() },
                },
                TestClickChecker::default(),
                &mut Default::default(),
                Instant::now(),
                false,
                &Default::default()
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
                    physical_key: None,
                    key: Key::ArrowLeft,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers { mac_cmd: true, shift: true, ..Default::default() },
                },
                TestClickChecker::default(),
                &mut Default::default(),
                Instant::now(),
                false,
                &Default::default()
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
                    physical_key: None,
                    key: Key::Home,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers { ..Default::default() },
                },
                TestClickChecker::default(),
                &mut Default::default(),
                Instant::now(),
                false,
                &Default::default()
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
    fn calc_cmd_home() {
        assert!(matches!(
            calc(
                &Event::Key {
                    physical_key: None,
                    key: Key::Home,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers { command: true, ..Default::default() },
                },
                TestClickChecker::default(),
                &mut Default::default(),
                Instant::now(),
                false,
                &Default::default()
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
    fn calc_shift_home() {
        assert!(matches!(
            calc(
                &Event::Key {
                    physical_key: None,
                    key: Key::Home,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers { shift: true, ..Default::default() },
                },
                TestClickChecker::default(),
                &mut Default::default(),
                Instant::now(),
                false,
                &Default::default()
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
    fn calc_cmd_shift_home() {
        assert!(matches!(
            calc(
                &Event::Key {
                    physical_key: None,
                    key: Key::Home,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers { command: true, shift: true, ..Default::default() },
                },
                TestClickChecker::default(),
                &mut Default::default(),
                Instant::now(),
                false,
                &Default::default()
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
}
