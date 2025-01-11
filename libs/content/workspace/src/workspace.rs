use egui::{Context, ViewportCommand};
use lb_rs::blocking::Lb;
use lb_rs::logic::filename::NameComponents;
use lb_rs::model::errors::LbErrKind;
use lb_rs::model::file_metadata::FileType;
use lb_rs::svg::buffer::Buffer;
use lb_rs::{svg, Uuid};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use std::{fs, thread};
use tracing::{debug, error, info, instrument, trace, warn};

use crate::output::{Response, WsStatus};
use crate::tab::image_viewer::{is_supported_image_fmt, ImageViewer};
use crate::tab::markdown_editor::Editor as Markdown;
use crate::tab::pdf_viewer::PdfViewer;
use crate::tab::svg_editor::SVGEditor;
use crate::tab::{Tab, TabContent, TabFailure, TabSaveContent};
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
    pub last_sync_completed: Option<Instant>,
    pub last_sync_status_refresh_completed: Option<Instant>,

    // Output
    pub status: WsStatus,
    pub out: Response,

    // Resources & configuration
    pub cfg: WsPersistentStore,
    pub ctx: Context,
    pub core: Lb,
    pub show_tabs: bool,              // set on mobile to hide the tab strip
    pub focused_parent: Option<Uuid>, // set to the folder where new files should be created

    // Transient state (consider removing)
    pub active_tab_changed: bool, // used to scroll to active tab when it changes
    pub last_touch_event: Option<Instant>, // used to disable tooltips on touch devices
}

impl Workspace {
    pub fn new(core: &Lb, ctx: &Context) -> Self {
        let writable_dir = core.get_config().writeable_path;
        let writeable_dir = Path::new(&writable_dir);
        let writeable_path = writeable_dir.join("ws_persistence.json");

        let mut ws = Self {
            tabs: Default::default(),
            active_tab: Default::default(),
            user_last_seen: Instant::now(),

            tasks: TaskManager::new(core.clone(), ctx.clone()),
            last_sync_completed: Default::default(),
            last_save_all: Default::default(),
            last_sync_status_refresh_completed: Default::default(),

            status: Default::default(),
            out: Default::default(),

            cfg: WsPersistentStore::new(writeable_path),
            ctx: ctx.clone(),
            core: core.clone(),
            show_tabs: true,
            focused_parent: Default::default(),

            active_tab_changed: Default::default(),
            last_touch_event: Default::default(),
        };

        let (open_tabs, active_tab) = ws.cfg.get_tabs();

        open_tabs.iter().for_each(|&file_id| {
            if core.get_file_by_id(file_id).is_ok() {
                info!(id = ?file_id, "opening persisted tab");
                ws.open_file(file_id, false, false);
            }
        });
        if let Some(active_tab) = active_tab {
            info!(id = ?active_tab, "setting persisted active tab");
            ws.active_tab = open_tabs
                .iter()
                .position(|&id| id == active_tab)
                .unwrap_or_default();
        }

        ws
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
                    // only stop a tab from closing if it's being made active (e.g. user clicked file tree node)
                    self.tabs[i].is_closing = false;
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
            is_closing: false,
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

    pub fn save_all_tabs(&mut self) {
        for i in 0..self.tabs.len() {
            self.save_tab(i);
        }
        self.last_save_all = Some(Instant::now());
    }

    pub fn save_tab(&mut self, i: usize) {
        if let Some(tab) = self.tabs.get_mut(i) {
            if tab.is_dirty(&self.tasks) {
                self.tasks.queue_save(SaveRequest { id: tab.id });
            }
        }
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
    }

    pub fn close_tab(&mut self, i: usize) {
        self.save_tab(i);
        self.tabs[i].is_closing = true;
    }

    pub fn remove_tab(&mut self, i: usize) {
        self.tabs.remove(i);
        let n_tabs = self.tabs.len();
        self.out.tabs_changed = true;
        if self.active_tab >= n_tabs && n_tabs > 0 {
            self.active_tab = n_tabs - 1;
        }
        self.active_tab_changed = true;
    }

    #[instrument(level = "trace", skip_all)]
    pub fn process_updates(&mut self) {
        let task_manager::Response {
            completed_loads,
            completed_saves,
            completed_sync,
            completed_sync_status_update,
        } = self.tasks.update();

        let start = Instant::now();
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
                    let core = self.core.clone();
                    let writeable_dir = &self.core.get_config().writeable_path;
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
                                            &svg.opened_content,
                                            &svg::buffer::Buffer::new(
                                                String::from_utf8_lossy(&bytes).as_ref(),
                                            ),
                                        );

                                        svg.open_file_hmac = maybe_hmac;
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

                        self.out.tabs_changed = true;
                    } else {
                        println!("failed to load file: tab not found");
                    };
                }
            }
        }
        if start.elapsed() > Duration::from_millis(100) {
            warn!("processing completed loads took {:?}", start.elapsed());
        }

        let start = Instant::now();
        for save in completed_saves {
            // nested scope indentation preserves git history
            {
                {
                    let CompletedSave {
                        request: SaveRequest { id },
                        seq,
                        content,
                        new_hmac_result,
                        timing: CompletedTiming { queued_at: _, started_at, completed_at: _ },
                    } = save;

                    let mut sync = false;
                    if let Some(tab) = self.get_mut_tab_by_id(id) {
                        match new_hmac_result {
                            Ok(hmac) => {
                                tab.last_saved = started_at;
                                match (tab.content.as_mut(), content) {
                                    (
                                        Some(TabContent::Markdown(md)),
                                        TabSaveContent::String(content),
                                    ) => {
                                        md.hmac = Some(hmac);
                                        md.buffer.saved(seq, content);
                                    }
                                    (Some(TabContent::Svg(svg)), TabSaveContent::Svg(content)) => {
                                        svg.open_file_hmac = Some(hmac);
                                        svg.opened_content = content;
                                    }
                                    _ => {}
                                }
                                sync = true; // todo: sync once when saving multiple tabs
                            }
                            Err(err) => {
                                if err.kind == LbErrKind::ReReadRequired {
                                    debug!("reloading file after save failed with re-read required: {}", id);
                                    self.open_file(id, false, false);
                                } else {
                                    tab.failure = Some(TabFailure::Unexpected(format!("{:?}", err)))
                                }
                            }
                        }
                    }
                    if sync {
                        self.tasks.queue_sync();
                    }
                }
            }
        }
        if start.elapsed() > Duration::from_millis(100) {
            warn!("processing completed saves took {:?}", start.elapsed());
        }

        let start = Instant::now();
        if let Some(sync) = completed_sync {
            self.sync_done(sync)
        }
        if start.elapsed() > Duration::from_millis(100) {
            warn!("processing completed sync took {:?}", start.elapsed());
        }

        let start = Instant::now();
        if let Some(update) = completed_sync_status_update {
            self.sync_status_update_done(update)
        }
        if start.elapsed() > Duration::from_millis(100) {
            warn!("processing completed sync status update took {:?}", start.elapsed());
        }

        let start = Instant::now();
        {
            let tasks = self.tasks.tasks.lock().unwrap();
            if let Some(sync) = tasks.in_progress_sync.as_ref() {
                while let Ok(progress) = sync.progress.try_recv() {
                    trace!("sync {}", progress);
                    self.status.sync_message = Some(progress.msg);
                    self.out.status_updated = true;
                }
            }
        }
        if start.elapsed() > Duration::from_millis(100) {
            warn!("processing sync progress took {:?}", start.elapsed());
        }

        // background work: queue
        let now = Instant::now();
        let start = Instant::now();
        if let Some(last_sync_status_refresh) = self
            .tasks
            .sync_status_update_queued_at()
            .or(self.last_sync_status_refresh_completed)
        {
            let instant_of_next_sync_status_refresh =
                last_sync_status_refresh + Duration::from_secs(1);
            if instant_of_next_sync_status_refresh < now {
                self.tasks.queue_sync_status_update();
            } else {
                let duration_until_next_sync_status_refresh =
                    instant_of_next_sync_status_refresh - now;
                self.ctx
                    .request_repaint_after(duration_until_next_sync_status_refresh);
            }
        } else {
            self.tasks.queue_sync_status_update();
        }
        if start.elapsed() > Duration::from_millis(100) {
            warn!("processing sync status refresh took {:?}", start.elapsed());
        }

        let start = Instant::now();
        if self.cfg.get_auto_save() {
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
        if start.elapsed() > Duration::from_millis(100) {
            warn!("processing auto save took {:?}", start.elapsed());
        }

        let start = Instant::now();
        if self.cfg.get_auto_sync() {
            if let Some(last_sync) = self.tasks.sync_queued_at().or(self.last_sync_completed) {
                let focused = self.ctx.input(|i| i.focused);
                let user_active = self.user_last_seen.elapsed() < Duration::from_secs(60);
                let sync_period = if user_active && focused {
                    Duration::from_secs(5)
                } else {
                    Duration::from_secs(5 * 60)
                };

                let instant_of_next_sync = last_sync + sync_period;
                if instant_of_next_sync < now {
                    self.tasks.queue_sync();
                } else {
                    let duration_until_next_sync = instant_of_next_sync - now;
                    self.ctx.request_repaint_after(duration_until_next_sync);
                }
            } else {
                self.tasks.queue_sync();
            }
        }
        if start.elapsed() > Duration::from_millis(100) {
            warn!("processing auto sync took {:?}", start.elapsed());
        }

        // background work: launch
        let start = Instant::now();
        self.tasks.check_launch(&self.tabs);
        if start.elapsed() > Duration::from_millis(100) {
            warn!("processing task launch took {:?}", start.elapsed());
        }

        // background work: cleanup
        let mut removed_tabs = 0;
        for i in 0..self.tabs.len() {
            let i = i - removed_tabs;
            let tab = &self.tabs[i];
            if tab.is_closing
                && !self.tasks.load_or_save_queued(tab.id)
                && !self.tasks.load_or_save_in_progress(tab.id)
            {
                self.remove_tab(i);
                removed_tabs += 1;

                let title = match self.current_tab() {
                    Some(tab) => tab.name.clone(),
                    None => "Lockbook".to_owned(),
                };
                self.ctx.send_viewport_cmd(ViewportCommand::Title(title));

                self.out.selected_file = self.current_tab().map(|tab| tab.id);
            }
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

    pub fn status_message(&self) -> String {
        if let Some(error) = &self.status.sync_error {
            format!("sync error: {error}")
        } else if let Some(error) = &self.status.sync_status_update_error {
            format!("sync status update error: {error}")
        } else if self.status.offline {
            "Offline".to_string()
        } else if self.status.out_of_space {
            "You're out of space, buy more in settings!".to_string()
        } else if let (true, Some(msg)) = (self.visibly_syncing(), &self.status.sync_message) {
            msg.to_string()
        } else if !self.status.dirtyness.dirty_files.is_empty() {
            let size = self.status.dirtyness.dirty_files.len();
            if size == 1 {
                format!("{size} file needs to be synced")
            } else {
                format!("{size} files need to be synced")
            }
        } else {
            format!("Last synced: {}", self.status.dirtyness.last_synced)
        }
    }

    pub fn visibly_syncing(&self) -> bool {
        self.tasks
            .sync_started_at()
            .map(|s| s.elapsed().as_millis() > 300)
            .unwrap_or_default()
    }
}

#[derive(Clone)]
pub struct WsPersistentStore {
    pub path: PathBuf,
    data: Arc<RwLock<WsPresistentData>>,
}

#[derive(Clone, Serialize, Deserialize, Eq, PartialEq)]
struct WsPresistentData {
    open_tabs: Vec<Uuid>,
    active_tab: Option<Uuid>,
    auto_save: bool,
    auto_sync: bool,
}

impl Default for WsPresistentData {
    fn default() -> Self {
        Self { auto_save: true, auto_sync: true, open_tabs: Vec::default(), active_tab: None }
    }
}

impl WsPersistentStore {
    pub fn new(path: PathBuf) -> Self {
        let default = WsPresistentData::default();

        match fs::File::open(&path) {
            Ok(f) => WsPersistentStore {
                path,
                data: Arc::new(RwLock::new(serde_json::from_reader(f).unwrap_or(default))),
            },
            Err(err) => {
                error!("Could not open ws presistance file: {:#?}", err);
                WsPersistentStore { path, data: Arc::new(RwLock::new(default)) }
            }
        }
    }

    pub fn set_tabs(&mut self, tabs: &[Tab], active_tab_index: usize) {
        let mut data_lock = self.data.write().unwrap();
        data_lock.open_tabs = tabs.iter().map(|t| t.id).collect();
        if !tabs.is_empty() {
            data_lock.active_tab = Some(tabs[active_tab_index].id);
        }
        self.write_to_file();
    }

    pub fn get_tabs(&self) -> (Vec<Uuid>, Option<Uuid>) {
        let data_lock = self.data.read().unwrap();
        (data_lock.open_tabs.clone(), data_lock.active_tab)
    }

    pub fn get_auto_sync(&self) -> bool {
        self.data.read().unwrap().auto_save
    }

    pub fn set_auto_sync(&mut self, auto_sync: bool) {
        let mut data_lock = self.data.write().unwrap();
        data_lock.auto_sync = auto_sync;
        self.write_to_file();
    }

    pub fn get_auto_save(&self) -> bool {
        self.data.read().unwrap().auto_save
    }

    pub fn set_auto_save(&mut self, auto_save: bool) {
        let mut data_lock = self.data.write().unwrap();
        data_lock.auto_save = auto_save;
        self.write_to_file();
    }

    fn write_to_file(&self) {
        let data = self.data.clone();
        let path = self.path.clone();
        thread::spawn(move || {
            let data = data.read().unwrap();
            let content = serde_json::to_string(&*data).unwrap();
            fs::write(path, content)
        });
    }
}
