use std::time::{Duration, Instant};

use egui::{Frame, Margin, Separator, Stroke, Ui};
use lb_rs::model::text::offset_types::DocCharOffset;
use pulldown_cmark::{HeadingLevel, LinkType};

use crate::tab::{ExtendedInput as _, ExtendedOutput as _};
use crate::theme::icons::Icon;
use crate::widgets::IconButton;

use crate::tab::markdown_editor;
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
        &mut self, bounds: &Bounds, selection: (DocCharOffset, DocCharOffset),
        virtual_keyboard_shown: bool, ui: &mut Ui,
    ) {
        Frame::canvas(ui.style())
            .stroke(Stroke::NONE)
            .inner_margin(Margin::symmetric(10., 10.))
            .show(ui, |ui| self.show_inner(bounds, selection, virtual_keyboard_shown, ui))
            .inner
    }

    #[allow(clippy::option_map_unit_fn)] // use of .map() reduces line wrapping, improving readability
    pub fn show_inner(
        &mut self, bounds: &Bounds, selection: (DocCharOffset, DocCharOffset),
        virtual_keyboard_shown: bool, ui: &mut Ui,
    ) {
        egui::ScrollArea::horizontal().show(ui, |ui| {
            ui.horizontal(|ui| {
                let is_mobile = cfg!(target_os = "ios") || cfg!(target_os = "android");
                let s = ast.styles_at_offset(selection.1, &bounds.ast);

                ui.spacing_mut().button_padding = egui::vec2(5., 5.);

                let mut events = Vec::new();

                if is_mobile && virtual_keyboard_shown {
                    let resp = IconButton::new(&Icon::KEYBOARD_HIDE).show(ui);
                    if resp.clicked() {
                        ui.ctx().set_virtual_keyboard_shown(false);
                    }
                    add_seperator(ui);
                }

                if IconButton::new(&Icon::UNDO)
                    .tooltip("Undo")
                    .show(ui)
                    .clicked()
                {
                    events.push(Event::Undo);
                }
                ui.add_space(5.);
                if IconButton::new(&Icon::REDO)
                    .tooltip("Redo")
                    .show(ui)
                    .clicked()
                {
                    events.push(Event::Redo);
                }

                add_seperator(ui);

                self.heading_button(&s, ui).map(|e| events.push(e));
                ui.add_space(5.);
                inline(&Icon::BOLD, InlineNode::Bold, &s, ui).map(|e| events.push(e));
                ui.add_space(5.);
                inline(&Icon::ITALIC, InlineNode::Italic, &s, ui).map(|e| events.push(e));
                ui.add_space(5.);
                inline(&Icon::CODE, InlineNode::Code, &s, ui).map(|e| events.push(e));
                ui.add_space(5.);
                inline(&Icon::STRIKETHROUGH, InlineNode::Strikethrough, &s, ui)
                    .map(|e| events.push(e));

                add_seperator(ui);

                block(&Icon::NUMBER_LIST, BlockNode::ListItem(ListItem::Numbered(1), 0), &s, ui)
                    .map(|e| events.push(e));
                ui.add_space(5.);
                block(&Icon::BULLET_LIST, BlockNode::ListItem(ListItem::Bulleted, 0), &s, ui)
                    .map(|e| events.push(e));
                ui.add_space(5.);
                block(&Icon::TODO_LIST, BlockNode::ListItem(ListItem::Todo(false), 0), &s, ui)
                    .map(|e| events.push(e));

                add_seperator(ui);

                inline(
                    &Icon::LINK,
                    InlineNode::Link(LinkType::Inline, "".into(), "".into()),
                    &s,
                    ui,
                )
                .map(|e| events.push(e));

                add_seperator(ui);

                if IconButton::new(&Icon::INDENT)
                    .tooltip("Indent")
                    .show(ui)
                    .clicked()
                {
                    events.push(Event::Indent { deindent: false });
                }
                ui.add_space(5.);
                if IconButton::new(&Icon::DEINDENT)
                    .tooltip("De-indent")
                    .show(ui)
                    .clicked()
                {
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

        let level = if self.heading_last_click_at.elapsed() > Duration::from_secs(1) {
            1
        } else {
            current_heading_level.min(5) + 1
        };
        let style = MarkdownNode::Block(BlockNode::Heading(
            HeadingLevel::try_from(level).unwrap_or(HeadingLevel::H1),
        ));

        let resp = IconButton::new(&Icon::HEADER_1)
            .colored(applied)
            .tooltip(format!("{}", style))
            .show(ui);
        if resp.clicked() {
            self.heading_last_click_at = Instant::now();
            Some(Event::ToggleStyle { region: Region::Selection, style })
        } else {
            None
        }
    }
}

fn inline(
    icon: &'static Icon, style: InlineNode, styles_at_cursor: &[MarkdownNode], ui: &mut Ui,
) -> Option<Event> {
    button(icon, MarkdownNode::Inline(style), styles_at_cursor, ui)
}

fn block(
    icon: &'static Icon, style: BlockNode, styles_at_cursor: &[MarkdownNode], ui: &mut Ui,
) -> Option<Event> {
    button(icon, MarkdownNode::Block(style), styles_at_cursor, ui)
}

fn button(
    icon: &'static Icon, style: MarkdownNode, styles_at_cursor: &[MarkdownNode], ui: &mut Ui,
) -> Option<Event> {
    let applied = styles_at_cursor.iter().any(|s| s == &style);
    let resp = IconButton::new(icon)
        .colored(applied)
        .tooltip(format!("{}", style))
        .show(ui);
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
