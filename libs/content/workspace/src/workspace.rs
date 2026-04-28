use chrono::Local;
use egui::{Context, ViewportCommand};

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
use lb_rs::{Uuid, spawn};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use tracing::{debug, error, info, instrument, warn};
use web_time::{Duration, Instant};

use crate::file_cache::{FileCache, FilesExt};
use crate::landing::LandingPage;
use crate::output::Response;
use crate::resolvers::FileCacheLinkResolver;
use crate::resolvers::image_embed::ImageEmbedResolver;
use crate::show::DocType;
use crate::space_inspector::show::SpaceInspector;
use crate::tab::chat::Chat;
use crate::tab::image_viewer::ImageViewer;
use crate::tab::markdown_editor::{
    Editor as Markdown, HttpClient, MdConfig, MdPersistence, MdResources,
};
use crate::tab::pdf_viewer::PdfViewer;
use crate::tab::svg_editor::{CanvasSettings, SVGEditor};
use crate::tab::{
    ContentState, Destination, ExtendedInput as _, Tab, TabContent, TabFailure, TabSaveContent,
    TabSlot,
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

pub struct Workspace {
    // User activity
    pub tabs: TabCache,
    pub tab_strip: Vec<TabSlot>,
    pub current_tab: Option<Destination>,
    pub landing_page: LandingPage,
    pub account: Account,

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

    pub show_tabs: bool,              // set on mobile to hide the tab strip
    pub focused_parent: Option<Uuid>, // set to the folder where new files should be created

    // Transient state (consider removing)
    pub landing_page_first_frame: bool,
    pub current_tab_changed: bool, // used to scroll to current tab when it changes
    pub last_touch_event: Option<Instant>, // used to disable tooltips on touch devices

    // Transient rename state for the landing page file table
    pub landing_rename_target: Option<lb_rs::Uuid>,
    pub landing_rename_buffer: String,
}

impl Workspace {
    pub fn new(core: &Lb, ctx: &Context, show_tabs: bool) -> Self {
        let writable_dir = core.get_config().writeable_path;
        let writeable_dir = Path::new(&writable_dir);
        let writeable_path = writeable_dir.join("ws_persistence.json");
        let files =
            Arc::new(RwLock::new(FileCache::new(core).expect("failed to initialize file cache")));
        let images =
            ImageCache::new(ctx.clone(), HttpClient::default(), core.clone(), Arc::clone(&files));

        let cfg = WsPersistentStore::new(core.recent_panic().unwrap_or(true), writeable_path);
        ctx.set_zoom_factor(cfg.get_zoom_factor());
        let mut ws = Self {
            tabs: TabCache::new(),
            tab_strip: Vec::new(),
            current_tab: None,
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
            focused_parent: Default::default(),

            landing_page_first_frame: true,
            current_tab_changed: Default::default(),
            last_touch_event: Default::default(),
            landing_rename_target: None,
            landing_rename_buffer: String::new(),
            lb_rx: core.subscribe(),
        };

        let (open_tabs, current_tab) = ws.cfg.get_tabs();

        open_tabs.iter().for_each(|&file_id| {
            if core.get_file_by_id(file_id).is_ok() {
                info!(id = ?file_id, "opening persisted tab");
                ws.open_file(file_id, false, true);
            }
        });
        if let Some(current_tab) = current_tab {
            info!(id = ?current_tab, "setting persisted current tab");
            ws.make_current_by_id(current_tab);
        }

        let core = ws.core.clone();
        let ctx = ctx.clone();

        #[cfg(not(target_family = "wasm"))]
        spawn!(lb_frames(ctx, core));

        ws
    }

    /// Ensure a tab exists for `dest`. Creates it if absent, determining
    /// content from the destination variant. Does not add to tab_strip or
    /// make current — callers decide visibility.
    pub fn open_dest(&mut self, dest: &Destination) {
        if self.tabs.contains_key(dest) {
            return;
        }
        let id = dest.id();
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
        };
        let now = Instant::now();
        self.tabs.insert(
            dest.clone(),
            Tab {
                destination: dest.clone(),
                content,
                last_changed: now,
                last_saved: now,
                read_only: false,
            },
        );
        if needs_load {
            self.tasks
                .queue_load(LoadRequest { id, tab_created: true, make_current: false });
        }
    }

    pub fn create_tab(&mut self, dest: Destination, make_current: bool) {
        self.open_dest(&dest);
        if !self.tab_strip.iter().any(|s| s.dest == dest) {
            self.tab_strip.push(TabSlot::new(dest.clone()));
        }
        if make_current {
            self.current_tab = Some(dest);
            self.mark_current_tab_changed();
        }
    }

    pub fn get_mut_tab_by_id(&mut self, id: Uuid) -> Option<&mut Tab> {
        let dest = self
            .tab_strip
            .iter()
            .find(|s| s.dest.id() == id)?
            .dest
            .clone();
        self.tabs.get_mut(&dest)
    }

    pub fn get_idx_by_id(&mut self, id: Uuid) -> Option<usize> {
        self.tab_strip.iter().position(|s| s.dest.id() == id)
    }

    pub fn is_empty(&self) -> bool {
        self.tab_strip.is_empty()
    }

    pub fn current_tab(&self) -> Option<&Tab> {
        self.current_tab.as_ref().and_then(|d| self.tabs.get(d))
    }

    pub fn current_tab_id(&self) -> Option<Uuid> {
        self.current_tab().and_then(|tab| tab.id())
    }

    fn mark_current_tab_changed(&mut self) {
        self.current_tab_changed = true;
        self.out.selected_file = self.current_tab_id();
    }

    pub fn current_tab_title(&self) -> Option<String> {
        self.current_tab().map(|tab| self.tab_title(tab))
    }

    pub fn current_tab_mut(&mut self) -> Option<&mut Tab> {
        self.current_tab
            .as_ref()
            .cloned()
            .and_then(|d| self.tabs.get_mut(&d))
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

    pub fn make_current(&mut self, i: usize) -> bool {
        let Some(slot) = self.tab_strip.get(i) else { return false };
        let dest = slot.dest.clone();
        if self.tabs.get(&dest).is_none() {
            return false;
        };
        self.current_tab = Some(dest.clone());
        self.mark_current_tab_changed();
        let tab = self.tabs.get(&dest).unwrap();
        self.ctx
            .send_viewport_cmd(ViewportCommand::Title(self.tab_title(tab)));

        if let Some(md) = self.current_tab_markdown() {
            md.focus(&self.ctx);
        }

        self.ctx.request_repaint();

        true
    }

    /// Makes the tab with the given id the current tab, if it exists. Returns true if the tab exists.
    pub fn make_current_by_id(&mut self, id: Uuid) -> bool {
        if let Some(i) = self.tab_strip.iter().position(|s| s.dest.id() == id) {
            self.make_current(i)
        } else {
            false
        }
    }

    pub fn save_all_tabs(&mut self) {
        let slots: Vec<_> = self.tab_strip.clone();
        for slot in &slots {
            let dest = &slot.dest;
            if let Some(tab) = self.tabs.get(dest) {
                if let Some(id) = tab.id() {
                    if tab.is_dirty(&self.tasks) {
                        self.tasks.queue_save(SaveRequest { id });
                    }
                }
            }
        }
        self.last_save_all = Some(Instant::now());
    }

    pub fn save_tab(&mut self, i: usize) {
        let Some(slot) = self.tab_strip.get(i) else { return };
        let dest = slot.dest.clone();
        if let Some(tab) = self.tabs.get(&dest) {
            if let Some(id) = tab.id() {
                if tab.is_dirty(&self.tasks) {
                    self.tasks.queue_save(SaveRequest { id });
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

    pub fn open_file(&mut self, id: Uuid, make_current: bool, in_new_tab: bool) {
        let dest = Destination::File(id);

        // already in strip — just focus it
        if let Some(pos) = self.tab_strip.iter().position(|s| s.dest == dest) {
            if make_current {
                self.make_current(pos);
            }
            return;
        }

        if in_new_tab {
            self.create_tab(dest, make_current);
            return;
        }

        // navigate within current tab
        let Some(old_dest) = self.current_tab.clone() else {
            self.create_tab(dest, make_current);
            return;
        };

        // Find the slot index for the current tab
        let Some(slot_idx) = self.tab_strip.iter().position(|s| s.dest == old_dest) else {
            self.create_tab(dest, make_current);
            return;
        };

        // Push old dest id to slot's back, clear forward
        self.tab_strip[slot_idx].back.push(old_dest.id());
        self.tab_strip[slot_idx].forward.clear();
        // Update the slot's dest to the new destination
        self.tab_strip[slot_idx].dest = dest.clone();

        // Ensure the tab exists in cache for the new destination
        self.open_dest(&dest);

        self.current_tab = Some(dest);
        self.mark_current_tab_changed();
    }

    pub fn back(&mut self) {
        let Some(current_dest) = self.current_tab.clone() else { return };
        let Some(slot_idx) = self.tab_strip.iter().position(|s| s.dest == current_dest) else {
            return;
        };
        let Some(back_id) = self.tab_strip[slot_idx].back.pop() else { return };
        self.tab_strip[slot_idx].forward.push(current_dest.id());

        let new_dest = Destination::File(back_id);
        self.tab_strip[slot_idx].dest = new_dest.clone();

        self.open_dest(&new_dest);
        self.current_tab = Some(new_dest);
        self.mark_current_tab_changed();
    }

    pub fn can_back(&self) -> bool {
        let Some(current_dest) = self.current_tab.as_ref() else { return false };
        self.tab_strip
            .iter()
            .find(|s| &s.dest == current_dest)
            .map(|slot| !slot.back.is_empty())
            .unwrap_or(false)
    }

    pub fn forward(&mut self) {
        let Some(current_dest) = self.current_tab.clone() else { return };
        let Some(slot_idx) = self.tab_strip.iter().position(|s| s.dest == current_dest) else {
            return;
        };
        let Some(forward_id) = self.tab_strip[slot_idx].forward.pop() else { return };
        self.tab_strip[slot_idx].back.push(current_dest.id());

        let new_dest = Destination::File(forward_id);
        self.tab_strip[slot_idx].dest = new_dest.clone();

        self.open_dest(&new_dest);
        self.current_tab = Some(new_dest);
        self.mark_current_tab_changed();
    }

    pub fn can_forward(&self) -> bool {
        let Some(current_dest) = self.current_tab.as_ref() else { return false };
        self.tab_strip
            .iter()
            .find(|s| &s.dest == current_dest)
            .map(|slot| !slot.forward.is_empty())
            .unwrap_or(false)
    }

    pub fn close_tab(&mut self, i: usize) {
        let Some(slot) = self.tab_strip.get(i) else { return };
        let dest = slot.dest.clone();
        #[cfg(not(target_family = "wasm"))]
        if let Some(tab) = self.tabs.get_mut(&dest) {
            if let ContentState::Open(TabContent::MindMap(mm)) = &mut tab.content {
                mm.stop();
            }
        }

        self.save_tab(i);

        if let Some(tab) = self.tabs.get_mut(&dest) {
            if let Some(md) = tab.markdown_mut() {
                md.surrender_focus(&self.ctx);
            }
        }

        // Remove from tab_strip
        self.tab_strip.remove(i);
        self.out.tabs_changed = true;

        // Update current_tab
        if self.current_tab.as_ref() == Some(&dest) {
            // Pick a neighbor
            if self.tab_strip.is_empty() {
                self.current_tab = None;
            } else {
                let new_idx = i.min(self.tab_strip.len() - 1);
                self.current_tab = Some(self.tab_strip[new_idx].dest.clone());
            }
        }
        self.mark_current_tab_changed();
    }

    #[instrument(level = "trace", skip_all)]
    pub fn process_lb_updates(&mut self) {
        let mut refresh_cache = false;
        let mut remove_deleted_file_tabs = false;
        loop {
            match self.lb_rx.try_recv() {
                Ok(evt) => match evt {
                    Event::MetadataChanged(_) => {
                        refresh_cache = true;
                        remove_deleted_file_tabs = true;
                    }
                    Event::DocumentWritten(id, actor) => {
                        refresh_cache = true;

                        if actor == Actor::Sync {
                            self.core.app_foregrounded();
                            let has_open_tab = self
                                .tab_strip
                                .iter()
                                .any(|s| s.dest.id() == id && self.tabs.contains_key(&s.dest));
                            if has_open_tab {
                                self.tasks.queue_load(LoadRequest {
                                    id,
                                    tab_created: false,
                                    make_current: false,
                                });
                            }
                        }
                    }
                    _ => {}
                },
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

        if refresh_cache {
            *self.files.write().unwrap() =
                FileCache::new(&self.core).expect("failed to refresh file cache");
        }
        if remove_deleted_file_tabs {
            let files_arc = Arc::clone(&self.files);
            let files_guard = files_arc.read().unwrap();
            let files = &*files_guard;
            let mut tabs_to_delete = vec![];
            for slot in &self.tab_strip {
                let id = slot.dest.id();
                if files.get_by_id(id).is_none() {
                    tabs_to_delete.push(id);
                }
            }

            for id in tabs_to_delete {
                if let Some(idx) = self.tab_strip.iter().position(|s| s.dest.id() == id) {
                    self.close_tab(idx);
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
            if md.edit.renderer.readonly { None } else { Some(md.edit.file_id) }
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

                        let rel_path = {
                            let guard = self.files.read().unwrap();
                            let parent = guard.get_by_id(file_id).unwrap().parent;
                            let mut augmented = guard.files.clone();
                            if augmented.get_by_id(file.parent).is_none() {
                                if let Ok(folder) = self.core.get_file_by_id(file.parent) {
                                    augmented.push(folder);
                                }
                            }
                            augmented.push(file.clone());
                            crate::file_cache::relative_path(
                                &augmented.path(parent),
                                &augmented.path(file.id),
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
                    request: LoadRequest { id, tab_created, make_current },
                    content_result,
                    timing: _,
                } = load;

                let ctx = self.ctx.clone();
                let core = self.core.clone();
                let show_tabs = self.show_tabs;

                let key = Destination::File(id);
                if let Some(tab) = self.tabs.get_mut(&key) {
                    let files_clone = self.files.clone();
                    let files_guard = files_clone.read().unwrap();

                    let account = &self.account;
                    let Some(file) = files_guard.get_by_id(id) else { continue };

                    let doc_type = DocType::from_name(&file.name);
                    let read_only = files_guard.access(id, account) == UserAccessMode::Read;
                    let ext = file
                        .name
                        .split('.')
                        .next_back()
                        .unwrap_or_default()
                        .to_owned();

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

                    tab.read_only = read_only;

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
                                                self.cfg.get_markdown().image_dims(&id),
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

                    self.out.tabs_changed = true;
                } else {
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
                        request: SaveRequest { id },
                        seq,
                        content,
                        new_hmac_result,
                        timing: CompletedTiming { queued_at: _, started_at, completed_at: _ },
                    } = save;

                    let key = Destination::File(id);
                    if let Some(tab) = self.tabs.get_any_mut(&key) {
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
        let dest = self
            .tab_strip
            .iter()
            .find(|s| s.dest.id() == id)
            .map(|s| s.dest.clone());
        if let Some(tab) = dest.as_ref().and_then(|d| self.tabs.get(d)) {
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
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub struct WsPresistentData {
    open_tabs: Vec<Uuid>,
    current_tab: Option<Uuid>,
    canvas: CanvasSettings,
    pub markdown: MdPersistence,
    auto_save: bool,
    auto_sync: bool,
    landing_page: LandingPage,
    zoom_factor: f32,
}

impl Default for WsPresistentData {
    fn default() -> Self {
        Self {
            auto_save: true,
            auto_sync: true,
            open_tabs: Vec::default(),
            current_tab: None,
            canvas: CanvasSettings::default(),
            markdown: MdPersistence::default(),
            landing_page: LandingPage::default(),
            zoom_factor: 1.,
        }
    }
}

impl WsPersistentStore {
    pub fn new(recent_crash: bool, path: PathBuf) -> Self {
        let default = WsPresistentData::default();

        if recent_crash && path.exists() {
            warn!("removing persistence file due to recent crash");
            fs::remove_file(&path).log_and_ignore();
        }

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
    pub fn set_tabs(&mut self, tab_strip: &[TabSlot], current_tab: &Option<Destination>) {
        let mut data_lock = self.data.write().unwrap();
        data_lock.open_tabs = tab_strip
            .iter()
            .filter(|s| matches!(s.dest, Destination::File(_)))
            .map(|s| s.dest.id())
            .collect();
        data_lock.current_tab = current_tab.as_ref().map(|d| d.id());
        self.write_to_file();
    }

    pub fn get_tabs(&self) -> (Vec<Uuid>, Option<Uuid>) {
        let data_lock = self.data.read().unwrap();
        (data_lock.open_tabs.clone(), data_lock.current_tab)
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

    pub fn get_zoom_factor(&self) -> f32 {
        self.data.read().unwrap().zoom_factor
    }

    pub fn set_zoom_factor(&mut self, zoom_factor: f32) {
        let mut data_lock = self.data.write().unwrap();
        data_lock.zoom_factor = zoom_factor;
        self.write_to_file();
    }

    pub fn write_to_file(&self) {
        let data = self.data.clone();
        let path = self.path.clone();
        spawn!({
            let data = data.read().unwrap().clone(); // clone to avoid holding lock during serialization or file write
            let content = serde_json::to_string(&data).unwrap();
            let _ = fs::write(path, content);
        });
    }
}
pub fn lb_frames(ctx: Context, lb: Lb) {
    let mut events = lb.subscribe();

    loop {
        match events.blocking_recv() {
            Ok(evt) => match evt {
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
