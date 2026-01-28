use std::mem;
use std::time::{Duration, Instant};

use comrak::Arena;
use comrak::nodes::{AstNode, ListType, NodeHeading, NodeList, NodeValue};
use egui::scroll_area::ScrollBarVisibility;
use egui::{Frame, Margin, Rect, ScrollArea, Sense, Separator, Stroke, Ui, Vec2};
use lb_rs::model::text::offset_types::RangeExt as _;
use serde::{Deserialize, Serialize};

use crate::tab::markdown_editor::widget::MARGIN;
use crate::tab::markdown_editor::widget::utils::NodeValueExt;
use crate::tab::{ExtendedInput as _, ExtendedOutput as _};
use crate::theme::icons::Icon;
use crate::widgets::IconButton;

use crate::tab::markdown_editor::{self, Editor, print_ast};
use markdown_editor::Event;
use markdown_editor::input::Region;

pub const MOBILE_TOOL_BAR_SIZE: f32 = 45.0;
pub const ICON_SIZE: f32 = 16.0;

pub struct Toolbar {
    pub settings_open: bool,
    heading_last_click_at: Instant,
}

impl Default for Toolbar {
    fn default() -> Self {
        Self { settings_open: false, heading_last_click_at: Instant::now() }
    }
}

#[derive(Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ToolbarPersistence {
    undo: bool,
    redo: bool,
    heading: bool,
    strong: bool,
    emph: bool,
    code: bool,
    strikethrough: bool,
    ordered_list: bool,
    unordered_list: bool,
    task_list: bool,
    link: bool,
    image: bool,
    indent: bool,
    deindent: bool,
}

impl<'ast> Editor {
    fn toolbar_is_default(&self) -> bool {
        self.persisted.toolbar == Default::default()
    }

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

                    let mut any_undo_redo = false;
                    if self.persisted.toolbar.undo || self.toolbar_is_default() {
                        if IconButton::new(Icon::UNDO.size(16.0))
                            .tooltip("Undo")
                            .show(ui)
                            .clicked()
                        {
                            events.push(Event::Undo);
                        }

                        any_undo_redo = true;
                    }
                    if self.persisted.toolbar.redo || self.toolbar_is_default() {
                        if any_undo_redo {
                            ui.add_space(5.);
                        }

                        if IconButton::new(Icon::REDO.size(16.0))
                            .tooltip("Redo")
                            .show(ui)
                            .clicked()
                        {
                            events.push(Event::Redo);
                        }

                        any_undo_redo = true;
                    }
                    if any_undo_redo {
                        add_seperator(ui);
                    }

                    let mut any_style = false;
                    if self.persisted.toolbar.heading || self.toolbar_is_default() {
                        self.heading_button(root, ui).map(|e| events.push(e));
                        any_style = true;
                    }
                    if self.persisted.toolbar.strong || self.toolbar_is_default() {
                        if any_style {
                            ui.add_space(5.);
                        }
                        self.style(Icon::BOLD.size(ICON_SIZE), NodeValue::Strong, root, ui)
                            .map(|e| events.push(e));
                        any_style = true;
                    }
                    if self.persisted.toolbar.emph || self.toolbar_is_default() {
                        if any_style {
                            ui.add_space(5.);
                        }
                        self.style(Icon::ITALIC.size(ICON_SIZE), NodeValue::Emph, root, ui)
                            .map(|e| events.push(e));
                        any_style = true;
                    }
                    if self.persisted.toolbar.code || self.toolbar_is_default() {
                        if any_style {
                            ui.add_space(5.);
                        }
                        self.style(
                            Icon::CODE.size(ICON_SIZE),
                            NodeValue::Code(Default::default()),
                            root,
                            ui,
                        )
                        .map(|e| events.push(e));
                        any_style = true;
                    }
                    if self.persisted.toolbar.strikethrough || self.toolbar_is_default() {
                        if any_style {
                            ui.add_space(5.);
                        }
                        self.style(
                            Icon::STRIKETHROUGH.size(ICON_SIZE),
                            NodeValue::Strikethrough,
                            root,
                            ui,
                        )
                        .map(|e| events.push(e));
                        any_style = true;
                    }
                    if any_style {
                        add_seperator(ui);
                    }

                    let mut any_list = false;
                    if self.persisted.toolbar.ordered_list || self.toolbar_is_default() {
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
                        any_list = true;
                    }
                    if self.persisted.toolbar.unordered_list || self.toolbar_is_default() {
                        if any_list {
                            ui.add_space(5.);
                        }
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
                        any_list = true;
                    }
                    if self.persisted.toolbar.task_list || self.toolbar_is_default() {
                        if any_list {
                            ui.add_space(5.);
                        }
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
                        any_list = true;
                    }
                    if any_list {
                        add_seperator(ui);
                    }

                    let mut any_media = false;
                    if self.persisted.toolbar.link || self.toolbar_is_default() {
                        self.style(
                            Icon::LINK.size(ICON_SIZE),
                            NodeValue::Link(Default::default()),
                            root,
                            ui,
                        )
                        .map(|e| events.push(e));
                        any_media = true;
                    }
                    if self.persisted.toolbar.image || self.toolbar_is_default() {
                        // only supported on iOS (for now)
                        if is_ios {
                            if any_media {
                                ui.add_space(5.);
                            }
                            if IconButton::new(Icon::CAMERA.size(ICON_SIZE))
                                .tooltip("Camera")
                                .show(ui)
                                .clicked()
                            {
                                events.push(Event::Camera);
                            }
                            any_media = true;
                        }
                    }
                    if any_media {
                        add_seperator(ui);
                    }

                    let mut any_indent = false;
                    if self.persisted.toolbar.indent || self.toolbar_is_default() {
                        if IconButton::new(Icon::INDENT.size(ICON_SIZE))
                            .tooltip("Indent")
                            .show(ui)
                            .clicked()
                        {
                            events.push(Event::Indent { deindent: false });
                        }
                        any_indent = true;
                    }
                    if self.persisted.toolbar.deindent || self.toolbar_is_default() {
                        if any_indent {
                            ui.add_space(5.);
                        }
                        if IconButton::new(Icon::DEINDENT.size(ICON_SIZE))
                            .tooltip("De-indent")
                            .show(ui)
                            .clicked()
                        {
                            events.push(Event::Indent { deindent: true });
                        }
                        any_indent = true;
                    }

                    if is_mobile {
                        if any_indent {
                            add_seperator(ui);
                        }

                        if IconButton::new(Icon::SETTINGS.size(ICON_SIZE))
                            .tooltip("Toolbar Settings")
                            .show(ui)
                            .clicked()
                        {
                            self.toolbar.settings_open = !self.toolbar.settings_open;
                        }
                        ui.add_space(5.);
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

    pub fn show_toolbar_settings(&mut self, ui: &mut Ui) {
        let margin: Margin =
            if cfg!(target_os = "android") { Margin::symmetric(0.0, 60.0) } else { MARGIN.into() };
        ScrollArea::vertical()
            .drag_to_scroll(true)
            .id_source("toolbar_settings")
            .show(ui, |ui| {
                ui.vertical_centered_justified(|ui| {
                    Frame::canvas(ui.style())
                        .inner_margin(margin)
                        .stroke(Stroke::NONE)
                        .fill(self.theme.bg().neutral_primary)
                        .show(ui, |ui| {
                            // store values
                            let buffer = mem::replace(&mut self.buffer, "==highlight==\n".into());
                            let paragraphs = mem::take(&mut self.bounds.paragraphs);
                            let inline_paragraphs = mem::take(&mut self.bounds.inline_paragraphs);
                            let source_lines = mem::take(&mut self.bounds.source_lines);
                            self.layout_cache.clear(); // non-functional recompute

                            let galleys = mem::take(&mut self.galleys.galleys);
                            let wrap_lines = mem::take(&mut self.bounds.wrap_lines);
                            let touch_consuming_rects = mem::take(&mut self.touch_consuming_rects);

                            // parse
                            let arena = Arena::new();
                            let options = Self::comrak_options();
                            let text_with_newline = self.buffer.current.text.to_string() + "\n";
                            let root = comrak::parse_document(&arena, &text_with_newline, &options);
                            print_ast(root);

                            // pre-render work
                            self.calc_source_lines();
                            self.compute_bounds(root);
                            self.bounds.paragraphs.sort();
                            self.bounds.inline_paragraphs.sort();
                            self.calc_words();

                            let scroll_view_height = ui.max_rect().height();
                            ui.allocate_space(Vec2 { x: ui.available_width(), y: 0. });
                            let padding = (ui.available_width() - self.width) / 2.;

                            let top_left = ui.max_rect().min + Vec2::new(padding, 0.);
                            let height = {
                                let document_height = self.height(root);
                                let unfilled_space = if document_height < scroll_view_height {
                                    scroll_view_height - document_height
                                } else {
                                    0.
                                };

                                document_height + unfilled_space
                            };
                            let rect = Rect::from_min_size(top_left, Vec2::new(self.width, height));
                            let rect = rect.expand2(Vec2::X * margin.left); // clickable margins (more forgivable to click beginning of line)

                            ui.ctx().check_for_id_clash(self.id(), rect, ""); // registers this widget so it's not forgotten by next frame
                            let focused = self.focused(ui.ctx());
                            let response = ui.interact(
                                rect,
                                self.id(),
                                Sense { click: true, drag: !self.touch_mode, focusable: true },
                            );
                            if focused && !self.focused(ui.ctx()) {
                                // interact surrenders focus if we don't have sense focusable, but also if user clicks elsewhere, even on a child
                                self.focus(ui.ctx());
                            }
                            let response_properly_clicked =
                                response.clicked() && !response.fake_primary_click;
                            if response.hovered() || response_properly_clicked {
                                ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Text);
                                // overridable by widgets
                            }

                            ui.advance_cursor_after_rect(rect);

                            ui.allocate_ui_at_rect(rect, |ui| {
                                self.show_block(ui, root, top_left);
                            });

                            // restore stored value
                            self.buffer = buffer;
                            self.bounds.paragraphs = paragraphs;
                            self.bounds.inline_paragraphs = inline_paragraphs;
                            self.bounds.source_lines = source_lines;

                            self.galleys.galleys = galleys;
                            self.bounds.wrap_lines = wrap_lines;
                            self.touch_consuming_rects = touch_consuming_rects;
                        });
                });
                self.galleys.galleys.sort_by_key(|g| g.range);
            });
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
