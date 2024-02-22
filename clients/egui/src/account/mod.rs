mod full_doc_search;
mod modals;
mod suggested_docs;
mod syncing;
mod tree;

use std::ffi::OsStr;
use std::sync::atomic::Ordering;
use std::sync::{mpsc, Arc, RwLock};
use std::time::{Duration, Instant};
use std::{path, thread};

use eframe::egui;
use lb::{FileType, Uuid};
use workspace_rs::background::BwIncomingMsg;
use workspace_rs::tab::image_viewer::{is_supported_image_fmt, ImageViewer};
use workspace_rs::tab::markdown::Markdown;
use workspace_rs::tab::plain_text::PlainText;
use workspace_rs::tab::{Tab, TabContent, TabFailure};
use workspace_rs::theme::icons::Icon;
use workspace_rs::widgets::{separator, Button};
use workspace_rs::workspace::{Workspace, WsConfig};

use crate::model::{AccountScreenInitData, Usage};
use crate::settings::Settings;
use crate::util::data_dir;
use crate::UpdateOutput;

use self::full_doc_search::FullDocSearch;
use self::modals::*;

use self::suggested_docs::SuggestedDocs;
use self::syncing::SyncPanel;
use self::tree::{FileTree, TreeNode};

pub struct AccountScreen {
    settings: Arc<RwLock<Settings>>,
    pub core: lb::Core,
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
        settings: Arc<RwLock<Settings>>, core: lb::Core, acct_data: AccountScreenInitData,
        ctx: &egui::Context, is_new_user: bool,
    ) -> Self {
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
            full_search_doc: FullDocSearch::new(),
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

    pub fn update(&mut self, ctx: &egui::Context, output: &mut UpdateOutput) {
        self.process_updates(ctx, output);
        self.process_keys(ctx);
        self.process_dropped_files(ctx);
        self.toasts.show(ctx);

        if self.shutdown.is_some() {
            egui::CentralPanel::default()
                .show(ctx, |ui| ui.centered_and_justified(|ui| ui.label("Shutting down...")));
            return Default::default();
        }

        self.workspace
            .background_tx
            .send(BwIncomingMsg::EguiUpdate)
            .unwrap();

        let is_expanded = !self.settings.read().unwrap().zen_mode;

        egui::SidePanel::left("sidebar_panel")
            .frame(egui::Frame::none().fill(ctx.style().visuals.panel_fill))
            .min_width(300.0)
            .show_animated(ctx, is_expanded, |ui| {
                ui.set_enabled(!self.is_any_modal_open());

                ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);

                    egui::Frame::default()
                        .inner_margin(egui::Margin::symmetric(20.0, 20.0))
                        .show(ui, |ui| {
                            self.show_sync_panel(ui);
                            self.show_nav_panel(ui);
                        });

                    ui.vertical(|ui| {
                        ui.add_space(15.0);
                        if let Some(&file) = self.full_search_doc.show(ui, &self.core) {
                            self.workspace.open_file(file, false);
                        }
                        ui.add_space(15.0);

                        if self.full_search_doc.results.is_empty() {
                            if let Some(file) = self.suggested.show(ui) {
                                self.workspace.open_file(file, false);
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
                ui.set_enabled(!self.is_any_modal_open());
                let settings = self.settings.read().unwrap();
                self.workspace.cfg.update(
                    settings.auto_save,
                    settings.auto_sync,
                    settings.zen_mode,
                );
                drop(settings);
                self.workspace.focused_parent = Some(self.focused_parent());
                let wso = self.workspace.show_workspace(ui);
                output.set_window_title = wso.window_title;
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
                    self.tree.reveal_file(file, &self.core);
                    ctx.request_repaint();
                }

                if let Some(done) = wso.sync_done {
                    self.refresh_tree_and_workspace(ctx, done);
                }
            });

        if self.is_new_user {
            self.modals.account_backup = Some(AccountBackup);
            self.is_new_user = false;
        }
        self.show_any_modals(ctx, 0.0);
    }

    fn process_updates(&mut self, ctx: &egui::Context, output: &mut UpdateOutput) {
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
                            self.core.clone(),
                            FilePickerAction::AcceptShare(target),
                        ));
                    }
                    OpenModal::PickDropParent(target) => {
                        self.modals.file_picker = Some(FilePicker::new(
                            self.core.clone(),
                            FilePickerAction::DroppedFile(target),
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
                                            output.set_window_title =
                                                Some(self.workspace.tabs[i].name.clone());
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
        const CTRL: egui::Modifiers = egui::Modifiers::CTRL;

        // Escape (without modifiers) to close something such as an open modal.
        // We don't want to consume it unless something is closed.
        if ctx.input(|i| i.key_pressed(egui::Key::Escape))
            && ctx.input(|i| i.modifiers.is_none())
            && self.close_something()
        {
            ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        }

        // Ctrl-E toggle zen mode
        if ctx.input_mut(|i| i.consume_key(CTRL, egui::Key::E)) {
            let mut zen_mode = false;
            if let Ok(settings) = &self.settings.read() {
                zen_mode = !settings.zen_mode;
            }
            self.settings.write().unwrap().zen_mode = zen_mode;
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
    }

    fn process_dropped_files(&mut self, ctx: &egui::Context) {
        let has_dropped_files = ctx.input(|inp| !inp.raw.dropped_files.is_empty());

        if has_dropped_files {
            // todo: handle multiple dropped files
            let dropped_file = ctx.input(|inp| inp.raw.dropped_files[0].clone());

            if let Some(upd) = dropped_file
                .path
                .map(OpenModal::PickDropParent)
                .map(AccountUpdate::OpenModal)
            {
                self.update_tx.send(upd).unwrap()
            }
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
            self.workspace.open_file(id, false);
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
        let visuals_before_button = ui.style().clone();

        ui.allocate_ui_with_layout(
            egui::vec2(ui.available_size_before_wrap().x, 40.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                // ui.add_space(10.0);

                let text_stroke = egui::Stroke {
                    color: ui.visuals().widgets.active.bg_fill,
                    ..Default::default()
                };
                ui.visuals_mut().widgets.inactive.fg_stroke = text_stroke;
                ui.visuals_mut().widgets.hovered.fg_stroke = text_stroke;
                ui.visuals_mut().widgets.active.fg_stroke = text_stroke;

                ui.visuals_mut().widgets.inactive.bg_fill =
                    ui.visuals().widgets.active.bg_fill.gamma_multiply(0.1);
                ui.visuals_mut().widgets.hovered.bg_fill =
                    ui.visuals().widgets.active.bg_fill.gamma_multiply(0.2);

                ui.visuals_mut().widgets.active.bg_fill =
                    ui.visuals().widgets.active.bg_fill.gamma_multiply(0.3);

                let sync_btn = Button::default()
                    .text("Sync")
                    .icon(&Icon::SYNC)
                    .icon_alignment(egui::Align::RIGHT)
                    .padding(egui::vec2(10.0, 7.0))
                    .frame(true)
                    .rounding(egui::Rounding::same(5.0))
                    .is_loading(self.workspace.pers_status.syncing)
                    .show(ui);

                if sync_btn.clicked() {
                    self.workspace.perform_sync();
                }

                if let Some(sync_message) = &self.workspace.pers_status.sync_message {
                    sync_btn.on_hover_ui_at_pointer(|ui| {
                        ui.label(sync_message);
                    });
                } else {
                    if let Ok(sync_freshness) = &self.sync.status {
                        sync_btn.on_hover_ui_at_pointer(|ui| {
                            ui.label(format!("Synced {sync_freshness}"));
                        });
                    }
                    // match &self.sync.status {
                    //     Ok(s) => ui.label(
                    //         egui::RichText::new(format!("Updated {s}"))
                    //             .color(ui.visuals().widgets.active.bg_fill)
                    //             .size(15.0),
                    //     ),
                    //     Err(msg) => ui.label(egui::RichText::new(msg).color(egui::Color32::RED)),
                    // };
                }

                ui.set_style(visuals_before_button);

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let settings_btn = Button::default().icon(&Icon::SETTINGS).show(ui);
                    if settings_btn.clicked() {
                        self.update_tx.send(OpenModal::Settings.into()).unwrap();
                        ui.ctx().request_repaint();
                    };
                    settings_btn.on_hover_text("Settings");

                    let incoming_shares_btn = Button::default()
                        .icon(
                            &Icon::SHARED_FOLDER.badge(
                                !self
                                    .workspace
                                    .pers_status
                                    .dirtyness
                                    .pending_shares
                                    .is_empty(),
                            ),
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

    pub fn refresh_tree_and_workspace(&self, ctx: &egui::Context, work: lb::SyncStatus) {
        let opened_ids = self
            .workspace
            .tabs
            .iter()
            .map(|t| t.id)
            .collect::<Vec<lb::Uuid>>();

        let core = self.core.clone();
        let ctx = ctx.clone();

        let settings = &self.settings.read().unwrap();
        let toolbar_visibility = settings.toolbar_visibility;
        let update_tx = self.update_tx.clone();

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

                let content = core
                    .read_document(id)
                    .map_err(|err| TabFailure::Unexpected(format!("{:?}", err))) // todo(steve)
                    .map(|bytes| {
                        if ext == "md" {
                            TabContent::Markdown(Markdown::new(
                                core.clone(),
                                &bytes,
                                &toolbar_visibility,
                                // update_tx.clone(),
                                false,
                            ))
                        } else if is_supported_image_fmt(ext) {
                            TabContent::Image(ImageViewer::new(id.to_string(), &bytes))
                        } else {
                            TabContent::PlainText(PlainText::new(&bytes))
                        }
                    });
                let now = Instant::now();
                update_tx
                    .send(AccountUpdate::ReloadTab(
                        id,
                        Ok(Tab {
                            id,
                            name,
                            path,
                            rename: None,
                            content: content.ok(),
                            failure: None,
                            last_changed: now,
                            last_saved: now,
                            is_new_file: false,
                        }),
                    ))
                    .unwrap();
            }

            ctx.request_repaint();
        });
    }

    fn open_new_folder_modal(&mut self, maybe_parent: Option<lb::File>) {
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

    fn open_share_modal(&mut self, target: lb::File) {
        self.modals.create_share = Some(CreateShareModal::new(target));
    }

    fn create_folder(&mut self, params: NewFileParams) {
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

    fn focused_parent(&mut self) -> Uuid {
        let mut focused_parent = self.tree.root.file.id;
        for id in self.tree.state.selected.drain() {
            focused_parent = id;
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

    fn accept_share(&self, ctx: &egui::Context, target: lb::File, parent: lb::File) {
        let core = self.core.clone();
        let update_tx = self.update_tx.clone();
        let ctx = ctx.clone();

        thread::spawn(move || {
            let result = core
                .create_file(&target.name, parent.id, FileType::Link { target: target.id })
                .map_err(|err| format!("{:?}", err));

            update_tx
                .send(AccountUpdate::ShareAccepted(result))
                .unwrap();

            ctx.request_repaint();
        });
    }

    fn delete_share(&self, target: lb::File) {
        let core = self.core.clone();

        thread::spawn(move || {
            core.delete_pending_share(target.id)
                .map_err(|err| format!("{:?}", err))
                .unwrap();
        });
    }

    fn dropped_file(&self, ctx: &egui::Context, target: path::PathBuf, parent: lb::File) {
        let core = self.core.clone();
        let ctx = ctx.clone();
        let update_tx = self.update_tx.clone();

        thread::spawn(move || {
            let result =
                core.import_files(&[target], parent.id, &|_| println!("imported one file"));

            let all_metas = core.list_metadatas().unwrap();
            let root = tree::create_root_node(all_metas);

            let result = result.map(|_| root).map_err(|err| format!("{:?}", err));

            update_tx.send(AccountUpdate::FileImported(result)).unwrap();
            ctx.request_repaint();
        });
    }

    fn delete_files(&mut self, ctx: &egui::Context, files: Vec<lb::File>) {
        let core = self.core.clone();
        let update_tx = self.update_tx.clone();
        let ctx = ctx.clone();

        let tab_ids = self
            .workspace
            .tabs
            .iter()
            .map(|t| t.id)
            .collect::<Vec<lb::Uuid>>();

        for (i, f) in files.iter().enumerate() {
            if tab_ids.contains(&f.id) {
                self.workspace.close_tab(i)
            }
        }
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

    fn file_created(&mut self, ctx: &egui::Context, result: Result<lb::File, String>) {
        match result {
            Ok(f) => {
                let (id, is_doc) = (f.id, f.is_document());
                self.tree.root.insert(f);
                self.tree.reveal_file(id, &self.core);
                if is_doc {
                    self.workspace.open_file(id, true);
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

    FileCreated(Result<lb::File, String>),
    FileShared(Result<(), String>),
    FileDeleted(lb::File),

    /// if a file has been imported successfully refresh the tree, otherwise show what went wrong
    FileImported(Result<TreeNode, String>),

    ShareAccepted(Result<lb::File, String>),

    DoneDeleting,

    ReloadTree(TreeNode),
    ReloadTab(Uuid, Result<Tab, TabFailure>),

    FinalSyncAttemptDone,
}

pub enum OpenModal {
    NewFolder(Option<lb::File>),
    InitiateShare(lb::File),
    Settings,
    AcceptShare,
    PickShareParent(lb::File),
    PickDropParent(path::PathBuf),
    ConfirmDelete(Vec<lb::File>),
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

fn ids_changed_on_server(work: &lb::SyncStatus) -> Vec<lb::Uuid> {
    work.work_units
        .iter()
        .filter_map(|wu| match wu {
            lb::WorkUnit::LocalChange { .. } => None,
            lb::WorkUnit::ServerChange(id) => Some(*id),
        })
        .collect()
}
