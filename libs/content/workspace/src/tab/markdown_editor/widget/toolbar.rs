use std::mem;
use std::sync::Arc;
use web_time::{Duration, Instant};

use comrak::Arena;
use comrak::nodes::{AstNode, ListType, NodeHeading, NodeList, NodeValue};
use egui::scroll_area::ScrollBarVisibility;
use egui::{
    FontId, Frame, Label, Layout, Margin, Pos2, Rect, Response, RichText, ScrollArea, Separator,
    Stroke, Ui, Vec2, Widget,
};
use lb_rs::model::text::offset_types::{IntoRangeExt, RangeExt as _};
use lb_rs::model::text::operation_types::Operation;
use serde::{Deserialize, Serialize};

use crate::tab::markdown_editor::widget::utils::NodeValueExt;
use crate::tab::{ExtendedInput as _, ExtendedOutput as _};
use crate::theme::icons::Icon;
use crate::widgets::IconButton;

use crate::tab::markdown_editor::{self, Editor};
use markdown_editor::Event;
use markdown_editor::input::Region;

pub const MOBILE_TOOL_BAR_SIZE: f32 = 45.0;
pub const ICON_SIZE: f32 = 16.0;
pub const MENU_SPACE: f32 = 20.; // space used for separators between menu sections
pub const MENU_MARGIN: f32 = 20.; // space on left and right side

pub struct Toolbar {
    pub menu_open: bool,
    heading_last_click_at: Instant,
}

impl Default for Toolbar {
    fn default() -> Self {
        Self { menu_open: false, heading_last_click_at: Instant::now() }
    }
}

#[derive(Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ToolbarPersistence {
    undo: bool,
    redo: bool,
    heading: bool,
    bold: bool,
    emph: bool,
    code: bool,
    strikethrough: bool,
    highlight: bool,
    underline: bool,
    spoiler: bool,
    subscript: bool,
    superscript: bool,
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
                        if IconButton::new(Icon::KEYBOARD_HIDE.size(16.0))
                            .show(ui)
                            .clicked()
                        {
                            ui.ctx().set_virtual_keyboard_shown(false);
                        }
                        add_seperator(ui);
                    }

                    let mut any_undo_redo = false;
                    if self.persisted.toolbar.undo || self.toolbar_is_default() {
                        if IconButton::new(Icon::UNDO.size(16.0))
                            .disabled(self.toolbar.menu_open)
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
                            .disabled(self.toolbar.menu_open)
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
                    if self.persisted.toolbar.bold || self.toolbar_is_default() {
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
                    if self.persisted.toolbar.highlight || self.toolbar_is_default() {
                        if any_style {
                            ui.add_space(5.);
                        }
                        self.style(Icon::HIGHLIGHT.size(ICON_SIZE), NodeValue::Highlight, root, ui)
                            .map(|e| events.push(e));
                        any_style = true;
                    }
                    if self.persisted.toolbar.underline || self.toolbar_is_default() {
                        if any_style {
                            ui.add_space(5.);
                        }
                        self.style(Icon::UNDERLINE.size(ICON_SIZE), NodeValue::Underline, root, ui)
                            .map(|e| events.push(e));
                        any_style = true;
                    }
                    if self.persisted.toolbar.spoiler || self.toolbar_is_default() {
                        if any_style {
                            ui.add_space(5.);
                        }
                        self.style(
                            Icon::SPOILER.size(ICON_SIZE),
                            NodeValue::SpoileredText,
                            root,
                            ui,
                        )
                        .map(|e| events.push(e));
                        any_style = true;
                    }
                    if self.persisted.toolbar.subscript || self.toolbar_is_default() {
                        if any_style {
                            ui.add_space(5.);
                        }
                        self.style(Icon::SUBSCRIPT.size(ICON_SIZE), NodeValue::Subscript, root, ui)
                            .map(|e| events.push(e));
                        any_style = true;
                    }
                    if self.persisted.toolbar.superscript || self.toolbar_is_default() {
                        if any_style {
                            ui.add_space(5.);
                        }
                        self.style(
                            Icon::SUPERSCRIPT.size(ICON_SIZE),
                            NodeValue::Superscript,
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
                                .disabled(self.toolbar.menu_open)
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
                            .disabled(self.toolbar.menu_open)
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
                            .disabled(self.toolbar.menu_open)
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

                        // fill remaining space
                        const MENU_TOGGLE_SPACE: f32 = 40.;
                        if ui.available_width() > MENU_TOGGLE_SPACE {
                            ui.add_space(ui.available_width() - MENU_TOGGLE_SPACE);
                        }

                        if IconButton::new(
                            if self.toolbar.menu_open {
                                Icon::CHEVRON_DOWN
                            } else {
                                Icon::CHEVRON_UP
                            }
                            .size(ICON_SIZE),
                        )
                        .tooltip("Toolbar Settings")
                        .colored(self.toolbar.menu_open)
                        .show(ui)
                        .clicked()
                        {
                            self.toolbar.menu_open = !self.toolbar.menu_open;
                            ui.ctx().set_virtual_keyboard_shown(false);
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
            .disabled(self.toolbar.menu_open)
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
            .disabled(self.toolbar.menu_open)
            .show(ui);
        if resp.clicked() {
            Some(Event::ToggleStyle { region: Region::Selection, style })
        } else {
            None
        }
    }

    pub fn show_toolbar_menu(&mut self, ui: &mut Ui) {
        let margin: Margin =
            if cfg!(target_os = "android") { Margin::symmetric(0.0, 60.0) } else { Margin::ZERO };
        ScrollArea::vertical()
            .drag_to_scroll(true)
            .id_source("toolbar_settings")
            .scroll_bar_visibility(ScrollBarVisibility::AlwaysHidden)
            .show(ui, |ui| {
                ui.vertical_centered_justified(|ui| {
                    Frame::canvas(ui.style())
                        .inner_margin(margin)
                        .stroke(Stroke::NONE)
                        .fill(self.theme.bg().neutral_primary)
                        .show(ui, |ui| {
                            // setup
                            ui.visuals_mut().widgets.active.bg_fill = self.theme.fg().blue;

                            let is_ios = cfg!(target_os = "ios");

                            let scroll_view_height = ui.max_rect().height();
                            ui.allocate_space(Vec2 { x: ui.available_width(), y: 0. });
                            let padding = (ui.available_width() - self.width) / 2.;

                            let mut top_left =
                                ui.max_rect().min + (padding + MENU_MARGIN) * Vec2::X;
                            let md_width = self.width - 2. * MENU_MARGIN;

                            // store values
                            let source_lines = mem::take(&mut self.bounds.source_lines);
                            let buffer = mem::take(&mut self.buffer);
                            let paragraphs = mem::take(&mut self.bounds.paragraphs);
                            let inline_paragraphs = mem::take(&mut self.bounds.inline_paragraphs);

                            let galleys = mem::take(&mut self.galleys.galleys);
                            let wrap_lines = mem::take(&mut self.bounds.wrap_lines);
                            let touch_consuming_rects = mem::take(&mut self.touch_consuming_rects);

                            self.layout_cache.clear();

                            // page title
                            ui.add_space(MENU_SPACE);
                            top_left.y += MENU_SPACE;

                            ui.vertical_centered_justified(|ui| {
                                let font =
                                    FontId::new(16.0, egui::FontFamily::Name(Arc::from("Bold")));
                                Label::new(RichText::from("Show / Hide Toolbar Buttons").font(font))
                                    .ui(ui)
                            });
                            top_left.y += ui.text_style_height(&egui::TextStyle::Heading)
                                + ui.spacing().item_spacing.y;

                            ui.add_space(MENU_SPACE);
                            top_left.y += MENU_SPACE;

                            // undo / redo
                            if self
                                .menu_toggle(
                                    ui,
                                    top_left,
                                    md_width,
                                    "Undo",
                                    IconButton::new(Icon::UNDO.size(ICON_SIZE))
                                        .colored(self.persisted.toolbar.undo),
                                )
                                .clicked()
                            {
                                self.persisted.toolbar.undo ^= true;
                            }
                            top_left.y += self.menu_toggle_height("Undo");

                            if self
                                .menu_toggle(
                                    ui,
                                    top_left,
                                    md_width,
                                    "Redo",
                                    IconButton::new(Icon::REDO.size(ICON_SIZE))
                                        .colored(self.persisted.toolbar.redo),
                                )
                                .clicked()
                            {
                                self.persisted.toolbar.redo ^= true;
                            }
                            top_left.y += self.menu_toggle_height("Redo");

                            Separator::default().spacing(MENU_SPACE).ui(ui);
                            top_left.y += MENU_SPACE;

                            // styles
                            if self
                                .menu_toggle(
                                    ui,
                                    top_left,
                                    md_width,
                                    "### Heading",
                                    IconButton::new(Icon::HEADER_1.size(ICON_SIZE))
                                        .colored(self.persisted.toolbar.heading),
                                )
                                .clicked()
                            {
                                self.persisted.toolbar.heading ^= true;
                            }
                            top_left.y += self.menu_toggle_height("### Heading");

                            if self
                                .menu_toggle(
                                    ui,
                                    top_left,
                                    md_width,
                                    "**Bold**",
                                    IconButton::new(Icon::BOLD.size(ICON_SIZE))
                                        .colored(self.persisted.toolbar.bold),
                                )
                                .clicked()
                            {
                                self.persisted.toolbar.bold ^= true;
                            }
                            top_left.y += self.menu_toggle_height("**Bold**");

                            if self
                                .menu_toggle(
                                    ui,
                                    top_left,
                                    md_width,
                                    "*Italic*",
                                    IconButton::new(Icon::ITALIC.size(ICON_SIZE))
                                        .colored(self.persisted.toolbar.emph),
                                )
                                .clicked()
                            {
                                self.persisted.toolbar.emph ^= true;
                            }
                            top_left.y += self.menu_toggle_height("*Italic*");

                            if self
                                .menu_toggle(
                                    ui,
                                    top_left,
                                    md_width,
                                    "`Code`",
                                    IconButton::new(Icon::CODE.size(ICON_SIZE))
                                        .colored(self.persisted.toolbar.code),
                                )
                                .clicked()
                            {
                                self.persisted.toolbar.code ^= true;
                            }
                            top_left.y += self.menu_toggle_height("`Code`");

                            if self
                                .menu_toggle(
                                    ui,
                                    top_left,
                                    md_width,
                                    "~~Strikethrough~~",
                                    IconButton::new(Icon::STRIKETHROUGH.size(ICON_SIZE))
                                        .colored(self.persisted.toolbar.strikethrough),
                                )
                                .clicked()
                            {
                                self.persisted.toolbar.strikethrough ^= true;
                            }
                            top_left.y += self.menu_toggle_height("~~Strikethrough~~");

                            if self
                                .menu_toggle(
                                    ui,
                                    top_left,
                                    md_width,
                                    "==Highlight==",
                                    IconButton::new(Icon::HIGHLIGHT.size(ICON_SIZE))
                                        .colored(self.persisted.toolbar.highlight),
                                )
                                .clicked()
                            {
                                self.persisted.toolbar.highlight ^= true;
                            }
                            top_left.y += self.menu_toggle_height("==Highlight==");

                            if self
                                .menu_toggle(
                                    ui,
                                    top_left,
                                    md_width,
                                    "__Underline__",
                                    IconButton::new(Icon::UNDERLINE.size(ICON_SIZE))
                                        .colored(self.persisted.toolbar.underline),
                                )
                                .clicked()
                            {
                                self.persisted.toolbar.underline ^= true;
                            }
                            top_left.y += self.menu_toggle_height("__Underline__");

                            if self
                                .menu_toggle(
                                    ui,
                                    top_left,
                                    md_width,
                                    "||Spoiler||",
                                    IconButton::new(Icon::SPOILER.size(ICON_SIZE))
                                        .colored(self.persisted.toolbar.spoiler),
                                )
                                .clicked()
                            {
                                self.persisted.toolbar.spoiler ^= true;
                            }
                            top_left.y += self.menu_toggle_height("||Spoiler||");

                            if self
                                .menu_toggle(
                                    ui,
                                    top_left,
                                    md_width,
                                    "~Subscript~",
                                    IconButton::new(Icon::SUBSCRIPT.size(ICON_SIZE))
                                        .colored(self.persisted.toolbar.subscript),
                                )
                                .clicked()
                            {
                                self.persisted.toolbar.subscript ^= true;
                            }
                            top_left.y += self.menu_toggle_height("~Subscript~");

                            if self
                                .menu_toggle(
                                    ui,
                                    top_left,
                                    md_width,
                                    "^Superscript^",
                                    IconButton::new(Icon::SUPERSCRIPT.size(ICON_SIZE))
                                        .colored(self.persisted.toolbar.superscript),
                                )
                                .clicked()
                            {
                                self.persisted.toolbar.superscript ^= true;
                            }
                            top_left.y += self.menu_toggle_height("^Superscript^");

                            Separator::default().spacing(MENU_SPACE).ui(ui);
                            top_left.y += MENU_SPACE;

                            // lists
                            if self
                                .menu_toggle(
                                    ui,
                                    top_left,
                                    md_width,
                                    "1. Ordered List",
                                    IconButton::new(Icon::NUMBER_LIST.size(ICON_SIZE))
                                        .colored(self.persisted.toolbar.ordered_list),
                                )
                                .clicked()
                            {
                                self.persisted.toolbar.ordered_list ^= true;
                            }
                            top_left.y += self.menu_toggle_height("1. Ordered List");

                            if self
                                .menu_toggle(
                                    ui,
                                    top_left,
                                    md_width,
                                    "- Unordered List",
                                    IconButton::new(Icon::BULLET_LIST.size(ICON_SIZE))
                                        .colored(self.persisted.toolbar.unordered_list),
                                )
                                .clicked()
                            {
                                self.persisted.toolbar.unordered_list ^= true;
                            }
                            top_left.y += self.menu_toggle_height("- Unordered List");

                            if self
                                .menu_toggle(
                                    ui,
                                    top_left,
                                    md_width,
                                    "- [ ] Task List",
                                    IconButton::new(Icon::TODO_LIST.size(ICON_SIZE))
                                        .colored(self.persisted.toolbar.task_list),
                                )
                                .clicked()
                            {
                                self.persisted.toolbar.task_list ^= true;
                            }
                            top_left.y += self.menu_toggle_height("- [ ] Task List");

                            Separator::default().spacing(MENU_SPACE).ui(ui);
                            top_left.y += MENU_SPACE;

                            // media
                            if self
                                .menu_toggle(
                                    ui,
                                    top_left,
                                    md_width,
                                    "[Link](url)",
                                    IconButton::new(Icon::LINK.size(ICON_SIZE))
                                        .colored(self.persisted.toolbar.link),
                                )
                                .clicked()
                            {
                                self.persisted.toolbar.link ^= true;
                            }
                            top_left.y += self.menu_toggle_height("[Link](url)");

                            if is_ios {
                                if self
                                    .menu_toggle(
                                        ui,
                                        top_left,
                                        md_width,
                                        "![Image](url)",
                                        IconButton::new(Icon::CAMERA.size(ICON_SIZE))
                                            .colored(self.persisted.toolbar.image),
                                    )
                                    .clicked()
                                {
                                    self.persisted.toolbar.image ^= true;
                                }
                                top_left.y += self.menu_toggle_height("![Image](url)");
                            }

                            Separator::default().spacing(MENU_SPACE).ui(ui);
                            top_left.y += MENU_SPACE;

                            // indent
                            if self
                                .menu_toggle(
                                    ui,
                                    top_left,
                                    md_width,
                                    "Indent",
                                    IconButton::new(Icon::INDENT.size(ICON_SIZE))
                                        .colored(self.persisted.toolbar.indent),
                                )
                                .clicked()
                            {
                                self.persisted.toolbar.indent ^= true;
                            }
                            top_left.y += self.menu_toggle_height("Indent");

                            if self
                                .menu_toggle(
                                    ui,
                                    top_left,
                                    md_width,
                                    "De-indent",
                                    IconButton::new(Icon::DEINDENT.size(ICON_SIZE))
                                        .colored(self.persisted.toolbar.deindent),
                                )
                                .clicked()
                            {
                                self.persisted.toolbar.deindent ^= true;
                            }
                            top_left.y += self.menu_toggle_height("De-indent");

                            // fill remaining space
                            let cumulative_height = top_left.y - ui.max_rect().min.y;
                            let height = if cumulative_height < scroll_view_height {
                                scroll_view_height - cumulative_height
                            } else {
                                0.
                            };
                            let rect = Rect::from_min_size(top_left, Vec2::new(self.width, height));

                            ui.advance_cursor_after_rect(rect);

                            // restore stored values
                            self.buffer = buffer;
                            self.bounds.source_lines = source_lines;
                            self.bounds.paragraphs = paragraphs;
                            self.bounds.inline_paragraphs = inline_paragraphs;
                            self.calc_words();

                            self.galleys.galleys = galleys;
                            self.bounds.wrap_lines = wrap_lines;
                            self.touch_consuming_rects = touch_consuming_rects;
                        });
                });
            });
    }

    pub fn menu_toggle_height(&mut self, md: &str) -> f32 {
        let md_height = self.markdown_label_height(md);
        md_height.max(40.)
    }

    pub fn menu_toggle(
        &mut self, ui: &mut Ui, top_left: Pos2, width: f32, md: &str, icon_button: IconButton,
    ) -> Response {
        let md_height = self.markdown_label_height(md);
        let height = md_height.max(40.);

        let margin = (height - md_height) / 2.;
        let md_top_left = top_left + margin * Vec2::Y;
        self.markdown_label(ui, md_top_left, width, md);

        let padding = (ui.max_rect().width() - width) / 2.;
        let resp = ui.allocate_ui_with_layout(
            Vec2::new(width, height),
            Layout::right_to_left(egui::Align::Center),
            |ui| {
                ui.add_space(padding);
                icon_button.show(ui)
            },
        );

        resp.inner
    }

    pub fn markdown_label_height(&mut self, md: &str) -> f32 {
        self.buffer = md.into();

        // place cursor (affects capture)
        self.buffer.queue(vec![Operation::Select(
            self.buffer.current.segs.last_cursor_position().into_range(),
        )]);
        self.buffer.update();

        // parse
        let arena = Arena::new();
        let options = Self::comrak_options();
        let text_with_newline = self.buffer.current.text.to_string() + "\n";
        let root = comrak::parse_document(&arena, &text_with_newline, &options);

        // pre-render work
        self.calc_source_lines();
        self.compute_bounds(root);
        self.bounds.paragraphs.sort();
        self.bounds.inline_paragraphs.sort();
        self.calc_words();

        let height = self.height(root);

        self.layout_cache.clear();

        height
    }

    pub fn markdown_label(&mut self, ui: &mut Ui, top_left: Pos2, width: f32, md: &str) {
        self.buffer = md.into();

        // place cursor (affects capture)
        self.buffer.queue(vec![Operation::Select(
            self.buffer.current.segs.last_cursor_position().into_range(),
        )]);
        self.buffer.update();

        // parse
        let arena = Arena::new();
        let options = Self::comrak_options();
        let text_with_newline = self.buffer.current.text.to_string() + "\n";
        let root = comrak::parse_document(&arena, &text_with_newline, &options);

        // pre-render work
        self.calc_source_lines();
        self.compute_bounds(root);
        self.bounds.paragraphs.sort();
        self.bounds.inline_paragraphs.sort();
        self.calc_words();

        let height = self.height(root);
        let rect = Rect::from_min_size(top_left, Vec2::new(width, height));

        self.show_block(&mut ui.child_ui(rect, *ui.layout(), None), root, top_left);

        self.layout_cache.clear();
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
