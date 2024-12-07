use egui::{Context, ViewportCommand};
use lb_rs::blocking::Lb;
use lb_rs::logic::filename::NameComponents;
use lb_rs::model::errors::LbErrKind;
use lb_rs::model::file_metadata::FileType;
use lb_rs::svg::buffer::Buffer;
use lb_rs::Uuid;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use std::{fs, io, mem, thread};

use crate::output::{Response, WsStatus};
use crate::tab::image_viewer::{is_supported_image_fmt, ImageViewer};
use crate::tab::markdown_editor::Editor as Markdown;
use crate::tab::pdf_viewer::PdfViewer;
use crate::tab::svg_editor::SVGEditor;
use crate::tab::{Tab, TabContent, TabFailure};
use crate::task_manager::{
    self, CompletedLoad, CompletedSave, CompletedTiming, LoadRequest, SaveRequest, TaskManager,
};

pub struct Workspace {
    // User activity
    pub tabs: Vec<Tab>,
    pub active_tab: usize,
    pub user_last_seen: Instant,

    // Files and task status
    pub tasks: TaskManager,
    pub last_save_all: Option<Instant>,
    pub last_sync: Option<Instant>,
    pub last_sync_status_refresh: Option<Instant>,

    // Output
    pub status: WsStatus,
    pub out: Response,

    // Resources & configuration
    pub cfg: WsConfig,
    pub ctx: Context,
    pub core: Lb,
    pub show_tabs: bool,              // set on mobile to hide the tab strip
    pub focused_parent: Option<Uuid>, // set to the folder where new files should be created

    // Transient state (consider removing)
    pub active_tab_changed: bool, // used to scroll to active tab when it changes
    pub last_touch_event: Option<Instant>, // used to disable tooltips on touch devices
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct WsConfig {
    pub auto_save: Arc<AtomicBool>,
    pub auto_sync: Arc<AtomicBool>,
    pub zen_mode: Arc<AtomicBool>,

    pub last_open_tabs: Arc<Vec<Uuid>>,

    #[serde(skip_serializing, skip_deserializing)]
    pub path: String,
}

impl Default for WsConfig {
    fn default() -> Self {
        Self {
            path: "".to_string(), // todo: potentially a bad idea
            auto_save: Arc::new(AtomicBool::new(true)),
            auto_sync: Arc::new(AtomicBool::new(true)),
            zen_mode: Arc::new(AtomicBool::new(false)),
            last_open_tabs: Arc::new(vec![]),
        }
    }
}

impl WsConfig {
    pub fn new(dir: String, auto_save: bool, auto_sync: bool, zen_mode: bool) -> Self {
        let mut s = Self { path: dir, ..Default::default() };
        s.update(auto_save, auto_sync, zen_mode);
        s
    }

    pub fn from_file(path: PathBuf) -> Self {
        let mut s: Self = match fs::File::open(&path) {
            Ok(f) => serde_json::from_reader(f).unwrap_or_default(),
            Err(_) => Self::default(),
        };
        s.path = path.to_string_lossy().to_string();
        s
    }

    pub fn update(&mut self, auto_save: bool, auto_sync: bool, zen_mode: bool) {
        self.auto_save.store(auto_save, Ordering::Relaxed);
        self.auto_sync.store(auto_sync, Ordering::Relaxed);
        self.zen_mode.store(zen_mode, Ordering::Relaxed);
    }

    pub fn update_last_open_tabs(&mut self, tabs: &Vec<Tab>, active_tab_index: usize) {
        let mut active_tab = None;
        let mut tab_ids: Vec<Uuid> = tabs
            .iter()
            .enumerate()
            .filter_map(|(i, t)| {
                if i == active_tab_index {
                    active_tab = Some(t.id);
                    None
                } else {
                    Some(t.id)
                }
            })
            .collect();

        if let Some(tab) = active_tab {
            tab_ids.push(tab);
        }

        self.last_open_tabs = Arc::new(tab_ids);

        if let Err(msg) = self.to_file() {
            println!("{:#?}", msg);
        }
    }

    pub fn to_file(&self) -> io::Result<()> {
        let content = serde_json::to_string(&self).ok().unwrap();
        fs::write(&self.path, content)
    }
}

impl Workspace {
    pub fn new(core: &Lb, ctx: &Context) -> Self {
        let (updates_tx, updates_rx) = channel();
        let background = BackgroundWorker::new(ctx, &updates_tx);
        let background_tx = background.spawn_worker();
        let status = Default::default();
        let output = Default::default();

        let writable_dir = core.get_config().writeable_path;
        let writeable_dir = Path::new(&writable_dir);
        let writeable_path = writeable_dir.join("ws_conf.json");
        Self {
            cfg: WsConfig::from_file(writeable_path),
            tabs: vec![],
            active_tab: 0,
            active_tab_changed: false,
            user_last_seen: Instant::now(),

            tasks: TaskManager::new(core.clone(), ctx.clone()),
            last_sync: Default::default(),
            last_save_all: Default::default(),
            last_sync_status_refresh: Default::default(),

            status: Default::default(),
            out: Default::default(),

            cfg,
            ctx: ctx.clone(),
            core: core.clone(),
            show_tabs: true,
            focused_parent: Default::default(),

            active_tab_changed: Default::default(),
            last_touch_event: Default::default(),
        }
    }

    pub fn invalidate_egui_references(&mut self, ctx: &Context, core: &Lb) {
        self.ctx = ctx.clone();
        self.core = core.clone();

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
        if self.ctx.frame_nr() == 0 {
            self.cfg.last_open_tabs.clone().iter().for_each(|&file_id| {
                self.open_file(file_id, true, true);
            });
        }

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

        if self.active_tab_changed {
            self.cfg.update_last_open_tabs(&self.tabs, self.active_tab);
        }

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
                                    ui.label(format!(
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
        self.last_save_all = Some(Instant::now());
    }

    pub fn save_tab(&mut self, i: usize) {
        if let Some(tab) = self.tabs.get_mut(i) {
            if tab.is_dirty() {
                if let Some(request) = tab.make_save_request() {
                    self.tasks.queue_save(request);
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

        // self.cfg.update_last_open_tabs(&self.tabs, self.active_tab);
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

        self.tasks
            .queue_load(LoadRequest { id, is_new_file, tab_created });
        let core = self.core.clone();
        let ctx = self.ctx.clone();
        let update_tx = self.updates_tx.clone();

        thread::spawn(move || {
            let content = core
                .read_document_with_hmac(id)
                .map_err(|err| TabFailure::Unexpected(format!("{:?}", err)));
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

        // self.cfg.update_last_open_tabs(&self.tabs, self.active_tab);
    }

    pub fn process_updates(&mut self) {
        let task_manager::Response { completed_loads, completed_saves, completed_sync } =
            self.tasks.update();

        for load in completed_loads {
            // nested scope indentation preserves git history
            {
                {
                    let CompletedLoad {
                        request: LoadRequest { id, is_new_file, tab_created },
                        content_result,
                        timing: _,
                    } = load;

                    if let Some((name, id)) =
                        self.current_tab().map(|tab| (tab.name.clone(), tab.id))
                    {
                        self.ctx.send_viewport_cmd(ViewportCommand::Title(name));
                        self.out.selected_file = Some(id);
                    };

                    let ctx = self.ctx.clone();
                    let writeable_dir = &self.core.get_config().writeable_path;
                    let core = self.core.clone();
                    let show_tabs = self.show_tabs;

                    let canvas_settings = self.tabs.iter().find_map(|t| {
                        if let Some(TabContent::Svg(svg)) = &t.content {
                            Some(svg.settings)
                        } else {
                            None
                        }
                    });

                    if let Some(tab) = self.get_mut_tab_by_id(id) {
                        let (maybe_hmac, bytes) = match content_result {
                            Ok((hmac, bytes)) => (hmac, bytes),
                            Err(err) => {
                                println!("failed to load file: {:?}", err);
                                tab.failure = Some(TabFailure::Unexpected(format!("{:?}", err)));
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
                                writeable_dir,
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
                                    canvas_settings,
                                )));
                            } else {
                                match tab.content.as_mut() {
                                    Some(TabContent::Svg(svg)) => {
                                        Buffer::reload(
                                            &mut svg.buffer.elements,
                                            &mut svg.buffer.weak_images,
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
                    } else {
                        println!("failed to load file: tab not found");
                    };
                }
            }
        }

        for save in completed_saves {
            // nested scope indentation preserves git history
            {
                {
                    let CompletedSave {
                        request: SaveRequest { id, old_hmac: _, seq, content },
                        new_hmac_result,
                        timing: CompletedTiming { queued_at: _, started_at, completed_at: _ },
                    } = save;

                    let mut sync = false;
                    if let Some(tab) = self.get_mut_tab_by_id(id) {
                        match new_hmac_result {
                            Ok(hmac) => {
                                tab.last_saved = started_at;
                                match tab.content.as_mut() {
                                    Some(TabContent::Markdown(md)) => {
                                        md.hmac = Some(hmac);
                                        md.buffer.saved(seq, content);
                                    }
                                    Some(TabContent::Svg(svg)) => {
                                        svg.buffer.open_file_hmac = Some(hmac);
                                        svg.buffer.opened_content = content;
                                    }
                                    _ => {}
                                }
                                sync = true; // todo: sync once when saving multiple tabs
                            }
                            Err(err) => {
                                if err.kind == LbErrKind::ReReadRequired {
                                    self.open_file(id, false, false);
                                } else {
                                    tab.failure = Some(TabFailure::Unexpected(format!("{:?}", err)))
                                }
                            }
                        }
                    }
                    if sync {
                        self.perform_sync();
                    }
                }
            }
        }

        if let Some(sync) = completed_sync {
            self.sync_done(sync)
        }

        {
            let tasks = self.tasks.tasks.lock().unwrap();
            if let Some(sync) = tasks.in_progress_sync.as_ref() {
                while let Ok(progress) = sync.progress.try_recv() {
                    self.out.status_updated = true;
                    self.status.sync_progress = progress.progress as f32 / progress.total as f32;
                    self.status.sync_message = Some(progress.msg);
                }
            }
        }

        // background work
        let now = Instant::now();
        if self.cfg.auto_sync.load(Ordering::Relaxed) {
            if let Some(last_sync) = self.last_sync {
                let focused = self.ctx.input(|i| i.focused);
                let user_active = self.user_last_seen.elapsed() < Duration::from_secs(10);
                let sync_period = if user_active && focused {
                    Duration::from_secs(5)
                } else {
                    Duration::from_secs(60 * 60)
                };

                let instant_of_next_sync = last_sync + sync_period;
                if instant_of_next_sync < now {
                    self.perform_sync();
                } else {
                    let duration_until_next_sync = instant_of_next_sync - now;
                    self.ctx.request_repaint_after(duration_until_next_sync);
                }
            } else {
                self.tasks.queue_sync();
            }
        }
        if self.cfg.auto_save.load(Ordering::Relaxed) {
            if let Some(last_save_all) = self.last_save_all {
                let instant_of_next_save_all = last_save_all + Duration::from_secs(1);
                if instant_of_next_save_all < now {
                    self.save_all_tabs();
                } else {
                    let duration_until_next_save_all = instant_of_next_save_all - now;
                    self.ctx.request_repaint_after(duration_until_next_save_all);
                }
            } else {
                self.save_all_tabs();
            }
        }
        if let Some(last_sync_status_refresh) = self.last_sync_status_refresh {
            let instant_of_next_sync_status_refresh =
                last_sync_status_refresh + Duration::from_secs(1);
            if instant_of_next_sync_status_refresh < now {
                self.refresh_sync_status();
            } else {
                let duration_until_next_sync_status_refresh =
                    instant_of_next_sync_status_refresh - now;
                self.ctx
                    .request_repaint_after(duration_until_next_sync_status_refresh);
            }
        } else {
            self.refresh_sync_status();
        }
    }

    pub fn create_file(&mut self, is_drawing: bool) {
        let focused_parent = self
            .focused_parent
            .unwrap_or_else(|| self.core.get_root().unwrap().id);

        let focused_parent = self.core.get_file_by_id(focused_parent).unwrap();
        let focused_parent = if focused_parent.file_type == FileType::Document {
            focused_parent.parent
        } else {
            focused_parent.id
        };

        let file_format = if is_drawing { "svg" } else { "md" };
        let new_file = NameComponents::from(&format!("untitled.{}", file_format))
            .next_in_children(self.core.get_children(&focused_parent).unwrap());

        let result = self
            .core
            .create_file(new_file.to_name().as_str(), &focused_parent, FileType::Document)
            .map_err(|err| format!("{:?}", err));

        self.out.file_created = Some(result);
        self.ctx.request_repaint();
    }

    pub fn rename_file(&mut self, req: (Uuid, String)) {
        let (id, new_name) = req;
        self.core.rename_file(&id, &new_name).unwrap(); // TODO

        self.file_renamed(id, new_name);
    }

    pub fn file_renamed(&mut self, id: Uuid, new_name: String) {
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

        self.out.file_renamed = Some((id, new_name.clone()));
        self.ctx.request_repaint();
    }

    pub fn move_file(&mut self, req: (Uuid, Uuid)) {
        let (id, new_parent) = req;
        self.core.move_file(&id, &new_parent).unwrap(); // TODO

        self.out.file_moved = Some((id, new_parent));
        self.ctx.request_repaint();
    }
}
