use std::time::{Duration, Instant};

use comrak::nodes::{AstNode, NodeHeading, NodeValue};
use egui::{Frame, Margin, Separator, Stroke, Ui};
use lb_rs::model::text::offset_types::RangeExt as _;
use pulldown_cmark::{HeadingLevel, LinkType};

use crate::tab::{ExtendedInput as _, ExtendedOutput as _};
use crate::theme::icons::Icon;
use crate::widgets::IconButton;

use crate::tab::markdown_editor::{self, Editor};
use markdown_editor::Event;
use markdown_editor::input::Region;
use markdown_editor::style::{BlockNode, InlineNode, ListItem, MarkdownNode};

pub const MOBILE_TOOL_BAR_SIZE: f32 = 45.0;

pub struct Toolbar {
    heading_last_click_at: Instant,
}

impl Default for Toolbar {
    fn default() -> Self {
        Self { heading_last_click_at: Instant::now() }
    }
}

impl<'ast> Editor {
    pub fn show_toolbar(&mut self, root: &'ast AstNode<'ast>, ui: &mut Ui) {
        Frame::canvas(ui.style())
            .stroke(Stroke::NONE)
            .inner_margin(Margin::symmetric(10., 10.))
            .show(ui, |ui| self.show_toolbar_inner(root, ui))
            .inner
    }

    #[allow(clippy::option_map_unit_fn)] // use of .map() reduces line wrapping, improving readability
    pub fn show_toolbar_inner(&mut self, root: &'ast AstNode<'ast>, ui: &mut Ui) {
        egui::ScrollArea::horizontal().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.visuals_mut().widgets.active.bg_fill = self.theme.fg().blue;

                let is_mobile = cfg!(target_os = "ios") || cfg!(target_os = "android");

                ui.spacing_mut().button_padding = egui::vec2(5., 5.);

                let mut events = Vec::new();

                if is_mobile && self.virtual_keyboard_shown {
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

                self.heading_button(root, ui).map(|e| events.push(e));
                ui.add_space(5.);
                self.inline(&Icon::BOLD, InlineNode::Bold, root, ui)
                    .map(|e| events.push(e));
                ui.add_space(5.);
                self.inline(&Icon::ITALIC, InlineNode::Italic, root, ui)
                    .map(|e| events.push(e));
                ui.add_space(5.);
                self.inline(&Icon::CODE, InlineNode::Code, root, ui)
                    .map(|e| events.push(e));
                ui.add_space(5.);
                self.inline(&Icon::STRIKETHROUGH, InlineNode::Strikethrough, root, ui)
                    .map(|e| events.push(e));

                add_seperator(ui);

                self.block(
                    &Icon::NUMBER_LIST,
                    BlockNode::ListItem(ListItem::Numbered(1), 0),
                    root,
                    ui,
                )
                .map(|e| events.push(e));
                ui.add_space(5.);
                self.block(
                    &Icon::BULLET_LIST,
                    BlockNode::ListItem(ListItem::Bulleted, 0),
                    root,
                    ui,
                )
                .map(|e| events.push(e));
                ui.add_space(5.);
                self.block(
                    &Icon::TODO_LIST,
                    BlockNode::ListItem(ListItem::Todo(false), 0),
                    root,
                    ui,
                )
                .map(|e| events.push(e));

                add_seperator(ui);

                self.inline(
                    &Icon::LINK,
                    InlineNode::Link(LinkType::Inline, "".into(), "".into()),
                    root,
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

    fn heading_button(&mut self, root: &'ast AstNode<'ast>, ui: &mut Ui) -> Option<Event> {
        let mut current_heading_level = 0;
        let mut applied = false;

        for node in root.descendants() {
            if let NodeValue::Heading(NodeHeading { level, .. }) = &node.data.borrow().value {
                if self
                    .node_range(node)
                    .contains_range(&self.buffer.current.selection, true, true)
                {
                    current_heading_level = *level as usize;
                    applied = true;
                    break;
                }
            }
        }

        let level = if self.toolbar.heading_last_click_at.elapsed() > Duration::from_secs(1) {
            1
        } else {
            current_heading_level.min(5) + 1
        };
        let style = MarkdownNode::Block(BlockNode::Heading(
            HeadingLevel::try_from(level).unwrap_or(HeadingLevel::H1),
        ));

        let resp = IconButton::new(&Icon::HEADER_1)
            .colored(applied)
            .tooltip(format!("{style}"))
            .show(ui);
        if resp.clicked() {
            self.toolbar.heading_last_click_at = Instant::now();
            Some(Event::ToggleStyle { region: Region::Selection, style })
        } else {
            None
        }
    }

    fn inline(
        &self, icon: &'static Icon, style: InlineNode, root: &'ast AstNode<'ast>, ui: &mut Ui,
    ) -> Option<Event> {
        self.button(icon, MarkdownNode::Inline(style), root, ui)
    }

    fn block(
        &self, icon: &'static Icon, style: BlockNode, root: &'ast AstNode<'ast>, ui: &mut Ui,
    ) -> Option<Event> {
        self.button(icon, MarkdownNode::Block(style), root, ui)
    }

    fn button(
        &self, icon: &'static Icon, style: MarkdownNode, root: &'ast AstNode<'ast>, ui: &mut Ui,
    ) -> Option<Event> {
        let applied = self.styled(root, self.buffer.current.selection, &style);
        let resp = IconButton::new(icon)
            .colored(applied)
            .tooltip(format!("{style}"))
            .show(ui);
        if resp.clicked() {
            Some(Event::ToggleStyle { region: Region::Selection, style })
        } else {
            None
        }
    }
}

fn add_seperator(ui: &mut Ui) {
    ui.add(
        Separator::default()
            .shrink(ui.available_height() * 0.3)
            .spacing(20.),
    );
}
