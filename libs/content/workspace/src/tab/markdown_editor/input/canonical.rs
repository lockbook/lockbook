use crate::tab::markdown_editor::{self, Editor};
use comrak::nodes::{ListType, NodeHeading, NodeLink, NodeList, NodeValue};
use egui::{self, Key, Modifiers};
use lb_rs::model::text::offset_types::RangeExt as _;
use markdown_editor::input::{Bound, Event, Increment, Offset, Region};

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
                if self.readonly {
                    return None;
                }

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
                    Some(Event::ToggleStyle {
                        region: Region::Selection,
                        style: NodeValue::Link(
                            NodeLink { url: text.clone(), ..Default::default() }.into(),
                        ),
                    })
                } else {
                    Some(Event::Replace {
                        region: Region::Selection,
                        text: text.clone(),
                        advance_cursor: true,
                    })
                }
            }
            egui::Event::Text(text) => {
                if self.readonly {
                    return None;
                }
                Some(Event::Replace {
                    region: Region::Selection,
                    text: text.clone(),
                    advance_cursor: true,
                })
            }
            egui::Event::Key { key, pressed: true, modifiers, .. }
                if matches!(key, Key::Backspace | Key::Delete) =>
            {
                if self.readonly {
                    return None;
                }
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
                if self.readonly {
                    return None;
                }
                Some(Event::Newline { shift: modifiers.shift })
            }
            egui::Event::Key { key: Key::Tab, pressed: true, modifiers, .. } if !modifiers.alt => {
                if self.readonly {
                    return None;
                }
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
                if self.readonly {
                    return None;
                }
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
                if self.readonly {
                    return None;
                }
                if !modifiers.shift { Some(Event::Undo) } else { Some(Event::Redo) }
            }
            egui::Event::Key { key: Key::B, pressed: true, modifiers, .. } if modifiers.command => {
                if self.readonly {
                    return None;
                }
                Some(Event::ToggleStyle { region: Region::Selection, style: NodeValue::Strong })
            }
            egui::Event::Key { key: Key::I, pressed: true, modifiers, .. } if modifiers.command => {
                if self.readonly {
                    return None;
                }
                Some(Event::ToggleStyle { region: Region::Selection, style: NodeValue::Emph })
            }
            egui::Event::Key { key: Key::C, pressed: true, modifiers, .. }
                if modifiers.command && modifiers.shift =>
            {
                if self.readonly {
                    return None;
                }
                if !modifiers.alt {
                    Some(Event::ToggleStyle {
                        region: Region::Selection,
                        style: NodeValue::Code(Default::default()),
                    })
                } else {
                    Some({
                        Event::ToggleStyle {
                            region: Region::Bound { bound: Bound::Paragraph, backwards: false },
                            style: NodeValue::CodeBlock(Default::default()),
                        }
                    })
                }
            }
            egui::Event::Key { key: Key::X, pressed: true, modifiers, .. }
                if modifiers.command && modifiers.shift =>
            {
                if self.readonly {
                    return None;
                }
                Some(Event::ToggleStyle {
                    region: Region::Selection,
                    style: NodeValue::Strikethrough,
                })
            }
            egui::Event::Key { key: Key::H, pressed: true, modifiers, .. }
                if modifiers.command && modifiers.shift =>
            {
                if self.readonly {
                    return None;
                }
                Some(Event::ToggleStyle { region: Region::Selection, style: NodeValue::Highlight })
            }
            egui::Event::Key { key: Key::U, pressed: true, modifiers, .. } if modifiers.command => {
                if self.readonly {
                    return None;
                }
                Some(Event::ToggleStyle { region: Region::Selection, style: NodeValue::Underline })
            }
            egui::Event::Key { key: Key::P, pressed: true, modifiers, .. }
                if modifiers.command && modifiers.shift =>
            {
                if self.readonly {
                    return None;
                }
                Some(Event::ToggleStyle {
                    region: Region::Selection,
                    style: NodeValue::SpoileredText,
                })
            }
            egui::Event::Key { key: Key::S, pressed: true, modifiers, .. }
                if modifiers.command && modifiers.shift =>
            {
                if self.readonly {
                    return None;
                }
                Some(Event::ToggleStyle { region: Region::Selection, style: NodeValue::Subscript })
            }
            egui::Event::Key { key: Key::E, pressed: true, modifiers, .. }
                if modifiers.command && modifiers.shift =>
            {
                if self.readonly {
                    return None;
                }
                Some(Event::ToggleStyle {
                    region: Region::Selection,
                    style: NodeValue::Superscript,
                })
            }
            egui::Event::Key { key: Key::K, pressed: true, modifiers, .. } if modifiers.command => {
                if self.readonly {
                    return None;
                }
                Some(Event::ToggleStyle {
                    region: Region::Selection,
                    style: NodeValue::Link(Default::default()),
                })
            }
            egui::Event::Key { key: Key::Num7, pressed: true, modifiers, .. }
                if modifiers.command && modifiers.shift =>
            {
                if self.readonly {
                    return None;
                }
                Some({
                    Event::ToggleStyle {
                        region: Region::Bound { bound: Bound::Paragraph, backwards: false },
                        style: NodeValue::List(NodeList {
                            list_type: ListType::Ordered,
                            ..Default::default()
                        }),
                    }
                })
            }
            egui::Event::Key { key: Key::Num8, pressed: true, modifiers, .. }
                if modifiers.command && modifiers.shift =>
            {
                if self.readonly {
                    return None;
                }
                Some({
                    Event::ToggleStyle {
                        region: Region::Bound { bound: Bound::Paragraph, backwards: false },
                        style: NodeValue::List(NodeList {
                            list_type: ListType::Bullet,
                            ..Default::default()
                        }),
                    }
                })
            }
            egui::Event::Key { key: Key::Num9, pressed: true, modifiers, .. }
                if modifiers.command && modifiers.shift =>
            {
                if self.readonly {
                    return None;
                }
                Some({
                    Event::ToggleStyle {
                        region: Region::Bound { bound: Bound::Paragraph, backwards: false },
                        style: NodeValue::List(NodeList {
                            list_type: ListType::Bullet,
                            is_task_list: true,
                            ..Default::default()
                        }),
                    }
                })
            }
            egui::Event::Key { key: Key::Num1, pressed: true, modifiers, .. }
                if modifiers.command && modifiers.alt =>
            {
                if self.readonly {
                    return None;
                }
                Some({
                    Event::ToggleStyle {
                        region: Region::Bound { bound: Bound::Paragraph, backwards: false },
                        style: NodeValue::Heading(NodeHeading { level: 1, ..Default::default() }),
                    }
                })
            }
            egui::Event::Key { key: Key::Num2, pressed: true, modifiers, .. }
                if modifiers.command && modifiers.alt =>
            {
                if self.readonly {
                    return None;
                }
                Some({
                    Event::ToggleStyle {
                        region: Region::Bound { bound: Bound::Paragraph, backwards: false },
                        style: NodeValue::Heading(NodeHeading { level: 2, ..Default::default() }),
                    }
                })
            }
            egui::Event::Key { key: Key::Num3, pressed: true, modifiers, .. }
                if modifiers.command && modifiers.alt =>
            {
                if self.readonly {
                    return None;
                }
                Some({
                    Event::ToggleStyle {
                        region: Region::Bound { bound: Bound::Paragraph, backwards: false },
                        style: NodeValue::Heading(NodeHeading { level: 3, ..Default::default() }),
                    }
                })
            }
            egui::Event::Key { key: Key::Num4, pressed: true, modifiers, .. }
                if modifiers.command && modifiers.alt =>
            {
                if self.readonly {
                    return None;
                }
                Some({
                    Event::ToggleStyle {
                        region: Region::Bound { bound: Bound::Paragraph, backwards: false },
                        style: NodeValue::Heading(NodeHeading { level: 4, ..Default::default() }),
                    }
                })
            }
            egui::Event::Key { key: Key::Num5, pressed: true, modifiers, .. }
                if modifiers.command && modifiers.alt =>
            {
                if self.readonly {
                    return None;
                }
                Some({
                    Event::ToggleStyle {
                        region: Region::Bound { bound: Bound::Paragraph, backwards: false },
                        style: NodeValue::Heading(NodeHeading { level: 5, ..Default::default() }),
                    }
                })
            }
            egui::Event::Key { key: Key::Num6, pressed: true, modifiers, .. }
                if modifiers.command && modifiers.alt =>
            {
                if self.readonly {
                    return None;
                }
                Some({
                    Event::ToggleStyle {
                        region: Region::Bound { bound: Bound::Paragraph, backwards: false },
                        style: NodeValue::Heading(NodeHeading { level: 6, ..Default::default() }),
                    }
                })
            }
            egui::Event::Key { key: Key::Q, pressed: true, modifiers, .. }
                if modifiers.command && modifiers.alt =>
            {
                if self.readonly {
                    return None;
                }
                Some({
                    Event::ToggleStyle {
                        region: Region::Bound { bound: Bound::Paragraph, backwards: false },
                        style: NodeValue::BlockQuote,
                    }
                })
            }
            egui::Event::Key { key: Key::R, pressed: true, modifiers, .. }
                if modifiers.command && modifiers.alt =>
            {
                if self.readonly {
                    return None;
                }
                Some({
                    Event::ToggleStyle {
                        region: Region::Bound { bound: Bound::Paragraph, backwards: false },
                        style: NodeValue::ThematicBreak,
                    }
                })
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
impl Event {}
