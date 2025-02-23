use egui::{Context, ViewportCommand};

use lb_rs::blocking::Lb;
use lb_rs::model::errors::{LbErr, LbErrKind};
use lb_rs::model::file_metadata::FileType;
use lb_rs::model::filename::NameComponents;
use lb_rs::model::svg;
use lb_rs::model::svg::buffer::Buffer;
use lb_rs::Uuid;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::{fs, thread};
use tracing::{debug, error, info, instrument, trace, warn};
use web_time::{Duration, Instant};

use crate::file_cache::FileCache;
use crate::mind_map::show::MindMap;
use crate::output::{Response, WsStatus};
use crate::tab::image_viewer::{is_supported_image_fmt, ImageViewer};
use crate::tab::markdown_editor::Editor as Markdown;
#[cfg(not(target_family = "wasm"))]
use crate::tab::pdf_viewer::PdfViewer;
use crate::tab::svg_editor::SVGEditor;
use crate::tab::{ContentState, Tab, TabContent, TabFailure, TabSaveContent, TabsExt as _};
use crate::task_manager;
use crate::task_manager::{
    CompletedLoad, CompletedSave, CompletedTiming, LoadRequest, SaveRequest, TaskManager,
};

pub struct Workspace {
    // User activity
    pub tabs: Vec<Tab>,
    pub current_tab: usize,
    pub user_last_seen: Instant,

    // Files and task status
    pub tasks: TaskManager,
    pub files: Option<FileCache>,
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
    pub current_tab_changed: bool, // used to scroll to current tab when it changes
    pub last_touch_event: Option<Instant>, // used to disable tooltips on touch devices
}

impl Workspace {
    pub fn new(core: &Lb, ctx: &Context) -> Self {
        let writable_dir = core.get_config().writeable_path;
        let writeable_dir = Path::new(&writable_dir);
        let writeable_path = writeable_dir.join("ws_persistence.json");

        let mut ws = Self {
            tabs: Default::default(),
            current_tab: Default::default(),
            user_last_seen: Instant::now(),

            tasks: TaskManager::new(core.clone(), ctx.clone()),
            files: None,
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

            current_tab_changed: Default::default(),
            last_touch_event: Default::default(),
        };

        let (open_tabs, current_tab) = ws.cfg.get_tabs();

        open_tabs.iter().for_each(|&file_id| {
            if core.get_file_by_id(file_id).is_ok() {
                info!(id = ?file_id, "opening persisted tab");
                ws.open_file(file_id, false, false);
            }
        });
        if let Some(current_tab) = current_tab {
            info!(id = ?current_tab, "setting persisted current tab");
            ws.current_tab = open_tabs
                .iter()
                .position(|&id| id == current_tab)
                .unwrap_or_default();
        }

        ws
    }

    // todo: what happens if a save is in progress? what about non-file tabs?
    pub fn invalidate_egui_references(&mut self, ctx: &Context, core: &Lb) {
        self.ctx = ctx.clone();
        self.core = core.clone();

        let ids: Vec<Uuid> = self.tabs.iter().flat_map(|tab| tab.id()).collect();
        let maybe_current_tab_id = self.current_tab().map(|tab| tab.id());

        while self.current_tab != 0 {
            self.close_tab(self.tabs.len() - 1);
        }

        for id in ids {
            self.open_file(id, false, false)
        }

        if let Some(current_tab_id) = maybe_current_tab_id {
            self.current_tab = self
                .tabs
                .iter()
                .position(|tab| tab.id() == current_tab_id)
                .unwrap_or(0);
            self.current_tab_changed = true;
        }
    }

    /// Creates a loading-state tab for a file if needed and returns true if a tab was created
    pub fn upsert_loading_tab(&mut self, id: Uuid, make_current: bool) -> bool {
        let tab_exists = self.tabs.iter().any(|t| t.id() == Some(id));
        if !tab_exists {
            self.create_tab(ContentState::Loading(id), make_current);
        } else if make_current {
            self.make_current_by_id(id);
        }
        !tab_exists
    }

    pub fn create_tab(&mut self, content: ContentState, make_current: bool) {
        let now = Instant::now();
        let new_tab =
            Tab { content, last_changed: now, last_saved: now, rename: None, is_closing: false };
        self.tabs.push(new_tab);
        if make_current {
            self.current_tab = self.tabs.len() - 1;
            self.current_tab_changed = true;
        }
    }

    pub fn get_mut_tab_by_id(&mut self, id: Uuid) -> Option<&mut Tab> {
        self.tabs.get_mut_by_id(id)
    }

    pub fn is_empty(&self) -> bool {
        self.tabs.is_empty()
    }

    pub fn current_tab(&self) -> Option<&Tab> {
        self.tabs.get(self.current_tab)
    }

    pub fn current_tab_id(&self) -> Option<Uuid> {
        self.tabs.get(self.current_tab).and_then(|tab| tab.id())
    }

    pub fn current_tab_title(&self) -> Option<String> {
        self.current_tab().map(|tab| self.tab_title(tab))
    }

    pub fn current_tab_mut(&mut self) -> Option<&mut Tab> {
        self.tabs.get_mut(self.current_tab)
    }

    pub fn current_tab_markdown(&self) -> Option<&Markdown> {
        self.current_tab()?.markdown()
    }

    pub fn current_tab_markdown_mut(&mut self) -> Option<&mut Markdown> {
        self.current_tab_mut()?.markdown_mut()
    }
    pub fn current_tab_svg_mut(&mut self) -> Option<&mut SVGEditor> {
        self.current_tab_mut()?.svg_mut()
    }

    /// Makes the tab the current tab, if it exists. Returns true if the tab exists.
    pub fn make_current(&mut self, i: usize) -> bool {
        if i < self.tabs.len() {
            self.current_tab = i;
            self.current_tab_changed = true;
            self.tabs[i].is_closing = false;
            self.out.selected_file = self.tabs[i].id();
            self.ctx
                .send_viewport_cmd(ViewportCommand::Title(self.tab_title(&self.tabs[i])));

            true
        } else {
            false
        }
    }

    /// Makes the tab with the given id the current tab, if it exists. Returns true if the tab exists.
    pub fn make_current_by_id(&mut self, id: Uuid) -> bool {
        if let Some(i) = self.tabs.iter().position(|t| t.id() == Some(id)) {
            self.make_current(i)
        } else {
            false
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
            if let Some(id) = tab.id() {
                if tab.is_dirty(&self.tasks) {
                    self.tasks.queue_save(SaveRequest { id });
                }
            }
        }
    }

    pub fn open_file(&mut self, id: Uuid, is_new_file: bool, make_current: bool) {
        let tab_created = self.upsert_loading_tab(id, make_current);
        self.tasks
            .queue_load(LoadRequest { id, is_new_file, tab_created });
    }

    pub fn close_tab(&mut self, i: usize) {
        if let ContentState::Open(TabContent::MindMap(mm)) = &mut self.tabs[i].content {
            mm.stop();
        }

        self.save_tab(i);
        self.tabs[i].is_closing = true;
    }

    pub fn remove_tab(&mut self, i: usize) {
        if let Some(md) = self.tabs[i].markdown_mut() {
            md.surrender_focus(&self.ctx);
        }

        self.tabs.remove(i);
        self.out.tabs_changed = true;

        if !self.tabs.is_empty() && self.current_tab >= self.tabs.len() {
            self.current_tab -= 1;
        }
        self.current_tab_changed = true;
    }

    #[instrument(level = "trace", skip_all)]

    pub fn process_updates(&mut self) {
        let task_manager::Response {
            completed_loads,
            completed_saves,
            completed_sync,
            completed_sync_status_update,
            completed_file_cache_refresh,
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

                    if let Some(tab) = self.current_tab() {
                        self.ctx
                            .send_viewport_cmd(ViewportCommand::Title(self.tab_title(tab)));
                        self.out.selected_file = tab.id();
                    };

                    let ctx = self.ctx.clone();
                    let core = self.core.clone();

                    let canvas_settings = self
                        .tabs
                        .iter()
                        .find_map(|tab| tab.svg().map(|svg| svg.settings));

                    if let Some(tab) = self.tabs.get_mut_by_id(id) {
                        let (maybe_hmac, bytes) = match content_result {
                            Ok((hmac, bytes)) => (hmac, bytes),
                            Err(err) => {
                                let msg = format!("failed to load file: {:?}", err);
                                error!(msg);
                                tab.content =
                                    ContentState::Failed(TabFailure::Unexpected(msg.clone()));
                                self.out.failure_messages.push(msg);
                                return;
                            }
                        };

                        let ext = match self.core.get_file_by_id(id) {
                            Ok(file) => file.name.split('.').last().unwrap_or_default().to_owned(),
                            Err(e) => {
                                self.out
                                    .failure_messages
                                    .push(format!("failed to get id for loaded file: {:?}", e));
                                continue;
                            }
                        };

                        if is_supported_image_fmt(&ext) {
                            tab.content = ContentState::Open(TabContent::Image(ImageViewer::new(
                                id, &ext, &bytes,
                            )));
                        } else if ext == "pdf" {
                            #[cfg(not(target_family = "wasm"))]
                            {
                                tab.content = ContentState::Open(TabContent::Pdf(PdfViewer::new(
                                    id,
                                    &bytes,
                                    &ctx,
                                    &self.core.get_config().writeable_path,
                                    !self.show_tabs, // todo: use settings to determine toolbar visibility
                                )));
                            }
                        } else if ext == "svg" {
                            if tab_created {
                                tab.content = ContentState::Open(TabContent::Svg(SVGEditor::new(
                                    &bytes,
                                    &ctx,
                                    core.clone(),
                                    id,
                                    maybe_hmac,
                                    canvas_settings,
                                )));
                            } else {
                                let svg = tab.svg_mut().unwrap();

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
                        } else if ext == "md" || ext == "txt" {
                            if tab_created {
                                tab.content =
                                    ContentState::Open(TabContent::Markdown(Markdown::new(
                                        core.clone(),
                                        &String::from_utf8_lossy(&bytes),
                                        id,
                                        maybe_hmac,
                                        is_new_file,
                                        ext != "md",
                                    )));
                            } else {
                                let md = tab.markdown_mut().unwrap();
                                md.reload(String::from_utf8_lossy(&bytes).into());
                                md.hmac = maybe_hmac;
                            }
                        } else {
                            tab.content = ContentState::Failed(TabFailure::SimpleMisc(format!(
                                "Unsupported file extension: {}",
                                ext
                            )));
                        };

                        self.out.tabs_changed = true;
                    } else {
                        error!("failed to load file: tab not found");
                    };
                }
            }
        }
        start.warn_after("processing completed loads", Duration::from_millis(100));

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
                                if let Some(md) = tab.markdown_mut() {
                                    if let TabSaveContent::String(content) = content {
                                        md.hmac = Some(hmac);
                                        md.buffer.saved(seq, content);
                                    }
                                } else if let Some(svg) = tab.svg_mut() {
                                    if let TabSaveContent::Svg(content) = content {
                                        svg.open_file_hmac = Some(hmac);
                                        svg.opened_content = content;
                                    }
                                }
                                sync = true;
                            }
                            Err(err) => {
                                if err.kind == LbErrKind::ReReadRequired {
                                    debug!("reloading file after save failed with re-read required: {}", id);
                                    self.open_file(id, false, false);
                                } else {
                                    tab.content = ContentState::Failed(TabFailure::Unexpected(
                                        format!("{:?}", err),
                                    ))
                                }
                            }
                        }
                    }
                    if sync {
                        self.tasks.queue_sync();
                    }
                    self.tasks.queue_file_cache_refresh();
                }
            }
        }
        start.warn_after("processing completed saves", Duration::from_millis(100));

        let start = Instant::now();
        if let Some(sync) = completed_sync {
            self.sync_done(sync)
        }
        start.warn_after("processing completed sync", Duration::from_millis(100));

        let start = Instant::now();
        if let Some(update) = completed_sync_status_update {
            self.sync_status_update_done(update)
        }
        start.warn_after("processing completed sync status update", Duration::from_millis(100));

        let start = Instant::now();
        if let Some(refresh) = completed_file_cache_refresh {
            match refresh.cache_result {
                Ok(cache) => self.files = Some(cache),
                Err(err) => error!("failed to refresh file cache: {:?}", err),
            }
        }
        start.warn_after("processing completed file cache refresh", Duration::from_millis(100));

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
        start.warn_after("processing sync progress", Duration::from_millis(100));

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
        start.warn_after("processing sync status refresh", Duration::from_millis(100));

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
        start.warn_after("processing auto save", Duration::from_millis(100));

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
        start.warn_after("processing auto sync", Duration::from_millis(100));

        // background work: launch
        let start = Instant::now();
        self.tasks.check_launch(&self.tabs);
        start.warn_after("processing task launch", Duration::from_millis(100));

        // background work: cleanup
        let mut removed_tabs = 0;
        for i in 0..self.tabs.len() {
            let i = i - removed_tabs;
            let tab = &self.tabs[i];
            if tab.is_closing
                && !tab
                    .id()
                    .map(|id| self.tasks.load_or_save_queued(id))
                    .unwrap_or_default()
                && !tab
                    .id()
                    .map(|id| self.tasks.load_or_save_in_progress(id))
                    .unwrap_or_default()
            {
                self.remove_tab(i);
                removed_tabs += 1;

                let title = match self.current_tab() {
                    Some(tab) => self.tab_title(tab),
                    None => "Lockbook".to_owned(),
                };
                self.ctx.send_viewport_cmd(ViewportCommand::Title(title));

                self.out.selected_file = self.current_tab().and_then(|tab| tab.id());
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

        self.out.file_created = Some(result.clone());
        self.ctx.request_repaint();
    }

    /// Opens or focuses the tab for the mind map
    pub fn upsert_mind_map(&mut self, core: Lb) {
        if let Some(i) = self.tabs.iter().position(|t| t.mind_map().is_some()) {
            self.make_current(i);
        } else {
            self.create_tab(ContentState::Open(TabContent::MindMap(MindMap::new(&core))), true);
        };
    }

    pub fn rename_file(&mut self, req: (Uuid, String), by_user: bool) {
        let (id, new_name) = req;
        match self.core.rename_file(&id, &new_name) {
            Ok(()) => {
                self.file_renamed(id, new_name);
            }
            Err(LbErr { kind, .. }) => {
                if by_user {
                    self.out
                        .failure_messages
                        .push(format!("Rename failed: {}", kind));
                }
                warn!(?id, "failed to rename file: {:?}", kind);
            }
        }
    }

    pub fn file_renamed(&mut self, id: Uuid, new_name: String) {
        let mut different_file_type = false;
        if let Some(tab) = self.tabs.get_by_id(id) {
            different_file_type = !NameComponents::from(&new_name)
                .extension
                .eq(&NameComponents::from(&self.tab_title(tab)).extension);
        }

        if Some(id) == self.current_tab_id() {
            self.ctx
                .send_viewport_cmd(ViewportCommand::Title(new_name.clone()));
        }

        if different_file_type {
            self.open_file(id, false, false);
        }

        self.out.file_renamed = Some((id, new_name));
        self.tasks.queue_file_cache_refresh();
        self.ctx.request_repaint();
    }

    pub fn move_file(&mut self, req: (Uuid, Uuid)) {
        let (id, new_parent) = req;
        match self.core.move_file(&id, &new_parent) {
            Ok(()) => {
                self.out.file_moved = Some((id, new_parent));
                self.tasks.queue_file_cache_refresh();
                self.ctx.request_repaint();
            }
            Err(LbErr { kind, .. }) => {
                self.out
                    .failure_messages
                    .push(format!("Move failed: {}", kind));
                warn!(?id, "failed to move file: {:?}", kind);
            }
        }
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
    current_tab: Option<Uuid>,
    auto_save: bool,
    auto_sync: bool,
}

impl Default for WsPresistentData {
    fn default() -> Self {
        Self { auto_save: true, auto_sync: true, open_tabs: Vec::default(), current_tab: None }
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

    // todo: store non-file (mind map) tabs?
    pub fn set_tabs(&mut self, tabs: &[Tab], current_tab_index: usize) {
        let mut data_lock = self.data.write().unwrap();
        data_lock.open_tabs = tabs.iter().flat_map(|t| t.id()).collect();
        if !tabs.is_empty() {
            if let Some(tab) = tabs.get(current_tab_index) {
                data_lock.current_tab = tab.id();
            } else {
                data_lock.current_tab = tabs[0].id();
            }
        }
        self.write_to_file();
    }

    pub fn get_tabs(&self) -> (Vec<Uuid>, Option<Uuid>) {
        let data_lock = self.data.read().unwrap();
        (data_lock.open_tabs.clone(), data_lock.current_tab)
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

trait InstantExt {
    fn warn_after(self, work: &str, duration: Duration);
}

impl InstantExt for Instant {
    fn warn_after(self, work: &str, duration: Duration) {
        let elapsed = self.elapsed();
        if elapsed > duration {
            warn!("{} took {:?}", work, elapsed);
        }
    }
}
