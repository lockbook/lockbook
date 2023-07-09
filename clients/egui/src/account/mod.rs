mod background;
mod modals;
mod syncing;
mod tabs;
mod tree;
mod workspace;

use std::collections::HashMap;
use std::sync::{mpsc, Arc, RwLock};
use std::thread;
use std::time::Instant;

use eframe::egui;

use crate::model::{AccountScreenInitData, Usage};
use crate::settings::Settings;
use crate::theme::Icon;
use crate::util::NUM_KEYS;
use crate::widgets::{separator, Button};

use self::background::*;
use self::modals::*;
use self::syncing::{SyncPanel, SyncUpdate};
use self::tabs::{Drawing, ImageViewer, Markdown, PlainText, Tab, TabContent, TabFailure};
use self::tree::{FileTree, TreeNode};
use self::workspace::Workspace;

pub struct AccountScreen {
    ctx: egui::Context,
    settings: Arc<RwLock<Settings>>,
    core: lb::Core,

    update_tx: mpsc::Sender<AccountUpdate>,
    update_rx: mpsc::Receiver<AccountUpdate>,

    background_tx: mpsc::Sender<BackgroundEvent>,

    tree: FileTree,
    sync: SyncPanel,
    usage: Result<Usage, String>,
    workspace: Workspace,
    modals: Modals,
    shutdown: Option<AccountShutdownProgress>,
}

impl AccountScreen {
    pub fn new(
        settings: Arc<RwLock<Settings>>, core: lb::Core, acct_data: AccountScreenInitData,
        ctx: &egui::Context,
    ) -> Self {
        let (update_tx, update_rx) = mpsc::channel();

        let AccountScreenInitData { sync_status, files, usage } = acct_data;

        let background = BackgroundWorker::new(ctx, &update_tx);
        let background_tx = background.spawn_worker();

        Self {
            settings,
            core,
            update_tx,
            update_rx,
            background_tx,
            tree: FileTree::new(files),
            sync: SyncPanel::new(sync_status),
            usage,
            workspace: Workspace::new(),
            modals: Modals::default(),
            shutdown: None,
            ctx: ctx.clone(),
        }
    }

    pub fn begin_shutdown(&mut self) {
        self.shutdown = Some(AccountShutdownProgress::default());
        self.save_all_tabs(&self.ctx);
        self.background_tx.send(BackgroundEvent::Shutdown).unwrap();
    }

    pub fn is_shutdown(&self) -> bool {
        match &self.shutdown {
            Some(s) => s.done_saving && s.done_syncing,
            None => false,
        }
    }

    pub fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.process_updates(ctx, frame);
        self.process_keys(ctx, frame);

        if self.shutdown.is_some() {
            egui::CentralPanel::default()
                .show(ctx, |ui| ui.centered_and_justified(|ui| ui.label("Shutting down...")));
            return;
        }

        self.background_tx
            .send(BackgroundEvent::EguiUpdate)
            .unwrap();

        let sidebar_width = egui::SidePanel::left("sidebar_panel")
            .frame(egui::Frame::none().fill(ctx.style().visuals.panel_fill))
            .min_width(300.0)
            .show(ctx, |ui| {
                ui.set_enabled(!self.is_any_modal_open());

                ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);

                    self.show_sync_panel(ui);

                    separator(ui);

                    self.show_nav_panel(ui);

                    self.show_tree(ui);
                });
            })
            .response
            .rect
            .max
            .x;

        egui::CentralPanel::default()
            .frame(egui::Frame::default().fill(ctx.style().visuals.widgets.noninteractive.bg_fill))
            .show(ctx, |ui| self.show_workspace(frame, ui));

        self.show_any_modals(ctx, 0.0 - (sidebar_width / 2.0));
    }

    fn process_updates(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        while let Ok(update) = self.update_rx.try_recv() {
            match update {
                AccountUpdate::AutoSaveSignal => {
                    if self.settings.read().unwrap().auto_save {
                        self.save_all_tabs(ctx);
                    }
                }
                AccountUpdate::SaveResult(id, result) => {
                    if let Some(tab) = self.workspace.get_mut_tab_by_id(id) {
                        match result {
                            Ok(time_saved) => tab.last_saved = time_saved,
                            Err(err) => {
                                tab.failure = Some(TabFailure::Unexpected(format!("{:?}", err)))
                            }
                        }
                    }
                }
                AccountUpdate::OpenModal(open_modal) => match open_modal {
                    OpenModal::AcceptShare => {
                        self.modals.accept_share = Some(AcceptShareModal::new(&self.core));
                    }
                    OpenModal::ConfirmDelete(files) => {
                        self.modals.confirm_delete = Some(ConfirmDeleteModal::new(files));
                    }
                    OpenModal::FilePicker(target) => {
                        self.modals.file_picker = Some(FilePicker::new(self.core.clone(), target));
                    }
                    OpenModal::InitiateShare(target) => self.open_share_modal(target),
                    OpenModal::NewDoc(maybe_parent) => self.open_new_doc_modal(maybe_parent),
                    OpenModal::NewFolder(maybe_parent) => self.open_new_folder_modal(maybe_parent),
                    OpenModal::Settings => {
                        self.modals.settings = Some(SettingsModal::new(&self.core, &self.settings));
                    }
                },
                AccountUpdate::ShareAccepted(result) => match result {
                    Ok(_) => {
                        self.modals.file_picker = None;
                        self.perform_sync(ctx);
                    }
                    Err(msg) => self.modals.error = Some(ErrorModal::new(msg)),
                },
                AccountUpdate::FileCreated(result) => match result {
                    Ok(f) => {
                        let (id, is_doc) = (f.id, f.is_document());
                        self.tree.root.insert(f);
                        if is_doc {
                            self.open_file(id, ctx);
                        }
                        // Close whichever new file modal was open.
                        self.modals.new_doc = None;
                        self.modals.new_folder = None;
                    }
                    Err(msg) => {
                        if let Some(m) = &mut self.modals.new_doc {
                            m.err_msg = Some(msg)
                        } else if let Some(m) = &mut self.modals.new_folder {
                            m.err_msg = Some(msg)
                        }
                    }
                },
                AccountUpdate::FileLoaded(id, content_result) => {
                    if let Some(tab) = self.workspace.get_mut_tab_by_id(id) {
                        frame.set_window_title(&tab.name);
                        match content_result {
                            Ok(content) => tab.content = Some(content),
                            Err(fail) => tab.failure = Some(fail),
                        }
                    }
                }
                AccountUpdate::FileRenamed { id, new_name, new_child_paths } => {
                    if let Some(node) = self.tree.root.find_mut(id) {
                        node.file.name = new_name.clone();
                    }
                    if let Some(tab) = self.workspace.get_mut_tab_by_id(id) {
                        tab.name = new_name.clone();
                    }
                    if let Some(tab) = self.workspace.current_tab() {
                        if tab.id == id {
                            frame.set_window_title(&tab.name);
                        }
                    }
                    // If any of this file's children are open, we need to update their restore
                    // paths in case a sync deletes them.
                    for tab in &mut self.workspace.tabs {
                        if let Some(new_path) = new_child_paths.get(&tab.id) {
                            tab.path = new_path.clone();
                        }
                    }
                }
                AccountUpdate::FileDeleted(f) => self.tree.remove(&f),
                AccountUpdate::SyncUpdate(update) => self.process_sync_update(ctx, update),
                AccountUpdate::DoneDeleting => self.modals.confirm_delete = None,
                AccountUpdate::ReloadTree(root) => self.tree.root = root,
                AccountUpdate::ReloadTab(id, res) => {
                    let focussed_tab_id = self.workspace.current_tab().map(|tab| tab.id);
                    for i in 0..self.workspace.tabs.len() {
                        let tab_id = self.workspace.tabs[i].id;

                        if tab_id == id {
                            match res {
                                Ok(new_tab) => {
                                    self.workspace.tabs[i] = new_tab;
                                    if let Some(open_tab) = focussed_tab_id {
                                        if tab_id == open_tab {
                                            frame.set_window_title(&self.workspace.tabs[i].name);
                                        }
                                    }
                                    break;
                                }
                                Err(fail) => {
                                    self.workspace.tabs[i].failure = Some(fail);
                                    break;
                                }
                            }
                        }
                    }
                }
                AccountUpdate::BackgroundWorkerDone => {
                    if let Some(s) = &mut self.shutdown {
                        s.done_saving = true;
                        self.perform_final_sync(ctx);
                    }
                }
                AccountUpdate::FinalSyncAttemptDone => {
                    if let Some(s) = &mut self.shutdown {
                        s.done_syncing = true;
                    }
                }
                AccountUpdate::FileShared(result) => match result {
                    Ok(_) => {
                        self.modals.create_share = None;
                        self.perform_sync(ctx);
                    }
                    Err(msg) => {
                        if let Some(m) = &mut self.modals.create_share {
                            m.err_msg = Some(msg)
                        }
                    }
                },
                AccountUpdate::SyncStatusSignal => self.refresh_sync_status(ctx),
                AccountUpdate::AutoSyncSignal => {
                    if self.settings.read().unwrap().auto_sync {
                        self.perform_sync(ctx)
                    }
                }
            }
        }
    }

    fn process_keys(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        const ALT: egui::Modifiers = egui::Modifiers::ALT;
        const CTRL: egui::Modifiers = egui::Modifiers::CTRL;

        // Escape (without modifiers) to close something such as an open modal.
        // We don't want to consume it unless something is closed.
        if ctx.input(|i| i.key_pressed(egui::Key::Escape))
            && ctx.input(|i| i.modifiers.is_none())
            && self.close_something()
        {
            ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        }

        // Ctrl-N pressed while new file modal is not open.
        if self.modals.new_doc.is_none() && ctx.input_mut(|i| i.consume_key(CTRL, egui::Key::N)) {
            self.open_new_doc_modal(None);
        }

        // Ctrl-S to save current tab.
        if ctx.input_mut(|i| i.consume_key(CTRL, egui::Key::S)) {
            self.save_tab(ctx, self.workspace.active_tab);
        }

        // Ctrl-W to close current tab.
        if ctx.input_mut(|i| i.consume_key(CTRL, egui::Key::W)) && !self.workspace.is_empty() {
            self.close_tab(ctx, self.workspace.active_tab);
            frame.set_window_title(
                self.workspace
                    .current_tab()
                    .map(|tab| tab.name.as_str())
                    .unwrap_or("Lockbook"),
            );
        }

        // Ctrl-Space or Ctrl-L pressed while search modal is not open.
        let is_search_open = ctx.input_mut(|i| {
            i.consume_key(CTRL, egui::Key::Space) || i.consume_key(CTRL, egui::Key::L)
        });
        if is_search_open {
            if let Some(search) = &mut self.modals.search {
                search.focus_select_all();
            } else {
                self.modals.search = Some(SearchModal::new(&self.core, ctx));
            }
        }

        // Ctrl-, to open settings modal.
        if self.modals.settings.is_none() && consume_key(ctx, ',') {
            self.modals.settings = Some(SettingsModal::new(&self.core, &self.settings));
        }

        // Alt-H pressed to toggle the help modal.
        if ctx.input_mut(|i| i.consume_key(ALT, egui::Key::H)) {
            let d = &mut self.modals.help;
            *d = match d {
                Some(_) => None,
                None => Some(HelpModal),
            };
        }

        // Alt-{1-9} to easily navigate tabs (9 will always go to the last tab).
        ctx.input_mut(|input| {
            for i in 1..10 {
                if input.consume_key(ALT, NUM_KEYS[i - 1]) {
                    self.workspace.goto_tab(i);
                    // Remove any text event that's also present this frame so that it doesn't show up
                    // in the editor.
                    if let Some(index) = input
                        .events
                        .iter()
                        .position(|evt| *evt == egui::Event::Text(i.to_string()))
                    {
                        input.events.remove(index);
                    }
                    if let Some(tab) = self.workspace.current_tab() {
                        frame.set_window_title(&tab.name);
                    }
                    break;
                }
            }
        });
    }

    fn show_tree(&mut self, ui: &mut egui::Ui) {
        let resp = egui::ScrollArea::both()
            .show(ui, |ui| self.tree.show(ui))
            .inner;

        if let Some(file) = resp.new_doc_modal {
            self.update_tx
                .send(OpenModal::NewDoc(Some(file)).into())
                .unwrap();
            ui.ctx().request_repaint();
        }

        if let Some(file) = resp.new_folder_modal {
            self.update_tx
                .send(OpenModal::NewFolder(Some(file)).into())
                .unwrap();
            ui.ctx().request_repaint();
        }

        if let Some(file) = resp.create_share_modal {
            self.update_tx
                .send(OpenModal::InitiateShare(file).into())
                .unwrap();
            ui.ctx().request_repaint();
        }

        if let Some(rename_req) = resp.rename_request {
            self.rename_file(rename_req, ui.ctx());
        }

        for id in resp.open_requests {
            self.open_file(id, ui.ctx());
        }

        if resp.delete_request {
            let selected_files = self.tree.get_selected_files();
            if !selected_files.is_empty() {
                self.update_tx
                    .send(OpenModal::ConfirmDelete(selected_files).into())
                    .unwrap();
            }
        }

        if let Some(id) = resp.dropped_on {
            self.move_selected_files_to(ui.ctx(), id);
        }
    }

    fn show_nav_panel(&self, ui: &mut egui::Ui) {
        ui.allocate_ui_with_layout(
            egui::vec2(ui.available_size_before_wrap().x, 70.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.add_space(10.0);

                if Button::default()
                    .text("Settings ")
                    .icon(&Icon::SETTINGS)
                    .show(ui)
                    .clicked()
                {
                    self.update_tx.send(OpenModal::Settings.into()).unwrap();
                    ui.ctx().request_repaint();
                };
                ui.add_space(20.0);

                if Button::default()
                    .icon(
                        &Icon::SHARED_FOLDER.badge(
                            !self
                                .core
                                .get_pending_shares()
                                .unwrap_or_default()
                                .is_empty(),
                        ),
                    )
                    .show(ui)
                    .clicked()
                {
                    self.update_tx.send(OpenModal::AcceptShare.into()).unwrap();
                    ui.ctx().request_repaint();
                };
            },
        );
    }

    fn save_settings(&mut self) {
        if let Err(err) = self.settings.read().unwrap().to_file() {
            self.modals.error = Some(ErrorModal::new(err));
        }
    }

    pub fn refresh_tree_and_workspace(&self, ctx: &egui::Context, work: lb::WorkCalculated) {
        let opened_ids = self
            .workspace
            .tabs
            .iter()
            .map(|t| t.id)
            .collect::<Vec<lb::Uuid>>();

        let core = self.core.clone();
        let update_tx = self.update_tx.clone();
        let ctx = ctx.clone();

        thread::spawn(move || {
            let all_metas = core.list_metadatas().unwrap();
            let root = tree::create_root_node(all_metas);
            update_tx.send(AccountUpdate::ReloadTree(root)).unwrap();
            ctx.request_repaint();

            let server_ids = ids_changed_on_server(&work);
            let stale_tab_ids = server_ids.iter().filter(|id| opened_ids.contains(id));

            for &id in stale_tab_ids {
                let name = match core.get_file_by_id(id) {
                    Ok(file) => file.name,
                    Err(err) => {
                        update_tx
                            .send(AccountUpdate::ReloadTab(
                                id,
                                Err(match err.kind {
                                    lb::CoreError::FileNonexistent => TabFailure::DeletedFromSync,
                                    _ => TabFailure::Unexpected(format!("{:?}", err)),
                                }),
                            ))
                            .unwrap();
                        continue;
                    }
                };

                let path = core.get_path_by_id(id).unwrap(); // TODO

                let ext = name.split('.').last().unwrap_or_default();

                let content = if ext == "draw" {
                    core.get_drawing(id)
                        .map_err(TabFailure::from)
                        .map(|drawing| TabContent::Drawing(Drawing::boxed(drawing)))
                } else {
                    core.read_document(id)
                        .map_err(|err| TabFailure::Unexpected(format!("{:?}", err))) // todo(steve)
                        .map(|bytes| {
                            if ext == "md" {
                                TabContent::Markdown(Markdown::boxed(&bytes))
                            } else if is_supported_image_fmt(ext) {
                                TabContent::Image(ImageViewer::boxed(id.to_string(), &bytes))
                            } else {
                                TabContent::PlainText(PlainText::boxed(&bytes))
                            }
                        })
                };

                let now = Instant::now();
                update_tx
                    .send(AccountUpdate::ReloadTab(
                        id,
                        Ok(Tab {
                            id,
                            name,
                            path,
                            content: content.ok(),
                            failure: None,
                            last_changed: now,
                            last_saved: now,
                        }),
                    ))
                    .unwrap();
            }

            ctx.request_repaint();
        });
    }

    fn open_new_doc_modal(&mut self, maybe_parent: Option<lb::File>) {
        self.open_new_file_modal(maybe_parent, lb::FileType::Document);
    }

    fn open_new_folder_modal(&mut self, maybe_parent: Option<lb::File>) {
        self.open_new_file_modal(maybe_parent, lb::FileType::Folder);
    }

    fn open_share_modal(&mut self, target: lb::File) {
        self.modals.create_share = Some(CreateShareModal::new(target));
    }

    fn open_new_file_modal(&mut self, maybe_parent: Option<lb::File>, typ: lb::FileType) {
        let parent_id = match maybe_parent {
            Some(f) => {
                if f.is_folder() {
                    f.id
                } else {
                    f.parent
                }
            }
            None => self.core.get_root().unwrap().id,
        };

        let parent_path = self.core.get_path_by_id(parent_id).unwrap();

        if typ == lb::FileType::Folder {
            self.modals.new_folder = Some(NewFolderModal::new(parent_path));
        } else {
            self.modals.new_doc = Some(NewDocModal::new(parent_path));
        }
    }

    fn create_file(&mut self, params: NewFileParams) {
        let parent = self.core.get_by_path(&params.parent_path).unwrap();

        let core = self.core.clone();
        let update_tx = self.update_tx.clone();
        thread::spawn(move || {
            let result = core
                .create_file(&params.name, parent.id, params.ftype)
                .map_err(|err| format!("{:?}", err));
            update_tx.send(AccountUpdate::FileCreated(result)).unwrap();
        });
    }

    fn create_share(&mut self, params: CreateShareParams) {
        let core = self.core.clone();
        let update_tx = self.update_tx.clone();

        thread::spawn(move || {
            let result = core
                .share_file(params.id, &params.username, params.mode)
                .map_err(|err| format!("{:?}", err.kind));
            update_tx.send(AccountUpdate::FileShared(result)).unwrap();
        });
    }

    fn open_file(&mut self, id: lb::Uuid, ctx: &egui::Context) {
        if self.workspace.goto_tab_id(id) {
            ctx.request_repaint();
            return;
        }

        let fname = self
            .core
            .get_file_by_id(id)
            .unwrap() // TODO
            .name;

        let fpath = self.core.get_path_by_id(id).unwrap(); // TODO

        self.workspace.open_tab(id, &fname, &fpath);

        let core = self.core.clone();
        let update_tx = self.update_tx.clone();
        let ctx = ctx.clone();

        thread::spawn(move || {
            let ext = fname.split('.').last().unwrap_or_default();

            let content = if ext == "draw" {
                core.get_drawing(id)
                    .map_err(TabFailure::from)
                    .map(|drawing| TabContent::Drawing(Drawing::boxed(drawing)))
            } else {
                core.read_document(id)
                    .map_err(|err| TabFailure::Unexpected(format!("{:?}", err))) // todo(steve)
                    .map(|bytes| {
                        if ext == "md" {
                            TabContent::Markdown(Markdown::boxed(&bytes))
                        } else if is_supported_image_fmt(ext) {
                            TabContent::Image(ImageViewer::boxed(id.to_string(), &bytes))
                        } else {
                            TabContent::PlainText(PlainText::boxed(&bytes))
                        }
                    })
            };

            update_tx
                .send(AccountUpdate::FileLoaded(id, content))
                .unwrap();
            ctx.request_repaint();
        });
    }

    fn move_selected_files_to(&mut self, ctx: &egui::Context, target: lb::Uuid) {
        let files = self.tree.get_selected_files();

        for f in files {
            if f.parent == target {
                continue;
            }
            if let Err(err) = self.core.move_file(f.id, target) {
                println!("{:?}", err);
                return;
            } else {
                let parent = self.tree.root.find_mut(f.parent).unwrap();
                let node = parent.remove(f.id).unwrap();
                let target_node = self.tree.root.find_mut(target).unwrap();
                target_node.insert_node(node);
                if let Some(tab) = self.workspace.get_mut_tab_by_id(f.id) {
                    tab.path = self.core.get_path_by_id(f.id).unwrap();
                }
                ctx.request_repaint();
            }
        }

        ctx.request_repaint();
    }

    fn rename_file(&self, req: (lb::Uuid, String), ctx: &egui::Context) {
        let core = self.core.clone();
        let update_tx = self.update_tx.clone();
        let ctx = ctx.clone();

        thread::spawn(move || {
            let (id, new_name) = req;
            core.rename_file(id, &new_name).unwrap(); // TODO

            let mut new_child_paths = HashMap::new();
            for f in core.get_and_get_children_recursively(id).unwrap() {
                new_child_paths.insert(f.id, core.get_path_by_id(f.id).unwrap());
            }

            update_tx
                .send(AccountUpdate::FileRenamed { id, new_name, new_child_paths })
                .unwrap();
            ctx.request_repaint();
        });
    }

    fn accept_share(&self, target: lb::File, parent: lb::File) {
        let core = self.core.clone();
        let update_tx = self.update_tx.clone();

        thread::spawn(move || {
            let result = core
                .create_file(&target.name, parent.id, lb::FileType::Link { target: target.id })
                .map_err(|err| format!("{:?}", err));

            update_tx
                .send(AccountUpdate::ShareAccepted(result))
                .unwrap()
        });
    }

    fn delete_share(&self, target: lb::File) {
        let core = self.core.clone();

        thread::spawn(move || {
            core.delete_pending_share(target.id)
                .map_err(|err| format!("{:?}", err))
        });
    }

    fn delete_files(&self, ctx: &egui::Context, files: Vec<lb::File>) {
        let core = self.core.clone();
        let update_tx = self.update_tx.clone();
        let ctx = ctx.clone();

        thread::spawn(move || {
            for f in &files {
                core.delete_file(f.id).unwrap(); // TODO
                update_tx
                    .send(AccountUpdate::FileDeleted(f.clone()))
                    .unwrap();
            }
            update_tx.send(AccountUpdate::DoneDeleting).unwrap();
            ctx.request_repaint();
        });
    }
}

pub enum AccountUpdate {
    AutoSaveSignal,
    SaveResult(lb::Uuid, Result<Instant, lb::LbError>),

    /// To open some modals, we queue an update for the next frame so that the actions used to open
    /// each modal (such as the release of a click that would then be in the "outside" area of the
    /// modal) don't automatically close the modal during the same frame.
    OpenModal(OpenModal),

    FileCreated(Result<lb::File, String>),
    FileShared(Result<(), String>),
    FileLoaded(lb::Uuid, Result<TabContent, TabFailure>),
    FileRenamed {
        id: lb::Uuid,
        new_name: String,
        new_child_paths: HashMap<lb::Uuid, String>,
    },
    FileDeleted(lb::File),

    SyncUpdate(SyncUpdate),
    SyncStatusSignal,
    AutoSyncSignal,

    ShareAccepted(Result<lb::File, String>),

    DoneDeleting,

    ReloadTree(TreeNode),
    ReloadTab(lb::Uuid, Result<Tab, TabFailure>),

    BackgroundWorkerDone,
    FinalSyncAttemptDone,
}

pub enum OpenModal {
    NewDoc(Option<lb::File>),
    NewFolder(Option<lb::File>),
    InitiateShare(lb::File),
    Settings,
    AcceptShare,
    FilePicker(lb::File),
    ConfirmDelete(Vec<lb::File>),
}

impl From<OpenModal> for AccountUpdate {
    fn from(v: OpenModal) -> Self {
        Self::OpenModal(v)
    }
}

impl From<SyncUpdate> for AccountUpdate {
    fn from(v: SyncUpdate) -> Self {
        Self::SyncUpdate(v)
    }
}

#[derive(Default)]
struct AccountShutdownProgress {
    done_saving: bool,
    done_syncing: bool,
}

fn is_supported_image_fmt(ext: &str) -> bool {
    const IMG_FORMATS: [&str; 7] = ["png", "jpeg", "jpg", "gif", "webp", "bmp", "ico"];
    IMG_FORMATS.contains(&ext)
}

fn consume_key(ctx: &egui::Context, key: char) -> bool {
    ctx.input_mut(|input| {
        let m = &input.modifiers;
        if m.ctrl && !m.alt && !m.shift {
            if let Some(index) = input
                .events
                .iter()
                .position(|evt| *evt == egui::Event::Text(key.to_string()))
            {
                input.events.remove(index);
                return true;
            }
        }
        false
    })
}

fn ids_changed_on_server(work: &lb::WorkCalculated) -> Vec<lb::Uuid> {
    work.work_units
        .iter()
        .filter_map(|wu| match wu {
            lb::WorkUnit::LocalChange { .. } => None,
            lb::WorkUnit::ServerChange { metadata } => Some(metadata.id),
        })
        .collect()
}
