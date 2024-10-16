use std::time::Instant;

use egui::{Frame, Margin, Separator, Stroke, Ui};
use pulldown_cmark::{HeadingLevel, LinkType};

use crate::tab::{ExtendedInput as _, ExtendedOutput as _};
use crate::theme::icons::Icon;
use crate::widgets::Button;

use crate::tab::markdown_editor;
use markdown_editor::input::Region;
use markdown_editor::style::{BlockNode, InlineNode, ListItem, MarkdownNode};
use markdown_editor::Event;

pub const MOBILE_TOOL_BAR_SIZE: f32 = 45.0;

pub struct Toolbar {
    header_click_count: usize,
    header_last_click_at: Instant,
}

impl Default for Toolbar {
    fn default() -> Self {
        Self { header_click_count: 0, header_last_click_at: Instant::now() }
    }
}

impl Toolbar {
    pub fn show(&mut self, virtual_keyboard_shown: bool, ui: &mut Ui) {
        Frame::canvas(ui.style())
            .stroke(Stroke::NONE)
            .inner_margin(Margin::symmetric(10., 10.))
            .show(ui, |ui| self.show_inner(virtual_keyboard_shown, ui))
            .inner
    }

    pub fn show_inner(&mut self, virtual_keyboard_shown: bool, ui: &mut Ui) {
        let is_mobile = cfg!(target_os = "ios") || cfg!(target_os = "android");

        egui::ScrollArea::horizontal().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().button_padding = egui::vec2(10., 5.);

                // hide virtual keyboard
                if is_mobile && virtual_keyboard_shown {
                    let resp = Button::default().icon(&Icon::KEYBOARD_HIDE).show(ui);
                    if resp.clicked() {
                        ui.ctx().set_virtual_keyboard_shown(false);
                    }

                    ui.add(
                        Separator::default()
                            .shrink(ui.available_height() * 0.3)
                            .spacing(20.),
                    );
                }

                // undo
                let resp = Button::default().icon(&Icon::UNDO).show(ui);
                if resp.clicked() {
                    ui.ctx().push_markdown_event(Event::Undo);
                    if is_mobile {
                        ui.ctx().request_repaint();
                    }
                }

                // redo
                let resp = Button::default().icon(&Icon::REDO).show(ui);
                if resp.clicked() {
                    ui.ctx().push_markdown_event(Event::Redo);
                    if is_mobile {
                        ui.ctx().request_repaint();
                    }
                }

                // separator
                ui.add(
                    Separator::default()
                        .shrink(ui.available_height() * 0.3)
                        .spacing(20.),
                );

                // header
                let resp = Button::default().icon(&Icon::HEADER_1).show(ui);
                if resp.clicked() {
                    ui.ctx().push_markdown_event(Event::ToggleStyle {
                        region: Region::Selection,
                        style: MarkdownNode::Block(BlockNode::Heading(HeadingLevel::H1)),
                    });
                    if is_mobile {
                        ui.ctx().request_repaint();
                    }
                }

                // bold
                let resp = Button::default().icon(&Icon::BOLD).show(ui);
                if resp.clicked() {
                    ui.ctx().push_markdown_event(Event::ToggleStyle {
                        region: Region::Selection,
                        style: MarkdownNode::Inline(InlineNode::Bold),
                    });
                    if is_mobile {
                        ui.ctx().request_repaint();
                    }
                }

                // italic
                let resp = Button::default().icon(&Icon::ITALIC).show(ui);
                if resp.clicked() {
                    ui.ctx().push_markdown_event(Event::ToggleStyle {
                        region: Region::Selection,
                        style: MarkdownNode::Inline(InlineNode::Italic),
                    });
                    if is_mobile {
                        ui.ctx().request_repaint();
                    }
                }

                // code
                let resp = Button::default().icon(&Icon::CODE).show(ui);
                if resp.clicked() {
                    ui.ctx().push_markdown_event(Event::ToggleStyle {
                        region: Region::Selection,
                        style: MarkdownNode::Inline(InlineNode::Code),
                    });
                    if is_mobile {
                        ui.ctx().request_repaint();
                    }
                }

                // strikethrough
                let resp = Button::default().icon(&Icon::STRIKETHROUGH).show(ui);
                if resp.clicked() {
                    ui.ctx().push_markdown_event(Event::ToggleStyle {
                        region: Region::Selection,
                        style: MarkdownNode::Inline(InlineNode::Strikethrough),
                    });
                    if is_mobile {
                        ui.ctx().request_repaint();
                    }
                }

                // separator
                ui.add(
                    Separator::default()
                        .shrink(ui.available_height() * 0.3)
                        .spacing(20.),
                );

                // numbered list
                let resp = Button::default().icon(&Icon::NUMBER_LIST).show(ui);
                if resp.clicked() {
                    ui.ctx()
                        .push_markdown_event(Event::toggle_block_style(BlockNode::ListItem(
                            ListItem::Numbered(1),
                            0,
                        )));
                    if is_mobile {
                        ui.ctx().request_repaint();
                    }
                }

                // bulleted list
                let resp = Button::default().icon(&Icon::BULLET_LIST).show(ui);
                if resp.clicked() {
                    ui.ctx()
                        .push_markdown_event(Event::toggle_block_style(BlockNode::ListItem(
                            ListItem::Bulleted,
                            0,
                        )));
                    if is_mobile {
                        ui.ctx().request_repaint();
                    }
                }

                // todo list
                let resp = Button::default().icon(&Icon::TODO_LIST).show(ui);
                if resp.clicked() {
                    ui.ctx()
                        .push_markdown_event(Event::toggle_block_style(BlockNode::ListItem(
                            ListItem::Todo(false),
                            0,
                        )));
                    if is_mobile {
                        ui.ctx().request_repaint();
                    }
                }

                // separator
                ui.add(
                    Separator::default()
                        .shrink(ui.available_height() * 0.3)
                        .spacing(20.),
                );

                // link
                let resp = Button::default().icon(&Icon::LINK).show(ui);
                if resp.clicked() {
                    ui.ctx().push_markdown_event(Event::ToggleStyle {
                        region: Region::Selection,
                        style: MarkdownNode::Inline(InlineNode::Link(
                            LinkType::Inline,
                            "".into(),
                            "".into(),
                        )),
                    });
                    if is_mobile {
                        ui.ctx().request_repaint();
                    }
                }

                // separator
                ui.add(
                    Separator::default()
                        .shrink(ui.available_height() * 0.3)
                        .spacing(20.),
                );

                // indent
                let resp = Button::default().icon(&Icon::INDENT).show(ui);
                if resp.clicked() {
                    ui.ctx()
                        .push_markdown_event(Event::Indent { deindent: false });
                    if is_mobile {
                        ui.ctx().request_repaint();
                    }
                }

                // deindent
                let resp = Button::default().icon(&Icon::DEINDENT).show(ui);
                if resp.clicked() {
                    ui.ctx()
                        .push_markdown_event(Event::Indent { deindent: true });
                    if is_mobile {
                        ui.ctx().request_repaint();
                    }
                }

                // fill remaining space
                ui.add_space(ui.available_width());
            })
        });
    }
}
