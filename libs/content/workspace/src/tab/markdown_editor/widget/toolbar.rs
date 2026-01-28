use std::time::{Duration, Instant};

use comrak::nodes::{AstNode, ListType, NodeHeading, NodeList, NodeValue};
use egui::scroll_area::ScrollBarVisibility;
use egui::{Frame, Margin, ScrollArea, Separator, Stroke, Ui};
use lb_rs::model::text::offset_types::RangeExt as _;

use crate::tab::markdown_editor::widget::utils::NodeValueExt;
use crate::tab::{ExtendedInput as _, ExtendedOutput as _};
use crate::theme::icons::Icon;
use crate::widgets::IconButton;

use crate::tab::markdown_editor::{self, Editor};
use markdown_editor::Event;
use markdown_editor::input::Region;

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

                    let is_ios = cfg!(target_os = "ios");
                    let is_mobile = is_ios || cfg!(target_os = "android");

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
                    self.style(Icon::BOLD.size(ICON_SIZE), NodeValue::Strong, root, ui)
                        .map(|e| events.push(e));
                    ui.add_space(5.);
                    self.style(Icon::ITALIC.size(ICON_SIZE), NodeValue::Emph, root, ui)
                        .map(|e| events.push(e));
                    ui.add_space(5.);
                    self.style(
                        Icon::CODE.size(ICON_SIZE),
                        NodeValue::Code(Default::default()),
                        root,
                        ui,
                    )
                    .map(|e| events.push(e));
                    ui.add_space(5.);
                    self.style(
                        Icon::STRIKETHROUGH.size(ICON_SIZE),
                        NodeValue::Strikethrough,
                        root,
                        ui,
                    )
                    .map(|e| events.push(e));

                    add_seperator(ui);

                    self.style(
                        Icon::NUMBER_LIST.size(ICON_SIZE),
                        NodeValue::List(NodeList {
                            list_type: ListType::Ordered,
                            ..Default::default()
                        }),
                        root,
                        ui,
                    )
                    .map(|e| events.push(e));
                    ui.add_space(5.);
                    self.style(
                        Icon::BULLET_LIST.size(ICON_SIZE),
                        NodeValue::List(NodeList {
                            list_type: ListType::Bullet,
                            ..Default::default()
                        }),
                        root,
                        ui,
                    )
                    .map(|e| events.push(e));
                    ui.add_space(5.);
                    self.style(
                        Icon::TODO_LIST.size(ICON_SIZE),
                        NodeValue::List(NodeList {
                            list_type: ListType::Bullet,
                            is_task_list: true,
                            ..Default::default()
                        }),
                        root,
                        ui,
                    )
                    .map(|e| events.push(e));

                    add_seperator(ui);

                    self.style(
                        Icon::LINK.size(ICON_SIZE),
                        NodeValue::Link(Default::default()),
                        root,
                        ui,
                    )
                    .map(|e| events.push(e));

                    // only supported on iOS (for now)
                    if is_ios
                        && IconButton::new(Icon::CAMERA.size(ICON_SIZE))
                            .tooltip("Camera")
                            .show(ui)
                            .clicked()
                    {
                        events.push(Event::Camera);
                    }

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
                    current_heading_level = *level;
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
        let style = NodeValue::Heading(NodeHeading { level, ..Default::default() });

        let resp = IconButton::new(Icon::HEADER_1.size(ICON_SIZE))
            .colored(applied)
            .tooltip(style.name())
            .show(ui);
        if resp.clicked() {
            self.toolbar.heading_last_click_at = Instant::now();
            Some(Event::ToggleStyle { region: Region::Selection, style })
        } else {
            None
        }
    }

    fn style(
        &self, icon: Icon, style: NodeValue, root: &'ast AstNode<'ast>, ui: &mut Ui,
    ) -> Option<Event> {
        let applied = if style.is_inline() {
            self.inline_styled(root, self.buffer.current.selection, &style)
        } else {
            self.unapply_block(root, &style)
        };

        self.button(icon, style, applied, ui)
    }

    fn button(&self, icon: Icon, style: NodeValue, applied: bool, ui: &mut Ui) -> Option<Event> {
        let resp = IconButton::new(icon)
            .colored(applied)
            .tooltip(style.name())
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

trait Name {
    fn name(&self) -> &'static str;
}

impl Name for NodeValue {
    fn name(&self) -> &'static str {
        match self {
            NodeValue::Document => "",
            NodeValue::FrontMatter(_) => "",
            NodeValue::BlockQuote => "Quote",
            NodeValue::List(NodeList {
                list_type: ListType::Bullet, is_task_list: false, ..
            }) => "Bulleted List",
            NodeValue::List(NodeList { list_type: ListType::Ordered, .. }) => "Numbered List",
            NodeValue::List(NodeList {
                list_type: ListType::Bullet, is_task_list: true, ..
            }) => "Task List",
            NodeValue::Item(_) => "Item",
            NodeValue::DescriptionList => "",
            NodeValue::DescriptionItem(_) => "",
            NodeValue::DescriptionTerm => "",
            NodeValue::DescriptionDetails => "",
            NodeValue::CodeBlock(_) => "",
            NodeValue::HtmlBlock(_) => "",
            NodeValue::Paragraph => "",
            NodeValue::Heading(_) => "Heading",
            NodeValue::ThematicBreak => "",
            NodeValue::FootnoteDefinition(_) => "",
            NodeValue::Table(_) => "",
            NodeValue::TableRow(_) => "",
            NodeValue::TableCell => "",
            NodeValue::Text(_) => "",
            NodeValue::TaskItem(_) => "",
            NodeValue::SoftBreak => "",
            NodeValue::LineBreak => "",
            NodeValue::Code(_) => "Code",
            NodeValue::HtmlInline(_) => "",
            NodeValue::Raw(_) => "",
            NodeValue::Emph => "Italic",
            NodeValue::Strong => "Bold",
            NodeValue::Strikethrough => "Strikethrough",
            NodeValue::Highlight => "Highlight",
            NodeValue::Superscript => "Superscript",
            NodeValue::Link(_) => "Link",
            NodeValue::Image(_) => "Image",
            NodeValue::FootnoteReference(_) => "",
            NodeValue::ShortCode(_) => "",
            NodeValue::Math(_) => "",
            NodeValue::MultilineBlockQuote(_) => "",
            NodeValue::Escaped => "",
            NodeValue::WikiLink(_) => "",
            NodeValue::Underline => "Underline",
            NodeValue::Subscript => "Subscript",
            NodeValue::SpoileredText => "SpoileredText",
            NodeValue::EscapedTag(_) => "",
            NodeValue::Alert(_) => "",
            NodeValue::Subtext => "",
        }
    }
}
