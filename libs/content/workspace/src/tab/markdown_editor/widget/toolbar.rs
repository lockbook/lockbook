use std::mem;
use std::sync::Arc;
use web_time::{Duration, Instant};

use comrak::Arena;
use comrak::nodes::{AstNode, ListType, NodeHeading, NodeList, NodeValue};
use egui::scroll_area::{ScrollBarVisibility, ScrollSource};
use egui::{
    FontId, Frame, Label, Layout, Margin, Pos2, Rect, Response, RichText, ScrollArea, Sense,
    Separator, Stroke, Ui, UiBuilder, Vec2, Widget,
};
use lb_rs::model::text::offset_types::{IntoRangeExt, RangeExt as _};
use lb_rs::model::text::operation_types::Operation;
use serde::{Deserialize, Serialize};

use crate::tab::markdown_editor::MdRender;
use crate::tab::markdown_editor::widget::utils::NodeValueExt;
use crate::tab::{ExtendedInput as _, ExtendedOutput as _};
use crate::theme::palette_v2::ThemeExt;
use crate::theme::phosphor;
use crate::widgets::PhosphorIconButton;
use crate::workspace::CHROME_STRIP_H;

use crate::tab::markdown_editor::{self, Editor};
use markdown_editor::Event;
use markdown_editor::input::Region;

/// Phone docked toolbar (taller touch targets). Desktop uses [`TOOLBAR_H`].
pub const MOBILE_TOOL_BAR_SIZE: f32 = 45.0;
/// Desktop markdown toolbar band — same as the tab strip.
pub const TOOLBAR_H: f32 = CHROME_STRIP_H;
/// Fixed square tool hit target (fits inside [`TOOLBAR_H`]).
pub const TOOL_BTN: f32 = 28.0;
/// Glyph size inside [`TOOL_BTN`] (slot-first layout; not mesh-driven).
pub const ICON_SIZE: f32 = 15.0;
pub const MENU_SPACE: f32 = 20.; // space used for separators between menu sections
pub const MENU_MARGIN: f32 = 20.; // space on left and right side
/// Gap between adjacent tools in a group.
const TOOL_GAP: f32 = 2.0;
/// Horizontal inset from toolbar edges.
const TOOLBAR_PAD_X: f32 = 8.0;
/// Soft group rule width.
const SEP_W: f32 = 12.0;

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
    search: bool,
}

impl<'ast> Editor {
    pub fn show_toolbar(&mut self, root: &'ast AstNode<'ast>, ui: &mut Ui) {
        // Fixed band matching the tab strip (`CHROME_STRIP_H` / `TOOLBAR_H`).
        let (_, band) = ui.allocate_space(egui::vec2(ui.available_width(), TOOLBAR_H));
        ui.scope_builder(UiBuilder::new().max_rect(band), |ui| {
            ui.set_min_height(TOOLBAR_H);
            ui.set_max_height(TOOLBAR_H);
            self.show_toolbar_inner(root, ui);
        });
    }

    /// Computes the toolbar's content width without drawing it.
    pub fn toolbar_width(&self) -> f32 {
        let btn = TOOL_BTN;
        let gap = TOOL_GAP;
        let sep = SEP_W;

        let persistence = self.persistence.get_markdown().toolbar;
        let is_default = persistence == Default::default();
        let is_ios = cfg!(target_os = "ios");

        // width of a group of n buttons with intra-group spacing + trailing separator
        let group = |n: usize| -> f32 {
            if n > 0 { btn * n as f32 + gap * (n - 1) as f32 + sep } else { 0. }
        };
        let count =
            |flags: &[bool]| -> usize { flags.iter().filter(|&&on| on || is_default).count() };

        let mut w = 2. * TOOLBAR_PAD_X;

        if is_ios && (persistence.search || is_default) {
            w += group(1);
        }

        w += group(count(&[persistence.undo, persistence.redo]));
        w += group(count(&[
            persistence.heading,
            persistence.bold,
            persistence.emph,
            persistence.code,
            persistence.strikethrough,
            persistence.highlight,
            persistence.underline,
            persistence.spoiler,
            persistence.subscript,
            persistence.superscript,
        ]));
        w += group(count(&[
            persistence.ordered_list,
            persistence.unordered_list,
            persistence.task_list,
        ]));

        let mut media = count(&[persistence.link]);
        if (persistence.image || is_default) && is_ios {
            media += 1;
        }
        w += group(media);

        let n = count(&[persistence.indent, persistence.deindent]);
        if n > 0 {
            w += btn * n as f32 + gap * (n - 1) as f32;
        }

        w
    }

    #[allow(clippy::option_map_unit_fn)] // use of .map() reduces line wrapping, improving readability
    pub fn show_toolbar_inner(&mut self, root: &'ast AstNode<'ast>, ui: &mut Ui) {
        // Center the tool row in the fixed-height band.
        let toolbar_w = self.toolbar_width();
        let available = ui.available_width();
        let offset = ((available - toolbar_w) / 2.).max(0.);

        ScrollArea::horizontal()
            .scroll_bar_visibility(ScrollBarVisibility::AlwaysHidden)
            .max_height(TOOLBAR_H)
            .show(ui, |ui| {
                ui.with_layout(Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.visuals_mut().widgets.active.bg_fill =
                        self.edit.renderer.ctx.get_lb_theme().fg().blue;
                    ui.spacing_mut().item_spacing = egui::vec2(TOOL_GAP, 0.0);
                    ui.spacing_mut().button_padding = egui::vec2(0.0, 0.0);

                    let is_ios = cfg!(target_os = "ios");
                    ui.add_space(offset + TOOLBAR_PAD_X);

                    let persistence = self.persistence.get_markdown().toolbar;
                    let toolbar_is_default = persistence == Default::default();

                    let mut events = Vec::new();

                    if is_ios && (persistence.search || toolbar_is_default) {
                        let find_open = self.find.term.is_some();
                        if tool_btn(
                            phosphor::MAGNIFYING_GLASS,
                            "Search",
                            find_open,
                            self.toolbar.menu_open,
                            ui,
                        )
                        .clicked()
                        {
                            if find_open {
                                self.find.term = None;
                                self.find.matches.clear();
                                self.find.current_match = None;
                            } else {
                                self.find.open_requested = true;
                            }
                        }
                        add_seperator(ui);
                    }

                    let mut any_undo_redo = false;
                    if persistence.undo || toolbar_is_default {
                        if tool_btn(
                            phosphor::ARROW_U_UP_LEFT,
                            "Undo",
                            false,
                            self.toolbar.menu_open,
                            ui,
                        )
                        .clicked()
                        {
                            events.push(Event::Undo);
                        }
                        any_undo_redo = true;
                    }
                    if persistence.redo || toolbar_is_default {
                        if tool_btn(
                            phosphor::ARROW_U_UP_RIGHT,
                            "Redo",
                            false,
                            self.toolbar.menu_open,
                            ui,
                        )
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
                    if persistence.heading || toolbar_is_default {
                        self.heading_button(root, ui).map(|e| events.push(e));
                        any_style = true;
                    }
                    if persistence.bold || toolbar_is_default {
                        self.style(phosphor::TEXT_B, NodeValue::Strong, root, ui)
                            .map(|e| events.push(e));
                        any_style = true;
                    }
                    if persistence.emph || toolbar_is_default {
                        self.style(phosphor::TEXT_ITALIC, NodeValue::Emph, root, ui)
                            .map(|e| events.push(e));
                        any_style = true;
                    }
                    if persistence.code || toolbar_is_default {
                        self.style(phosphor::CODE, NodeValue::Code(Default::default()), root, ui)
                            .map(|e| events.push(e));
                        any_style = true;
                    }
                    if persistence.strikethrough || toolbar_is_default {
                        self.style(phosphor::TEXT_STRIKETHROUGH, NodeValue::Strikethrough, root, ui)
                            .map(|e| events.push(e));
                        any_style = true;
                    }
                    if persistence.highlight || toolbar_is_default {
                        self.style(phosphor::HIGHLIGHTER_CIRCLE, NodeValue::Highlight, root, ui)
                            .map(|e| events.push(e));
                        any_style = true;
                    }
                    if persistence.underline || toolbar_is_default {
                        self.style(phosphor::TEXT_UNDERLINE, NodeValue::Underline, root, ui)
                            .map(|e| events.push(e));
                        any_style = true;
                    }
                    if persistence.spoiler || toolbar_is_default {
                        self.style(phosphor::EYE_SLASH, NodeValue::SpoileredText, root, ui)
                            .map(|e| events.push(e));
                        any_style = true;
                    }
                    if persistence.subscript || toolbar_is_default {
                        self.style(phosphor::TEXT_SUBSCRIPT, NodeValue::Subscript, root, ui)
                            .map(|e| events.push(e));
                        any_style = true;
                    }
                    if persistence.superscript || toolbar_is_default {
                        self.style(phosphor::TEXT_SUPERSCRIPT, NodeValue::Superscript, root, ui)
                            .map(|e| events.push(e));
                        any_style = true;
                    }
                    if any_style {
                        add_seperator(ui);
                    }

                    let mut any_list = false;
                    if persistence.ordered_list || toolbar_is_default {
                        self.style(
                            phosphor::LIST_NUMBERS,
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
                    if persistence.unordered_list || toolbar_is_default {
                        self.style(
                            phosphor::LIST_BULLETS,
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
                    if persistence.task_list || toolbar_is_default {
                        self.style(
                            phosphor::LIST_CHECKS,
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
                    if persistence.link || toolbar_is_default {
                        self.style(phosphor::LINK, NodeValue::Link(Default::default()), root, ui)
                            .map(|e| events.push(e));
                        any_media = true;
                    }
                    if persistence.image || toolbar_is_default {
                        // only supported on iOS (for now)
                        if is_ios {
                            if tool_btn(
                                phosphor::CAMERA,
                                "Camera",
                                false,
                                self.toolbar.menu_open,
                                ui,
                            )
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

                    if persistence.indent || toolbar_is_default {
                        if tool_btn(
                            phosphor::TEXT_INDENT,
                            "Indent",
                            false,
                            self.toolbar.menu_open,
                            ui,
                        )
                        .clicked()
                        {
                            events.push(Event::Indent { deindent: false });
                        }
                    }
                    if persistence.deindent || toolbar_is_default {
                        if tool_btn(
                            phosphor::TEXT_OUTDENT,
                            "De-indent",
                            false,
                            self.toolbar.menu_open,
                            ui,
                        )
                        .clicked()
                        {
                            events.push(Event::Indent { deindent: true });
                        }
                    }

                    if self.edit.phone_mode {
                        add_seperator(ui);
                        // Push settings caret to the trailing edge.
                        let caret_w = TOOL_BTN + TOOLBAR_PAD_X;
                        if ui.available_width() > caret_w {
                            ui.add_space(ui.available_width() - caret_w);
                        }
                        let caret = if self.toolbar.menu_open {
                            phosphor::CARET_DOWN
                        } else {
                            phosphor::CARET_UP
                        };
                        if tool_btn(caret, "Toolbar Settings", self.toolbar.menu_open, false, ui)
                            .clicked()
                        {
                            self.toolbar.menu_open = !self.toolbar.menu_open;
                            ui.ctx().set_virtual_keyboard_shown(false);
                        }
                    }

                    ui.add_space(TOOLBAR_PAD_X);

                    for event in events {
                        ui.ctx().push_markdown_event(event);
                        if self.edit.phone_mode {
                            // bottom toolbar painted after editor events processed
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
                if self.edit.renderer.node_range(node).contains_range(
                    &self.edit.renderer.buffer.current.selection,
                    true,
                    true,
                ) {
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

        let resp = tool_btn(
            phosphor::TEXT_H,
            style.name(),
            applied,
            self.toolbar.menu_open,
            ui,
        );
        if resp.clicked() {
            self.toolbar.heading_last_click_at = Instant::now();
            Some(Event::ToggleStyle { region: Region::Selection, style })
        } else {
            None
        }
    }

    fn style(
        &self, glyph: &'static str, style: NodeValue, root: &'ast AstNode<'ast>, ui: &mut Ui,
    ) -> Option<Event> {
        let applied = if style.is_inline() {
            self.edit
                .inline_styled(root, self.edit.renderer.buffer.current.selection, &style)
        } else {
            self.edit.unapply_block(root, &style)
        };

        self.button(glyph, style, applied, ui)
    }

    fn button(
        &self, glyph: &'static str, style: NodeValue, applied: bool, ui: &mut Ui,
    ) -> Option<Event> {
        let resp = tool_btn(glyph, style.name(), applied, self.toolbar.menu_open, ui);
        if resp.clicked() {
            Some(Event::ToggleStyle { region: Region::Selection, style })
        } else {
            None
        }
    }

    pub fn show_toolbar_menu(&mut self, ui: &mut Ui) {
        let margin: Margin =
            if cfg!(target_os = "android") { Margin::symmetric(0, 60) } else { Margin::ZERO };
        ScrollArea::vertical()
            .scroll_source(ScrollSource::ALL)
            .id_salt("toolbar_settings")
            .scroll_bar_visibility(ScrollBarVisibility::AlwaysHidden)
            .show(ui, |ui| {
                ui.vertical_centered_justified(|ui| {
                    Frame::canvas(ui.style())
                        .inner_margin(margin)
                        .stroke(Stroke::NONE)
                        .fill(self.edit.renderer.ctx.get_lb_theme().neutral_bg())
                        .show(ui, |ui| {
                            // setup
                            ui.visuals_mut().widgets.active.bg_fill =
                                self.edit.renderer.ctx.get_lb_theme().fg().blue;

                            let is_android = cfg!(target_os = "android");
                            let is_ios = cfg!(target_os = "ios");

                            let persistence = self.persistence.get_markdown().toolbar;

                            let scroll_view_height = ui.max_rect().height();
                            ui.allocate_space(Vec2 { x: ui.available_width(), y: 0. });
                            let padding = (ui.available_width() - self.edit.renderer.width) / 2.;

                            let mut top_left =
                                ui.max_rect().min + (padding + MENU_MARGIN) * Vec2::X;
                            let md_width = self.edit.renderer.width - 2. * MENU_MARGIN;

                            // store values
                            let source_lines =
                                mem::take(&mut self.edit.renderer.bounds.source_lines);
                            let buffer = mem::take(&mut self.edit.renderer.buffer);
                            let inline_paragraphs =
                                mem::take(&mut self.edit.renderer.bounds.inline_paragraphs);

                            let fragments = mem::take(&mut self.edit.renderer.fragments);
                            let wrap_lines = mem::take(&mut self.edit.renderer.bounds.wrap_lines);
                            let touch_consuming_rects =
                                mem::take(&mut self.edit.renderer.touch_consuming_rects);

                            // menu labels: force blue links + plain image-link text
                            let link_resolver =
                                mem::replace(&mut self.edit.renderer.link_resolver, Box::new(()));
                            self.edit.renderer.disable_images = true;

                            // labels are static exemplars — never reveal syntax
                            // at the editor's (unrelated) selection offsets
                            let reveal_selection =
                                mem::take(&mut self.edit.renderer.reveal_selection);

                            self.edit.renderer.layout_cache.clear();

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

                            if !is_android {
                                // search
                                if self
                                    .menu_toggle(
                                        ui,
                                        top_left,
                                        md_width,
                                        "Search",
                                        phosphor::MAGNIFYING_GLASS,
                                        persistence.search,
                                    )
                                    .clicked()
                                {
                                    let mut persistence = self.persistence.data.write().unwrap();
                                    let persistence = &mut persistence.markdown.toolbar;
                                    persistence.search ^= true;
                                    self.persistence.write_to_file();
                                }
                                top_left.y += self.menu_toggle_height("Search");

                                Separator::default().spacing(MENU_SPACE).ui(ui);
                                top_left.y += MENU_SPACE;
                            }

                            // undo / redo
                            if self
                                .menu_toggle(
                                    ui,
                                    top_left,
                                    md_width,
                                    "Undo",
                                    phosphor::ARROW_U_UP_LEFT,
                                    persistence.undo,
                                )
                                .clicked()
                            {
                                let mut persistence = self.persistence.data.write().unwrap();
                                let persistence = &mut persistence.markdown.toolbar;
                                persistence.undo ^= true;
                                self.persistence.write_to_file();
                            }
                            top_left.y += self.menu_toggle_height("Undo");

                            if self
                                .menu_toggle(
                                    ui,
                                    top_left,
                                    md_width,
                                    "Redo",
                                    phosphor::ARROW_U_UP_RIGHT,
                                    persistence.redo,
                                )
                                .clicked()
                            {
                                let mut persistence = self.persistence.data.write().unwrap();
                                let persistence = &mut persistence.markdown.toolbar;
                                persistence.redo ^= true;
                                self.persistence.write_to_file();
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
                                    phosphor::TEXT_H,
                                    persistence.heading,
                                )
                                .clicked()
                            {
                                let mut persistence = self.persistence.data.write().unwrap();
                                let persistence = &mut persistence.markdown.toolbar;
                                persistence.heading ^= true;
                                self.persistence.write_to_file();
                            }
                            top_left.y += self.menu_toggle_height("### Heading");

                            if self
                                .menu_toggle(
                                    ui,
                                    top_left,
                                    md_width,
                                    "**Bold**",
                                    phosphor::TEXT_B,
                                    persistence.bold,
                                )
                                .clicked()
                            {
                                let mut persistence = self.persistence.data.write().unwrap();
                                let persistence = &mut persistence.markdown.toolbar;
                                persistence.bold ^= true;
                                self.persistence.write_to_file();
                            }
                            top_left.y += self.menu_toggle_height("**Bold**");

                            if self
                                .menu_toggle(
                                    ui,
                                    top_left,
                                    md_width,
                                    "*Italic*",
                                    phosphor::TEXT_ITALIC,
                                    persistence.emph,
                                )
                                .clicked()
                            {
                                let mut persistence = self.persistence.data.write().unwrap();
                                let persistence = &mut persistence.markdown.toolbar;
                                persistence.emph ^= true;
                                self.persistence.write_to_file();
                            }
                            top_left.y += self.menu_toggle_height("*Italic*");

                            if self
                                .menu_toggle(
                                    ui,
                                    top_left,
                                    md_width,
                                    "`Code`",
                                    phosphor::CODE,
                                    persistence.code,
                                )
                                .clicked()
                            {
                                let mut persistence = self.persistence.data.write().unwrap();
                                let persistence = &mut persistence.markdown.toolbar;
                                persistence.code ^= true;
                                self.persistence.write_to_file();
                            }
                            top_left.y += self.menu_toggle_height("`Code`");

                            if self
                                .menu_toggle(
                                    ui,
                                    top_left,
                                    md_width,
                                    "~~Strikethrough~~",
                                    phosphor::TEXT_STRIKETHROUGH,
                                    persistence.strikethrough,
                                )
                                .clicked()
                            {
                                let mut persistence = self.persistence.data.write().unwrap();
                                let persistence = &mut persistence.markdown.toolbar;
                                persistence.strikethrough ^= true;
                                self.persistence.write_to_file();
                            }
                            top_left.y += self.menu_toggle_height("~~Strikethrough~~");

                            if self
                                .menu_toggle(
                                    ui,
                                    top_left,
                                    md_width,
                                    "==Highlight==",
                                    phosphor::HIGHLIGHTER_CIRCLE,
                                    persistence.highlight,
                                )
                                .clicked()
                            {
                                let mut persistence = self.persistence.data.write().unwrap();
                                let persistence = &mut persistence.markdown.toolbar;
                                persistence.highlight ^= true;
                                self.persistence.write_to_file();
                            }
                            top_left.y += self.menu_toggle_height("==Highlight==");

                            if self
                                .menu_toggle(
                                    ui,
                                    top_left,
                                    md_width,
                                    "__Underline__",
                                    phosphor::TEXT_UNDERLINE,
                                    persistence.underline,
                                )
                                .clicked()
                            {
                                let mut persistence = self.persistence.data.write().unwrap();
                                let persistence = &mut persistence.markdown.toolbar;
                                persistence.underline ^= true;
                                self.persistence.write_to_file();
                            }
                            top_left.y += self.menu_toggle_height("__Underline__");

                            if self
                                .menu_toggle(
                                    ui,
                                    top_left,
                                    md_width,
                                    "||Spoiler||",
                                    phosphor::EYE_SLASH,
                                    persistence.spoiler,
                                )
                                .clicked()
                            {
                                let mut persistence = self.persistence.data.write().unwrap();
                                let persistence = &mut persistence.markdown.toolbar;
                                persistence.spoiler ^= true;
                                self.persistence.write_to_file();
                            }
                            top_left.y += self.menu_toggle_height("||Spoiler||");

                            if self
                                .menu_toggle(
                                    ui,
                                    top_left,
                                    md_width,
                                    "~Subscript~",
                                    phosphor::TEXT_SUBSCRIPT,
                                    persistence.subscript,
                                )
                                .clicked()
                            {
                                let mut persistence = self.persistence.data.write().unwrap();
                                let persistence = &mut persistence.markdown.toolbar;
                                persistence.subscript ^= true;
                                self.persistence.write_to_file();
                            }
                            top_left.y += self.menu_toggle_height("~Subscript~");

                            if self
                                .menu_toggle(
                                    ui,
                                    top_left,
                                    md_width,
                                    "^Superscript^",
                                    phosphor::TEXT_SUPERSCRIPT,
                                    persistence.superscript,
                                )
                                .clicked()
                            {
                                let mut persistence = self.persistence.data.write().unwrap();
                                let persistence = &mut persistence.markdown.toolbar;
                                persistence.superscript ^= true;
                                self.persistence.write_to_file();
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
                                    phosphor::LIST_NUMBERS,
                                    persistence.ordered_list,
                                )
                                .clicked()
                            {
                                let mut persistence = self.persistence.data.write().unwrap();
                                let persistence = &mut persistence.markdown.toolbar;
                                persistence.ordered_list ^= true;
                                self.persistence.write_to_file();
                            }
                            top_left.y += self.menu_toggle_height("1. Ordered List");

                            if self
                                .menu_toggle(
                                    ui,
                                    top_left,
                                    md_width,
                                    "- Unordered List",
                                    phosphor::LIST_BULLETS,
                                    persistence.unordered_list,
                                )
                                .clicked()
                            {
                                let mut persistence = self.persistence.data.write().unwrap();
                                let persistence = &mut persistence.markdown.toolbar;
                                persistence.unordered_list ^= true;
                                self.persistence.write_to_file();
                            }
                            top_left.y += self.menu_toggle_height("- Unordered List");

                            if self
                                .menu_toggle(
                                    ui,
                                    top_left,
                                    md_width,
                                    "- [ ] Task List",
                                    phosphor::LIST_CHECKS,
                                    persistence.task_list,
                                )
                                .clicked()
                            {
                                let mut persistence = self.persistence.data.write().unwrap();
                                let persistence = &mut persistence.markdown.toolbar;
                                persistence.task_list ^= true;
                                self.persistence.write_to_file();
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
                                    phosphor::LINK,
                                    persistence.link,
                                )
                                .clicked()
                            {
                                let mut persistence = self.persistence.data.write().unwrap();
                                let persistence = &mut persistence.markdown.toolbar;
                                persistence.link ^= true;
                                self.persistence.write_to_file();
                            }
                            top_left.y += self.menu_toggle_height("[Link](url)");

                            if is_ios {
                                if self
                                    .menu_toggle(
                                        ui,
                                        top_left,
                                        md_width,
                                        "![Image](url)",
                                        phosphor::CAMERA,
                                        persistence.image,
                                    )
                                    .clicked()
                                {
                                    let mut persistence = self.persistence.data.write().unwrap();
                                    let persistence = &mut persistence.markdown.toolbar;
                                    persistence.image ^= true;
                                    self.persistence.write_to_file();
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
                                    phosphor::TEXT_INDENT,
                                    persistence.indent,
                                )
                                .clicked()
                            {
                                let mut persistence = self.persistence.data.write().unwrap();
                                let persistence = &mut persistence.markdown.toolbar;
                                persistence.indent ^= true;
                                self.persistence.write_to_file();
                            }
                            top_left.y += self.menu_toggle_height("Indent");

                            if self
                                .menu_toggle(
                                    ui,
                                    top_left,
                                    md_width,
                                    "De-indent",
                                    phosphor::TEXT_OUTDENT,
                                    persistence.deindent,
                                )
                                .clicked()
                            {
                                let mut persistence = self.persistence.data.write().unwrap();
                                let persistence = &mut persistence.markdown.toolbar;
                                persistence.deindent ^= true;
                                self.persistence.write_to_file();
                            }
                            top_left.y += self.menu_toggle_height("De-indent");

                            // fill remaining space
                            let cumulative_height = top_left.y - ui.max_rect().min.y;
                            let height = if cumulative_height < scroll_view_height {
                                scroll_view_height - cumulative_height
                            } else {
                                0.
                            };
                            let rect = Rect::from_min_size(
                                top_left,
                                Vec2::new(self.edit.renderer.width, height),
                            );

                            ui.advance_cursor_after_rect(rect);

                            // submit shaped text — `MdEdit::show` (which
                            // normally drains text_areas) doesn't run while
                            // the menu is open
                            let text_areas = mem::take(&mut self.edit.renderer.text_areas);
                            if !text_areas.is_empty() {
                                ui.painter().add(
                                    egui_wgpu_renderer::egui_wgpu::Callback::new_paint_callback(
                                        ui.clip_rect(),
                                        crate::GlyphonRendererCallback::new(text_areas),
                                    ),
                                );
                            }

                            // restore stored values
                            self.edit.renderer.buffer = buffer;
                            self.edit.renderer.bounds.source_lines = source_lines;
                            self.edit.renderer.bounds.inline_paragraphs = inline_paragraphs;
                            self.edit.renderer.calc_words();

                            self.edit.renderer.fragments = fragments;
                            self.edit.renderer.bounds.wrap_lines = wrap_lines;
                            self.edit.renderer.touch_consuming_rects = touch_consuming_rects;

                            self.edit.renderer.link_resolver = link_resolver;
                            self.edit.renderer.disable_images = false;
                            self.edit.renderer.reveal_selection = reveal_selection;
                        });
                });
            });
    }

    pub fn menu_toggle_height(&mut self, md: &str) -> f32 {
        let md_height = self.markdown_label_height(md);
        md_height.max(40.)
    }

    pub fn menu_toggle(
        &mut self, ui: &mut Ui, top_left: Pos2, width: f32, md: &str, glyph: &'static str,
        colored: bool,
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
                PhosphorIconButton::new(glyph)
                    .icon_size(ICON_SIZE)
                    .size(TOOL_BTN)
                    .colored(colored)
                    .show(ui)
            },
        );

        resp.inner
    }

    pub fn markdown_label_height(&mut self, md: &str) -> f32 {
        self.edit.renderer.buffer = md.into();

        // place cursor (affects capture)
        self.edit.renderer.buffer.queue(vec![Operation::Select(
            self.edit
                .renderer
                .buffer
                .current
                .segs
                .last_cursor_position()
                .into_range(),
        )]);
        self.edit.renderer.buffer.update();

        // parse
        let arena = Arena::new();
        let options = MdRender::comrak_options();
        let text_with_newline = self.edit.renderer.buffer.current.text.to_string() + "\n";
        let root = comrak::parse_document(&arena, &text_with_newline, &options);

        // pre-render work
        self.edit.renderer.calc_source_lines();
        self.edit.renderer.calc_fold_bounds(root);
        self.edit.renderer.calc_image_bounds(root);
        self.edit.renderer.populate_hidden_by_fold(root);
        self.edit.renderer.compute_bounds(root);
        self.edit.renderer.bounds.inline_paragraphs.sort();
        self.edit.renderer.calc_words();

        let height = self.edit.renderer.height(root);

        self.edit.renderer.layout_cache.clear();

        height
    }

    pub fn markdown_label(&mut self, ui: &mut Ui, top_left: Pos2, width: f32, md: &str) {
        self.edit.renderer.buffer = md.into();

        // place cursor (affects capture)
        self.edit.renderer.buffer.queue(vec![Operation::Select(
            self.edit
                .renderer
                .buffer
                .current
                .segs
                .last_cursor_position()
                .into_range(),
        )]);
        self.edit.renderer.buffer.update();

        // parse
        let arena = Arena::new();
        let options = MdRender::comrak_options();
        let text_with_newline = self.edit.renderer.buffer.current.text.to_string() + "\n";
        let root = comrak::parse_document(&arena, &text_with_newline, &options);

        // pre-render work
        self.edit.renderer.calc_source_lines();
        self.edit.renderer.calc_fold_bounds(root);
        self.edit.renderer.calc_image_bounds(root);
        self.edit.renderer.populate_hidden_by_fold(root);
        self.edit.renderer.compute_bounds(root);
        self.edit.renderer.bounds.inline_paragraphs.sort();
        self.edit.renderer.calc_words();

        let height = self.edit.renderer.height(root);
        let rect = Rect::from_min_size(top_left, Vec2::new(width, height));

        self.edit.renderer.show_block(
            &mut ui.new_child(UiBuilder::new().max_rect(rect).layout(*ui.layout())),
            root,
            top_left,
        );

        self.edit.renderer.layout_cache.clear();
    }
}

fn tool_btn(
    glyph: &'static str, tooltip: impl Into<String>, colored: bool, disabled: bool, ui: &mut Ui,
) -> Response {
    PhosphorIconButton::new(glyph)
        .icon_size(ICON_SIZE)
        .size(TOOL_BTN)
        .tooltip(tooltip)
        .colored(colored)
        .disabled(disabled)
        .show(ui)
}

/// Soft vertical rule between tool groups — fixed size, not content-height.
fn add_seperator(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(SEP_W, TOOL_BTN), Sense::hover());
    let theme = ui.ctx().get_lb_theme();
    let y0 = rect.center().y - TOOL_BTN * 0.28;
    let y1 = rect.center().y + TOOL_BTN * 0.28;
    ui.painter().vline(
        rect.center().x,
        y0..=y1,
        Stroke::new(1.0, theme.neutral()),
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
