use crate::tab::markdown_editor::{self, Editor};
use egui::{self, Key, Modifiers};
use lb_rs::model::text::offset_types::RangeExt as _;
use markdown_editor::input::{Bound, Event, Increment, Offset, Region};
use markdown_editor::style::{BlockNode, InlineNode, ListItem, MarkdownNode};
use pulldown_cmark::{HeadingLevel, LinkType};

impl From<Modifiers> for Offset {
    fn from(modifiers: Modifiers) -> Self {
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

const PAGE_LINES: usize = 50;

impl Editor {
    pub fn translate_egui_keyboard_event(&self, event: egui::Event) -> Option<Event> {
        match event {
            egui::Event::Key { key, pressed: true, modifiers, .. }
                if matches!(key, Key::ArrowUp | Key::ArrowDown | Key::PageUp | Key::PageDown) =>
            {
                let lines = if matches!(key, Key::PageUp | Key::PageDown) { PAGE_LINES } else { 1 };
                Some(Event::Select {
                    region: Region::ToOffset {
                        offset: if modifiers.mac_cmd {
                            Offset::To(Bound::Doc)
                        } else {
                            Offset::By(Increment::Lines(lines))
                        },
                        backwards: matches!(key, Key::ArrowUp | Key::PageUp),
                        extend_selection: modifiers.shift,
                    },
                })
            }
            egui::Event::Key { key, pressed: true, modifiers, .. }
                if matches!(key, Key::ArrowRight | Key::ArrowLeft | Key::Home | Key::End) =>
            {
                Some(Event::Select {
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
            egui::Event::Paste(text) => {
                // with text selected, pasting a link turns selected text into a
                // markdown link
                let mut link_paste = false;
                if !self.buffer.current.selection.is_empty() {
                    // use comrak's auto-link detector
                    let arena = comrak::Arena::new();
                    let mut options = comrak::Options::default();
                    options.extension.autolink = true;
                    let text_with_newline = text.to_string() + "\n"; // todo: probably not okay but this parser quirky af sometimes
                    let root = comrak::parse_document(&arena, &text_with_newline, &options);
                    for node in root.descendants() {
                        let value = &node.data.borrow().value;
                        if let comrak::nodes::NodeValue::Link(node_link) = value {
                            if node_link.url == text {
                                link_paste = true;
                                break;
                            }
                        }
                    }
                }
                if link_paste {
                    Some(Event::Link { region: Region::Selection, url: text.clone() })
                } else {
                    Some(Event::Replace {
                        region: Region::Selection,
                        text: text.clone(),
                        advance_cursor: true,
                    })
                }
            }
            egui::Event::Text(text) => Some(Event::Replace {
                region: Region::Selection,
                text: text.clone(),
                advance_cursor: true,
            }),
            egui::Event::Key { key, pressed: true, modifiers, .. }
                if matches!(key, Key::Backspace | Key::Delete) =>
            {
                Some(Event::Delete {
                    region: Region::SelectionOrOffset {
                        offset: Offset::from(modifiers),
                        backwards: key == Key::Backspace,
                    },
                })
            }
            egui::Event::Key { key: Key::Enter, pressed: true, modifiers, .. }
                if !cfg!(target_os = "ios") =>
            {
                Some(Event::Newline { shift: modifiers.shift })
            }
            egui::Event::Key { key: Key::Tab, pressed: true, modifiers, .. } if !modifiers.alt => {
                if !modifiers.shift && cfg!(target_os = "ios") {
                    None
                } else {
                    Some(Event::Indent { deindent: modifiers.shift })
                }
            }
            egui::Event::Key { key: Key::A, pressed: true, modifiers, .. }
                if modifiers.command && !cfg!(target_os = "ios") =>
            {
                Some(Event::Select { region: Region::Bound { bound: Bound::Doc, backwards: true } })
            }
            egui::Event::Cut => Some(Event::Cut),
            egui::Event::Key { key: Key::X, pressed: true, modifiers, .. }
                if modifiers.command && !modifiers.shift && !cfg!(target_os = "ios") =>
            {
                Some(Event::Cut)
            }
            egui::Event::Copy => Some(Event::Copy),
            egui::Event::Key { key: Key::C, pressed: true, modifiers, .. }
                if modifiers.command && !modifiers.shift && !cfg!(target_os = "ios") =>
            {
                Some(Event::Copy)
            }
            egui::Event::Key { key: Key::Z, pressed: true, modifiers, .. }
                if modifiers.command && !cfg!(target_os = "ios") =>
            {
                if !modifiers.shift {
                    Some(Event::Undo)
                } else {
                    Some(Event::Redo)
                }
            }
            egui::Event::Key { key: Key::B, pressed: true, modifiers, .. } if modifiers.command => {
                Some(Event::ToggleStyle {
                    region: Region::Selection,
                    style: MarkdownNode::Inline(InlineNode::Bold),
                })
            }
            egui::Event::Key { key: Key::I, pressed: true, modifiers, .. } if modifiers.command => {
                Some(Event::ToggleStyle {
                    region: Region::Selection,
                    style: MarkdownNode::Inline(InlineNode::Italic),
                })
            }
            egui::Event::Key { key: Key::C, pressed: true, modifiers, .. }
                if modifiers.command && modifiers.shift =>
            {
                if !modifiers.alt {
                    Some(Event::ToggleStyle {
                        region: Region::Selection,
                        style: MarkdownNode::Inline(InlineNode::Code),
                    })
                } else {
                    Some(Event::toggle_block_style(BlockNode::Code("".into())))
                }
            }
            egui::Event::Key { key: Key::X, pressed: true, modifiers, .. }
                if modifiers.command && modifiers.shift =>
            {
                Some(Event::ToggleStyle {
                    region: Region::Selection,
                    style: MarkdownNode::Inline(InlineNode::Strikethrough),
                })
            }
            egui::Event::Key { key: Key::K, pressed: true, modifiers, .. } if modifiers.command => {
                Some(Event::ToggleStyle {
                    region: Region::Selection,
                    style: MarkdownNode::Inline(InlineNode::Link(
                        LinkType::Inline,
                        "".into(),
                        "".into(),
                    )),
                })
            }
            egui::Event::Key { key: Key::Num7, pressed: true, modifiers, .. }
                if modifiers.command && modifiers.shift =>
            {
                Some(Event::toggle_block_style(BlockNode::ListItem(ListItem::Numbered(1), 0)))
            }
            egui::Event::Key { key: Key::Num8, pressed: true, modifiers, .. }
                if modifiers.command && modifiers.shift =>
            {
                Some(Event::toggle_block_style(BlockNode::ListItem(ListItem::Bulleted, 0)))
            }
            egui::Event::Key { key: Key::Num9, pressed: true, modifiers, .. }
                if modifiers.command && modifiers.shift =>
            {
                Some(Event::toggle_block_style(BlockNode::ListItem(ListItem::Todo(false), 0)))
            }
            egui::Event::Key { key: Key::Num1, pressed: true, modifiers, .. }
                if modifiers.command && modifiers.alt =>
            {
                Some(Event::toggle_block_style(BlockNode::Heading(HeadingLevel::H1)))
            }
            egui::Event::Key { key: Key::Num2, pressed: true, modifiers, .. }
                if modifiers.command && modifiers.alt =>
            {
                Some(Event::toggle_block_style(BlockNode::Heading(HeadingLevel::H2)))
            }
            egui::Event::Key { key: Key::Num3, pressed: true, modifiers, .. }
                if modifiers.command && modifiers.alt =>
            {
                Some(Event::toggle_block_style(BlockNode::Heading(HeadingLevel::H3)))
            }
            egui::Event::Key { key: Key::Num4, pressed: true, modifiers, .. }
                if modifiers.command && modifiers.alt =>
            {
                Some(Event::toggle_block_style(BlockNode::Heading(HeadingLevel::H4)))
            }
            egui::Event::Key { key: Key::Num5, pressed: true, modifiers, .. }
                if modifiers.command && modifiers.alt =>
            {
                Some(Event::toggle_block_style(BlockNode::Heading(HeadingLevel::H5)))
            }
            egui::Event::Key { key: Key::Num6, pressed: true, modifiers, .. }
                if modifiers.command && modifiers.alt =>
            {
                Some(Event::toggle_block_style(BlockNode::Heading(HeadingLevel::H6)))
            }
            egui::Event::Key { key: Key::Q, pressed: true, modifiers, .. }
                if modifiers.command && modifiers.alt =>
            {
                Some(Event::toggle_block_style(BlockNode::Quote))
            }
            egui::Event::Key { key: Key::R, pressed: true, modifiers, .. }
                if modifiers.command && modifiers.alt =>
            {
                Some(Event::toggle_block_style(BlockNode::Rule))
            }
            egui::Event::Key { key: Key::F2, pressed: true, .. } => Some(Event::ToggleDebug),
            egui::Event::Key { key: Key::Equals, pressed: true, modifiers, .. }
                if modifiers.command =>
            {
                Some(Event::IncrementBaseFontSize)
            }
            egui::Event::Key { key: Key::Minus, pressed: true, modifiers, .. }
                if modifiers.command =>
            {
                Some(Event::DecrementBaseFontSize)
            }
            _ => None,
        }
    }
}
impl Event {
    pub fn toggle_block_style(block: BlockNode) -> Self {
        Event::ToggleStyle {
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
