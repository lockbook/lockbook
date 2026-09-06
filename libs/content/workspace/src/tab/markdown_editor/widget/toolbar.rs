use std::mem;
use std::sync::Arc;
use web_time::{Duration, Instant};

use comrak::Arena;
use comrak::nodes::{AstNode, ListType, NodeHeading, NodeList, NodeValue};
use egui::scroll_area::{ScrollBarVisibility, ScrollSource};
use egui::{
    FontId, Frame, Label, Layout, Margin, Pos2, Rect, Response, RichText, ScrollArea, Separator,
    Stroke, Ui, UiBuilder, Vec2, Widget,
};
use lb_rs::model::text::offset_types::{IntoRangeExt, RangeExt as _};
use lb_rs::model::text::operation_types::Operation;
use serde::{Deserialize, Serialize};

use crate::style::{
    CHROME_BAND_GLYPH, CHROME_BAND_H, STROKE_HAIRLINE, ThemeExt, control_height, icon_button_glyph,
    phosphor, place_at, tip_text,
};
use crate::tab::markdown_editor::MdRender;
use crate::tab::markdown_editor::widget::utils::NodeValueExt;
use crate::tab::{ExtendedInput as _, ExtendedOutput as _};

use crate::tab::markdown_editor::{self, Editor};
use markdown_editor::Event;
use markdown_editor::input::Region;

pub const MOBILE_TOOL_BAR_SIZE: f32 = CHROME_BAND_H;
pub const MENU_SPACE: f32 = 20.; // space used for separators between menu sections
pub const MENU_MARGIN: f32 = 20.; // space on left and right side

fn toolbar_icon(
    ui: &mut Ui, t: &crate::style::Theme, icon: &'static str, applied: bool, menu_open: bool,
    tip: &str,
) -> bool {
    let r = icon_button_glyph(
        ui,
        t,
        icon,
        applied && !menu_open,
        t.neutral_bg(),
        control_height(),
        CHROME_BAND_GLYPH,
    );
    tip_text(ui.ctx(), &r, tip);
    !menu_open && r.clicked()
}

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
        ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
        let w = ui.available_width();
        let (band, _) = ui.allocate_exact_size(egui::vec2(w, CHROME_BAND_H), egui::Sense::hover());
        place_at(ui, band, Layout::left_to_right(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
            self.show_toolbar_inner(root, ui);
        });
    }

    /// Computes the toolbar's content width without drawing it.
    pub fn toolbar_width(&self) -> f32 {
        let btn = control_height();
        let sep = 20.; // separator with .spacing(20.)
        let gap = 5.; // explicit add_space(5.) between buttons
        let margin = 10.; // padding on each side

        let persistence = self.persistence.get_markdown().toolbar;
        let is_default = persistence == Default::default();
        let is_ios = cfg!(target_os = "ios");

        // width of a group of n buttons with intra-group spacing + trailing separator
        let group = |n: usize| -> f32 {
            if n > 0 { btn * n as f32 + gap * (n - 1) as f32 + sep } else { 0. }
        };
        let count =
            |flags: &[bool]| -> usize { flags.iter().filter(|&&on| on || is_default).count() };

        let mut w = 2. * margin;

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
        // center the toolbar content horizontally
        let toolbar_w = self.toolbar_width();
        let available = ui.available_width();
        let offset = ((available - toolbar_w) / 2.).max(0.);

        ScrollArea::horizontal()
            .scroll_bar_visibility(ScrollBarVisibility::AlwaysHidden)
            .max_height(control_height())
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
                    let t = ui.ctx().get_lb_theme();
                    let menu_open = self.toolbar.menu_open;
                    let is_ios = cfg!(target_os = "ios");

                    let toolbar_margin = 10.;
                    // offset centers the full toolbar_w (including margins);
                    // add margin so buttons start after the leading margin
                    ui.add_space(offset + toolbar_margin);

                    let persistence = self.persistence.get_markdown().toolbar;
                    let toolbar_is_default = persistence == Default::default();

                    let mut events = Vec::new();

                    if is_ios && (persistence.search || toolbar_is_default) {
                        let find_open = self.find.term.is_some();
                        if toolbar_icon(ui, &t, phosphor::SEARCH, find_open, menu_open, "Search") {
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
                        if toolbar_icon(
                            ui,
                            &t,
                            phosphor::ARROW_COUNTER_CLOCKWISE,
                            true,
                            menu_open,
                            "Undo",
                        ) {
                            events.push(Event::Undo);
                        }
                        any_undo_redo = true;
                    }
                    if persistence.redo || toolbar_is_default {
                        if any_undo_redo {
                            ui.add_space(5.);
                        }
                        if toolbar_icon(ui, &t, phosphor::ARROW_CLOCKWISE, true, menu_open, "Redo")
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
                        if any_style {
                            ui.add_space(5.);
                        }
                        self.style(phosphor::TEXT_B, NodeValue::Strong, root, ui)
                            .map(|e| events.push(e));
                        any_style = true;
                    }
                    if persistence.emph || toolbar_is_default {
                        if any_style {
                            ui.add_space(5.);
                        }
                        self.style(phosphor::TEXT_ITALIC, NodeValue::Emph, root, ui)
                            .map(|e| events.push(e));
                        any_style = true;
                    }
                    if persistence.code || toolbar_is_default {
                        if any_style {
                            ui.add_space(5.);
                        }
                        self.style(phosphor::CODE, NodeValue::Code(Default::default()), root, ui)
                            .map(|e| events.push(e));
                        any_style = true;
                    }
                    if persistence.strikethrough || toolbar_is_default {
                        if any_style {
                            ui.add_space(5.);
                        }
                        self.style(
                            phosphor::TEXT_STRIKETHROUGH,
                            NodeValue::Strikethrough,
                            root,
                            ui,
                        )
                        .map(|e| events.push(e));
                        any_style = true;
                    }
                    if persistence.highlight || toolbar_is_default {
                        if any_style {
                            ui.add_space(5.);
                        }
                        self.style(phosphor::HIGHLIGHTER, NodeValue::Highlight, root, ui)
                            .map(|e| events.push(e));
                        any_style = true;
                    }
                    if persistence.underline || toolbar_is_default {
                        if any_style {
                            ui.add_space(5.);
                        }
                        self.style(phosphor::TEXT_UNDERLINE, NodeValue::Underline, root, ui)
                            .map(|e| events.push(e));
                        any_style = true;
                    }
                    if persistence.spoiler || toolbar_is_default {
                        if any_style {
                            ui.add_space(5.);
                        }
                        self.style(phosphor::EYE_SLASH, NodeValue::SpoileredText, root, ui)
                            .map(|e| events.push(e));
                        any_style = true;
                    }
                    if persistence.subscript || toolbar_is_default {
                        if any_style {
                            ui.add_space(5.);
                        }
                        self.style(phosphor::TEXT_SUBSCRIPT, NodeValue::Subscript, root, ui)
                            .map(|e| events.push(e));
                        any_style = true;
                    }
                    if persistence.superscript || toolbar_is_default {
                        if any_style {
                            ui.add_space(5.);
                        }
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
                        if any_list {
                            ui.add_space(5.);
                        }
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
                        if any_list {
                            ui.add_space(5.);
                        }
                        self.style(
                            phosphor::CHECK_SQUARE,
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
                            if any_media {
                                ui.add_space(5.);
                            }
                            if toolbar_icon(ui, &t, phosphor::CAMERA, true, menu_open, "Camera") {
                                events.push(Event::Camera);
                            }
                            any_media = true;
                        }
                    }
                    if any_media {
                        add_seperator(ui);
                    }

                    let mut any_indent = false;
                    if persistence.indent || toolbar_is_default {
                        if toolbar_icon(ui, &t, phosphor::TEXT_INDENT, true, menu_open, "Indent") {
                            events.push(Event::Indent { deindent: false });
                        }
                        any_indent = true;
                    }
                    if persistence.deindent || toolbar_is_default {
                        if any_indent {
                            ui.add_space(5.);
                        }
                        if toolbar_icon(
                            ui,
                            &t,
                            phosphor::TEXT_OUTDENT,
                            true,
                            menu_open,
                            "De-indent",
                        ) {
                            events.push(Event::Indent { deindent: true });
                        }
                        any_indent = true;
                    }

                    if self.edit.phone_mode {
                        if any_indent {
                            add_seperator(ui);
                        }

                        // fill remaining space
                        const MENU_TOGGLE_SPACE: f32 = 40.;
                        if ui.available_width() > MENU_TOGGLE_SPACE {
                            ui.add_space(ui.available_width() - MENU_TOGGLE_SPACE);
                        }

                        let chevron = if self.toolbar.menu_open {
                            phosphor::CARET_DOWN
                        } else {
                            phosphor::CARET_UP
                        };
                        if toolbar_icon(
                            ui,
                            &t,
                            chevron,
                            self.toolbar.menu_open,
                            false,
                            "Toolbar Settings",
                        ) {
                            self.toolbar.menu_open = !self.toolbar.menu_open;
                            ui.ctx().set_virtual_keyboard_shown(false);
                        }
                        ui.add_space(5.);
                    }

                    ui.add_space(toolbar_margin);
                    ui.add_space(ui.available_width());

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

        let t = ui.ctx().get_lb_theme();
        if toolbar_icon(ui, &t, phosphor::TEXT_H_ONE, applied, self.toolbar.menu_open, style.name())
        {
            self.toolbar.heading_last_click_at = Instant::now();
            Some(Event::ToggleStyle { region: Region::Selection, style })
        } else {
            None
        }
    }

    fn style(
        &self, icon: &'static str, style: NodeValue, root: &'ast AstNode<'ast>, ui: &mut Ui,
    ) -> Option<Event> {
        let applied = if style.is_inline() {
            self.edit
                .inline_styled(root, self.edit.renderer.buffer.current.selection, &style)
        } else {
            self.edit.unapply_block(root, &style)
        };

        self.button(icon, style, applied, ui)
    }

    fn button(
        &self, icon: &'static str, style: NodeValue, applied: bool, ui: &mut Ui,
    ) -> Option<Event> {
        let t = ui.ctx().get_lb_theme();
        if toolbar_icon(ui, &t, icon, applied, self.toolbar.menu_open, style.name()) {
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
                            ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);

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
                                        phosphor::SEARCH,
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
                                    phosphor::ARROW_COUNTER_CLOCKWISE,
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
                                    phosphor::ARROW_CLOCKWISE,
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
                                    phosphor::TEXT_H_ONE,
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
                                    phosphor::HIGHLIGHTER,
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
                                    phosphor::CHECK_SQUARE,
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
        &mut self, ui: &mut Ui, top_left: Pos2, width: f32, md: &str, icon: &'static str, on: bool,
    ) -> Response {
        let md_height = self.markdown_label_height(md);
        let height = md_height.max(40.);

        let margin = (height - md_height) / 2.;
        let md_top_left = top_left + margin * Vec2::Y;
        self.markdown_label(ui, md_top_left, width, md);

        let padding = (ui.max_rect().width() - width) / 2.;
        let t = ui.ctx().get_lb_theme();
        let resp = ui.allocate_ui_with_layout(
            Vec2::new(width, height),
            Layout::right_to_left(egui::Align::Center),
            |ui| {
                ui.add_space(padding);
                let r = icon_button_glyph(
                    ui,
                    &t,
                    icon,
                    on,
                    t.neutral_bg(),
                    control_height(),
                    CHROME_BAND_GLYPH,
                );
                tip_text(ui.ctx(), &r, md);
                r
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

fn add_seperator(ui: &mut Ui) {
    let t = ui.ctx().get_lb_theme();
    // Fixed to the icon row. Stock Separator in a horizontal layout uses
    // available_height (the leftover editor), which stretched the strip.
    let hit = control_height();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(20.0, hit), egui::Sense::hover());
    let h = hit * 0.7;
    ui.painter().vline(
        rect.center().x,
        rect.center().y - h / 2.0..=rect.center().y + h / 2.0,
        Stroke::new(STROKE_HAIRLINE, t.neutral()),
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
