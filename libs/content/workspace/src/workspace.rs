use egui::{Color32, Context};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::thread;
use std::time::Instant;

use crate::background::{BackgroundWorker, BwIncomingMsg, Signal};
use crate::tab::image_viewer::{is_supported_image_fmt, ImageViewer};
use crate::tab::markdown::Markdown;
use crate::tab::pdf_viewer::PdfViewer;
use crate::tab::plain_text::PlainText;
use crate::tab::svg_editor::SVGEditor;
use crate::tab::{Tab, TabContent, TabFailure};
use crate::theme::icons::Icon;
use crate::widgets::{separator, Button, ToolBarVisibility};
use egui_extras::RetainedImage;
use lb_rs::{LbError, SyncProgress, SyncStatus, Uuid};

pub struct Workspace {
    pub cfg: WsConfig,

    pub tabs: Vec<Tab>,
    pub active_tab: usize,
    pub backdrop: RetainedImage,

    pub ctx: egui::Context,
    pub core: lb_rs::Core,

    pub syncing: Arc<AtomicBool>,

    pub updates_tx: Sender<WsMsg>,
    pub updates_rx: Receiver<WsMsg>,
    pub background_tx: Sender<BwIncomingMsg>,
}

pub enum WsMsg {
    FileLoaded(Uuid, Result<TabContent, TabFailure>),
    SaveResult(Uuid, Result<Instant, LbError>),
    FileRenamed { id: Uuid, new_name: String },

    BgSignal(Signal),
    SyncMsg(SyncProgress),
    SyncDone(Result<SyncStatus, LbError>),
}

#[derive(Default)]
pub struct WsOutput {
    /// What file the workspace is currently showing
    pub selected_file: Option<Uuid>,

    /// What the window title should be (based on filename generally)
    pub window_title: Option<String>,

    pub file_renamed: Option<(Uuid, String)>,

    pub error: Option<String>,

    pub settings_updated: bool,

    pub offline: bool,
    pub update_req: bool,
    pub out_of_space: bool,
    pub usage: f64,
    pub syncing: bool,
    pub sync_progress: f32,
    pub message: Option<String>,
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
        let syncing = Default::default();

        Self {
            cfg,
            tabs: vec![],
            active_tab: 0,
            backdrop: RetainedImage::from_image_bytes("logo-backdrop", LOGO_BACKDROP).unwrap(),
            ctx: ctx.clone(),
            core: core.clone(),
            updates_rx,
            updates_tx,
            background_tx,
            syncing,
        }
    }

    pub fn open_tab(&mut self, id: lb_rs::Uuid, name: &str, path: &str, is_new_file: bool) {
        let now = Instant::now();
        self.tabs.push(Tab {
            id,
            rename: None,
            name: name.to_owned(),
            path: path.to_owned(),
            failure: None,
            content: None,
            last_changed: now,
            is_new_file,
            last_saved: now,
        });
        self.active_tab = self.tabs.len() - 1;
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
        let fill = if ctx.style().visuals.dark_mode { Color32::BLACK } else { Color32::WHITE };
        egui::CentralPanel::default()
            .frame(egui::Frame::default().fill(fill))
            .show(ctx, |ui| self.show_workspace(ui))
            .inner
    }

    pub fn show_workspace(&mut self, ui: &mut egui::Ui) -> WsOutput {
        let mut output = WsOutput::default();
        self.process_updates(&mut output);

        if self.is_empty() {
            self.show_empty_workspace(ui, &mut output);
        } else {
            ui.centered_and_justified(|ui| self.show_tabs(&mut output, ui));
        }

        if self.cfg.zen_mode.load(Ordering::Relaxed) {
            let mut min = ui.clip_rect().left_bottom();
            min.y -= 37.0; // 37 is approximating the height of the button
            let max = ui.clip_rect().left_bottom();

            let rect = egui::Rect { min, max };
            ui.allocate_ui_at_rect(rect, |ui| {
                let zen_mode_btn = Button::default()
                    .icon(&Icon::SHOW_SIDEBAR)
                    .frame(true)
                    .show(ui);
                if zen_mode_btn.clicked() {
                    self.cfg.zen_mode.store(false, Ordering::Relaxed);
                    output.settings_updated = true;
                }
                zen_mode_btn.on_hover_text("Show side panel");
            });
        }

        output
    }

    fn show_empty_workspace(&mut self, ui: &mut egui::Ui, out: &mut WsOutput) {
        ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
            ui.add_space(ui.clip_rect().height() / 3.0);
            self.backdrop.show_size(ui, egui::vec2(100.0, 100.0));

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
                .frame(true)
                .show(ui)
                .clicked()
            {
                // self.create_file(false); todo!
            }
            ui.visuals_mut().widgets.inactive.fg_stroke =
                egui::Stroke { color: ui.visuals().widgets.active.bg_fill, ..Default::default() };
            ui.visuals_mut().widgets.hovered.fg_stroke =
                egui::Stroke { color: ui.visuals().widgets.active.bg_fill, ..Default::default() };
            // if Button::default().text("New folder").show(ui).clicked() {
            //     self.update_tx
            //         .send(OpenModal::NewFolder(None).into())
            //         .unwrap();
            //     ui.ctx().request_repaint();
            // } todo:
        });
    }

    fn show_tabs(&mut self, output: &mut WsOutput, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);

        ui.vertical(|ui| {
            if !self.tabs.is_empty() {
                ui.horizontal(|ui| {
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

                                        let active_name = self.tabs[i].name.clone();

                                        let mut rename_edit_state =
                                            egui::text_edit::TextEditState::default();
                                        rename_edit_state.set_ccursor_range(Some(
                                            egui::text_edit::CCursorRange {
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
                                        output.window_title = Some(self.tabs[i].name.clone());
                                        output.selected_file = Some(self.tabs[i].id);
                                    }
                                }
                                TabLabelResponse::Closed => {
                                    self.close_tab(i);
                                    output.window_title = Some(match self.current_tab() {
                                        Some(tab) => tab.name.clone(),
                                        None => "Lockbook".to_owned(),
                                    });
                                }
                                TabLabelResponse::Renamed(name) => {
                                    self.tabs[i].rename = None;
                                    let id = self.current_tab().unwrap().id;
                                    if let Some(tab) = self.get_mut_tab_by_id(id) {
                                        if let Some(TabContent::Markdown(md)) = &mut tab.content {
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

                separator(ui);
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

                                if let Some(new_name) = resp.document_renamed {
                                    rename_req = Some((tab.id, new_name))
                                }
                            }
                            TabContent::PlainText(txt) => txt.show(ui),
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

    pub fn open_file(&mut self, id: Uuid, is_new_file: bool) {
        if self.goto_tab_id(id) {
            self.ctx.request_repaint();
            return;
        }

        let fname = self
            .core
            .get_file_by_id(id)
            .unwrap() // TODO
            .name;

        let fpath = self.core.get_path_by_id(id).unwrap(); // TODO

        self.open_tab(id, &fname, &fpath, is_new_file);

        let core = self.core.clone();
        let ctx = self.ctx.clone();

        // todo
        // let settings = &self.settings.read().unwrap();
        // let toolbar_visibility = settings.toolbar_visibility;
        let toolbar_visibility = ToolBarVisibility::Maximized;
        let update_tx = self.updates_tx.clone();
        let cfg = self.cfg.clone();

        thread::spawn(move || {
            let ext = fname.split('.').last().unwrap_or_default();

            let content = core
                .read_document(id)
                .map_err(|err| TabFailure::Unexpected(format!("{:?}", err))) // todo(steve)
                .map(|bytes| {
                    if ext == "md" {
                        TabContent::Markdown(Markdown::new(
                            core.clone(),
                            &bytes,
                            &toolbar_visibility,
                            is_new_file,
                        ))
                    } else if is_supported_image_fmt(ext) {
                        TabContent::Image(ImageViewer::new(id.to_string(), &bytes))
                    } else if ext == "pdf" {
                        TabContent::Pdf(PdfViewer::new(&bytes, &ctx, &cfg.data_dir))
                    } else if ext == "svg" {
                        TabContent::Svg(SVGEditor::new(&bytes))
                    } else {
                        TabContent::PlainText(PlainText::new(&bytes))
                    }
                });
            println!("file loaded message sent: {id}, success: {}", content.is_ok());
            update_tx.send(WsMsg::FileLoaded(id, content)).unwrap();
            println!("sent successfully");
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

    pub fn process_updates(&mut self, out: &mut WsOutput) {
        while let Ok(update) = self.updates_rx.try_recv() {
            match update {
                WsMsg::FileLoaded(id, content) => {
                    if let Some(tab) = self.get_mut_tab_by_id(id) {
                        out.window_title = Some(tab.name.clone());
                        out.selected_file = Some(id);

                        match content {
                            Ok(content) => tab.content = Some(content),
                            Err(fail) => tab.failure = Some(fail),
                        }
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
                    // todo
                }
                WsMsg::SyncMsg(prog) => self.sync_message(prog, out),
                WsMsg::FileRenamed { id, new_name } => {
                    println! {"8"};
                    out.file_renamed = Some((id, new_name.clone()));
                    if let Some(tab) = self.get_mut_tab_by_id(id) {
                        tab.name = new_name.clone();
                    }

                    if let Some(tab) = self.current_tab() {
                        if tab.id == id {
                            out.window_title = Some(tab.name.clone());
                        }
                    }
                }
                WsMsg::SyncDone(sync_outcome) => self.sync_done(sync_outcome, out),
            }
        }
        // while let Ok(update) = self.updates_rx.try_recv() {}
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
        let visuals = &ui.style().interact(&resp).clone();

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
            text.paint_with_visuals(ui.painter(), text_pos, visuals);

            if close_hovered {
                ui.painter().rect(
                    close_btn_rect,
                    0.0,
                    ui.visuals().widgets.hovered.bg_fill,
                    egui::Stroke::NONE,
                );
            }

            let icon_draw_pos = egui::pos2(
                rect.max.x - padding.x - x_icon.size - 1.0,
                rect.center().y - x_icon.size / 4.1 - 1.0,
            );

            let icon: egui::WidgetText = (&x_icon).into();
            icon.into_galley(ui, Some(false), wrap_width, egui::TextStyle::Body)
                .paint_with_visuals(ui.painter(), icon_draw_pos, visuals);

            let close_resp = ui.interact(
                close_btn_rect,
                egui::Id::new(format!("close-btn-{}", t.id)),
                egui::Sense::click(),
            );
            // First, we check if the close button was clicked.
            if close_resp.clicked() {
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

const LOGO_BACKDROP: &[u8] = include_bytes!("../lockbook-backdrop.png");
