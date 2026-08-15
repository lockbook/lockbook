use chrono::Local;
use egui::Context;

use lb_rs::blocking::Lb;
use lb_rs::model::access_info::UserAccessMode;
use lb_rs::model::account::Account;
use lb_rs::model::errors::{LbErr, LbErrKind, Unexpected};
use lb_rs::model::file::File;
use lb_rs::model::file_metadata::FileType;
use lb_rs::model::filename::NameComponents;
use lb_rs::model::svg;
use lb_rs::model::svg::buffer::Buffer;
use lb_rs::service::events::{self, Actor, Event};
use lb_rs::{LbResult, Uuid, spawn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, RwLock, mpsc};
use tracing::{debug, error, info, instrument, warn};
use web_time::{Duration, Instant};

use crate::file_cache::{FileCache, FilesExt};
use crate::landing::LandingPage;
use crate::output::Response;
use crate::resolvers::FileCacheLinkResolver;
use crate::resolvers::image_embed::ImageEmbedResolver;
use crate::search::{Search, SearchType};
use crate::show::DocType;
use crate::space_inspector::show::SpaceInspector;
#[cfg(not(target_family = "wasm"))]
use crate::tab::chat::Chat;
use crate::tab::image_viewer::ImageViewer;
use crate::tab::markdown_editor::{
    Editor as Markdown, HttpClient, MdConfig, MdEdit, MdPersistence, MdResources,
};
use crate::tab::pdf_viewer::PdfViewer;
use crate::tab::svg_editor::{CanvasSettings, SVGEditor};
use crate::tab::{
    ContentState, Destination, ExtendedInput as _, Session, SessionId, Tab, TabAction, TabContent,
    TabFailure, TabSaveContent, index_of_dest_to_activate, tab_action_for_open,
};
use crate::task_manager;
use crate::task_manager::{
    CompletedLoad, CompletedSave, CompletedTiming, LoadRequest, SaveRequest, TaskManager,
};
use crate::widgets::image_cache::ImageCache;
use crate::widgets::tab_cache::TabCache;

#[cfg(not(target_family = "wasm"))]
use crate::mind_map::show::MindMap;
#[cfg(not(target_family = "wasm"))]
use tokio::sync::broadcast::error::TryRecvError;

/// A tab removed from the strip, with enough context to reopen it near where
/// it lived. Neighbors are session ids; resolved against the live strip at
/// reopen time. `index` is a layout fallback when anchors are gone.
struct ClosedTab {
    slot: Session,
    left: Option<SessionId>,
    right: Option<SessionId>,
    index: usize,
}

pub struct Workspace {
    // User activity
    pub tabs: TabCache,
    pub tab_strip: Vec<Session>,
    /// Active session (not its destination — two sessions may share a dest).
    pub current_tab: Option<SessionId>,

    /// Most-recently-active last; used to pick focus when the current tab closes.
    activation_history: Vec<SessionId>,
    /// Closed tabs, most recent last (LIFO for `reopen_closed_tab`).
    closed_tabs: Vec<ClosedTab>,

    pub landing_page: LandingPage,
    pub account: Account,

    pub preview: Option<Tab>,

    pending_open_range: Option<(Uuid, std::ops::Range<usize>)>,

    // Files and task status
    pub tasks: TaskManager,
    pub files: Arc<RwLock<FileCache>>,
    pub images: ImageCache,
    pub last_save_all: Option<Instant>,
    pub last_sync_completed: Option<Instant>,

    // Output
    pub out: Response,

    // Resources & configuration
    pub cfg: WsPersistentStore,
    pub ctx: Context,

    pub core: Lb,
    pub lb_rx: events::Receiver<Event>,

    pub show_tabs: bool, // set on mobile to hide the tab strip
    pub tab_strip_left_inset: f32,
    pub tab_strip_min_height: f32,
    pub sidebar_open: bool,
    pub focused_parent: Option<Uuid>, // set to the folder where new files should be created

    // Transient state (consider removing)
    pub landing_page_first_frame: bool,
    pub current_tab_changed: bool, // used to scroll to current tab when it changes
    pub last_touch_event: Option<Instant>, // used to disable tooltips on touch devices
    pub last_set_title: Option<String>, // used to avoid re-setting the window title every frame

    // Transient rename state for the landing page file table
    pub landing_rename_target: Option<lb_rs::Uuid>,
    pub landing_rename_buffer: String,

    pub ws_rx: Receiver<WsUpdates>,
}

pub enum WsUpdates {
    FileCacheComputed(LbResult<FileCache>),
}

impl Workspace {
    pub fn new(
        core: &Lb, ctx: &Context, show_tabs: bool, persist: bool,
        file_cache: Option<Arc<RwLock<FileCache>>>,
    ) -> Self {
        let writable_dir = core.get_config().writeable_path;
        let writeable_dir = Path::new(&writable_dir);
        let writeable_path = writeable_dir.join("ws_persistence.json");
        let files = file_cache.unwrap_or_else(|| {
            Arc::new(RwLock::new(FileCache::new(core).expect("failed to initialize file cache")))
        });
        let cfg =
            WsPersistentStore::new(core.recent_panic().unwrap_or(true), writeable_path, persist);
        let images = ImageCache::new(
            ctx.clone(),
            HttpClient::default(),
            core.clone(),
            Arc::clone(&files),
            cfg.clone(),
        );
        ctx.set_zoom_factor(cfg.get_zoom_factor());

        let (ws_tx, ws_rx) = mpsc::channel();

        let mut ws = Self {
            tabs: TabCache::new(),
            tab_strip: Vec::new(),
            current_tab: None,
            activation_history: Vec::new(),
            closed_tabs: Vec::new(),
            landing_page: cfg.get_landing_page(),
            account: core.get_account().expect("failed to get account"),

            tasks: TaskManager::new(core.clone(), ctx.clone()),
            files,
            images,
            last_sync_completed: Default::default(),
            last_save_all: Default::default(),

            out: Default::default(),

            cfg,
            ctx: ctx.clone(),
            core: core.clone(),

            show_tabs,
            tab_strip_left_inset: 0.0,
            tab_strip_min_height: 0.0,
            sidebar_open: false,
            focused_parent: Default::default(),

            landing_page_first_frame: true,
            current_tab_changed: Default::default(),
            last_touch_event: Default::default(),
            last_set_title: Default::default(),
            landing_rename_target: None,
            landing_rename_buffer: String::new(),
            lb_rx: core.subscribe(),
            preview: None,
            pending_open_range: None,
            ws_rx,
        };

        {
            let files = Arc::clone(&ws.files);
            let files = files.read().unwrap();
            ws.landing_page.update_recent_files(&files);
        }

        let (open_sessions, current_tab_index) = ws.cfg.get_sessions();

        open_sessions.into_iter().for_each(|session| {
            let exists = session
                .dest
                .backing_file()
                .is_none_or(|id| core.get_file_by_id(id).is_ok());
            if exists {
                info!(dest = ?session.dest, "opening persisted session");
                ws.resume_session(session, false);
            }
        });
        if let Some(idx) = current_tab_index {
            if idx < ws.tab_strip.len() {
                info!(idx, "setting persisted current tab");
                ws.make_current(idx);
            }
        }

        let core = ws.core.clone();
        let ctx = ctx.clone();

        #[cfg(not(target_family = "wasm"))]
        spawn!(lb_bg_worker(ctx, core, ws_tx));

        ws
    }

    /// Ensure tab `tab_id` has loaded content for `dest`. Creates content if
    /// absent. Does not add to tab_strip or make current — callers decide
    /// visibility.
    pub fn open_dest(&mut self, tab_id: SessionId, dest: &Destination) {
        self.closed_tabs.retain(|c| c.slot.id != tab_id);

        if self.tabs.contains_key(&tab_id) {
            self.tabs.promote(&tab_id);
            if self
                .tabs
                .get(&tab_id)
                .is_some_and(|t| &t.destination == dest)
            {
                return;
            }
        }
        let file_id = dest.id();
        let mut needs_load = false;
        let content = match dest {
            Destination::File(id) => {
                if self.is_image(*id) {
                    self.image_content(*id)
                } else {
                    needs_load = true;
                    ContentState::Loading(*id)
                }
            }
            #[cfg(not(target_family = "wasm"))]
            Destination::MindMap(_) => {
                ContentState::Open(TabContent::MindMap(MindMap::new(&self.core)))
            }
            #[cfg(target_family = "wasm")]
            Destination::MindMap(_) => return,
            Destination::SpaceInspector(root_id) => {
                let file = self.files.read().unwrap().get_by_id(*root_id).cloned();
                ContentState::Open(TabContent::SpaceInspector(SpaceInspector::new(
                    &self.core,
                    file,
                    self.ctx.clone(),
                )))
            }
            Destination::Search => {
                ContentState::Open(TabContent::Search(Search::new(&self.core, &self.ctx)))
            }
        };
        let now = Instant::now();
        self.tabs.insert(
            tab_id,
            Tab {
                destination: dest.clone(),
                content,
                last_changed: now,
                last_saved: now,
                read_only: false,
                rename: None,
            },
        );
        if needs_load {
            self.tasks.queue_load(LoadRequest {
                id: file_id,
                tab_created: true,
                make_current: false,
                is_preview: false,
            });
        }
    }

    /// Resume a persisted or recently-closed session in a new tab.
    pub fn resume_session(&mut self, session: Session, make_current: bool) {
        let dest = session.dest.clone();
        let tab_id = session.id;
        self.open_dest(tab_id, &dest);
        self.tab_strip.push(session);
        self.out.tabs_changed = true;
        if make_current {
            self.set_current_tab(Some(tab_id));
        }
    }

    pub fn create_tab(&mut self, dest: Destination, make_current: bool) {
        self.resume_session(Session::new(dest), make_current);
    }

    pub fn begin_tab_rename(&mut self, tab_id: SessionId, name: String) {
        if let Some(tab) = self.tabs.get_mut(&tab_id) {
            tab.rename = Some(name);
        }
    }

    pub fn clear_tab_rename(&mut self, tab_id: SessionId) {
        if let Some(tab) = self.tabs.get_mut(&tab_id) {
            tab.rename = None;
        }
    }

    pub fn set_preview(&mut self, id: Option<lb_rs::Uuid>) {
        let id = id.filter(|id| {
            self.files
                .read()
                .unwrap()
                .get_by_id(*id)
                .is_some_and(|f| f.is_document())
        });

        if self.preview.as_ref().and_then(|t| t.id()) == id {
            return;
        }

        match id {
            Some(id) => {
                let now = Instant::now();
                self.preview = Some(Tab {
                    destination: Destination::File(id),
                    content: ContentState::Loading(id),
                    last_changed: now,
                    last_saved: now,
                    read_only: true,
                    rename: None,
                });
                self.tasks.queue_load(LoadRequest {
                    id,
                    tab_created: true,
                    make_current: false,
                    is_preview: true,
                });
            }
            None => self.preview = None,
        }
    }

    pub fn get_mut_tab_by_id(&mut self, id: Uuid) -> Option<&mut Tab> {
        let tab_id = self.tab_strip.iter().find(|s| s.dest.id() == id)?.id;
        self.tabs.get_mut(&tab_id)
    }

    pub fn get_idx_by_id(&mut self, id: Uuid) -> Option<usize> {
        self.tab_strip.iter().position(|s| s.dest.id() == id)
    }

    pub fn current_slot_index(&self) -> Option<usize> {
        let id = self.current_tab?;
        self.tab_strip.iter().position(|s| s.id == id)
    }

    pub fn current_dest(&self) -> Option<&Destination> {
        let id = self.current_tab?;
        self.tab_strip.iter().find(|s| s.id == id).map(|s| &s.dest)
    }

    pub fn is_empty(&self) -> bool {
        self.tab_strip.is_empty()
    }

    pub fn current_tab(&self) -> Option<&Tab> {
        self.current_tab.as_ref().and_then(|id| self.tabs.get(id))
    }

    pub fn current_tab_id(&self) -> Option<Uuid> {
        self.current_tab().and_then(|tab| tab.id())
    }

    fn mark_current_tab_changed(&mut self) {
        self.current_tab_changed = true;
        self.out.selected_file = self.current_tab_id();
    }

    fn set_current_tab(&mut self, tab_id: Option<SessionId>) {
        if let Some(old_id) = self.current_tab.take() {
            if Some(old_id) != tab_id {
                self.activation_history.retain(|id| *id != old_id);
                self.activation_history.push(old_id);
            }
        }
        self.current_tab = tab_id;
        self.mark_current_tab_changed();
    }

    /// Reload every open chat's provider config off-thread. Called when an
    /// `/.agent` file changes (edited, deleted from the tree, or synced) so a
    /// chat reflects the change without needing a tab switch.
    #[cfg(not(target_family = "wasm"))]
    fn reload_chat_configs(&mut self) {
        for tab in self.tabs.values_mut() {
            if let Some(chat) = tab.chat_mut() {
                chat.kick_config_load();
            }
        }
    }

    pub fn current_tab_title(&self) -> Option<String> {
        self.current_tab().map(|tab| self.tab_title(tab))
    }

    pub fn current_tab_mut(&mut self) -> Option<&mut Tab> {
        self.current_tab.and_then(|id| self.tabs.get_mut(&id))
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

    /// The active editable text widget for native (iOS) text input — the
    /// markdown document's editor or the chat tab's composer. Lets the
    /// `UITextInput` FFI bridge target whichever editor is current without
    /// caring which kind of tab owns it.
    pub fn focused_mdedit_mut(&mut self) -> Option<&mut MdEdit> {
        let tab = self.current_tab_mut()?;
        if tab.markdown().is_some() {
            tab.markdown_mut().map(|md| &mut md.edit)
        } else {
            #[cfg(not(target_family = "wasm"))]
            {
                // While the connect step is open, the key field is the focused
                // editor so the native keyboard/caret target it, not the
                // composer.
                tab.chat_mut().map(|chat| chat.focused_field())
            }
            #[cfg(target_family = "wasm")]
            {
                None
            }
        }
    }

    pub fn make_current(&mut self, i: usize) -> bool {
        let Some(slot) = self.tab_strip.get(i) else { return false };
        let tab_id = slot.id;
        let dest = slot.dest.clone();
        // Tab content may only live in the previous-frame buffer after close.
        self.tabs.promote(&tab_id);
        if self.tabs.get(&tab_id).is_none() {
            self.open_dest(tab_id, &dest);
        }
        if self.tabs.get(&tab_id).is_none() {
            return false;
        };
        self.set_current_tab(Some(tab_id));

        if let Some(md) = self.current_tab_markdown() {
            md.focus(&self.ctx);
        }

        self.ctx.request_repaint();

        true
    }

    /// Makes the tab with the given id the current tab, if it exists. Returns true if the tab exists.
    pub fn make_current_by_id(&mut self, id: Uuid) -> bool {
        let dest = Destination::File(id);
        if let Some(i) = index_of_dest_to_activate(
            &self.tab_strip,
            self.current_tab,
            &self.activation_history,
            &dest,
        ) {
            self.make_current(i)
        } else {
            false
        }
    }

    pub fn save_all_tabs(&mut self) {
        let slots: Vec<_> = self.tab_strip.clone();
        for slot in &slots {
            if let Some(tab) = self.tabs.get(&slot.id) {
                if let Some(id) = tab.id() {
                    if tab.is_dirty(&self.tasks) {
                        self.tasks.queue_save(SaveRequest { id, origin: slot.id });
                    }
                }
            }
        }
        self.last_save_all = Some(Instant::now());
    }

    pub fn save_tab(&mut self, i: usize) {
        let Some(slot) = self.tab_strip.get(i) else { return };
        let tab_id = slot.id;
        if let Some(tab) = self.tabs.get(&tab_id) {
            if let Some(id) = tab.id() {
                if tab.is_dirty(&self.tasks) {
                    self.tasks.queue_save(SaveRequest { id, origin: tab_id });
                }
            }
        }
    }

    fn image_content(&self, id: Uuid) -> ContentState {
        ContentState::Open(TabContent::Image(ImageViewer::new(id, self.images.clone())))
    }

    fn is_image(&self, id: Uuid) -> bool {
        self.files
            .read()
            .unwrap()
            .get_by_id(id)
            .map(|f| DocType::from_name(&f.name) == DocType::Image)
            .unwrap_or(false)
    }

    pub(crate) fn is_folder(&self, id: Uuid) -> bool {
        self.files
            .read()
            .unwrap()
            .get_by_id(id)
            .map(|f| f.is_folder())
            .unwrap_or(false)
    }

    /// How a file-tree / pins / recents / create-file open should treat tabs.
    /// `in_new_tab` is the explicit "open in new tab" request.
    fn tab_action_for_open(&self, dest: &Destination, in_new_tab: bool) -> TabAction {
        tab_action_for_open(
            self.tab_strip.iter().any(|s| &s.dest == dest),
            in_new_tab,
            self.show_tabs,
            self.cfg.get_open_in_new_tab(),
        )
    }

    /// Create a session for `dest` (file tree, pins, recents, create file).
    /// Tab policy: activate / replace / create based on platform, settings,
    /// and the explicit `in_new_tab` flag.
    pub fn open_file(&mut self, id: Uuid, make_current: bool, in_new_tab: bool) {
        self.open_dest_as_session(Destination::File(id), make_current, in_new_tab);
    }

    pub fn open_dest_as_session(
        &mut self, dest: Destination, make_current: bool, in_new_tab: bool,
    ) {
        match self.tab_action_for_open(&dest, in_new_tab) {
            TabAction::Activate => {
                if let Some(pos) = index_of_dest_to_activate(
                    &self.tab_strip,
                    self.current_tab,
                    &self.activation_history,
                    &dest,
                ) {
                    if make_current {
                        self.make_current(pos);
                    }
                    self.closed_tabs.retain(|c| c.slot.dest != dest);
                } else {
                    self.create_tab(dest, make_current);
                }
            }
            TabAction::Create => self.create_tab(dest, make_current),
            TabAction::Replace => self.replace_current_session(dest, make_current),
        }
    }

    /// Reuse the current tab for a fresh session at `dest` (empty history).
    /// Creates a tab if the strip is empty.
    pub fn replace_current_session(&mut self, dest: Destination, make_current: bool) {
        let Some(slot_idx) = self.current_slot_index() else {
            self.create_tab(dest, make_current);
            return;
        };
        let tab_id = self.tab_strip[slot_idx].id;
        if self.tab_strip[slot_idx].dest == dest && self.tab_strip[slot_idx].back.is_empty() {
            if make_current {
                self.make_current(slot_idx);
            }
            return;
        }
        self.tab_strip[slot_idx].replace(dest.clone());
        self.open_dest(tab_id, &dest);
        self.out.tabs_changed = true;
        if make_current {
            self.set_current_tab(Some(tab_id));
        }
    }

    /// Navigate the current tab to `dest`, pushing the previous dest onto
    /// its back stack. Used for links, search results, mind map, space inspector.
    pub fn navigate_to(&mut self, dest: Destination) {
        let Some(slot_idx) = self.current_slot_index() else {
            self.create_tab(dest, true);
            return;
        };
        if self.tab_strip[slot_idx].dest == dest {
            self.make_current(slot_idx);
            return;
        }
        let tab_id = self.tab_strip[slot_idx].id;
        self.tab_strip[slot_idx].navigate(dest.clone());
        self.open_dest(tab_id, &dest);
        self.out.tabs_changed = true;
        self.set_current_tab(Some(tab_id));
    }

    /// Replace the Search tab with `dest`. If that dest is already open,
    /// focus it and close Search instead. Desktop result clicks use this so
    /// they do not follow the open-in-new-tab setting.
    pub fn open_file_replacing_search(&mut self, id: Uuid) {
        let dest = Destination::File(id);
        let current_search = self.current_slot_index().filter(|&i| {
            self.tab_strip
                .get(i)
                .is_some_and(|s| matches!(s.dest, Destination::Search))
        });
        if self.tab_strip.iter().any(|s| s.dest == dest) {
            if let Some(i) = current_search {
                self.close_tab(i);
            }
            if let Some(pos) = self.tab_strip.iter().position(|s| s.dest == dest) {
                self.make_current(pos);
            }
            return;
        }
        if current_search.is_some() {
            self.replace_current_session(dest, true);
            return;
        }
        self.create_tab(dest, true);
    }

    pub fn open_file_at_range(
        &mut self, id: Uuid, byte_range: std::ops::Range<usize>, in_new_tab: bool,
    ) {
        self.open_file(id, true, in_new_tab);
        self.pending_open_range = Some((id, byte_range));
    }

    pub fn navigate_to_range(&mut self, id: Uuid, byte_range: std::ops::Range<usize>) {
        self.navigate_to(Destination::File(id));
        self.pending_open_range = Some((id, byte_range));
    }

    pub(crate) fn apply_pending_open_range(&mut self) {
        let Some((id, range)) = self.pending_open_range.clone() else { return };
        if let Some(md) = self.get_mut_tab_by_id(id).and_then(|t| t.markdown_mut()) {
            if md.initialized {
                md.open_navigate(range);
                self.pending_open_range = None;
            }
        } else if self.tab_strip.iter().all(|s| s.dest.id() != id) {
            self.pending_open_range = None;
        }
    }

    pub fn back(&mut self) {
        let Some(slot_idx) = self.current_slot_index() else { return };
        if !self.tab_strip[slot_idx].go_back() {
            return;
        }
        let tab_id = self.tab_strip[slot_idx].id;
        let dest = self.tab_strip[slot_idx].dest.clone();
        self.open_dest(tab_id, &dest);
        self.out.tabs_changed = true;
        self.set_current_tab(Some(tab_id));
    }

    pub fn can_back(&self) -> bool {
        self.current_slot_index()
            .and_then(|i| self.tab_strip.get(i))
            .is_some_and(|s| !s.back.is_empty())
    }

    pub fn forward(&mut self) {
        let Some(slot_idx) = self.current_slot_index() else { return };
        if !self.tab_strip[slot_idx].go_forward() {
            return;
        }
        let tab_id = self.tab_strip[slot_idx].id;
        let dest = self.tab_strip[slot_idx].dest.clone();
        self.open_dest(tab_id, &dest);
        self.out.tabs_changed = true;
        self.set_current_tab(Some(tab_id));
    }

    pub fn can_forward(&self) -> bool {
        self.current_slot_index()
            .and_then(|i| self.tab_strip.get(i))
            .is_some_and(|s| !s.forward.is_empty())
    }

    pub fn move_tab(&mut self, from: usize, to: usize) {
        if from == to || from >= self.tab_strip.len() || to >= self.tab_strip.len() {
            return;
        }

        let slot = self.tab_strip.remove(from);
        self.tab_strip.insert(to, slot);
        self.out.tabs_changed = true;
        self.ctx.request_repaint();
    }

    pub fn close_tab(&mut self, i: usize) {
        let Some(slot) = self.tab_strip.get(i) else { return };
        let tab_id = slot.id;
        #[cfg(not(target_family = "wasm"))]
        if let Some(tab) = self.tabs.get_mut(&tab_id) {
            if let ContentState::Open(TabContent::MindMap(mm)) = &mut tab.content {
                mm.stop();
            }
        }

        self.save_tab(i);

        if let Some(tab) = self.tabs.get_mut(&tab_id) {
            if let Some(md) = tab.markdown_mut() {
                md.surrender_focus(&self.ctx);
            }
        }

        // Capture placement before remove: adjacent tab ids + index.
        let left = i
            .checked_sub(1)
            .and_then(|j| self.tab_strip.get(j))
            .map(|s| s.id);
        let right = self.tab_strip.get(i + 1).map(|s| s.id);

        let closed_slot = self.tab_strip.remove(i);
        self.out.tabs_changed = true;
        self.activation_history.retain(|id| *id != tab_id);
        self.closed_tabs.retain(|c| c.slot.id != tab_id);
        self.closed_tabs
            .push(ClosedTab { slot: closed_slot, left, right, index: i });
        const MAX_CLOSED_TABS: usize = 20;
        if self.closed_tabs.len() > MAX_CLOSED_TABS {
            self.closed_tabs.remove(0);
        }

        if self.current_tab == Some(tab_id) {
            let mut previous = None;
            while let Some(id) = self.activation_history.pop() {
                if self.tab_strip.iter().any(|s| s.id == id) {
                    previous = Some(id);
                    break;
                }
            }
            let next = previous.or_else(|| {
                if self.tab_strip.is_empty() {
                    None
                } else {
                    let new_idx = i.min(self.tab_strip.len() - 1);
                    Some(self.tab_strip[new_idx].id)
                }
            });
            self.current_tab = None;
            self.set_current_tab(next);
        }
    }

    pub fn close_all_tabs(&mut self) {
        while !self.tab_strip.is_empty() {
            self.close_tab(0);
        }
    }

    pub fn close_other_tabs(&mut self, keep: usize) {
        let Some(keep_id) = self.tab_strip.get(keep).map(|s| s.id) else {
            return;
        };
        for i in (0..self.tab_strip.len()).rev() {
            if self.tab_strip.get(i).is_some_and(|s| s.id != keep_id) {
                self.close_tab(i);
            }
        }
    }

    pub fn close_tabs_to_left(&mut self, i: usize) {
        if i == 0 || i >= self.tab_strip.len() {
            return;
        }
        for j in (0..i).rev() {
            self.close_tab(j);
        }
    }

    pub fn close_tabs_to_right(&mut self, i: usize) {
        if i + 1 >= self.tab_strip.len() {
            return;
        }
        for j in (i + 1..self.tab_strip.len()).rev() {
            self.close_tab(j);
        }
    }

    pub fn can_reopen_closed_tab(&self) -> bool {
        !self.closed_tabs.is_empty()
    }

    /// Restores the most recently closed tab (with back/forward) near its
    /// prior place in the strip, and focuses it.
    pub fn reopen_closed_tab(&mut self) {
        while let Some(closed) = self.closed_tabs.pop() {
            if self.restore_closed_tab(closed) {
                return;
            }
        }
    }

    /// Restores a closed file tab by id (most recent matching entry), preserving
    /// its back/forward stack and strip placement. Falls back to opening the
    /// file in a **new** tab if it is not in the closed stack.
    pub fn reopen_closed_file(&mut self, id: Uuid) {
        let dest = Destination::File(id);

        if let Some(pos) = self.tab_strip.iter().position(|s| s.dest == dest) {
            self.closed_tabs.retain(|c| c.slot.dest != dest);
            self.make_current(pos);
            return;
        }

        while let Some(i) = self.closed_tabs.iter().rposition(|c| c.slot.dest == dest) {
            let closed = self.closed_tabs.remove(i);
            if self.restore_closed_tab(closed) {
                return;
            }
        }

        // Not in history (or all entries invalid): always new tab, never in-place.
        self.create_tab(dest, true);
    }

    /// Insert a closed slot back onto the strip and load content.
    fn restore_closed_tab(&mut self, closed: ClosedTab) -> bool {
        let dest = closed.slot.dest.clone();
        let tab_id = closed.slot.id;
        if self.tab_strip.iter().any(|s| s.id == tab_id) {
            return false;
        }
        let exists = dest
            .backing_file()
            .is_none_or(|id| self.files.read().unwrap().get_by_id(id).is_some());
        if !exists {
            return false;
        }

        let at = self.reopen_insert_index(&closed);
        self.open_dest(tab_id, &dest);
        self.tabs.promote(&tab_id);
        self.tab_strip.insert(at, closed.slot);
        self.out.tabs_changed = true;
        if !self.make_current(at) {
            self.set_current_tab(Some(tab_id));
        }
        true
    }

    /// Resolve where a closed tab should land in the current strip.
    /// Prefer adjacent tab ids still open; else fall back to saved index.
    fn reopen_insert_index(&self, closed: &ClosedTab) -> usize {
        let pos = |id: &SessionId| self.tab_strip.iter().position(|s| &s.id == id);
        let left = closed.left.as_ref().and_then(pos);
        let right = closed.right.as_ref().and_then(pos);

        if let (Some(l), Some(r)) = (left, right) {
            if l < r {
                return l + 1;
            }
        }
        if let Some(l) = left {
            return l + 1;
        }
        if let Some(r) = right {
            return r;
        }
        closed.index.min(self.tab_strip.len())
    }

    /// File ids from `closed_tabs`, most recently closed first.
    pub fn recently_closed_tabs(&self) -> Vec<Uuid> {
        self.closed_tabs
            .iter()
            .rev()
            .filter_map(|c| match c.slot.dest {
                Destination::File(id) => Some(id),
                _ => None,
            })
            .collect()
    }

    pub fn process_bg_tasks(&mut self) {
        loop {
            match self.ws_rx.try_recv() {
                Ok(WsUpdates::FileCacheComputed(file_cache)) => {
                    let file_cache = file_cache.unwrap();
                    self.landing_page.update_recent_files(&file_cache);
                    *self.files.write().unwrap() = file_cache;
                    self.out.file_cache_updated = true;

                    for tab in self.tabs.values_mut() {
                        if let Some(md) = tab.markdown_mut() {
                            let renderer = &md.edit.renderer;
                            renderer.layout_cache.link_seq.store(
                                renderer.ws_seq.fetch_add(1, Ordering::Relaxed),
                                Ordering::Relaxed,
                            );
                        }
                    }
                    let files = self.files.read().unwrap();
                    let mut ids_to_delete = vec![];
                    for slot in &self.tab_strip {
                        if let Destination::File(id) = slot.dest {
                            if files.get_by_id(id).is_none() {
                                ids_to_delete.push(slot.id);
                            }
                        }
                    }
                    drop(files);

                    for tab_id in ids_to_delete {
                        if let Some(idx) = self.tab_strip.iter().position(|s| s.id == tab_id) {
                            self.close_tab(idx);
                        }
                    }
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    error!("ws_rx disconnected");
                    break;
                }
            }
        }
    }

    #[instrument(level = "trace", skip_all)]
    pub fn process_lb_updates(&mut self) {
        loop {
            match self.lb_rx.try_recv() {
                Ok(evt) => {
                    match evt {
                        Event::DocumentWritten(id, actor) => {
                            let event_origin = match actor {
                                Actor::Sync => {
                                    self.core.app_foregrounded();
                                    None
                                }
                                Actor::User(origin) => origin,
                            };
                            let open_session = self
                                .tab_strip
                                .iter()
                                .find(|s| s.dest.backing_file() == Some(id));
                            if let Some(session) = open_session {
                                if event_origin != Some(session.id.as_uuid()) {
                                    self.tasks.queue_load(LoadRequest {
                                        id,
                                        tab_created: false,
                                        make_current: false,
                                        is_preview: false,
                                    });
                                }
                            }
                            // A provider/prompt file's contents changed (edited
                            // here or synced) — refresh the chats that read it.
                            #[cfg(not(target_family = "wasm"))]
                            {
                                let is_agent_config =
                                    self.files.read().unwrap().path(id).starts_with("/.agent/");
                                if is_agent_config {
                                    self.reload_chat_configs();
                                }
                            }
                        }
                        // Create/delete/move/rename carries no id, so a deleted
                        // provider file (e.g. from the file-tree menu) can't be
                        // matched — refresh every chat's config unconditionally.
                        #[cfg(not(target_family = "wasm"))]
                        Event::MetadataChanged(_) => self.reload_chat_configs(),
                        _ => {}
                    }
                }
                #[cfg(not(target_family = "wasm"))]
                Err(TryRecvError::Empty) => {
                    break;
                }
                Err(e) => {
                    eprintln!("cannot recv events from lb-rs {e:?}");
                    break;
                }
            }
        }
    }

    /// Handle clipboard-like events (`Drop`/`Paste`). For image clips the
    /// workspace imports the image as a lockbook file and pushes a
    /// `Markdown::Replace` event with a relative-path `![…](…)` markdown
    /// link; the editor then processes it in its own `process_events` later
    /// this frame.
    ///
    /// Only runs when the current tab is a non-readonly markdown editor —
    /// other tab types (SVG, image viewer, PDF) handle clipboard events
    /// themselves. Non-clip events are left in the queue.
    #[instrument(level = "trace", skip_all)]
    pub fn process_clip_events(&mut self) {
        let Some(file_id) = self.current_tab().and_then(|tab| {
            let md = tab.markdown()?;
            // image paste inserts `![](…)` markdown — skip when the
            // renderer can't display it as a real image.
            if md.edit.renderer.readonly || md.edit.renderer.plaintext {
                None
            } else {
                Some(md.edit.file_id)
            }
        }) else {
            return;
        };

        let events = self.ctx.pop_events_where(&mut |e| {
            matches!(e, crate::tab::Event::Drop { .. } | crate::tab::Event::Paste { .. })
        });
        if events.is_empty() {
            return;
        }

        for event in events {
            let content = match event {
                crate::tab::Event::Drop { content, .. }
                | crate::tab::Event::Paste { content, .. } => content,
                _ => continue,
            };
            for clip in content {
                match clip {
                    crate::tab::ClipContent::Image(data) => {
                        let file = crate::tab::import_image(&self.core, file_id, &data);

                        // Refresh before the markdown event lands: the image
                        // cache's URL→id lookup reads `self.files` on first
                        // load and caches a sticky "image not found" failure
                        // if the file isn't there yet.
                        *self.files.write().unwrap() =
                            FileCache::new(&self.core).expect("failed to refresh file cache");

                        let rel_path = {
                            let guard = self.files.read().unwrap();
                            let parent = guard.get_by_id(file_id).unwrap().parent;
                            crate::file_cache::relative_path(
                                &guard.path(parent),
                                &guard.path(file.id),
                            )
                        };
                        let link = format!("![{}]({})", file.name, rel_path);

                        self.ctx
                            .push_markdown_event(crate::tab::markdown_editor::Event::Replace {
                                region: crate::tab::markdown_editor::input::Region::Selection,
                                text: link,
                                advance_cursor: true,
                            });
                    }
                    crate::tab::ClipContent::Files(..) => {
                        // todo: support file drop & paste
                    }
                }
            }
        }
    }

    // #[instrument(level = "trace", skip_all)]
    pub fn process_task_updates(&mut self) {
        let task_manager::Response { completed_loads, completed_saves } = self.tasks.update();

        let start = Instant::now();
        for load in completed_loads {
            // scope indentation preserves git history
            {
                let CompletedLoad {
                    request: LoadRequest { id, tab_created, make_current, is_preview },
                    content_result,
                    timing: _,
                } = load;

                let ctx = self.ctx.clone();
                let core = self.core.clone();
                let show_tabs = self.show_tabs;

                let tab_opt = if is_preview {
                    self.preview.as_mut().filter(|t| t.id() == Some(id))
                } else {
                    self.tabs.find_for_load_mut(id)
                };
                if let Some(tab) = tab_opt {
                    let files_clone = self.files.clone();
                    let files_guard = files_clone.read().unwrap();

                    let account = &self.account;
                    let (name, read_only) = if let Some(file) = files_guard.get_by_id(id) {
                        (file.name.clone(), files_guard.access(id, account) == UserAccessMode::Read)
                    } else if let Ok(file) = core.get_file_by_id(id) {
                        // read-through (can remove when we master cache
                        // refreshes): a freshly created file isn't in the
                        // async-rebuilt cache yet; without this the
                        // completed load is discarded and the tab is stuck
                        // loading forever. New/owned files are writable.
                        (file.name, false)
                    } else {
                        // genuinely gone (e.g. deleted): fail the tab
                        // rather than discard the load and hang in Loading.
                        let msg = format!("failed to load file: {id} not found");
                        error!(msg);
                        tab.content = ContentState::Failed(TabFailure::SimpleMisc(msg.clone()));
                        self.out.failure_messages.push(msg);
                        continue;
                    };

                    let doc_type = DocType::from_name(&name);
                    let ext = name.split('.').next_back().unwrap_or_default().to_owned();

                    let (maybe_hmac, bytes) = match content_result {
                        Ok((hmac, bytes)) => (hmac, bytes),
                        Err(err) => {
                            let msg = format!("failed to load file: {err:?}");
                            error!(msg);
                            tab.content = ContentState::Failed(TabFailure::Unexpected(msg.clone()));
                            self.out.failure_messages.push(msg);
                            continue;
                        }
                    };

                    tab.read_only = read_only || is_preview;

                    match doc_type {
                        DocType::Image => {
                            tab.content = ContentState::Open(TabContent::Image(ImageViewer::new(
                                id,
                                self.images.clone(),
                            )));
                        }
                        DocType::PDF => {
                            tab.content = ContentState::Open(TabContent::Pdf(PdfViewer::new(
                                id, bytes, &ctx,
                                !show_tabs, // todo: use settings to determine toolbar visibility
                            )));
                        }
                        DocType::SVG => {
                            let reload = if tab.svg().is_some() { !tab_created } else { false };
                            if !reload {
                                tab.content = ContentState::Open(TabContent::Svg(SVGEditor::new(
                                    &bytes,
                                    &ctx,
                                    core.clone(),
                                    id,
                                    maybe_hmac,
                                    &self.cfg,
                                    tab.read_only,
                                )));
                            } else {
                                let svg = tab.svg_mut().unwrap();

                                Buffer::reload(
                                    &mut svg.buffer.elements,
                                    &mut svg.buffer.weak_images,
                                    &mut svg.buffer.weak_path_pressures,
                                    &mut svg.buffer.weak_viewport_settings,
                                    &svg.opened_content,
                                    &svg::buffer::Buffer::new(
                                        String::from_utf8_lossy(&bytes).as_ref(),
                                    ),
                                );

                                svg.open_file_hmac = maybe_hmac;
                            }
                        }
                        #[cfg(not(target_family = "wasm"))]
                        DocType::Chat => {
                            let reload = tab.chat().is_some() && !tab_created;
                            if !reload {
                                tab.content = ContentState::Open(TabContent::Chat(Chat::new(
                                    &bytes,
                                    id,
                                    maybe_hmac,
                                    self.account.clone(),
                                    self.ctx.clone(),
                                    Arc::clone(&self.files),
                                    &self.core,
                                )));
                            } else {
                                let chat = tab.chat_mut().unwrap();
                                chat.reload(&bytes, maybe_hmac);
                            }
                        }
                        DocType::PlainText
                        | DocType::Markdown
                        | DocType::Code
                        | DocType::Unknown
                            if content_inspector::inspect(&bytes).is_text() =>
                        {
                            let reload =
                                if tab.markdown().is_some() { !tab_created } else { false };
                            if !reload {
                                tab.content =
                                    ContentState::Open(TabContent::Markdown(Markdown::new(
                                        &String::from_utf8_lossy(&bytes),
                                        id,
                                        maybe_hmac,
                                        MdResources {
                                            ctx: self.ctx.clone(),
                                            core: core.clone(),
                                            persistence: self.cfg.clone(),
                                            link_resolver: Box::new(FileCacheLinkResolver::new(
                                                Arc::clone(&self.files),
                                                id,
                                            )),
                                            files: Arc::clone(&self.files),
                                            embeds: Box::new(ImageEmbedResolver::new(
                                                self.images.clone(),
                                                id,
                                            )),
                                        },
                                        MdConfig {
                                            readonly: tab.read_only,
                                            ext: ext.clone(),
                                            tablet_or_desktop: show_tabs,
                                        },
                                    )));
                            } else {
                                let md = tab.markdown_mut().unwrap();
                                md.edit
                                    .renderer
                                    .buffer
                                    .reload(String::from_utf8_lossy(&bytes).into());
                                md.hmac = maybe_hmac;
                            }
                        }
                        _ => {
                            tab.content = ContentState::Failed(TabFailure::SimpleMisc(format!(
                                "Unsupported file extension: {ext}"
                            )));
                        }
                    };

                    if is_preview {
                        if let Some(md) = tab.markdown_mut() {
                            md.initialized = true;
                            md.id_salt = egui::Id::new("search_preview");
                        }
                        // A chat's first frame would focus its composer,
                        // stealing focus from the search query.
                        #[cfg(not(target_family = "wasm"))]
                        if let Some(chat) = tab.chat_mut() {
                            chat.initialized = true;
                        }
                    } else {
                        self.out.tabs_changed = true;
                    }
                } else if !is_preview {
                    error!("failed to load file: tab not found");
                };

                if make_current {
                    self.make_current_by_id(id);
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
                        request: SaveRequest { id, origin: _ },
                        seq,
                        content,
                        new_hmac_result,
                        timing: CompletedTiming { queued_at: _, started_at, completed_at: _ },
                    } = save;

                    if let Some(tab) = self.tabs.find_any_by_file_mut(id) {
                        match new_hmac_result {
                            Ok(hmac) => {
                                tab.last_saved = started_at;
                                if let Some(md) = tab.markdown_mut() {
                                    if let TabSaveContent::String(content) = content {
                                        md.hmac = Some(hmac);
                                        md.edit.renderer.buffer.saved(seq, content);
                                    }
                                } else if let Some(svg) = tab.svg_mut() {
                                    if let TabSaveContent::Svg(content) = content {
                                        svg.open_file_hmac = Some(hmac);
                                        svg.opened_content = *content;
                                    }
                                } else {
                                    #[cfg(not(target_family = "wasm"))]
                                    if let Some(chat) = tab.chat_mut() {
                                        if let TabSaveContent::Bytes(content) = content {
                                            chat.saved(hmac, content);
                                        }
                                    }
                                }
                            }
                            Err(err) => {
                                if err.kind == LbErrKind::ReReadRequired {
                                    debug!(
                                        "reloading file after save failed with re-read required: {}",
                                        id
                                    );
                                    self.tasks.queue_load(LoadRequest {
                                        id,
                                        tab_created: false,
                                        make_current: false,
                                        is_preview: false,
                                    });
                                } else {
                                    tab.content = ContentState::Failed(TabFailure::Unexpected(
                                        format!("{err:?}"),
                                    ))
                                }
                            }
                        }
                    }
                }
            }
        }
        start.warn_after("processing completed saves", Duration::from_millis(100));

        // background work: queue
        let now = Instant::now();

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

        // background work: launch
        let start = Instant::now();
        self.tasks.check_launch(&self.tabs);
        start.warn_after("processing task launch", Duration::from_millis(100));
    }

    pub fn create_doc_at(&mut self, is_drawing: bool, parent: Uuid) {
        let file_format = if is_drawing { "svg" } else { "md" };
        let date = Local::now().format("%Y-%m-%d");
        let mut new_file = NameComponents {
            name: date.to_string(),
            variant: None,
            extension: Some(file_format.into()),
        };
        new_file.next_in_children(self.core.get_children(&parent).unwrap());

        let result = self
            .core
            .create_file(new_file.to_name().as_str(), &parent, FileType::Document)
            .map_err(|err| format!("{err:?}"));

        if let Ok(file) = &result {
            self.files
                .write()
                .unwrap()
                .insert_created_file(file.clone());
            self.out.file_cache_updated = true;
        }
        self.out.file_created = Some(result);
        self.ctx.request_repaint();
    }

    pub fn create_folder_at(&mut self, parent: Uuid) {
        let date = Local::now().format("%Y-%m-%d");
        let mut new_file =
            NameComponents { name: date.to_string(), variant: None, extension: None };
        new_file.next_in_children(self.core.get_children(&parent).unwrap());

        let result = self
            .core
            .create_file(new_file.to_name().as_str(), &parent, FileType::Folder)
            .map_err(|err| format!("{err:?}"));

        if let Ok(file) = &result {
            self.files
                .write()
                .unwrap()
                .insert_created_file(file.clone());
            self.out.file_cache_updated = true;
        }
        self.out.file_created = Some(result);
        self.ctx.request_repaint();
    }

    pub fn effective_focused_parent(&self) -> Uuid {
        let get_by_id_cached_read_through = |id| {
            let files_arc = Arc::clone(&self.files);
            let files_guard = files_arc.read().unwrap();
            files_guard.get_by_id(id).cloned()
        };

        let focused_parent = || {
            if let Some(focused_parent) =
                self.focused_parent.and_then(get_by_id_cached_read_through)
            {
                return focused_parent;
            }
            if let Some(current_tab) = self
                .current_tab_id()
                .and_then(get_by_id_cached_read_through)
            {
                return current_tab;
            }

            let files_arc = Arc::clone(&self.files);
            let files_guard = files_arc.read().unwrap();
            files_guard.root.clone()
        };

        let focused_parent = focused_parent();

        if focused_parent.file_type == FileType::Document {
            focused_parent.parent
        } else {
            focused_parent.id
        }
    }

    pub fn create_doc(&mut self, is_drawing: bool) {
        let focused_parent = self.effective_focused_parent();
        self.create_doc_at(is_drawing, focused_parent);
    }

    pub fn create_folder(&mut self) {
        let focused_parent = self.effective_focused_parent();
        self.create_folder_at(focused_parent);
    }

    /// Opens or focuses the tab for the mind map
    #[cfg(not(target_family = "wasm"))]
    pub fn upsert_mind_map(&mut self, _core: Lb) {
        if let Some(i) = self
            .tab_strip
            .iter()
            .position(|s| matches!(s.dest, Destination::MindMap(_)))
        {
            self.make_current(i);
        } else {
            let root_id = self.core.get_root().map(|r| r.id).unwrap_or_default();
            self.create_tab(Destination::MindMap(root_id), true);
        };
    }
    #[cfg(target_family = "wasm")]
    pub fn upsert_mind_map(&mut self, core: Lb) {
        warn!("Mind map is not supported on wasm targets");
    }

    pub fn upsert_search(&mut self, search_type: Option<SearchType>) {
        if cfg!(target_os = "ios") {
            return;
        }
        if let Some(i) = self
            .tab_strip
            .iter()
            .position(|s| matches!(s.dest, Destination::Search))
        {
            self.make_current(i);
        } else {
            self.create_tab(Destination::Search, true);
        }
        // refocus the query field each time the tab is opened/focused
        if let Some(tab) = self.current_tab_mut() {
            if let ContentState::Open(TabContent::Search(search)) = &mut tab.content {
                if let Some(search_type) = search_type {
                    search.search_type = search_type;
                }
                search.initialized = false;
            }
        }
    }

    pub fn search_in_folder(&mut self, folder_id: Uuid) {
        self.upsert_search(Some(SearchType::Content));
        let path = self.files.read().unwrap().path(folder_id);
        if let Some(tab) = self.current_tab_mut() {
            if let ContentState::Open(TabContent::Search(search)) = &mut tab.content {
                search.scope_path = path;
                search.filters_open = true;
            }
        }
        self.out.selected_file = Some(folder_id);
    }

    pub fn start_space_inspector(&mut self, _core: Lb, folder: Option<File>) {
        if let Some(i) = self
            .tab_strip
            .iter()
            .position(|s| matches!(s.dest, Destination::SpaceInspector(_)))
        {
            self.close_tab(i);
        }
        let root_id = folder
            .map(|f| f.id)
            .unwrap_or_else(|| self.core.get_root().map(|r| r.id).unwrap_or_default());
        self.create_tab(Destination::SpaceInspector(root_id), true);
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
                        .push(format!("Rename failed: {kind}"));
                }
                warn!(?id, "failed to rename file: {:?}", kind);
            }
        }
    }

    pub fn file_renamed(&mut self, id: Uuid, new_name: String) {
        let mut different_file_type = false;
        let tab_id = self
            .tab_strip
            .iter()
            .find(|s| s.dest.id() == id)
            .map(|s| s.id);
        if let Some(tab) = tab_id.and_then(|tid| self.tabs.get(&tid)) {
            different_file_type = !NameComponents::from(&new_name)
                .extension
                .eq(&NameComponents::from(&self.tab_title(tab)).extension);
        }

        if different_file_type {
            // `ext`/`plaintext` are baked at editor construction; drop and re-open.
            let dest = Destination::File(id);
            if let Some(tab_id) = tab_id {
                self.tabs.remove(&tab_id);
                self.open_dest(tab_id, &dest);
            }
        }

        self.ctx.request_repaint();
    }

    pub fn move_file(&mut self, req: (Uuid, Uuid)) {
        let (id, new_parent) = req;
        match self.core.move_file(&id, &new_parent) {
            Ok(()) => {
                self.ctx.request_repaint();
            }
            Err(LbErr { kind, .. }) => {
                self.out
                    .failure_messages
                    .push(format!("Move failed: {kind}"));
                warn!(?id, "failed to move file: {:?}", kind);
            }
        }
    }

    pub fn delete_file(&mut self, id: Uuid) {
        match self.core.delete_file(&id) {
            Ok(()) => {
                self.out.file_deleted = Some(id);
                self.ctx.request_repaint();
            }
            Err(LbErr { kind, .. }) => {
                self.out
                    .failure_messages
                    .push(format!("Delete failed: {kind}"));
                warn!(?id, "failed to delete file: {:?}", kind);
            }
        }
    }
}

#[derive(Clone)]
pub struct WsPersistentStore {
    pub path: PathBuf,
    pub data: Arc<RwLock<WsPresistentData>>,
    enabled: bool,
}

fn default_open_in_new_tab() -> bool {
    true
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub struct WsPresistentData {
    /// Legacy dest-only list. Kept so older builds can still read this file;
    /// new writes populate `sessions` as the source of truth.
    #[serde(default)]
    open_tabs: Vec<Destination>,
    #[serde(default)]
    sessions: Vec<Session>,
    current_tab: Option<Destination>,
    #[serde(default)]
    current_tab_index: Option<usize>,
    canvas: CanvasSettings,
    pub markdown: MdPersistence,
    auto_save: bool,
    auto_sync: bool,
    landing_page: LandingPage,
    zoom_factor: f32,
    #[serde(default)]
    image_dims: HashMap<String, [f32; 2]>,
    /// Opt-in: show link previews by contacting each linked site for its
    /// title/favicon/card. Off by default — contacting a site reveals the
    /// reader's IP address and that they opened the note (and the URL may have
    /// arrived via a shared note). Routed through `crate::egress`.
    #[serde(default)]
    contact_linked_sites: bool,
    /// Desktop: file tree / pins / recents / create open in a new tab.
    /// Ignored on mobile (`show_tabs == false`), which always replaces.
    #[serde(default = "default_open_in_new_tab")]
    open_in_new_tab: bool,
}

impl Default for WsPresistentData {
    fn default() -> Self {
        Self {
            auto_save: true,
            auto_sync: true,
            open_tabs: Vec::default(),
            sessions: Vec::default(),
            current_tab: None,
            current_tab_index: None,
            open_in_new_tab: true,
            canvas: CanvasSettings::default(),
            markdown: MdPersistence::default(),
            landing_page: LandingPage::default(),
            zoom_factor: 1.,
            image_dims: HashMap::default(),
            contact_linked_sites: false,
        }
    }
}

impl WsPersistentStore {
    pub fn new(recent_crash: bool, path: PathBuf, enabled: bool) -> Self {
        let default = WsPresistentData::default();

        if enabled && recent_crash && path.exists() {
            warn!("removing persistence file due to recent crash");
            fs::remove_file(&path).log_and_ignore();
        }

        let store = match fs::File::open(&path) {
            Ok(f) => WsPersistentStore {
                path,
                data: Arc::new(RwLock::new(serde_json::from_reader(f).unwrap_or(default))),
                enabled,
            },
            Err(err) => {
                error!("Could not open ws presistance file: {:#?}", err);
                WsPersistentStore { path, data: Arc::new(RwLock::new(default)), enabled }
            }
        };

        if !store.enabled {
            let mut data_lock = store.data.write().unwrap();
            data_lock.open_tabs.clear();
            data_lock.sessions.clear();
            data_lock.current_tab = None;
            data_lock.current_tab_index = None;
        }

        store
    }

    pub fn set_tabs(&mut self, tab_strip: &[Session], current_tab: &Option<SessionId>) {
        let mut data_lock = self.data.write().unwrap();
        data_lock.sessions = tab_strip.to_vec();
        data_lock.open_tabs = tab_strip.iter().map(|s| s.dest.clone()).collect();
        data_lock.current_tab_index =
            current_tab.and_then(|id| tab_strip.iter().position(|s| s.id == id));
        data_lock.current_tab = data_lock
            .current_tab_index
            .and_then(|i| tab_strip.get(i))
            .map(|s| s.dest.clone());
        self.write_to_file();
    }

    pub fn get_sessions(&self) -> (Vec<Session>, Option<usize>) {
        let data_lock = self.data.read().unwrap();
        let sessions = if !data_lock.sessions.is_empty() {
            data_lock.sessions.clone()
        } else {
            data_lock
                .open_tabs
                .iter()
                .cloned()
                .map(Session::new)
                .collect()
        };
        let current_index = data_lock.current_tab_index.or_else(|| {
            data_lock
                .current_tab
                .as_ref()
                .and_then(|d| sessions.iter().position(|s| &s.dest == d))
        });
        (sessions, current_index)
    }

    pub fn get_open_in_new_tab(&self) -> bool {
        self.data.read().unwrap().open_in_new_tab
    }

    pub fn set_open_in_new_tab(&mut self, open_in_new_tab: bool) {
        let mut data_lock = self.data.write().unwrap();
        if data_lock.open_in_new_tab == open_in_new_tab {
            return;
        }
        data_lock.open_in_new_tab = open_in_new_tab;
        drop(data_lock);
        self.write_to_file();
    }

    pub fn set_canvas_settings(&mut self, canvas_settings: CanvasSettings) {
        let mut data_lock = self.data.write().unwrap();
        data_lock.canvas = canvas_settings;
        self.write_to_file();
    }

    pub fn get_canvas_settings(&mut self) -> CanvasSettings {
        self.data.read().unwrap().canvas
    }

    pub fn set_markdown(&mut self, value: MdPersistence) {
        let mut data_lock = self.data.write().unwrap();
        data_lock.markdown = value;
        self.write_to_file();
    }

    pub fn get_markdown(&self) -> MdPersistence {
        self.data.read().unwrap().markdown.clone()
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

    pub fn get_landing_page(&self) -> LandingPage {
        self.data.read().unwrap().landing_page.clone()
    }

    pub fn set_landing_page(&mut self, landing_page: LandingPage) {
        let mut data_lock = self.data.write().unwrap();
        data_lock.landing_page = landing_page;
        self.write_to_file();
    }

    pub fn get_contact_linked_sites(&self) -> bool {
        self.data.read().unwrap().contact_linked_sites
    }

    pub fn set_contact_linked_sites(&mut self, contact_linked_sites: bool) {
        let mut data_lock = self.data.write().unwrap();
        if data_lock.contact_linked_sites == contact_linked_sites {
            return; // no-op guard: mobile pushes this from its per-frame draw
        }
        data_lock.contact_linked_sites = contact_linked_sites;
        drop(data_lock);
        self.write_to_file();
    }

    pub fn get_zoom_factor(&self) -> f32 {
        self.data.read().unwrap().zoom_factor
    }

    pub fn set_zoom_factor(&mut self, zoom_factor: f32) {
        let mut data_lock = self.data.write().unwrap();
        data_lock.zoom_factor = zoom_factor;
        self.write_to_file();
    }

    pub fn image_dims(&self) -> HashMap<String, [f32; 2]> {
        self.data.read().unwrap().image_dims.clone()
    }

    pub fn merge_image_dims(&self, new_dims: HashMap<String, [f32; 2]>) {
        let mut data_lock = self.data.write().unwrap();
        data_lock.image_dims.extend(new_dims);
        drop(data_lock);
        self.write_to_file();
    }

    pub fn write_to_file(&self) {
        if !self.enabled {
            return;
        }

        let data = self.data.clone();
        let path = self.path.clone();
        spawn!({
            let started = web_time::Instant::now();
            let data = data.read().unwrap().clone(); // clone to avoid holding lock during serialization or file write
            let content = serde_json::to_string(&data).unwrap();
            let bytes = content.len();
            match fs::write(&path, content) {
                Ok(()) if started.elapsed() > Duration::from_millis(50) => {
                    warn!(?path, bytes, "ws persistence written ({:?})", started.elapsed());
                }
                Ok(()) => {
                    debug!(?path, bytes, "ws persistence written ({:?})", started.elapsed());
                }
                Err(err) => {
                    error!(
                        ?path,
                        bytes,
                        "ws persistence write failed ({:?}): {:?}",
                        started.elapsed(),
                        err
                    );
                }
            }
        });
    }
}
pub fn lb_bg_worker(ctx: Context, lb: Lb, ws_tx: Sender<WsUpdates>) {
    let mut events = lb.subscribe();

    loop {
        match events.blocking_recv() {
            Ok(evt) => match evt {
                Event::MetadataChanged(_) => {
                    if ws_tx
                        .send(WsUpdates::FileCacheComputed(FileCache::new(&lb)))
                        .is_err()
                    {
                        info!("workspace dropped, lb_bg_worker exiting");
                        return;
                    }
                    ctx.request_repaint();
                }
                Event::Sync(events::SyncIncrement::SyncFinished(_)) => {
                    ctx.request_repaint();
                }
                _ => {
                    continue;
                }
            },
            Err(e) => {
                error!("lb_frames died: {:?}", e);
                return;
            }
        }
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
