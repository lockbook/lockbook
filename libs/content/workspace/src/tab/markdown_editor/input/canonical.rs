use crate::file_cache::{FilesExt as _, link_id};
use crate::show::DocType;
use crate::tab::markdown_editor::widget::link_completions::{detect_destination, detect_wikilink};
use crate::tab::markdown_editor::{self, MdEdit};
use comrak::nodes::{AstNode, ListType, NodeHeading, NodeLink, NodeList, NodeValue};
use egui::{self, Key, Modifiers};
use lb_rs::model::text::offset_types::RangeExt as _;
use markdown_editor::input::{Advance, Bound, Event, Increment, Region};

impl From<Modifiers> for Advance {
    fn from(modifiers: Modifiers) -> Self {
        let should_jump_line = modifiers.mac_cmd;

        let is_apple = cfg!(target_vendor = "apple");
        let is_apple_alt = is_apple && modifiers.alt;
        let is_non_apple_ctrl = !is_apple && modifiers.ctrl;
        let should_jump_word = is_apple_alt || is_non_apple_ctrl;

        if should_jump_line {
            Advance::To(Bound::Line)
        } else if should_jump_word {
            Advance::Next(Bound::Word)
        } else {
            Advance::By(Increment::Char)
        }
    }
}

const PAGE_LINES: usize = 50;

impl MdEdit {
    /// For pasted text that is an id link (`lb://`, or a file's external URL)
    /// to a document here: the destination and wikilink title this note
    /// would link to it with, any `#fragment` kept, and the name to show.
    fn pasted_file_link(&self, text: &str) -> Option<(String, Option<String>, String)> {
        let url = text.trim();
        let id = link_id(url).filter(|_| !url.contains(char::is_whitespace))?;
        let files = self.renderer.files.read().unwrap();
        let file = files.get_by_id(id).filter(|f| f.is_document())?;
        let fragment = url.split_once('#').map_or("", |(_, fragment)| fragment);
        let with_fragment = |link: String| match fragment {
            "" => link,
            fragment => format!("{link}#{fragment}"),
        };
        Some((
            with_fragment(files.link_destination(id, self.file_id)),
            files.wikilink_title(id, self.file_id).map(with_fragment),
            DocType::from_name(&file.name)
                .display_name(&file.name)
                .to_string(),
        ))
    }
}

impl<'ast> MdEdit {
    pub fn translate_egui_keyboard_event(
        &self, event: egui::Event, root: &'ast AstNode<'ast>,
    ) -> Option<Event> {
        match event {
            egui::Event::Key { key, pressed: true, modifiers, .. }
                if matches!(key, Key::ArrowUp | Key::ArrowDown | Key::PageUp | Key::PageDown) =>
            {
                let lines = if matches!(key, Key::PageUp | Key::PageDown) { PAGE_LINES } else { 1 };
                Some(Event::Select {
                    region: Region::ToAdvance {
                        advance: if modifiers.mac_cmd {
                            Advance::To(Bound::Doc)
                        } else {
                            Advance::By(Increment::Lines(lines))
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
                    region: Region::ToAdvance {
                        advance: if matches!(key, Key::Home | Key::End) {
                            if modifiers.command {
                                Advance::To(Bound::Doc)
                            } else {
                                Advance::To(Bound::Line)
                            }
                        } else {
                            Advance::from(modifiers)
                        },
                        backwards: matches!(key, Key::ArrowLeft | Key::Home),
                        extend_selection: modifiers.shift,
                    },
                })
            }
            egui::Event::Paste(text) => {
                if self.renderer.readonly {
                    return None;
                }

                let text = text.replace('\u{a0}', " "); // parser does not interact well with non-breaking spaces

                let selection = self.renderer.buffer.current.selection;
                let around = |node: &&'ast AstNode<'ast>| {
                    let range = self.renderer.node_range(node);
                    range.intersects(&selection, false)
                        || (selection.is_empty() && range.contains(selection.0, false, false))
                };
                let inside = |kind: fn(&NodeValue) -> bool| {
                    root.descendants()
                        .any(|node| kind(&node.data().value) && around(&node))
                };
                let in_wikilink = inside(|v| matches!(v, NodeValue::WikiLink(_)))
                    || detect_wikilink(&self.renderer.buffer).is_some();
                let in_link = inside(|v| matches!(v, NodeValue::Link(_) | NodeValue::Image(_)));
                let in_code = inside(|v| matches!(v, NodeValue::Code(_) | NodeValue::CodeBlock(_)));

                // a pasted link to a file here is written the way this note
                // would link to it
                let file_link = self.pasted_file_link(&text).filter(|_| !in_code);
                if let Some((destination, wiki_title, name)) = file_link {
                    let text = if in_wikilink {
                        wiki_title.unwrap_or(text)
                    } else if in_link || detect_destination(&self.renderer.buffer).is_some() {
                        destination
                    } else if selection.is_empty() {
                        format!("[{name}]({destination})")
                    } else {
                        return Some(Event::ToggleStyle {
                            region: Region::Selection,
                            style: NodeValue::Link(
                                NodeLink { url: destination, ..Default::default() }.into(),
                            ),
                        });
                    };
                    return Some(Event::Replace {
                        region: Region::Selection,
                        text,
                        advance_cursor: true,
                    });
                }
                let in_link = in_link || in_wikilink;

                // with text selected, pasting a link turns selected text into a
                // markdown link, unless we're already in a link
                let mut link_paste = false;
                if !selection.is_empty() && !in_link {
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
                        style: NodeValue::Link(NodeLink { url: text, ..Default::default() }.into()),
                    })
                } else {
                    Some(Event::Replace { region: Region::Selection, text, advance_cursor: true })
                }
            }
            egui::Event::Text(text) => {
                if self.renderer.readonly {
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
                if self.renderer.readonly {
                    return None;
                }
                Some(Event::Delete {
                    region: Region::SelectionOrAdvance {
                        advance: Advance::from(modifiers),
                        backwards: key == Key::Backspace,
                    },
                })
            }
            egui::Event::Key { key: Key::Enter, pressed: true, modifiers, .. }
                if !cfg!(target_os = "ios") && modifiers.command =>
            {
                self.renderer
                    .open_links_in_selection(root, &self.renderer.ctx);
                None
            }
            egui::Event::Key { key: Key::Enter, pressed: true, modifiers, .. }
                if !cfg!(target_os = "ios") =>
            {
                if self.renderer.readonly {
                    return None;
                }
                Some(Event::Newline { shift: modifiers.shift })
            }
            egui::Event::Key { key: Key::Tab, pressed: true, modifiers, .. } if !modifiers.alt => {
                if self.renderer.readonly {
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
                if self.renderer.readonly {
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
                if self.renderer.readonly {
                    return None;
                }
                if !modifiers.shift { Some(Event::Undo) } else { Some(Event::Redo) }
            }
            egui::Event::Key { key: Key::B, pressed: true, modifiers, .. } if modifiers.command => {
                if self.renderer.readonly {
                    return None;
                }
                Some(Event::ToggleStyle { region: Region::Selection, style: NodeValue::Strong })
            }
            egui::Event::Key { key: Key::I, pressed: true, modifiers, .. } if modifiers.command => {
                if self.renderer.readonly {
                    return None;
                }
                Some(Event::ToggleStyle { region: Region::Selection, style: NodeValue::Emph })
            }
            egui::Event::Key { key: Key::C, pressed: true, modifiers, .. }
                if modifiers.command && modifiers.shift =>
            {
                if self.renderer.readonly {
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
                if self.renderer.readonly {
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
                if self.renderer.readonly {
                    return None;
                }
                Some(Event::ToggleStyle { region: Region::Selection, style: NodeValue::Highlight })
            }
            egui::Event::Key { key: Key::U, pressed: true, modifiers, .. } if modifiers.command => {
                if self.renderer.readonly {
                    return None;
                }
                Some(Event::ToggleStyle { region: Region::Selection, style: NodeValue::Underline })
            }
            egui::Event::Key { key: Key::P, pressed: true, modifiers, .. }
                if modifiers.command && modifiers.shift =>
            {
                if self.renderer.readonly {
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
                if self.renderer.readonly {
                    return None;
                }
                Some(Event::ToggleStyle { region: Region::Selection, style: NodeValue::Subscript })
            }
            egui::Event::Key { key: Key::E, pressed: true, modifiers, .. }
                if modifiers.command && modifiers.shift =>
            {
                if self.renderer.readonly {
                    return None;
                }
                Some(Event::ToggleStyle {
                    region: Region::Selection,
                    style: NodeValue::Superscript,
                })
            }
            egui::Event::Key { key: Key::K, pressed: true, modifiers, .. } if modifiers.command => {
                if self.renderer.readonly {
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
                if self.renderer.readonly {
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
                if self.renderer.readonly {
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
                if self.renderer.readonly {
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
                if self.renderer.readonly {
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
                if self.renderer.readonly {
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
                if self.renderer.readonly {
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
                if self.renderer.readonly {
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
                if self.renderer.readonly {
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
                if self.renderer.readonly {
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
                if self.renderer.readonly {
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
                if self.renderer.readonly {
                    return None;
                }
                Some({
                    Event::ToggleStyle {
                        region: Region::Bound { bound: Bound::Paragraph, backwards: false },
                        style: NodeValue::ThematicBreak,
                    }
                })
            }
            egui::Event::Key { key: Key::D, pressed: true, modifiers, .. }
                if modifiers.command && modifiers.shift =>
            {
                if self.renderer.readonly {
                    return None;
                }
                Some(Event::ToggleFold { node: None })
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
