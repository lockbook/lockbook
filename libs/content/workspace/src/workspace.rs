use egui::{vec2, Color32, Context, Image};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::{mem, thread};

use crate::background::{BackgroundWorker, BwIncomingMsg, Signal};
use crate::output::{DirtynessMsg, WsOutput, WsStatus};
use crate::tab::image_viewer::{is_supported_image_fmt, ImageViewer};
use crate::tab::markdown_editor::Markdown;
use crate::tab::pdf_viewer::PdfViewer;
use crate::tab::svg_editor::SVGEditor;
use crate::tab::{Tab, TabContent, TabFailure};
use crate::theme::icons::Icon;
use crate::widgets::{separator, Button, ToolBarVisibility};
use lb_rs::{File, FileType, LbError, NameComponents, SyncProgress, SyncStatus, Uuid};

pub struct Workspace {
    pub cfg: WsConfig,

    pub tabs: Vec<Tab>,
    pub active_tab: usize,
    pub backdrop: Image<'static>,

    pub ctx: Context,
    pub core: lb_rs::Core,

    pub updates_tx: Sender<WsMsg>,
    pub updates_rx: Receiver<WsMsg>,
    pub background_tx: Sender<BwIncomingMsg>,

    // todo set this in swift as well
    pub focused_parent: Option<Uuid>,
    pub show_tabs: bool,
    pub last_touch_event: Option<Instant>,

    pub status: WsStatus,
    pub out: WsOutput,
}

pub enum WsMsg {
    FileCreated(Result<File, String>),
    FileLoaded(Uuid, Result<TabContent, TabFailure>),
    SaveResult(Uuid, Result<Instant, LbError>),
    FileRenamed { id: Uuid, new_name: String },

    BgSignal(Signal),
    SyncMsg(SyncProgress),
    SyncDone(Result<SyncStatus, LbError>),
    Dirtyness(DirtynessMsg),
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
    pub fn new(cfg: WsConfig, core: &lb_rs::Core, ctx: &Context) -> Self {
        let (updates_tx, updates_rx) = channel();
        let background = BackgroundWorker::new(ctx, &updates_tx);
        let background_tx = background.spawn_worker();
        let status = Default::default();
        let output = Default::default();

        Self {
            cfg,
            tabs: vec![],
            active_tab: 0,
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

    #[cfg(target_os = "android")]
    pub fn invalidate_egui_references(&mut self, ctx: &Context, core: &lb_rs::Core) {
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
        }
    }

    pub fn upsert_tab(
        &mut self, id: lb_rs::Uuid, name: &str, path: &str, is_new_file: bool, make_active: bool,
    ) {
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
        };

        for (i, tab) in self.tabs.iter().enumerate() {
            if tab.id == id {
                self.tabs[i] = new_tab;
                if make_active {
                    self.active_tab = i;
                }
                return;
            }
        }

        self.tabs.push(new_tab);
        if make_active {
            self.active_tab = self.tabs.len() - 1;
        }
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
                return true;
            }
        }
        false
    }

    pub fn goto_tab(&mut self, i: usize) {
        if i == 0 || self.tabs.is_empty() {
            return;
        }
        let n_tabs = self.tabs.len();
        self.active_tab = if i == 9 || i >= n_tabs { n_tabs - 1 } else { i - 1 };
    }

    /// called by custom integrations
    pub fn draw(&mut self, ctx: &Context) -> WsOutput {
        egui_extras::install_image_loaders(ctx);

        let fill = if ctx.style().visuals.dark_mode { Color32::BLACK } else { Color32::WHITE };
        egui::CentralPanel::default()
            .frame(egui::Frame::default().fill(fill))
            .show(ctx, |ui| self.show_workspace(ui))
            .inner
    }

    pub fn show_workspace(&mut self, ui: &mut egui::Ui) -> WsOutput {
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

            separator(ui);

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

                                if let Some(new_name) = resp.suggested_rename {
                                    rename_req = Some((tab.id, new_name))
                                }

                                if resp.hide_virtual_keyboard {
                                    self.out.hide_virtual_keyboard = resp.hide_virtual_keyboard;
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
                                    self.out.markdown_editor_scroll_updated = true
                                }
                            }
                            TabContent::Image(img) => img.show(ui),
                            TabContent::Pdf(pdf) => pdf.show(ui),
                            TabContent::Svg(svg) => {
                                svg.show(ui);
                                tab.last_changed = Instant::now();
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
            let selectable_label = egui::widgets::Button::new(
                egui::RichText::new(self.tabs[0].name.clone()).size(30.0),
            )
            .frame(false)
            .fill(egui::Color32::TRANSPARENT);

            ui.allocate_ui(ui.available_size(), |ui| {
                ui.with_layout(egui::Layout::top_down_justified(egui::Align::LEFT), |ui| {
                    if ui.add(selectable_label).clicked() {
                        self.out.tab_title_clicked = true
                    }
                });
            })
        });
    }

    fn show_tab_strip(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            egui::ScrollArea::horizontal()
                .max_width(ui.available_width())
                .show(ui, |ui| {
                    for (i, maybe_resp) in self
                        .tabs
                        .iter_mut()
                        .enumerate()
                        .map(|(i, t)| (tab_label(ui, t, self.active_tab == i)))
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
                                        self.out.window_title = Some(self.tabs[i].name.clone());
                                        self.out.selected_file = Some(self.tabs[i].id);
                                    }
                                }
                                TabLabelResponse::Closed => {
                                    self.close_tab(i);
                                    self.out.window_title = Some(match self.current_tab() {
                                        Some(tab) => tab.name.clone(),
                                        None => "Lockbook".to_owned(),
                                    });

                                    self.out.selected_file = self.current_tab().map(|tab| tab.id);
                                }
                                TabLabelResponse::Renamed(name) => {
                                    self.tabs[i].rename = None;
                                    let id = self.current_tab().unwrap().id;
                                    if let Some(tab) = self.get_mut_tab_by_id(id) {
                                        if let Some(TabContent::Markdown(md)) = &mut tab.content {
                                            md.editor.needs_name = false;
                                        }
                                    }
                                    self.rename_file((id, name.clone()));
                                }
                            }
                            ui.ctx().request_repaint();
                        }
                    }
                });
        });
    }

    pub fn save_all_tabs(&self) {
        for (i, _) in self.tabs.iter().enumerate() {
            self.save_tab(i);
        }
    }

    pub fn save_tab(&self, i: usize) {
        if let Some(tab) = self.tabs.get(i) {
            if tab.is_dirty() {
                if let Some(save_req) = tab.make_save_request() {
                    let core = self.core.clone();
                    let update_tx = self.updates_tx.clone();
                    let ctx = self.ctx.clone();
                    thread::spawn(move || {
                        let content = save_req.content;
                        let id = save_req.id;

                        let result = core
                            .write_document(id, content.as_bytes())
                            .map(|_| Instant::now());

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
                .next_in_children(core.get_children(focused_parent).unwrap());

            let result = core
                .create_file(new_file.to_name().as_str(), focused_parent, FileType::Document)
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
                        lb_rs::CoreError::FileNonexistent => Some(TabFailure::DeletedFromSync),
                        _ => Some(err.into()),
                    }
                }
                return;
            }
        };

        let fpath = self.core.get_path_by_id(id).unwrap(); // TODO

        self.upsert_tab(id, &fname, &fpath, is_new_file, make_active);

        let core = self.core.clone();
        let ctx = self.ctx.clone();

        // todo
        // let settings = &self.settings.read().unwrap();
        // let toolbar_visibility = settings.toolbar_visibility;
        let toolbar_visibility = ToolBarVisibility::Maximized;
        let update_tx = self.updates_tx.clone();
        let cfg = self.cfg.clone();
        let is_mobile_viewport = !self.show_tabs;

        thread::spawn(move || {
            let ext = fname.split('.').last().unwrap_or_default();

            let content = core
                .read_document(id)
                .map_err(|err| TabFailure::Unexpected(format!("{:?}", err))) // todo(steve)
                .map(|bytes| {
                    if is_supported_image_fmt(ext) {
                        TabContent::Image(ImageViewer::new(&id.to_string(), ext, &bytes))
                    } else if ext == "pdf" {
                        TabContent::Pdf(PdfViewer::new(
                            &bytes,
                            &ctx,
                            &cfg.data_dir,
                            is_mobile_viewport,
                        ))
                    } else if ext == "svg" {
                        TabContent::Svg(SVGEditor::new(&bytes, core.clone(), id))
                    } else {
                        TabContent::Markdown(Markdown::new(
                            core.clone(),
                            &bytes,
                            &toolbar_visibility,
                            is_new_file,
                            id,
                            ext != "md",
                        ))
                    }
                });
            update_tx.send(WsMsg::FileLoaded(id, content)).unwrap();
            ctx.request_repaint();
        });
    }

    pub fn close_tab(&mut self, i: usize) {
        self.save_tab(i);
        self.tabs.remove(i);
        let n_tabs = self.tabs.len();
        if self.active_tab >= n_tabs && n_tabs > 0 {
            self.active_tab = n_tabs - 1;
        }
    }

    fn process_keys(&mut self) {
        const COMMAND: egui::Modifiers = egui::Modifiers::COMMAND;
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
            self.out.window_title = Some(
                self.current_tab()
                    .map(|tab| tab.name.as_str())
                    .unwrap_or("Lockbook")
                    .to_owned(),
            );

            self.out.selected_file = self.current_tab().map(|tab| tab.id);
        }

        // Ctrl-{1-9} to easily navigate tabs (9 will always go to the last tab).
        self.ctx.clone().input_mut(|input| {
            for i in 1..10 {
                if input.consume_key_exact(COMMAND, NUM_KEYS[i - 1]) {
                    self.goto_tab(i);
                    // Remove any text event that's also present this frame so that it doesn't show up
                    // in the editor.
                    if let Some(index) = input
                        .events
                        .iter()
                        .position(|evt| *evt == egui::Event::Text(i.to_string()))
                    {
                        input.events.remove(index);
                    }
                    if let Some((name, id)) =
                        self.current_tab().map(|tab| (tab.name.clone(), tab.id))
                    {
                        self.out.window_title = Some(name);
                        self.out.selected_file = Some(id);
                    };
                    break;
                }
            }
        });
    }

    pub fn process_updates(&mut self) {
        while let Ok(update) = self.updates_rx.try_recv() {
            match update {
                WsMsg::FileLoaded(id, content) => {
                    if let Some((name, id)) =
                        self.current_tab().map(|tab| (tab.name.clone(), tab.id))
                    {
                        self.out.window_title = Some(name);
                        self.out.selected_file = Some(id);
                    };

                    if let Some(tab) = self.get_mut_tab_by_id(id) {
                        match content {
                            Ok(content) => {
                                tab.content = Some(content);
                            }
                            Err(fail) => {
                                println!("failed to load file: {:?}", fail);
                                tab.failure = Some(fail);
                            }
                        }
                    } else {
                        println!("failed to load file: tab not found");
                    }
                }
                WsMsg::BgSignal(Signal::SaveAll) => {
                    if self.cfg.auto_save.load(Ordering::Relaxed) {
                        self.save_all_tabs();
                    }
                }
                WsMsg::SaveResult(id, result) => {
                    if let Some(tab) = self.get_mut_tab_by_id(id) {
                        match result {
                            Ok(time_saved) => tab.last_saved = time_saved,
                            Err(err) => {
                                tab.failure = Some(TabFailure::Unexpected(format!("{:?}", err)))
                            }
                        }
                    }
                }
                WsMsg::BgSignal(Signal::BwDone) => {
                    // todo!
                    // if let Some(s) = &mut self.shutdown {
                    //     s.done_saving = true;
                    //     self.perform_final_sync(ctx);
                    // }
                }
                WsMsg::BgSignal(Signal::Sync) => {
                    if self.cfg.auto_sync.load(Ordering::Relaxed) {
                        self.perform_sync();
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
                            self.out.window_title = Some(tab.name.clone());
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
            core.rename_file(id, &new_name).unwrap(); // TODO

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

fn tab_label(ui: &mut egui::Ui, t: &mut Tab, is_active: bool) -> Option<TabLabelResponse> {
    let mut lbl_resp = None;

    let padding = egui::vec2(15.0, 15.0);
    let wrap_width = ui.available_width();

    let text: egui::WidgetText = (&t.name).into();
    let text = text.into_galley(ui, Some(false), wrap_width, egui::TextStyle::Body);

    let x_icon = Icon::CLOSE.size(16.0);

    let w = text.size().x + padding.x * 3.0 + x_icon.size + 1.0;
    let h = text.size().y + padding.y * 2.0;

    let (rect, resp) = ui.allocate_exact_size((w, h).into(), egui::Sense::hover());

    if ui.is_rect_visible(rect) {
        let text_color = ui.style().interact(&resp).text_color();

        let close_btn_pos =
            egui::pos2(rect.max.x - padding.x - x_icon.size, rect.center().y - x_icon.size / 2.0);

        let close_btn_rect =
            egui::Rect::from_min_size(close_btn_pos, egui::vec2(x_icon.size, x_icon.size))
                .expand(2.0);

        let mut close_hovered = false;
        let pointer_pos = ui.input(|i| i.pointer.hover_pos());
        if let Some(pos) = pointer_pos {
            if close_btn_rect.contains(pos) {
                close_hovered = true;
            }
        }

        let text_pos = egui::pos2(rect.min.x + padding.x, rect.center().y - 0.5 * text.size().y);

        if let Some(ref mut str) = t.rename {
            let res = ui
                .allocate_ui_at_rect(rect, |ui| {
                    ui.add(
                        egui::TextEdit::singleline(str)
                            .frame(false)
                            .id(egui::Id::new("rename_tab")),
                    )
                })
                .inner;

            res.request_focus();

            if res.lost_focus()
                || ui.input(|i| {
                    i.pointer.primary_clicked()
                        && !rect.contains(i.pointer.interact_pos().unwrap_or_default())
                })
                || ui.input(|i| i.key_pressed(egui::Key::Enter))
            {
                lbl_resp = Some(TabLabelResponse::Renamed(str.to_owned()))
            }
        } else {
            if resp.hovered() {
                ui.output_mut(|o: &mut egui::PlatformOutput| {
                    o.cursor_icon = egui::CursorIcon::PointingHand
                });
            }

            let bg = if resp.hovered() && !close_hovered {
                ui.visuals().widgets.hovered.bg_fill
            } else {
                ui.visuals().widgets.noninteractive.bg_fill
            };
            ui.painter().rect(rect, 0.0, bg, egui::Stroke::NONE);

            ui.painter().galley(text_pos, text, text_color);

            if close_hovered {
                ui.painter().rect(
                    close_btn_rect,
                    0.0,
                    ui.visuals().widgets.hovered.bg_fill,
                    egui::Stroke::NONE,
                );
            }

            // todo: use galley size of icon instead of icon.size for a more accurate reading.
            let icon_draw_pos = egui::pos2(
                close_btn_rect.center().x - x_icon.size / 2.,
                close_btn_rect.center().y - x_icon.size / 2.2,
            );

            let icon: egui::WidgetText = (&x_icon).into();
            let icon = icon.into_galley(ui, Some(false), wrap_width, egui::TextStyle::Body);

            ui.painter().galley(icon_draw_pos, icon, text_color);

            // First, we check if the close button was clicked.
            // Since egui 0.26.2, ui.interact(close_btn_rect, ..).clicked() is always false for unknown reasons
            if ui.input(|i| i.pointer.primary_clicked()) && close_hovered {
                lbl_resp = Some(TabLabelResponse::Closed);
            } else {
                // Then, we check if the tab label was clicked so that a close button click
                // wouldn't also count here.
                let resp = resp.interact(egui::Sense::click());
                if resp.clicked() {
                    lbl_resp = Some(TabLabelResponse::Clicked);
                } else if resp.middle_clicked() {
                    lbl_resp = Some(TabLabelResponse::Closed);
                }
            }
        }

        if is_active {
            ui.painter().hline(
                rect.min.x + 0.5..=rect.max.x - 1.0,
                rect.max.y - 2.0,
                egui::Stroke::new(4.0, ui.visuals().widgets.active.bg_fill),
            );
        }

        let sep_stroke = if resp.hovered() && !close_hovered {
            egui::Stroke::new(1.0, egui::Color32::TRANSPARENT)
        } else {
            ui.visuals().widgets.noninteractive.bg_stroke
        };
        ui.painter().vline(rect.max.x, rect.y_range(), sep_stroke);
    }

    lbl_resp
}

pub const NUM_KEYS: [egui::Key; 9] = [
    egui::Key::Num1,
    egui::Key::Num2,
    egui::Key::Num3,
    egui::Key::Num4,
    egui::Key::Num5,
    egui::Key::Num6,
    egui::Key::Num7,
    egui::Key::Num8,
    egui::Key::Num9,
];

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
