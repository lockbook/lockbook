use basic_human_duration::ChronoHumanDuration;
use egui::os::OperatingSystem;
use egui::{Key, Modifiers, ViewportCommand};
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use std::mem;
use std::sync::{Arc, Mutex};
use web_time::{Duration, Instant};

use crate::file_cache::{FilesExt as _, link_id, split_fragment};
use crate::output::Response;
use crate::search::SearchType;
use crate::tab::{ExtendedInput as _, ExtendedOutput as _, image_viewer};
use crate::theme::palette_v2::ThemeExt;
use crate::theme::visuals;
use crate::widgets::GlyphonLabel;
use crate::widgets::glyphon_cache::GlyphonCache;
use crate::workspace::Workspace;
use lb_rs::Uuid;

pub const NEW_NOTE_SHORTCUT: egui::KeyboardShortcut =
    egui::KeyboardShortcut::new(Modifiers::COMMAND, Key::N);
pub const SEARCH_SHORTCUT: egui::KeyboardShortcut =
    egui::KeyboardShortcut::new(Modifiers::COMMAND, Key::O);

impl Workspace {
    pub fn show(&mut self, ui: &mut egui::Ui) -> Response {
        visuals::apply(ui.style_mut());

        if let Some(cache) = self
            .ctx
            .data(|d| d.get_temp::<Arc<Mutex<GlyphonCache>>>(egui::Id::NULL))
        {
            cache.lock().unwrap().begin_frame();
        }
        self.images.begin_frame();
        self.tabs.begin_frame();
        for slot in &self.tab_strip {
            self.tabs.promote(&slot.id);
        }

        if self.ctx.input(|inp| !inp.raw.events.is_empty()) || self.ctx.has_events() {
            self.core.app_foregrounded();
        }

        self.set_tooltip_visibility(ui);

        self.take_link_sheet_input();
        self.process_bg_tasks();
        self.process_lb_updates();
        self.process_task_updates();
        self.process_keys();
        self.process_clip_events();
        self.apply_pending_reveal();

        self.set_user_present();

        if self.is_empty() {
            self.show_landing_page(ui);

            self.landing_page_first_frame = false;
        } else {
            ui.centered_and_justified(|ui| self.show_open_tab(ui));
            self.landing_page_first_frame = true;
        }
        // A gesture no editor took this frame is not owed to the next one.
        self.ctx.pop_events_where(&mut |e| {
            matches!(
                e,
                crate::tab::Event::Markdown(crate::tab::markdown_editor::input::Event::Gesture(_))
            )
        });
        self.update_window_title();
        self.keep_note_index();
        if self.out.tabs_changed || self.current_tab_changed {
            self.cfg
                .set_tabs(&self.tab_strip, &self.current_tab, &self.last_search);
            self.current_tab_changed = false;
        }

        let zoom = self.ctx.zoom_factor();
        if zoom != self.cfg.get_zoom_factor() {
            self.cfg.set_zoom_factor(zoom);
        }

        if let Some(cache) = self
            .ctx
            .data(|d| d.get_temp::<Arc<Mutex<GlyphonCache>>>(egui::Id::NULL))
        {
            cache.lock().unwrap().end_frame();
        }
        self.images.end_frame();
        self.tabs.end_frame();

        mem::take(&mut self.out)
    }

    fn update_window_title(&mut self) {
        let title = self
            .current_tab_title()
            .unwrap_or_else(|| "Lockbook".to_string());
        if self.last_set_title.as_ref() != Some(&title) {
            self.last_set_title = Some(title.clone());
            self.ctx.send_viewport_cmd(ViewportCommand::Title(title));
        }
    }

    fn set_tooltip_visibility(&mut self, ui: &mut egui::Ui) {
        let has_touch = ui.input(|r| {
            r.events.iter().any(|e| {
                matches!(e, egui::Event::Touch { device_id: _, id: _, phase: _, pos: _, force: _ })
            })
        });
        if has_touch && self.last_touch_event.is_none() {
            self.last_touch_event = Some(Instant::now());
        }

        if let Some(last_touch_event) = self.last_touch_event {
            if Instant::now() - last_touch_event > Duration::from_secs(5) {
                self.ctx
                    .global_style_mut(|style| style.interaction.tooltip_delay = 0.0);
                self.last_touch_event = None;
            } else {
                self.ctx
                    .global_style_mut(|style| style.interaction.tooltip_delay = f32::MAX);
            }
        }
    }

    fn show_open_tab(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);

        ui.vertical(|ui| {
            ui.centered_and_justified(|ui| {
                self.show_current_tab_content(ui);

                // Links in notes open through their resolver; this catches the
                // rest. An id link to a file here opens in the app, and `lb:`
                // never reaches the OS.
                let mut open_ids: Vec<(Uuid, Option<String>, bool)> = Vec::new();
                let files = Arc::clone(&self.files);
                let files = files.read().unwrap();
                ui.ctx().output_mut(|w| {
                    w.commands.retain(|c| {
                        let egui::OutputCommand::OpenUrl(url) = c else { return true };
                        match link_id(&url.url) {
                            Some(id) if files.get_by_id(id).is_some_and(|f| f.is_document()) => {
                                let heading = split_fragment(&url.url).1.map(String::from);
                                open_ids.push((id, heading, url.new_tab));
                                false
                            }
                            _ => !url.url.to_ascii_lowercase().starts_with("lb:"),
                        }
                    });
                });
                drop(files);
                for (id, heading, new_tab) in open_ids {
                    self.open_link(id, heading, new_tab);
                }
                for (id, heading, new_tab) in ui.ctx().pop_open_files() {
                    self.open_link(id, heading, new_tab);
                }
                for (id, range, new_tab) in ui.ctx().pop_open_ranges() {
                    if new_tab {
                        self.open_file_at_range(id, range, true);
                    } else {
                        self.navigate_to_range(id, range);
                    }
                }
                for (id, range, new_tab) in ui.ctx().pop_link_ats() {
                    self.open_link_at(id, range, new_tab);
                }
                for (parent, names) in ui.ctx().pop_create_notes() {
                    self.create_note_at(parent, &names);
                }
                let sheets = ui.ctx().pop_link_sheets();
                if !sheets.is_empty() && self.link_sheets.is_empty() {
                    // the keyboard and the text selection's handles would cover it
                    ui.ctx().set_virtual_keyboard_shown(false);
                    self.link_sheet_tab = self.current_tab_id();
                }
                self.link_sheets.extend(sheets);
                self.show_link_sheet(ui.ctx());
            });
        });
    }

    /// Whether a link sheet covers the workspace; it takes every touch.
    pub fn link_sheet_open(&self) -> bool {
        !self.link_sheets.is_empty()
    }

    /// While a link sheet is up it takes all input: it reads its own keys,
    /// and nothing else in the workspace or the tab beneath sees any. Sheets
    /// close when their tab stops being current.
    fn take_link_sheet_input(&mut self) {
        if self.link_sheet_tab != self.current_tab_id() {
            self.link_sheets.clear();
        }
        self.link_sheet_keys = (false, false);
        if self.link_sheets.is_empty() {
            return;
        }
        self.link_sheet_keys = self.ctx.input_mut(|i| {
            let open = i.consume_key(Modifiers::COMMAND, Key::Enter);
            let cancel = i.consume_key(Modifiers::NONE, Key::Escape);
            i.events.clear();
            (open, cancel)
        });
    }

    /// Explains a link the reader tried to open, opening its file if they
    /// confirm.
    fn show_link_sheet(&mut self, ctx: &egui::Context) {
        let Some(sheet) = self.link_sheets.front().cloned() else { return };
        let (open, close) = link_sheet_modal(ctx, &sheet, self.link_sheet_keys);
        if let Some((id, new_tab)) = sheet.open.filter(|_| open) {
            self.open_link(id, sheet.heading.clone(), new_tab);
        }
        if close {
            self.link_sheets.pop_front();
            // sheets still queued from the same request follow it to the new tab
            self.link_sheet_tab = self.current_tab_id();
        }
    }

    fn show_current_tab_content(&mut self, ui: &mut egui::Ui) {
        // Search renders here (not via `Tab::show`) so its preview pane can use
        // the workspace's async file loader.
        if matches!(self.current_dest(), Some(crate::tab::Destination::Search(_))) {
            self.show_search_tab(ui);
            return;
        }

        let software_keyboard = self.software_keyboard_in_use;
        // iOS sets the tab policy from the size class and Android never does; compact
        // has no room for a page sidebar
        let compact = !self.desktop_tab_policy;
        let session = self.current_tab;
        if let Some(tab) = session.and_then(|sid| self.tabs.get_mut(&sid)) {
            if let Some(pdf) = tab.pdf_mut() {
                pdf.compact = compact;
            }
            // attaching or detaching a hardware keyboard toggles this while a doc is open
            if let Some(md) = tab.markdown_mut() {
                md.edit.phone_mode = md.edit.renderer.touch_mode && software_keyboard;
            }

            let resp = self.open_guard.show(tab, |tab| tab.show(ui));
            if let Some(id) = resp.open_anyway {
                self.open_anyway(id, session);
            }

            self.out.open_camera = resp.open_camera;
            if resp.text_updated {
                self.out.markdown_editor_text_updated = true;
                self.out.markdown_editor_selection_updated = true;
            }
            if resp.selection_updated {
                self.out.markdown_editor_selection_updated = true;
            }
            self.out.text_interaction_rect = resp.text_interaction_rect;
            self.out.mobile_toolbar_shown = resp.mobile_toolbar_shown;
            self.out.caret_covered = resp.caret_covered;
            if resp.scroll_updated {
                self.out.markdown_editor_scroll_updated = true;
            }
            if let Some(file) = resp.open_file {
                self.navigate_to(crate::tab::Destination::File(file));
            }
        }
    }

    fn process_keys(&mut self) {
        const APPLE: bool = cfg!(target_vendor = "apple");
        const COMMAND: Modifiers = Modifiers::COMMAND;
        const CTRL: Modifiers = Modifiers::CTRL;
        const SHIFT: Modifiers = Modifiers::SHIFT;
        const ALT: Modifiers = Modifiers::ALT;
        const NUM_KEYS: [Key; 10] = [
            Key::Num0,
            Key::Num1,
            Key::Num2,
            Key::Num3,
            Key::Num4,
            Key::Num5,
            Key::Num6,
            Key::Num7,
            Key::Num8,
            Key::Num9,
        ];

        // Ctrl-N pressed while new file modal is not open.
        if self.ctx.input_mut(|i| {
            i.consume_key_exact(NEW_NOTE_SHORTCUT.modifiers, NEW_NOTE_SHORTCUT.logical_key)
        }) {
            self.create_doc(false);
        }

        // Ctrl-S to save current tab.
        if self
            .ctx
            .input_mut(|i| i.consume_key_exact(COMMAND, egui::Key::S))
        {
            if let Some(idx) = self.current_slot_index() {
                self.save_tab(idx);
            }
        }

        // Ctrl-M to open mind map
        if self
            .ctx
            .input_mut(|i| i.consume_key_exact(COMMAND, egui::Key::M))
        {
            self.upsert_mind_map(self.core.clone());
        }

        if self
            .ctx
            .input_mut(|i| i.consume_key_exact(COMMAND | SHIFT, egui::Key::F))
        {
            self.upsert_search(Some(SearchType::Content));
        }

        if self.ctx.input_mut(|i| {
            i.consume_key_exact(SEARCH_SHORTCUT.modifiers, SEARCH_SHORTCUT.logical_key)
        }) {
            self.upsert_search(Some(SearchType::Path));
        }

        // Ctrl-W to close current tab, or return to root when on landing page.
        if self
            .ctx
            .input_mut(|i| i.consume_key_exact(COMMAND, egui::Key::W))
        {
            if !self.is_empty() {
                if let Some(idx) = self.current_slot_index() {
                    self.close_tab(idx);
                }
                self.out.selected_file = self.current_tab_id();
            } else {
                let root_id = self.files.read().unwrap().root().id;
                self.focused_parent = None;
                self.out.selected_file = Some(root_id);
            }
        }

        // Ctrl-shift-W to close all tabs
        if self
            .ctx
            .input_mut(|i| i.consume_key_exact(COMMAND | SHIFT, egui::Key::W))
            && !self.is_empty()
        {
            self.close_all_tabs();
            self.out.selected_file = None;
        }

        // Ctrl-shift-T to reopen the most recently closed tab
        if self
            .ctx
            .input_mut(|i| i.consume_key_exact(COMMAND | SHIFT, egui::Key::T))
        {
            self.reopen_closed_tab();
            self.out.selected_file = self.current_tab_id();
        }

        // reorder tabs
        // non-apple: ctrl+shift+pg down / up
        // apple: command+control+shift [ ]
        let change: i32 = self.ctx.input_mut(|input| {
            if APPLE {
                if input.consume_key_exact(Modifiers::MAC_CMD | CTRL | SHIFT, Key::OpenCurlyBracket)
                {
                    -1
                } else if input
                    .consume_key_exact(Modifiers::MAC_CMD | CTRL | SHIFT, Key::CloseCurlyBracket)
                {
                    1
                } else {
                    0
                }
            } else if input.consume_key_exact(CTRL | SHIFT, Key::PageUp) {
                -1
            } else if input.consume_key_exact(CTRL | SHIFT, Key::PageDown) {
                1
            } else {
                0
            }
        });
        if change != 0 {
            let current_idx = self.current_slot_index();
            if let Some(old) = current_idx {
                let new = old as i32 + change;
                if new >= 0 && new < self.tab_strip.len() as i32 {
                    self.tab_strip.swap(old, new as usize);
                    self.make_current(new as usize);
                }
            }
        }

        // tab navigation
        let completions_active = self
            .current_tab_markdown()
            .is_some_and(|md| md.edit.emoji_completions.active || md.edit.link_completions.active);
        // The search tab claims Cmd+1–9 to quick-open results, so don't let the
        // workspace consume them for tab switching while search is showing.
        let search_active = matches!(self.current_dest(), Some(crate::tab::Destination::Search(_)));
        let current_idx = self.current_slot_index().unwrap_or(0);
        let mut goto_tab = None;
        self.ctx.input_mut(|input| {
            // Cmd+1 through Cmd+8 to select tab by cardinal index
            for (i, &key) in NUM_KEYS.iter().enumerate().skip(1).take(8) {
                let cmd_consumed =
                    !completions_active && !search_active && input.consume_key_exact(COMMAND, key);
                let alt_consumed = !APPLE && input.consume_key_exact(Modifiers::ALT, key);
                if cmd_consumed || alt_consumed {
                    goto_tab = Some(i.min(self.tab_strip.len()) - 1);
                    if alt_consumed {
                        let digit = char::from_digit(i as u32, 10).unwrap().to_string();
                        // kinda wack, could fix in clients/desktop but maybe we'll want that to
                        // need the macOS beahvior one day
                        // https://github.com/emilk/egui/issues/5338
                        // https://github.com/emilk/egui/pull/5347
                        input
                            .events
                            .retain(|e| !matches!(e, egui::Event::Text(t) if *t == digit));
                    }
                }
            }

            // Cmd+9 to go to last tab
            let cmd9_consumed = !completions_active
                && !search_active
                && input.consume_key_exact(COMMAND, Key::Num9);
            let alt9_consumed = !APPLE && input.consume_key_exact(Modifiers::ALT, Key::Num9);
            if (cmd9_consumed || alt9_consumed) && !self.tab_strip.is_empty() {
                goto_tab = Some(self.tab_strip.len() - 1);
                if alt9_consumed {
                    input
                        .events
                        .retain(|e| !matches!(e, egui::Event::Text(t) if *t == "9"));
                }
            }

            // Cmd+Shift+[ or ctrl shift tab to go to previous tab
            if ((APPLE && input.consume_key_exact(COMMAND | SHIFT, Key::OpenCurlyBracket))
                || (!APPLE && input.consume_key_exact(CTRL | SHIFT, Key::Tab)))
                && current_idx > 0
            {
                goto_tab = Some(current_idx - 1);
            }

            // Cmd+Shift+] or ctrl tab to go to next tab
            if ((APPLE && input.consume_key_exact(COMMAND | SHIFT, Key::CloseCurlyBracket))
                || (!APPLE && input.consume_key_exact(CTRL, Key::Tab)))
                && current_idx + 1 < self.tab_strip.len()
            {
                goto_tab = Some(current_idx + 1);
            }
        });

        if let Some(goto_tab) = goto_tab {
            self.make_current(goto_tab);
        }

        // forward/back
        // non-apple: alt + arrows
        // apple: command + brackets
        let mut back = false;
        let mut forward = false;
        self.ctx.input_mut(|input| {
            if APPLE {
                if input.consume_key_exact(COMMAND, Key::OpenBracket) {
                    back = true;
                }
                if input.consume_key_exact(COMMAND, Key::CloseBracket) {
                    forward = true;
                }
            } else {
                if input.consume_key_exact(ALT, Key::ArrowLeft) {
                    back = true;
                }
                if input.consume_key_exact(ALT, Key::ArrowRight) {
                    forward = true;
                }
            }
        });

        if back {
            self.back();
        }
        if forward {
            self.forward();
        }
    }
}

// The only difference from count_and_consume_key is that here we use matches_exact instead of matches_logical,
// preserving the behavior before egui 0.25.0. The documentation for the 0.25.0 count_and_consume_key says
// "you should match most specific shortcuts first", but this doesn't go well with egui's usual pattern where widgets
// process input in the order in which they're drawn, with parent widgets (e.g. workspace) drawn before children
// (e.g. editor). Using this older way of doing things affects matching keyboard shortcuts with shift included e.g. '+'
pub trait InputStateExt {
    fn count_and_consume_key_exact(
        &mut self, modifiers: egui::Modifiers, logical_key: egui::Key,
    ) -> usize;
    fn consume_key_exact(&mut self, modifiers: egui::Modifiers, logical_key: egui::Key) -> bool;
}

impl InputStateExt for egui::InputState {
    fn count_and_consume_key_exact(
        &mut self, modifiers: egui::Modifiers, logical_key: egui::Key,
    ) -> usize {
        let mut count = 0usize;

        self.events.retain(|event| {
            let is_match = matches!(
                event,
                egui::Event::Key {
                    key: ev_key,
                    modifiers: ev_mods,
                    pressed: true,
                    ..
                } if *ev_key == logical_key && ev_mods.matches_exact(modifiers)
            );

            count += is_match as usize;

            !is_match
        });

        count
    }

    fn consume_key_exact(&mut self, modifiers: egui::Modifiers, logical_key: egui::Key) -> bool {
        self.count_and_consume_key_exact(modifiers, logical_key) > 0
    }
}

pub trait ElapsedHumanString {
    fn elapsed_human_string(&self) -> String;
}

impl ElapsedHumanString for time::Duration {
    fn elapsed_human_string(&self) -> String {
        let minutes = self.whole_minutes();
        let seconds = self.whole_seconds();
        if seconds > 0 && minutes == 0 {
            if seconds <= 1 { "1 second ago".to_string() } else { format!("{seconds} seconds ago") }
        } else {
            self.format_human().to_string()
        }
    }
}

impl ElapsedHumanString for std::time::Duration {
    fn elapsed_human_string(&self) -> String {
        time::Duration::milliseconds(self.as_millis() as _).elapsed_human_string()
    }
}

impl ElapsedHumanString for Instant {
    fn elapsed_human_string(&self) -> String {
        time::Duration::milliseconds(self.elapsed().as_millis() as _).elapsed_human_string()
    }
}

#[derive(PartialEq, Clone, Copy, Serialize, Deserialize)]
pub enum DocType {
    PlainText,
    Markdown,
    SVG,
    Image,
    ImageUnsupported,
    Code,
    PDF,
    Chat,
    Unknown,
}

impl Display for DocType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DocType::PlainText => write!(f, "Plain Text"),
            DocType::Markdown => write!(f, "Markdown"),
            DocType::SVG => write!(f, "SVG"),
            DocType::Image => write!(f, "Image"),
            DocType::ImageUnsupported => write!(f, "Image (Unsupported)"),
            DocType::Code => write!(f, "Code"),
            DocType::PDF => write!(f, "PDF"),
            DocType::Chat => write!(f, "Chat"),
            DocType::Unknown => write!(f, "Unknown"),
        }
    }
}

impl DocType {
    pub fn from_name(name: &str) -> Self {
        let ext = name
            .split('.')
            .next_back()
            .unwrap_or_default()
            .to_lowercase();
        match ext.as_str() {
            "draw" | "svg" => Self::SVG,
            "md" => Self::Markdown,
            "txt" => Self::PlainText,
            "cr2" => Self::ImageUnsupported,
            "pdf" => Self::PDF,
            "chat" => Self::Chat,
            _ if image_viewer::is_supported_image_fmt(&ext) => Self::Image,
            _ if crate::syntax::Lang::from_extension(&ext).is_some() => Self::Code,
            _ => Self::Unknown,
        }
    }

    pub fn hide_ext(&self) -> bool {
        match self {
            DocType::PlainText => false,
            DocType::Markdown => true,
            DocType::SVG => true,
            DocType::Image => false,
            DocType::ImageUnsupported => false,
            DocType::Code => false,
            DocType::PDF => true,
            DocType::Chat => true,
            DocType::Unknown => false,
        }
    }

    /// Returns the file name with the extension stripped when `hide_ext()` is true.
    pub fn display_name<'a>(&self, name: &'a str) -> &'a str {
        if self.hide_ext() {
            std::path::Path::new(name)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or(name)
        } else {
            name
        }
    }
}

/// The sheet explaining a link: whether the reader chose to open it, and
/// whether the sheet closes. Design-system sheet chrome: dim, canvas plate,
/// quiet dismiss, primary Open. Centered, clear of native bars and the
/// keyboard; near-full width on touch or narrow screens, where touch gets
/// taller buttons and no key badges.
pub(crate) fn link_sheet_modal(
    ctx: &egui::Context, sheet: &crate::tab::LinkSheet, (open_key, cancel_key): (bool, bool),
) -> (bool, bool) {
    use crate::style::chrome::{
        Radius, control_height, file_row_icon, shortcut_esc, shortcut_return,
    };
    use crate::style::tree_metrics::INDENT_BASE;
    use crate::style::{Button, FileRow, Space, Spacer, TypeRole, parent_crumbs, with_pad_fit};

    let t = ctx.get_lb_theme();
    let screen = ctx.content_rect();
    let touch = matches!(ctx.os(), OperatingSystem::Android | OperatingSystem::IOS);
    let compact = touch || screen.width() < 520.0;
    let pad = Space::Md.pts();
    let width = if compact { (screen.width() - pad * 4.0).min(480.0) } else { 320.0 };
    let button_h = if touch { control_height() * 1.4 } else { control_height() };
    let layer = egui::LayerId::new(egui::Order::Foreground, egui::Id::unique("link_sheet"));

    let mut dismissed = false;
    egui::Area::new(egui::Id::unique("link_sheet_dim"))
        .order(egui::Order::Middle)
        .fixed_pos(screen.min)
        .fade_in(false)
        .show(ctx, |ui| {
            let (rect, resp) = ui.allocate_exact_size(screen.size(), egui::Sense::click());
            ui.painter()
                .rect_filled(rect, 0.0, egui::Color32::from_black_alpha(40));
            let on_sheet = || {
                ui.ctx()
                    .pointer_interact_pos()
                    .is_some_and(|pos| ui.ctx().layer_id_at(pos) == Some(layer))
            };
            dismissed = resp.clicked() && !on_sheet();
        });

    let (mut open, mut cancel) = (open_key && sheet.open.is_some(), cancel_key);
    let title = if sheet.open.is_some() { "Open this link?" } else { "This link can't open" };
    let mut lines = sheet.message.lines();
    let why = lines.next().unwrap_or_default();
    egui::Area::new(layer.id)
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            crate::style::plate_content_shadow(
                ui,
                t.neutral_bg(),
                crate::style::overlay_shadow(&t),
                Radius::Surface.corner(),
                |ui| {
                    ui.set_width(width + pad * 2.0);
                    ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
                    with_pad_fit(ui, Space::Md, |ui| {
                        ui.label(
                            TypeRole::Heading
                                .rich(title)
                                .color(t.neutral_fg_secondary()),
                        );
                        ui.add(Spacer::new(Space::Md));
                        let line = |ui: &mut egui::Ui, text: &str, ink| {
                            ui.add(
                                GlyphonLabel::new_rich(crate::resolvers::rich_spans(text), ink)
                                    .font_size(TypeRole::Body.size())
                                    .line_height(TypeRole::Body.line_height())
                                    .max_width(width),
                            );
                        };
                        if sheet.open.is_some() {
                            // the file it would open, as the file tree shows one
                            let path = &sheet.shown;
                            let name = path.rsplit('/').find(|s| !s.is_empty()).unwrap_or(path);
                            let size = egui::vec2(width, FileRow::height_for(true));
                            let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
                            // the row's indent makes room for a hover fill this row never shows
                            let rect = rect.with_min_x(rect.min.x - INDENT_BASE);
                            FileRow::new(&t, name)
                                .icon(file_row_icon(name, false))
                                .subtitle(parent_crumbs(path))
                                .interactive(false)
                                .paint_at_hit(ui, rect, rect, layer.id.with("file"));
                        } else {
                            // nothing resolved: the link as written
                            line(ui, &format!("**{}**", sheet.shown), t.neutral_fg());
                        }
                        ui.add(Spacer::new(Space::Sm));
                        line(ui, why, t.neutral_fg());
                        for fix in lines {
                            ui.add(Spacer::new(Space::Xs));
                            line(ui, fix, t.neutral_fg_secondary());
                        }
                        ui.add(Spacer::new(Space::Md));
                        ui.horizontal(|ui| {
                            let dismiss = if sheet.open.is_some() { "Cancel" } else { "Close" };
                            let dismiss = if touch {
                                Button::secondary(&t, dismiss)
                            } else {
                                Button::quiet(&t, dismiss).shortcut(shortcut_esc())
                            }
                            .height(button_h);
                            cancel |= dismiss.show(ui).clicked();
                            if sheet.open.is_none() {
                                return;
                            }
                            let layout = egui::Layout::right_to_left(egui::Align::Center);
                            ui.with_layout(layout, |ui| {
                                let mut primary = Button::primary(&t, "Open").height(button_h);
                                if !touch {
                                    primary = primary.shortcut(shortcut_return());
                                }
                                open |= primary.show(ui).clicked();
                            });
                        });
                    });
                },
            );
        });
    (open, open || cancel || dismissed)
}
