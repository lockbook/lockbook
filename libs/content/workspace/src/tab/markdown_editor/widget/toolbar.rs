use std::time::{Duration, Instant};

use comrak::nodes::{AstNode, NodeHeading, NodeValue};
use egui::scroll_area::ScrollBarVisibility;
use egui::{Frame, Margin, ScrollArea, Separator, Stroke, Ui};
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
pub const ICON_SIZE: f32 = 16.0;

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
        ScrollArea::horizontal()
            .scroll_bar_visibility(ScrollBarVisibility::AlwaysHidden)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.visuals_mut().widgets.active.bg_fill = self.theme.fg().blue;

                    let is_mobile = cfg!(target_os = "ios") || cfg!(target_os = "android");

                    ui.spacing_mut().button_padding = egui::vec2(5., 5.);

                    let mut events = Vec::new();

                    if is_mobile && self.virtual_keyboard_shown {
                        let resp = IconButton::new(Icon::KEYBOARD_HIDE.size(16.0)).show(ui);
                        if resp.clicked() {
                            ui.ctx().set_virtual_keyboard_shown(false);
                        }
                        add_seperator(ui);
                    }

                    if IconButton::new(Icon::UNDO.size(16.0))
                        .tooltip("Undo")
                        .show(ui)
                        .clicked()
                    {
                        events.push(Event::Undo);
                    }
                    ui.add_space(5.);
                    if IconButton::new(Icon::REDO.size(16.0))
                        .tooltip("Redo")
                        .show(ui)
                        .clicked()
                    {
                        events.push(Event::Redo);
                    }

                    add_seperator(ui);

                    self.heading_button(root, ui).map(|e| events.push(e));
                    ui.add_space(5.);
                    self.inline(Icon::BOLD.size(ICON_SIZE), InlineNode::Bold, root, ui)
                        .map(|e| events.push(e));
                    ui.add_space(5.);
                    self.inline(Icon::ITALIC.size(ICON_SIZE), InlineNode::Italic, root, ui)
                        .map(|e| events.push(e));
                    ui.add_space(5.);
                    self.inline(Icon::CODE.size(ICON_SIZE), InlineNode::Code, root, ui)
                        .map(|e| events.push(e));
                    ui.add_space(5.);
                    self.inline(Icon::STRIKETHROUGH.size(ICON_SIZE), InlineNode::Strikethrough, root, ui)
                        .map(|e| events.push(e));

                    add_seperator(ui);

                    self.block(
                        Icon::NUMBER_LIST.size(ICON_SIZE),
                        BlockNode::ListItem(ListItem::Numbered(1), 0),
                        root,
                        ui,
                    )
                    .map(|e| events.push(e));
                    ui.add_space(5.);
                    self.block(
                        Icon::BULLET_LIST.size(ICON_SIZE),
                        BlockNode::ListItem(ListItem::Bulleted, 0),
                        root,
                        ui,
                    )
                    .map(|e| events.push(e));
                    ui.add_space(5.);
                    self.block(
                        Icon::TODO_LIST.size(ICON_SIZE),
                        BlockNode::ListItem(ListItem::Todo(false), 0),
                        root,
                        ui,
                    )
                    .map(|e| events.push(e));

                    add_seperator(ui);

                    self.inline(
                        Icon::LINK.size(ICON_SIZE),
                        InlineNode::Link(LinkType::Inline, "".into(), "".into()),
                        root,
                        ui,
                    )
                    .map(|e| events.push(e));

                    add_seperator(ui);

                    if IconButton::new(Icon::INDENT.size(ICON_SIZE))
                        .tooltip("Indent")
                        .show(ui)
                        .clicked()
                    {
                        events.push(Event::Indent { deindent: false });
                    }
                    ui.add_space(5.);
                    if IconButton::new(Icon::DEINDENT.size(ICON_SIZE))
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

        let resp = IconButton::new(Icon::HEADER_1.size(ICON_SIZE))
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
        &self, icon: Icon, style: InlineNode, root: &'ast AstNode<'ast>, ui: &mut Ui,
    ) -> Option<Event> {
        let applied = self.inline_styled(root, self.buffer.current.selection, &style);
        self.button(icon, MarkdownNode::Inline(style), applied, ui)
    }

    fn block(
        &self, icon: Icon, style: BlockNode, root: &'ast AstNode<'ast>, ui: &mut Ui,
    ) -> Option<Event> {
        let applied = self.unapply_block(root, &style);
        self.button(icon, MarkdownNode::Block(style), applied, ui)
    }

    fn button(
        &self, icon: Icon, style: MarkdownNode, applied: bool, ui: &mut Ui,
    ) -> Option<Event> {
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
