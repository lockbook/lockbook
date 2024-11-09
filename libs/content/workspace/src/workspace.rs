use core::f32;
use egui::emath::easing;
use egui::os::OperatingSystem;
use egui::{
    vec2, Context, EventFilter, Id, Image, Key, Modifiers, Sense, TextWrapMode, ViewportCommand,
};
use lb_rs::blocking::Lb;
use lb_rs::logic::crypto::DecryptedDocument;
use lb_rs::logic::filename::NameComponents;
use lb_rs::model::errors::{LbErr, LbErrKind};
use lb_rs::model::file::File;
use lb_rs::model::file_metadata::{DocumentHmac, FileType};
use lb_rs::service::sync::{SyncProgress, SyncStatus};
use lb_rs::svg::buffer::Buffer;
use lb_rs::Uuid;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::{mem, thread};

use crate::background::{BackgroundWorker, BwIncomingMsg, Signal};
use crate::output::{DirtynessMsg, Response, WsStatus};
use crate::tab::image_viewer::{is_supported_image_fmt, ImageViewer};
use crate::tab::markdown_editor::Editor as Markdown;
use crate::tab::pdf_viewer::PdfViewer;
use crate::tab::svg_editor::SVGEditor;
use crate::tab::{SaveRequest, Tab, TabContent, TabFailure};
use crate::theme::icons::Icon;
use crate::widgets::Button;

pub struct Workspace {
    pub cfg: WsConfig,

    pub tabs: Vec<Tab>,
    pub active_tab: usize,
    pub active_tab_changed: bool,
    pub user_last_seen: Instant,
    pub last_sync: Instant,
    pub backdrop: Image<'static>,

    pub ctx: Context,
    pub core: Lb,

    pub updates_tx: Sender<WsMsg>,
    pub updates_rx: Receiver<WsMsg>,
    pub background_tx: Sender<BwIncomingMsg>,

    // todo set this in swift as well
    pub focused_parent: Option<Uuid>,
    pub show_tabs: bool,
    pub last_touch_event: Option<Instant>,

    pub status: WsStatus,
    pub out: Response,
}

pub enum WsMsg {
    FileCreated(Result<File, String>),
    FileLoaded(FileLoadedMsg),
    SaveResult(Uuid, Result<SaveResult, LbErr>),
    FileRenamed { id: Uuid, new_name: String },

    BgSignal(Signal),
    SyncMsg(SyncProgress),
    SyncDone(Result<SyncStatus, LbErr>),
    Dirtyness(DirtynessMsg),
}

pub struct FileLoadedMsg {
    id: Uuid,
    is_new_file: bool,
    tab_created: bool,
    content: Result<(Option<DocumentHmac>, DecryptedDocument), TabFailure>,
}

pub struct SaveResult {
    content: String,
    new_hmac: Option<DocumentHmac>,
    completed_at: Instant,
    seq: usize,
}

#[derive(Clone)]
pub struct WsConfig {
    pub data_dir: String,

    pub auto_save: Arc<AtomicBool>,
    pub auto_sync: Arc<AtomicBool>,
    pub zen_mode: Arc<AtomicBool>,
}

impl Default for WsConfig {
    fn default() -> Self {
        Self {
            data_dir: "".to_string(), // todo: potentially a bad idea
            auto_save: Arc::new(AtomicBool::new(true)),
            auto_sync: Arc::new(AtomicBool::new(true)),
            zen_mode: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl WsConfig {
    pub fn new(dir: String, auto_save: bool, auto_sync: bool, zen_mode: bool) -> Self {
        let mut s = Self { data_dir: dir, ..Default::default() };
        s.update(auto_save, auto_sync, zen_mode);
        s
    }

    pub fn update(&mut self, auto_save: bool, auto_sync: bool, zen_mode: bool) {
        self.auto_save.store(auto_save, Ordering::Relaxed);
        self.auto_sync.store(auto_sync, Ordering::Relaxed);
        self.zen_mode.store(zen_mode, Ordering::Relaxed);
    }
}

impl Workspace {
    pub fn new(cfg: WsConfig, core: &Lb, ctx: &Context) -> Self {
        let (updates_tx, updates_rx) = channel();
        let background = BackgroundWorker::new(ctx, &updates_tx);
        let background_tx = background.spawn_worker();
        let status = Default::default();
        let output = Default::default();

        Self {
            cfg,
            tabs: vec![],
            active_tab: 0,
            active_tab_changed: false,
            user_last_seen: Instant::now(),
            last_sync: Instant::now(),
            backdrop: Image::new(egui::include_image!("../lockbook-backdrop.png")),
            ctx: ctx.clone(),
            core: core.clone(),
            updates_rx,
            updates_tx,
            background_tx,
            status,
            show_tabs: true,
            focused_parent: None,
            last_touch_event: None,
            out: output,
        }
    }

    pub fn invalidate_egui_references(&mut self, ctx: &Context, core: &Lb) {
        self.ctx = ctx.clone();
        self.core = core.clone();

        self.backdrop = Image::new(egui::include_image!("../lockbook-backdrop.png"));
        let ids: Vec<lb_rs::Uuid> = self.tabs.iter().map(|tab| tab.id).collect();
        let maybe_active_tab_id = self.current_tab().map(|tab| tab.id);

        while self.active_tab != 0 {
            self.close_tab(self.tabs.len() - 1);
        }

        for id in ids {
            self.open_file(id, false, false)
        }

        if let Some(active_tab_id) = maybe_active_tab_id {
            self.active_tab = self
                .tabs
                .iter()
                .position(|tab| tab.id == active_tab_id)
                .unwrap_or(0);
            self.active_tab_changed = true;
        }
    }

    /// upsert returns true if a tab was created
    pub fn upsert_tab(
        &mut self, id: lb_rs::Uuid, name: &str, path: &str, is_new_file: bool, make_active: bool,
    ) -> bool {
        for (i, tab) in self.tabs.iter().enumerate() {
            if tab.id == id {
                self.tabs[i].name = name.to_string();
                self.tabs[i].path = path.to_string();
                self.tabs[i].failure = None;
                if make_active {
                    self.active_tab = i;
                }
                // tab exists already
                return false;
            }
        }

        let now = Instant::now();

        let new_tab = Tab {
            id,
            rename: None,
            name: name.to_owned(),
            path: path.to_owned(),
            failure: None,
            content: None,
            last_changed: now,
            is_new_file,
            last_saved: now,
            is_saving_or_loading: false,
            load_queued: false,
        };
        self.tabs.push(new_tab);
        if make_active {
            self.active_tab = self.tabs.len() - 1;
            self.active_tab_changed = true;
        }

        // tab was created
        true
    }

    pub fn get_mut_tab_by_id(&mut self, id: lb_rs::Uuid) -> Option<&mut Tab> {
        self.tabs.iter_mut().find(|tab| tab.id == id)
    }

    pub fn is_empty(&self) -> bool {
        self.tabs.is_empty()
    }

    pub fn current_tab(&self) -> Option<&Tab> {
        self.tabs.get(self.active_tab)
    }

    pub fn current_tab_mut(&mut self) -> Option<&mut Tab> {
        self.tabs.get_mut(self.active_tab)
    }

    pub fn current_tab_markdown(&self) -> Option<&Markdown> {
        let current_tab = self.current_tab()?;

        if let Some(TabContent::Markdown(markdown)) = &current_tab.content {
            return Some(markdown);
        }

        None
    }

    pub fn current_tab_markdown_mut(&mut self) -> Option<&mut Markdown> {
        let current_tab = self.current_tab_mut()?;

        if let Some(TabContent::Markdown(markdown)) = &mut current_tab.content {
            return Some(markdown);
        }

        None
    }

    pub fn current_tab_svg_mut(&mut self) -> Option<&mut SVGEditor> {
        let current_tab = self.current_tab_mut()?;

        if let Some(TabContent::Svg(svg)) = &mut current_tab.content {
            return Some(svg);
        }

        None
    }

    pub fn goto_tab_id(&mut self, id: lb_rs::Uuid) -> bool {
        for (i, tab) in self.tabs.iter().enumerate() {
            if tab.id == id {
                self.active_tab = i;
                self.active_tab_changed = true;
                return true;
            }
        }
        false
    }

    pub fn show(&mut self, ui: &mut egui::Ui) -> Response {
        if self.ctx.input(|inp| !inp.raw.events.is_empty()) {
            self.user_last_seen = Instant::now();
        }

        self.set_tooltip_visibility(ui);

        self.process_updates();
        self.process_keys();
        self.status.populate_message();

        if self.is_empty() {
            self.show_empty_workspace(ui);
        } else {
            ui.centered_and_justified(|ui| self.show_tabs(ui));
        }

        if self.cfg.zen_mode.load(Ordering::Relaxed) {
            let mut min = ui.clip_rect().left_bottom();
            min.y -= 37.0; // 37 is approximating the height of the button
            let max = ui.clip_rect().left_bottom();

            let rect = egui::Rect { min, max };
            ui.allocate_ui_at_rect(rect, |ui| {
                let zen_mode_btn = Button::default()
                    .icon(&Icon::TOGGLE_SIDEBAR)
                    .frame(true)
                    .show(ui);
                if zen_mode_btn.clicked() {
                    self.cfg.zen_mode.store(false, Ordering::Relaxed);
                    self.out.settings_updated = true;
                }
                zen_mode_btn.on_hover_text("Show side panel");
            });
        }

        mem::take(&mut self.out)
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
                    .style_mut(|style| style.interaction.tooltip_delay = 0.0);
                self.last_touch_event = None;
            } else {
                self.ctx
                    .style_mut(|style| style.interaction.tooltip_delay = f32::MAX);
            }
        }
    }

    fn show_empty_workspace(&mut self, ui: &mut egui::Ui) {
        ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
            ui.add_space(ui.clip_rect().height() / 3.0);
            ui.add(self.backdrop.clone().fit_to_exact_size(vec2(100.0, 100.0)));

            ui.label(egui::RichText::new("Welcome to your Lockbook").size(40.0));
            ui.label(
                "Right click on your file tree to explore all that your lockbook has to offer",
            );

            ui.add_space(40.0);

            ui.visuals_mut().widgets.inactive.bg_fill = ui.visuals().widgets.active.bg_fill;
            ui.visuals_mut().widgets.hovered.bg_fill = ui.visuals().widgets.active.bg_fill;

            let text_stroke =
                egui::Stroke { color: ui.visuals().extreme_bg_color, ..Default::default() };
            ui.visuals_mut().widgets.inactive.fg_stroke = text_stroke;
            ui.visuals_mut().widgets.active.fg_stroke = text_stroke;
            ui.visuals_mut().widgets.hovered.fg_stroke = text_stroke;

            if Button::default()
                .text("New document")
                .rounding(egui::Rounding::same(3.0))
                .frame(true)
                .show(ui)
                .clicked()
            {
                self.create_file(false);
            }
            if Button::default()
                .text("New drawing")
                .rounding(egui::Rounding::same(3.0))
                .frame(true)
                .show(ui)
                .clicked()
            {
                self.create_file(true);
            }
            ui.visuals_mut().widgets.inactive.fg_stroke =
                egui::Stroke { color: ui.visuals().widgets.active.bg_fill, ..Default::default() };
            ui.visuals_mut().widgets.hovered.fg_stroke =
                egui::Stroke { color: ui.visuals().widgets.active.bg_fill, ..Default::default() };
            if Button::default().text("New folder").show(ui).clicked() {
                self.out.new_folder_clicked = true;
            }
        });
    }

    fn show_tabs(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);

        ui.vertical(|ui| {
            if !self.tabs.is_empty() {
                if self.show_tabs {
                    self.show_tab_strip(ui);
                } else {
                    self.show_mobile_title(ui);
                }
            }

            ui.centered_and_justified(|ui| {
                let mut rename_req = None;
                if let Some(tab) = self.tabs.get_mut(self.active_tab) {
                    if let Some(fail) = &tab.failure {
                        match fail {
                            TabFailure::DeletedFromSync => {
                                ui.vertical_centered(|ui| {
                                    ui.add_space(50.0);
                                    ui.label(&format!(
                                        "This file ({}) was deleted after syncing.",
                                        tab.path
                                    ));
                                });
                            }
                            TabFailure::SimpleMisc(msg) => {
                                ui.label(msg);
                            }
                            TabFailure::Unexpected(msg) => {
                                ui.label(msg);
                            }
                        };
                    } else if let Some(content) = &mut tab.content {
                        match content {
                            TabContent::Markdown(md) => {
                                let resp = md.show(ui);
                                // The editor signals a text change when the buffer is initially
                                // loaded. Since we use that signal to trigger saves, we need to
                                // check that this change was not from the initial frame.
                                if resp.text_updated && md.past_first_frame() {
                                    tab.last_changed = Instant::now();
                                }

                                if let Some(new_name) = resp.suggest_rename {
                                    rename_req = Some((tab.id, new_name))
                                }

                                if resp.text_updated {
                                    self.out.markdown_editor_text_updated = true;
                                }
                                if resp.selection_updated {
                                    // markdown_editor_selection_updated represents a change to the screen position of
                                    // the cursor, which is also updated when scrolling
                                    self.out.markdown_editor_selection_updated = true;
                                }
                                if resp.scroll_updated {
                                    self.out.markdown_editor_scroll_updated = true;
                                }
                            }
                            TabContent::Image(img) => img.show(ui),
                            TabContent::Pdf(pdf) => pdf.show(ui),
                            TabContent::Svg(svg) => {
                                let res = svg.show(ui);
                                if res.request_save {
                                    tab.last_changed = Instant::now();
                                }
                            }
                        };
                    } else {
                        ui.spinner();
                    }
                }
                if let Some(req) = rename_req {
                    self.rename_file(req);
                }
            });
        });
    }

    fn show_mobile_title(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let selectable_label =
                egui::widgets::Button::new(egui::RichText::new(self.tabs[0].name.clone()))
                    .frame(false)
                    .wrap_mode(TextWrapMode::Truncate)
                    .fill(if ui.visuals().dark_mode {
                        egui::Color32::BLACK
                    } else {
                        egui::Color32::WHITE
                    }); // matches iOS native toolbar

            ui.allocate_ui(ui.available_size(), |ui| {
                ui.with_layout(egui::Layout::top_down_justified(egui::Align::LEFT), |ui| {
                    if ui.add(selectable_label).clicked() {
                        self.out.tab_title_clicked = true
                    }
                });
            })
        });
    }

    fn show_tab_strip(&mut self, parent_ui: &mut egui::Ui) {
        let active_tab_changed = self.active_tab_changed;
        self.active_tab_changed = false;

        let mut ui =
            parent_ui.child_ui(parent_ui.painter().clip_rect(), egui::Layout::default(), None);

        let is_tab_strip_visible = self.tabs.len() > 1;
        let cursor = ui
            .horizontal(|ui| {
                egui::ScrollArea::horizontal()
                    .max_width(ui.available_width())
                    .show(ui, |ui| {
                        for (i, maybe_resp) in self
                            .tabs
                            .iter_mut()
                            .enumerate()
                            .map(|(i, t)| {
                                if is_tab_strip_visible {
                                    tab_label(ui, t, self.active_tab == i, active_tab_changed)
                                } else {
                                    None
                                }
                            })
                            .collect::<Vec<Option<TabLabelResponse>>>()
                            .iter()
                            .enumerate()
                        {
                            if let Some(resp) = maybe_resp {
                                match resp {
                                    TabLabelResponse::Clicked => {
                                        if self.active_tab == i {
                                            // we should rename the file.

                                            self.out.tab_title_clicked = true;
                                            let active_name = self.tabs[i].name.clone();

                                            let mut rename_edit_state =
                                                egui::text_edit::TextEditState::default();
                                            rename_edit_state.cursor.set_char_range(Some(
                                                egui::text::CCursorRange {
                                                    primary: egui::text::CCursor::new(
                                                        active_name
                                                            .rfind('.')
                                                            .unwrap_or(active_name.len()),
                                                    ),
                                                    secondary: egui::text::CCursor::new(0),
                                                },
                                            ));
                                            egui::TextEdit::store_state(
                                                ui.ctx(),
                                                egui::Id::new("rename_tab"),
                                                rename_edit_state,
                                            );
                                            self.tabs[i].rename = Some(active_name);
                                        } else {
                                            self.tabs[i].rename = None;
                                            self.active_tab = i;
                                            self.active_tab_changed = true;
                                            self.ctx.send_viewport_cmd(ViewportCommand::Title(
                                                self.tabs[i].name.clone(),
                                            ));
                                            self.out.selected_file = Some(self.tabs[i].id);
                                        }
                                    }
                                    TabLabelResponse::Closed => {
                                        self.close_tab(i);

                                        let title = match self.current_tab() {
                                            Some(tab) => tab.name.clone(),
                                            None => "Lockbook".to_owned(),
                                        };
                                        self.ctx.send_viewport_cmd(ViewportCommand::Title(title));

                                        self.out.selected_file =
                                            self.current_tab().map(|tab| tab.id);
                                    }
                                    TabLabelResponse::Renamed(name) => {
                                        self.tabs[i].rename = None;
                                        let id = self.current_tab().unwrap().id;
                                        if let Some(tab) = self.get_mut_tab_by_id(id) {
                                            if let Some(TabContent::Markdown(md)) = &mut tab.content
                                            {
                                                md.needs_name = false;
                                            }
                                        }
                                        self.rename_file((id, name.clone()));
                                    }
                                }
                                ui.ctx().request_repaint();
                            }
                        }
                    });
                ui.cursor()
            })
            .inner;

        ui.style_mut().animation_time = 2.0;

        let how_on = ui.ctx().animate_bool_with_easing(
            "toolbar_height".into(),
            is_tab_strip_visible,
            easing::cubic_in_out,
        );
        parent_ui.add_space(cursor.height() * how_on);
        ui.set_opacity(how_on);

        if is_tab_strip_visible {
            let end_of_tabs = cursor.min.x;
            let available_width = ui.available_width();
            let sep_stroke = ui.visuals().widgets.noninteractive.bg_stroke;
            ui.painter().hline(
                egui::Rangef { min: end_of_tabs, max: end_of_tabs + available_width },
                cursor.max.y,
                sep_stroke,
            );
        }
    }

    pub fn save_all_tabs(&mut self) {
        for i in 0..self.tabs.len() {
            self.save_tab(i);
        }
    }

    pub fn save_tab(&mut self, i: usize) {
        if let Some(tab) = self.tabs.get_mut(i) {
            if tab.is_dirty() {
                if let Some(save_req) = tab.make_save_request() {
                    if tab.is_saving_or_loading {
                        // we'll just try again next tick
                        return;
                    }
                    tab.is_saving_or_loading = true;

                    let core = self.core.clone();
                    let update_tx = self.updates_tx.clone();
                    let ctx = self.ctx.clone();

                    thread::spawn(move || {
                        let SaveRequest { seq, content, id, old_hmac, safe_write } = save_req;

                        let result = if safe_write {
                            core.safe_write(id, old_hmac, content.clone().into())
                                .map(|new_hmac| SaveResult {
                                    content,
                                    new_hmac: Some(new_hmac),
                                    completed_at: Instant::now(),
                                    seq,
                                })
                        } else {
                            core.write_document(id, content.as_bytes())
                                .map(|_| SaveResult {
                                    content,
                                    new_hmac: None,
                                    completed_at: Instant::now(),
                                    seq,
                                })
                        };

                        // re-read
                        update_tx.send(WsMsg::SaveResult(id, result)).unwrap();
                        ctx.request_repaint();
                    });
                }
            }
        }
    }

    pub fn create_file(&mut self, is_drawing: bool) {
        let core = self.core.clone();
        let update_tx = self.updates_tx.clone();
        let focused_parent = self
            .focused_parent
            .unwrap_or_else(|| core.get_root().unwrap().id);

        thread::spawn(move || {
            let focused_parent = core.get_file_by_id(focused_parent).unwrap();
            let focused_parent = if focused_parent.file_type == FileType::Document {
                focused_parent.parent
            } else {
                focused_parent.id
            };

            let file_format = if is_drawing { "svg" } else { "md" };
            let new_file = NameComponents::from(&format!("untitled.{}", file_format))
                .next_in_children(core.get_children(&focused_parent).unwrap());

            let result = core
                .create_file(new_file.to_name().as_str(), &focused_parent, FileType::Document)
                .map_err(|err| format!("{:?}", err));
            update_tx.send(WsMsg::FileCreated(result)).unwrap();
        });
    }

    pub fn open_file(&mut self, id: Uuid, is_new_file: bool, make_active: bool) {
        let fname = match self.core.get_file_by_id(id) {
            Ok(f) => f.name,
            Err(err) => {
                if let Some(t) = self.tabs.iter_mut().find(|t| t.id == id) {
                    t.failure = match err.kind {
                        LbErrKind::FileNonexistent => Some(TabFailure::DeletedFromSync),
                        _ => Some(err.into()),
                    }
                }
                return;
            }
        };

        let fpath = self.core.get_path_by_id(id).unwrap(); // TODO

        let tab_created = self.upsert_tab(id, &fname, &fpath, is_new_file, make_active);
        let Some(tab) = self.get_mut_tab_by_id(id) else {
            unreachable!("could not find a tab we just created")
        };
        if tab.is_saving_or_loading {
            // if we're already loading when we try to load, load again when the first load completes
            // this guarantees that the tab will eventually be up-to-date
            tab.load_queued = true;
            return;
        }
        tab.is_saving_or_loading = true;
        tab.load_queued = false;

        let core = self.core.clone();
        let ctx = self.ctx.clone();
        let update_tx = self.updates_tx.clone();

        thread::spawn(move || {
            let content = core
                .read_document_with_hmac(id)
                .map_err(|err| TabFailure::Unexpected(format!("{:?}", err))); // todo(steve)
            update_tx
                .send(WsMsg::FileLoaded(FileLoadedMsg { id, is_new_file, tab_created, content }))
                .unwrap();
            ctx.request_repaint();
        });
    }

    pub fn close_tab(&mut self, i: usize) {
        self.save_tab(i);
        self.tabs.remove(i);
        let n_tabs = self.tabs.len();
        self.out.tabs_changed = true;
        if self.active_tab >= n_tabs && n_tabs > 0 {
            self.active_tab = n_tabs - 1;
        }
        self.active_tab_changed = true;
    }

    fn process_keys(&mut self) {
        const COMMAND: Modifiers = Modifiers::COMMAND;
        const SHIFT: Modifiers = Modifiers::SHIFT;
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
        if self.ctx.input_mut(|i| i.consume_key(COMMAND, egui::Key::N)) {
            self.create_file(false);
        }

        // Ctrl-S to save current tab.
        if self.ctx.input_mut(|i| i.consume_key(COMMAND, egui::Key::S)) {
            self.save_tab(self.active_tab);
        }

        // Ctrl-W to close current tab.
        if self.ctx.input_mut(|i| i.consume_key(COMMAND, egui::Key::W)) && !self.is_empty() {
            self.close_tab(self.active_tab);
            self.ctx.send_viewport_cmd(ViewportCommand::Title(
                self.current_tab()
                    .map(|tab| tab.name.as_str())
                    .unwrap_or("Lockbook")
                    .to_owned(),
            ));

            self.out.selected_file = self.current_tab().map(|tab| tab.id);
        }

        // tab navigation
        let mut goto_tab = None;
        self.ctx.input_mut(|input| {
            // Cmd+1 through Cmd+8 to select tab by cardinal index
            for (i, &key) in NUM_KEYS.iter().enumerate().skip(1).take(8) {
                if input.consume_key_exact(COMMAND, key) {
                    goto_tab = Some(i.min(self.tabs.len()) - 1);
                }
            }

            // Cmd+9 to go to last tab
            if input.consume_key_exact(COMMAND, Key::Num9) {
                goto_tab = Some(self.tabs.len() - 1);
            }

            // Cmd+Shift+[ to go to previous tab
            if input.consume_key_exact(COMMAND | SHIFT, Key::OpenBracket) && self.active_tab != 0 {
                goto_tab = Some(self.active_tab - 1);
            }

            // Cmd+Shift+] to go to next tab
            if input.consume_key_exact(COMMAND | SHIFT, Key::CloseBracket)
                && self.active_tab != self.tabs.len() - 1
            {
                goto_tab = Some(self.active_tab + 1);
            }
        });
        if let Some(goto_tab) = goto_tab {
            if self.active_tab != goto_tab {
                self.active_tab_changed = true;
            }

            self.active_tab = goto_tab;

            if let Some((name, id)) = self.current_tab().map(|tab| (tab.name.clone(), tab.id)) {
                self.ctx.send_viewport_cmd(ViewportCommand::Title(name));
                self.out.selected_file = Some(id);
            };
        }
    }

    pub fn process_updates(&mut self) {
        while let Ok(update) = self.updates_rx.try_recv() {
            match update {
                WsMsg::FileLoaded(FileLoadedMsg {
                    id,
                    is_new_file,
                    tab_created,
                    content: load_result,
                }) => {
                    if let Some((name, id)) =
                        self.current_tab().map(|tab| (tab.name.clone(), tab.id))
                    {
                        self.ctx.send_viewport_cmd(ViewportCommand::Title(name));
                        self.out.selected_file = Some(id);
                    };

                    let ctx = self.ctx.clone();
                    let cfg = self.cfg.clone();
                    let core = self.core.clone();
                    let show_tabs = self.show_tabs;

                    if let Some(tab) = self.get_mut_tab_by_id(id) {
                        let (maybe_hmac, bytes) = match load_result {
                            Ok((hmac, bytes)) => (hmac, bytes),
                            Err(err) => {
                                println!("failed to load file: {:?}", err);
                                tab.failure = Some(err);
                                return;
                            }
                        };
                        let ext = tab.name.split('.').last().unwrap_or_default();

                        if is_supported_image_fmt(ext) {
                            tab.content = Some(TabContent::Image(ImageViewer::new(
                                &id.to_string(),
                                ext,
                                &bytes,
                            )));
                        } else if ext == "pdf" {
                            tab.content = Some(TabContent::Pdf(PdfViewer::new(
                                &bytes,
                                &ctx,
                                &cfg.data_dir,
                                !show_tabs, // todo: use settings to determine toolbar visibility
                            )));
                        } else if ext == "svg" {
                            if tab_created {
                                tab.content = Some(TabContent::Svg(SVGEditor::new(
                                    &bytes,
                                    &ctx,
                                    core.clone(),
                                    id,
                                    maybe_hmac,
                                )));
                            } else {
                                match tab.content.as_mut() {
                                    Some(TabContent::Svg(svg)) => {
                                        Buffer::reload(
                                            &mut svg.buffer.elements,
                                            svg.buffer.master_transform,
                                            &svg.buffer.opened_content,
                                            String::from_utf8_lossy(&bytes).as_ref(),
                                        );

                                        svg.buffer.open_file_hmac = maybe_hmac;
                                    }
                                    _ => unreachable!(),
                                };
                            }
                        } else if ext == "md" || ext == "txt" {
                            if tab_created {
                                tab.content = Some(TabContent::Markdown(Markdown::new(
                                    core.clone(),
                                    &String::from_utf8_lossy(&bytes),
                                    id,
                                    maybe_hmac,
                                    is_new_file,
                                    ext != "md",
                                )));
                            } else {
                                match tab.content.as_mut() {
                                    Some(TabContent::Markdown(md)) => {
                                        md.reload(String::from_utf8_lossy(&bytes).into());
                                        md.hmac = maybe_hmac;
                                    }
                                    _ => unreachable!(),
                                };
                            }
                        } else {
                            tab.failure = Some(TabFailure::SimpleMisc(format!(
                                "Unsupported file extension: {}",
                                ext
                            )));
                        };

                        tab.is_saving_or_loading = false;
                        if tab.load_queued {
                            self.open_file(id, false, false);
                        }
                    } else {
                        println!("failed to load file: tab not found");
                    };
                }
                WsMsg::BgSignal(Signal::SaveAll) => {
                    if self.cfg.auto_save.load(Ordering::Relaxed) {
                        self.save_all_tabs();
                    }
                }
                WsMsg::SaveResult(id, result) => {
                    let mut sync = false;
                    if let Some(tab) = self.get_mut_tab_by_id(id) {
                        match result {
                            Ok(SaveResult {
                                content,
                                new_hmac: hmac,
                                completed_at: time_saved,
                                seq,
                            }) => {
                                tab.last_saved = time_saved;
                                match tab.content.as_mut() {
                                    Some(TabContent::Markdown(md)) => {
                                        md.hmac = hmac;
                                        md.buffer.saved(seq, content);
                                    }
                                    Some(TabContent::Svg(svg)) => {
                                        svg.buffer.open_file_hmac = hmac;
                                        svg.buffer.opened_content = content;
                                    }
                                    _ => {}
                                }
                                sync = true; // todo: sync once when saving multiple tabs
                            }
                            Err(err) => {
                                if err.kind == LbErrKind::ReReadRequired {
                                    tab.failure = Some(TabFailure::Unexpected(format!("{:?}", err)))
                                }
                            }
                        }
                        tab.is_saving_or_loading = false;
                        if tab.load_queued {
                            self.open_file(id, false, false);
                        }
                    }
                    if sync {
                        self.perform_sync();
                    }
                }
                WsMsg::BgSignal(Signal::BwDone) => {
                    // todo!
                    // if let Some(s) = &mut self.shutdown {
                    //     s.done_saving = true;
                    //     self.perform_final_sync(ctx);
                    // }
                }
                WsMsg::BgSignal(Signal::MaybeSync) => {
                    if !self.cfg.auto_sync.load(Ordering::Relaxed) {
                        // auto sync disabled
                        continue;
                    }

                    let focused = self.ctx.input(|i| i.focused);

                    if self.user_last_seen.elapsed() < Duration::from_secs(10)
                        && focused
                        && self.last_sync.elapsed() > Duration::from_secs(5)
                    {
                        // the user is active if the app is in the foreground and they've done
                        // something in the last 10 seconds.
                        // during this time sync every 5 seconds
                        self.perform_sync();
                    } else if self.last_sync.elapsed() > Duration::from_secs(60 * 60) {
                        // sync every hour while the user is inactive
                        self.perform_sync()
                    }
                }
                WsMsg::BgSignal(Signal::UpdateStatus) => {
                    self.refresh_sync_status();
                }
                WsMsg::SyncMsg(prog) => self.sync_message(prog),
                WsMsg::FileRenamed { id, new_name } => {
                    self.out.file_renamed = Some((id, new_name.clone()));

                    let mut different_file_type = false;
                    if let Some(tab) = self.get_mut_tab_by_id(id) {
                        different_file_type = !NameComponents::from(&new_name)
                            .extension
                            .eq(&NameComponents::from(&tab.name).extension);

                        tab.name = new_name.clone();
                    }

                    let mut is_tab_active = false;
                    if let Some(tab) = self.current_tab() {
                        if tab.id == id {
                            self.ctx
                                .send_viewport_cmd(ViewportCommand::Title(tab.name.clone()));
                            is_tab_active = true;
                        }
                    }

                    if different_file_type {
                        self.open_file(id, false, is_tab_active);
                    }
                }
                WsMsg::SyncDone(sync_outcome) => self.sync_done(sync_outcome),
                WsMsg::Dirtyness(dirty_msg) => self.dirty_msg(dirty_msg),
                WsMsg::FileCreated(result) => self.out.file_created = Some(result),
            }
        }
    }

    pub fn rename_file(&self, req: (Uuid, String)) {
        let core = self.core.clone();
        let update_tx = self.updates_tx.clone();
        let ctx = self.ctx.clone();

        thread::spawn(move || {
            let (id, new_name) = req;
            core.rename_file(&id, &new_name).unwrap(); // TODO

            update_tx.send(WsMsg::FileRenamed { id, new_name }).unwrap();
            ctx.request_repaint();
        });
    }
}

enum TabLabelResponse {
    Clicked,
    Closed,
    Renamed(String),
}

fn tab_label(
    ui: &mut egui::Ui, t: &mut Tab, is_active: bool, active_tab_changed: bool,
) -> Option<TabLabelResponse> {
    let mut result = None;

    let x_icon = Icon::CLOSE.size(16.0);

    let padding_x = 10.;
    let w = 160.;
    let h = 40.;

    let (tab_label_rect, tab_label_resp) =
        ui.allocate_exact_size((w, h).into(), Sense { click: true, drag: false, focusable: false });

    if is_active {
        ui.painter().rect(
            tab_label_rect,
            0.,
            ui.style().visuals.extreme_bg_color,
            egui::Stroke::NONE,
        );
    };

    if is_active && active_tab_changed {
        tab_label_resp.scroll_to_me(None);
    }

    // renaming
    if let Some(ref mut str) = t.rename {
        let res = ui
            .allocate_ui_at_rect(tab_label_rect, |ui| {
                ui.add(
                    egui::TextEdit::singleline(str)
                        .frame(false)
                        .id(egui::Id::new("rename_tab")),
                )
            })
            .inner;

        if !res.has_focus() && !res.lost_focus() {
            // request focus on the first frame (todo: wrong but works)
            res.request_focus();
        }
        if res.has_focus() {
            // focus lock filter must be set every frame
            ui.memory_mut(|m| {
                m.set_focus_lock_filter(
                    res.id,
                    EventFilter {
                        tab: true, // suppress 'tab' behavior
                        horizontal_arrows: true,
                        vertical_arrows: true,
                        escape: false, // press 'esc' to release focus
                    },
                )
            })
        }

        // submit
        if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            result = Some(TabLabelResponse::Renamed(str.to_owned()));
            // t.rename = None; is done by code processing this response
        }

        // release focus to cancel ('esc' or click elsewhere)
        if res.lost_focus() {
            t.rename = None;
        }
    } else {
        // interact with button rect whether it's shown or not
        let close_button_pos = egui::pos2(
            tab_label_rect.max.x - padding_x - x_icon.size,
            tab_label_rect.center().y - x_icon.size / 2.0,
        );
        let close_button_rect =
            egui::Rect::from_min_size(close_button_pos, egui::vec2(x_icon.size, x_icon.size))
                .expand(2.0);
        let close_button_resp = ui.interact(
            close_button_rect,
            Id::new("tab label close button").with(t.id),
            Sense { click: true, drag: false, focusable: false },
        );

        // touch mode: always show close button
        let touch_mode = matches!(ui.ctx().os(), OperatingSystem::Android | OperatingSystem::IOS);
        let show_close_button =
            touch_mode || tab_label_resp.hovered() || close_button_resp.hovered();

        // draw backgrounds and set cursor icon
        if close_button_resp.hovered() {
            ui.painter().rect(
                close_button_rect,
                2.0,
                ui.visuals().code_bg_color,
                egui::Stroke::NONE,
            );
            ui.output_mut(|o: &mut egui::PlatformOutput| {
                o.cursor_icon = egui::CursorIcon::PointingHand
            });
        } else if tab_label_resp.hovered() {
            ui.output_mut(|o: &mut egui::PlatformOutput| {
                o.cursor_icon = egui::CursorIcon::PointingHand
            });
        }

        // draw text
        let text: egui::WidgetText = (&t.name).into();
        let wrap_width = if show_close_button {
            w - (padding_x * 3. + x_icon.size + 1.)
        } else {
            w - (padding_x * 2.)
        };

        // tooltip contains unelided text
        ui.ctx()
            .style_mut(|s| s.visuals.menu_rounding = (2.).into());
        let tab_label_resp = tab_label_resp.on_hover_ui(|ui| {
            let text = text.clone().into_galley(
                ui,
                Some(TextWrapMode::Extend),
                wrap_width,
                egui::TextStyle::Small,
            );
            ui.add(egui::Label::new(text));
        });

        let text =
            text.into_galley(ui, Some(TextWrapMode::Truncate), wrap_width, egui::TextStyle::Small);
        let text_color = ui.style().interact(&tab_label_resp).text_color();
        let text_pos = egui::pos2(
            tab_label_rect.min.x + padding_x,
            tab_label_rect.center().y - 0.5 * text.size().y,
        );
        ui.painter().galley(text_pos, text, text_color);

        // draw close button icon
        if show_close_button {
            let icon_draw_pos = egui::pos2(
                close_button_rect.center().x - x_icon.size / 2.,
                close_button_rect.center().y - x_icon.size / 2.2,
            );
            let icon: egui::WidgetText = (&x_icon).into();
            let icon_color = if close_button_resp.is_pointer_button_down_on() {
                ui.visuals().widgets.active.bg_fill
            } else {
                ui.visuals().text_color()
            };
            let icon =
                icon.into_galley(ui, Some(TextWrapMode::Extend), wrap_width, egui::TextStyle::Body);
            ui.painter().galley(icon_draw_pos, icon, icon_color);
        }

        // respond to input
        if close_button_resp.clicked() || tab_label_resp.middle_clicked() {
            result = Some(TabLabelResponse::Closed);
        } else if tab_label_resp.clicked() {
            result = Some(TabLabelResponse::Clicked);
        }
    }

    // draw separators
    let sep_stroke = ui.visuals().widgets.noninteractive.bg_stroke;
    if !is_active {
        ui.painter()
            .hline(tab_label_rect.x_range(), tab_label_rect.max.y, sep_stroke);
    }
    ui.painter()
        .vline(tab_label_rect.max.x, tab_label_rect.y_range(), sep_stroke);

    result
}

// The only difference from count_and_consume_key is that here we use matches_exact instead of matches_logical,
// preserving the behavior before egui 0.25.0. The documentation for the 0.25.0 count_and_consume_key says
// "you should match most specific shortcuts first", but this doesn't go well with egui's usual pattern where widgets
// process input in the order in which they're drawn, with parent widgets (e.g. workspace) drawn before children
// (e.g. editor). Using this older way of doing things affects matching keyboard shortcuts with shift included e.g. '+'
trait InputStateExt {
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
