mod full_doc_search;
mod modals;
mod suggested_docs;
mod syncing;
mod tree;

use std::ffi::OsStr;
use std::sync::atomic::Ordering;
use std::sync::{mpsc, Arc, RwLock};
use std::time::Duration;
use std::{path, process, thread};

use eframe::egui;
use lb::blocking::Lb;
use lb::model::file::File;
use lb::model::file_metadata::FileType;
use lb::service::import_export::ImportStatus;
use lb::Uuid;
use workspace_rs::background::BwIncomingMsg;
use workspace_rs::theme::icons::Icon;
use workspace_rs::widgets::Button;
use workspace_rs::workspace::{Workspace, WsConfig};

use crate::model::{AccountScreenInitData, Usage};
use crate::settings::Settings;
use crate::util::data_dir;

use self::full_doc_search::FullDocSearch;
use self::modals::*;

use self::suggested_docs::SuggestedDocs;
use self::syncing::SyncPanel;
use self::tree::{FileTree, TreeNode};

pub struct AccountScreen {
    settings: Arc<RwLock<Settings>>,
    pub core: Lb,
    toasts: egui_notify::Toasts,

    update_tx: mpsc::Sender<AccountUpdate>,
    update_rx: mpsc::Receiver<AccountUpdate>,

    tree: FileTree,
    is_new_user: bool,
    suggested: SuggestedDocs,
    full_search_doc: FullDocSearch,
    sync: SyncPanel,
    usage: Result<Usage, String>,
    workspace: Workspace,
    modals: Modals,
    shutdown: Option<AccountShutdownProgress>,
}

impl AccountScreen {
    pub fn new(
        settings: Arc<RwLock<Settings>>, core: &Lb, acct_data: AccountScreenInitData,
        ctx: &egui::Context, is_new_user: bool,
    ) -> Self {
        let core = core.clone();
        let (update_tx, update_rx) = mpsc::channel();

        let AccountScreenInitData { sync_status, files, usage } = acct_data;
        let core_clone = core.clone();

        let toasts = egui_notify::Toasts::default()
            .with_margin(egui::vec2(40.0, 30.0))
            .with_padding(egui::vec2(20.0, 20.0));
        let reference_settings = settings.read().unwrap();
        let ws_cfg = WsConfig::new(
            data_dir().unwrap(),
            reference_settings.auto_save,
            reference_settings.auto_sync,
            reference_settings.zen_mode,
        );
        drop(reference_settings);

        Self {
            settings,
            core,
            toasts,
            update_tx,
            update_rx,
            is_new_user,
            tree: FileTree::new(files, &core_clone),
            suggested: SuggestedDocs::new(&core_clone),
            full_search_doc: FullDocSearch::default(),
            sync: SyncPanel::new(sync_status),
            usage,
            workspace: Workspace::new(ws_cfg, &core_clone, &ctx.clone()),
            modals: Modals::default(),
            shutdown: None,
        }
    }

    pub fn begin_shutdown(&mut self) {
        self.shutdown = Some(AccountShutdownProgress::default());
        self.workspace.save_all_tabs();
        self.workspace
            .background_tx
            .send(BwIncomingMsg::Shutdown)
            .unwrap();
    }

    pub fn is_shutdown(&self) -> bool {
        match &self.shutdown {
            Some(s) => s.done_saving && s.done_syncing,
            None => false,
        }
    }

    pub fn update(&mut self, ctx: &egui::Context) {
        self.process_updates(ctx);
        self.process_keys(ctx);
        self.process_dropped_files(ctx);
        self.toasts.show(ctx);

        if self.shutdown.is_some() {
            egui::CentralPanel::default()
                .show(ctx, |ui| ui.centered_and_justified(|ui| ui.label("Shutting down...")));
            return Default::default();
        }

        let is_expanded = !self.settings.read().unwrap().zen_mode;
        self.show_any_modals(ctx, 0.0);

        egui::SidePanel::left("sidebar_panel")
            .frame(egui::Frame::none().fill(ctx.style().visuals.panel_fill))
            .min_width(300.0)
            .show_animated(ctx, is_expanded, |ui| {
                if self.is_any_modal_open() {
                    ui.disable();
                }

                ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);

                    egui::Frame::default()
                        .inner_margin(egui::Margin::symmetric(20.0, 20.0))
                        .show(ui, |ui| {
                            self.show_usage_panel(ui);
                            self.show_nav_panel(ui);

                            ui.add_space(15.0);
                            self.show_sync_error_warn(ui);
                        });

                    ui.vertical(|ui| {
                        ui.add_space(15.0);
                        if let Some(file) = self.full_search_doc.show(ui, &self.core) {
                            self.workspace.open_file(file, false, true);
                        }
                        ui.add_space(15.0);

                        let full_doc_search_results_empty = self
                            .full_search_doc
                            .results
                            .lock()
                            .map(|r| r.is_empty())
                            .unwrap_or(true);
                        if full_doc_search_results_empty {
                            if let Some(file) = self.suggested.show(ui) {
                                self.workspace.open_file(file, false, true);
                            }
                            ui.add_space(15.0);
                            self.show_tree(ui);
                        }
                    });
                });
            });

        egui::CentralPanel::default()
            .frame(egui::Frame::default().fill(ctx.style().visuals.widgets.noninteractive.bg_fill))
            .show(ctx, |ui| {
                if self.is_any_modal_open() {
                    ui.disable();
                }
                let settings = self.settings.read().unwrap();
                self.workspace.cfg.update(
                    settings.auto_save,
                    settings.auto_sync,
                    settings.zen_mode,
                );
                drop(settings);
                self.workspace.focused_parent = Some(self.focused_parent());
                let wso = self.workspace.show(ui);
                if wso.settings_updated {
                    self.settings.write().unwrap().zen_mode =
                        self.workspace.cfg.zen_mode.load(Ordering::Relaxed);
                    self.settings.read().unwrap().to_file().unwrap();
                }
                if let Some((id, new_name)) = wso.file_renamed {
                    if let Some(node) = self.tree.root.find_mut(id) {
                        node.file.name = new_name.clone();
                    }
                    self.suggested.recalc_and_redraw(ctx, &self.core);
                    ctx.request_repaint();
                }

                if let Some(result) = wso.file_created {
                    self.file_created(ctx, result);
                }

                if let Some(file) = wso.selected_file {
                    self.tree.reveal_file(file, ctx);
                }

                if wso.sync_done.is_some() {
                    self.refresh_tree(ctx);
                }
            });

        if self.is_new_user {
            self.modals.account_backup = Some(AccountBackup);
            self.is_new_user = false;
        }
    }

    fn process_updates(&mut self, ctx: &egui::Context) {
        while let Ok(update) = self.update_rx.try_recv() {
            match update {
                AccountUpdate::OpenModal(open_modal) => match open_modal {
                    OpenModal::AcceptShare => {
                        self.modals.accept_share = Some(AcceptShareModal::new(&self.core));
                    }
                    OpenModal::ConfirmDelete(files) => {
                        self.modals.confirm_delete = Some(ConfirmDeleteModal::new(files));
                    }
                    OpenModal::PickShareParent(target) => {
                        self.modals.file_picker = Some(FilePicker::new(
                            &self.core,
                            FilePickerAction::AcceptShare(target),
                        ));
                    }
                    OpenModal::PickDropParent(drops) => {
                        self.modals.file_picker = Some(FilePicker::new(
                            &self.core,
                            FilePickerAction::DroppedFiles(drops),
                        ));
                    }
                    OpenModal::InitiateShare(target) => self.open_share_modal(target),
                    OpenModal::NewFolder(maybe_parent) => self.open_new_folder_modal(maybe_parent),
                    OpenModal::Settings => {
                        self.modals.settings = Some(SettingsModal::new(&self.core, &self.settings));
                    }
                },
                AccountUpdate::ShareAccepted(result) => match result {
                    Ok(_) => {
                        self.modals.file_picker = None;
                        self.workspace.perform_sync();
                        // todo: figure out how to call reveal_file after the file tree is updated with the new sync info
                    }
                    Err(msg) => self.modals.error = Some(ErrorModal::new(msg)),
                },
                AccountUpdate::FileImported(result) => match result {
                    Ok(root) => {
                        self.tree.root = root;
                        self.modals.file_picker = None;
                    }
                    Err(msg) => self.modals.error = Some(ErrorModal::new(msg)),
                },
                AccountUpdate::FileCreated(result) => self.file_created(ctx, result),
                AccountUpdate::FileDeleted(f) => {
                    self.tree.remove(&f);
                    self.suggested.recalc_and_redraw(ctx, &self.core);
                }
                AccountUpdate::DoneDeleting => self.modals.confirm_delete = None,
                AccountUpdate::ReloadTree(root) => self.tree.root = root,

                AccountUpdate::FinalSyncAttemptDone => {
                    if let Some(s) = &mut self.shutdown {
                        s.done_syncing = true;
                    }
                }
                AccountUpdate::FileShared(result) => match result {
                    Ok(_) => {
                        self.modals.create_share = None;
                        self.workspace.perform_sync();
                    }
                    Err(msg) => {
                        if let Some(m) = &mut self.modals.create_share {
                            m.err_msg = Some(msg)
                        }
                    }
                },
            }
        }
    }

    /// See also workspace::process_keys
    fn process_keys(&mut self, ctx: &egui::Context) {
        const ALT: egui::Modifiers = egui::Modifiers::ALT;
        const COMMAND: egui::Modifiers = egui::Modifiers::COMMAND;

        // Escape (without modifiers) to close something such as an open modal.
        // We don't want to consume it unless something is closed.
        if ctx.input(|i| i.key_pressed(egui::Key::Escape))
            && ctx.input(|i| i.modifiers.is_none())
            && self.close_something()
        {
            ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        }

        // Ctrl-E toggle zen mode
        if ctx.input_mut(|i| i.consume_key(COMMAND, egui::Key::E)) {
            let mut zen_mode = false;
            if let Ok(settings) = &self.settings.read() {
                zen_mode = !settings.zen_mode;
            }
            self.settings.write().unwrap().zen_mode = zen_mode;
        }

        // Ctrl-Space or Ctrl-L pressed while search modal is not open.
        let is_search_open = ctx.input_mut(|i| {
            i.consume_key(COMMAND, egui::Key::Space) || i.consume_key(COMMAND, egui::Key::L)
        });
        if is_search_open {
            if let Some(search) = &mut self.modals.search {
                search.focus_select_all();
            } else {
                self.modals.search = Some(SearchModal::new(self.core.clone()));
            }
        }

        // Ctrl-, to open settings modal.
        if self.modals.settings.is_none()
            && ctx.input_mut(|i| i.consume_key(COMMAND, egui::Key::Comma))
        {
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

        // Ctrl-Q to quit.
        if ctx.input_mut(|i| i.consume_key(COMMAND, egui::Key::Q)) {
            process::exit(0); // todo: graceful shutdown (needs support from windows client, linux client, workspace)
        }
    }

    fn process_dropped_files(&mut self, ctx: &egui::Context) {
        let has_dropped_files = ctx.input(|inp| !inp.raw.dropped_files.is_empty());

        if has_dropped_files {
            let dropped_files = ctx.input(|inp| inp.raw.dropped_files.clone());
            self.update_tx
                .send(AccountUpdate::OpenModal(OpenModal::PickDropParent(dropped_files)))
                .unwrap();
        }
    }

    fn show_tree(&mut self, ui: &mut egui::Ui) {
        let resp = egui::ScrollArea::both()
            .show(ui, |ui| self.tree.show(ui))
            .inner;

        if resp.new_file.is_some() {
            self.workspace.create_file(false);
        }

        if resp.new_drawing.is_some() {
            self.workspace.create_file(true);
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
            self.workspace.rename_file(rename_req);
        }

        for id in resp.open_requests {
            self.workspace.open_file(id, false, true);
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

        if let Some(res) = resp.export_file {
            match res {
                Ok((src, dest)) => self.toasts.success(format!(
                    "Exported \"{}\" to \"{}\"",
                    src.name,
                    dest.file_name()
                        .unwrap_or(OsStr::new("/"))
                        .to_string_lossy()
                )),
                Err(err) => {
                    eprintln!("couldn't export file {:#?}", err.backtrace);
                    self.toasts
                        .error(format!("{:#?}, failed to export file", err.kind))
                }
            }
            .set_closable(false)
            .set_show_progress_bar(false)
            .set_duration(Some(Duration::from_secs(7)));
        }
    }

    fn show_nav_panel(&mut self, ui: &mut egui::Ui) {
        ui.allocate_ui_with_layout(
            egui::vec2(ui.available_size_before_wrap().x, 40.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                self.show_sync_btn(ui);

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let settings_btn = Button::default().icon(&Icon::SETTINGS).show(ui);
                    if settings_btn.clicked() {
                        self.update_tx.send(OpenModal::Settings.into()).unwrap();
                        ui.ctx().request_repaint();
                    };
                    settings_btn.on_hover_text("Settings");

                    let incoming_shares_btn = Button::default()
                        .icon(
                            &Icon::SHARED_FOLDER
                                .badge(!self.workspace.status.dirtyness.pending_shares.is_empty()),
                        )
                        .show(ui);

                    if incoming_shares_btn.clicked() {
                        self.update_tx.send(OpenModal::AcceptShare.into()).unwrap();
                        ui.ctx().request_repaint();
                    };
                    incoming_shares_btn.on_hover_text("Incoming shares");

                    let zen_mode_btn = Button::default().icon(&Icon::TOGGLE_SIDEBAR).show(ui);

                    if zen_mode_btn.clicked() {
                        self.settings.write().unwrap().zen_mode = true;
                        if let Err(err) = self.settings.read().unwrap().to_file() {
                            self.modals.error = Some(ErrorModal::new(err));
                        }
                    }

                    zen_mode_btn.on_hover_text("Hide side panel");
                });
            },
        );
    }

    fn save_settings(&mut self) {
        if let Err(err) = self.settings.read().unwrap().to_file() {
            self.modals.error = Some(ErrorModal::new(err));
        }
    }

    pub fn refresh_tree(&self, ctx: &egui::Context) {
        let core = self.core.clone();
        let ctx = ctx.clone();

        let update_tx = self.update_tx.clone();

        thread::spawn(move || {
            let all_metas = core.list_metadatas().unwrap();
            let root = tree::create_root_node(all_metas);
            update_tx.send(AccountUpdate::ReloadTree(root)).unwrap();
            ctx.request_repaint();
        });
    }

    fn open_new_folder_modal(&mut self, maybe_parent: Option<File>) {
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
        self.modals.new_folder = Some(NewFolderModal::new(parent_path));
    }

    fn open_share_modal(&mut self, target: File) {
        self.modals.create_share = Some(CreateShareModal::new(target));
    }

    fn create_folder(&mut self, params: NewFileParams) {
        let parent = self.core.get_by_path(&params.parent_path).unwrap();

        let core = self.core.clone();
        let update_tx = self.update_tx.clone();
        thread::spawn(move || {
            let result = core
                .create_file(&params.name, &parent.id, params.ftype)
                .map_err(|err| format!("{:?}", err));
            update_tx.send(AccountUpdate::FileCreated(result)).unwrap();
        });
    }

    fn focused_parent(&mut self) -> Uuid {
        let mut focused_parent = self.tree.root.file.id;
        for id in self.tree.state.selected.iter() {
            focused_parent = *id;
        }

        focused_parent
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

    fn move_selected_files_to(&mut self, ctx: &egui::Context, target: Uuid) {
        let files = self.tree.get_selected_files();

        for f in files {
            if f.parent == target {
                continue;
            }
            if let Err(err) = self.core.move_file(&f.id, &target) {
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

    fn accept_share(&self, ctx: &egui::Context, target: File, parent: File) {
        let core = self.core.clone();
        let update_tx = self.update_tx.clone();
        let ctx = ctx.clone();

        thread::spawn(move || {
            let result = core
                .create_file(&target.name, &parent.id, FileType::Link { target: target.id })
                .map_err(|err| format!("{:?}", err));

            update_tx
                .send(AccountUpdate::ShareAccepted(result))
                .unwrap();

            ctx.request_repaint();
        });
    }

    fn delete_share(&self, target: File) {
        let core = self.core.clone();

        thread::spawn(move || {
            core.delete_pending_share(&target.id)
                .map_err(|err| format!("{:?}", err))
                .unwrap();
        });
    }

    fn dropped_files(&self, ctx: &egui::Context, drops: Vec<egui::DroppedFile>, parent: File) {
        let core = self.core.clone();
        let ctx = ctx.clone();
        let update_tx = self.update_tx.clone();
        let paths = drops
            .into_iter()
            .filter_map(|d| d.path)
            .collect::<Vec<path::PathBuf>>();

        thread::spawn(move || {
            let result = core.import_files(&paths, parent.id, &|status| match status {
                ImportStatus::CalculatedTotal(count) => {
                    println!("importing {} files", count);
                }
                ImportStatus::StartingItem(item) => {
                    println!("starting import: {}", item);
                }
                ImportStatus::FinishedItem(item) => {
                    println!("finished import of {} as lb://{}", item.name, item.id);
                }
            });

            let all_metas = core.list_metadatas().unwrap();
            let root = tree::create_root_node(all_metas);

            let result = result.map(|_| root).map_err(|err| format!("{:?}", err));

            update_tx.send(AccountUpdate::FileImported(result)).unwrap();
            ctx.request_repaint();
        });
    }

    fn delete_files(&mut self, ctx: &egui::Context, files: Vec<File>) {
        let core = self.core.clone();
        let update_tx = self.update_tx.clone();
        let ctx = ctx.clone();

        let mut tabs_to_delete = vec![];
        for (i, tab) in self.workspace.tabs.iter().enumerate() {
            if files.iter().any(|f| f.id.eq(&tab.id)) {
                tabs_to_delete.push(i);
            }
        }
        for i in tabs_to_delete {
            self.workspace.close_tab(i);
        }

        thread::spawn(move || {
            for f in &files {
                core.delete_file(&f.id).unwrap(); // TODO
                update_tx
                    .send(AccountUpdate::FileDeleted(f.clone()))
                    .unwrap();
            }
            update_tx.send(AccountUpdate::DoneDeleting).unwrap();
            ctx.request_repaint();
        });
    }

    fn file_created(&mut self, ctx: &egui::Context, result: Result<File, String>) {
        match result {
            Ok(f) => {
                let (id, is_doc) = (f.id, f.is_document());
                self.tree.root.insert(f);
                self.tree.reveal_file(id, ctx);
                if is_doc {
                    self.workspace.open_file(id, true, true);
                }
                // Close whichever new file modal was open.
                self.modals.new_folder = None;
                ctx.request_repaint();
            }
            Err(msg) => {
                if let Some(m) = &mut self.modals.new_folder {
                    m.err_msg = Some(msg)
                }
            }
        }
    }
}

pub enum AccountUpdate {
    /// To open some modals, we queue an update for the next frame so that the actions used to open
    /// each modal (such as the release of a click that would then be in the "outside" area of the
    /// modal) don't automatically close the modal during the same frame.
    OpenModal(OpenModal),

    FileCreated(Result<File, String>),
    FileShared(Result<(), String>),
    FileDeleted(File),

    /// if a file has been imported successfully refresh the tree, otherwise show what went wrong
    FileImported(Result<TreeNode, String>),

    ShareAccepted(Result<File, String>),

    DoneDeleting,

    ReloadTree(TreeNode),

    FinalSyncAttemptDone,
}

pub enum OpenModal {
    NewFolder(Option<File>),
    InitiateShare(File),
    Settings,
    AcceptShare,
    PickShareParent(File),
    PickDropParent(Vec<egui::DroppedFile>),
    ConfirmDelete(Vec<File>),
}

impl From<OpenModal> for AccountUpdate {
    fn from(v: OpenModal) -> Self {
        Self::OpenModal(v)
    }
}

#[derive(Default)]
struct AccountShutdownProgress {
    done_saving: bool,
    done_syncing: bool,
}
