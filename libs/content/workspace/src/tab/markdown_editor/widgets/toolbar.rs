use std::mem;
use std::time::{Duration, Instant};

use egui::{Frame, Margin, Separator, Stroke, Ui};
use lb_rs::text::offset_types::DocCharOffset;
use pulldown_cmark::LinkType;

use crate::tab::{ExtendedInput as _, ExtendedOutput as _};
use crate::theme::icons::Icon;
use crate::widgets::Button;

use crate::tab::markdown_editor;
use markdown_editor::ast::Ast;
use markdown_editor::bounds::Bounds;
use markdown_editor::input::Region;
use markdown_editor::style::{BlockNode, InlineNode, ListItem, MarkdownNode};
use markdown_editor::Event;

pub const MOBILE_TOOL_BAR_SIZE: f32 = 45.0;

pub struct Toolbar {
    heading_last_click_at: Instant,
}

impl Default for Toolbar {
    fn default() -> Self {
        Self { heading_last_click_at: Instant::now() }
    }
}

impl Toolbar {
    pub fn show(
        &mut self, ast: &Ast, bounds: &Bounds, selection: (DocCharOffset, DocCharOffset),
        virtual_keyboard_shown: bool, ui: &mut Ui,
    ) {
        Frame::canvas(ui.style())
            .stroke(Stroke::NONE)
            .inner_margin(Margin::symmetric(10., 10.))
            .show(ui, |ui| self.show_inner(ast, bounds, selection, virtual_keyboard_shown, ui))
            .inner
    }

    #[allow(clippy::option_map_unit_fn)] // use of .map() reduces line wrapping, improving readability
    pub fn show_inner(
        &mut self, ast: &Ast, bounds: &Bounds, selection: (DocCharOffset, DocCharOffset),
        virtual_keyboard_shown: bool, ui: &mut Ui,
    ) {
        egui::ScrollArea::horizontal().show(ui, |ui| {
            ui.horizontal(|ui| {
                let is_mobile = cfg!(target_os = "ios") || cfg!(target_os = "android");
                let s = ast.styles_at_offset(selection.1, &bounds.ast);

                ui.spacing_mut().button_padding = egui::vec2(10., 5.);

                let mut events = Vec::new();

                if is_mobile && virtual_keyboard_shown {
                    let resp = Button::default().icon(&Icon::KEYBOARD_HIDE).show(ui);
                    if resp.clicked() {
                        ui.ctx().set_virtual_keyboard_shown(false);
                    }
                    add_seperator(ui);
                }

                if Button::default().icon(&Icon::UNDO).show(ui).clicked() {
                    events.push(Event::Undo);
                }
                if Button::default().icon(&Icon::REDO).show(ui).clicked() {
                    events.push(Event::Redo);
                }

                add_seperator(ui);

                self.heading_button(&s, ui).map(|e| events.push(e));
                inline_btn(Icon::BOLD, InlineNode::Bold, &s, ui).map(|e| events.push(e));
                inline_btn(Icon::ITALIC, InlineNode::Italic, &s, ui).map(|e| events.push(e));
                inline_btn(Icon::CODE, InlineNode::Code, &s, ui).map(|e| events.push(e));
                inline_btn(Icon::STRIKETHROUGH, InlineNode::Strikethrough, &s, ui)
                    .map(|e| events.push(e));

                add_seperator(ui);

                block_btn(Icon::NUMBER_LIST, BlockNode::ListItem(ListItem::Numbered(1), 0), &s, ui)
                    .map(|e| events.push(e));
                block_btn(Icon::BULLET_LIST, BlockNode::ListItem(ListItem::Bulleted, 0), &s, ui)
                    .map(|e| events.push(e));
                block_btn(Icon::TODO_LIST, BlockNode::ListItem(ListItem::Todo(false), 0), &s, ui)
                    .map(|e| events.push(e));

                add_seperator(ui);

                inline_btn(
                    Icon::LINK,
                    InlineNode::Link(LinkType::Inline, "".into(), "".into()),
                    &s,
                    ui,
                )
                .map(|e| events.push(e));

                add_seperator(ui);

                if Button::default().icon(&Icon::INDENT).show(ui).clicked() {
                    events.push(Event::Indent { deindent: false });
                }
                if Button::default().icon(&Icon::DEINDENT).show(ui).clicked() {
                    events.push(Event::Indent { deindent: true });
                }

                ui.add_space(ui.available_width());

                for event in events {
                    ui.ctx().push_markdown_event(event);
                    if is_mobile {
                        // mobile toolbar painted after editor events processed
                        ui.ctx().request_repaint();
                    }
                }
            })
        });
    }

    fn heading_button(&mut self, styles_at_cursor: &[MarkdownNode], ui: &mut Ui) -> Option<Event> {
        let mut current_heading_level = 0;
        let mut applied = false;
        for style in styles_at_cursor.iter() {
            if let MarkdownNode::Block(BlockNode::Heading(level)) = style {
                current_heading_level = *level as _;
                applied = true;
                break;
            }
        }
        let mut icon = Icon::HEADER_1;
        if applied {
            icon = icon.color(ui.visuals().widgets.active.bg_fill)
        }

        let resp = Button::default().icon(&icon).show(ui);
        if resp.clicked() {
            if mem::replace(&mut self.heading_last_click_at, Instant::now()).elapsed()
                > Duration::from_secs(1)
            {
                return Some(Event::toggle_heading_style(1));
            } else {
                current_heading_level = current_heading_level.min(5) + 1;
                return Some(Event::toggle_heading_style(current_heading_level));
            }
        }

        None
    }
}

fn inline_btn(
    icon: Icon, style: InlineNode, styles_at_cursor: &[MarkdownNode], ui: &mut Ui,
) -> Option<Event> {
    button(icon, MarkdownNode::Inline(style), styles_at_cursor, ui)
}

fn block_btn(
    icon: Icon, style: BlockNode, styles_at_cursor: &[MarkdownNode], ui: &mut Ui,
) -> Option<Event> {
    button(icon, MarkdownNode::Block(style), styles_at_cursor, ui)
}

fn button(
    mut icon: Icon, style: MarkdownNode, styles_at_cursor: &[MarkdownNode], ui: &mut Ui,
) -> Option<Event> {
    let applied = styles_at_cursor.iter().any(|s| s == &style);
    if applied {
        icon = icon.color(ui.visuals().widgets.active.bg_fill)
    }
    let resp = Button::default().icon(&icon).show(ui);
    if resp.clicked() {
        Some(Event::ToggleStyle { region: Region::Selection, style })
    } else {
        None
    }
}

fn add_seperator(ui: &mut Ui) {
    ui.add(
        Separator::default()
            .shrink(ui.available_height() * 0.3)
            .spacing(20.),
    );
}
