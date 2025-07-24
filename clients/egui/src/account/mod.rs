mod full_doc_search;
mod modals;
mod syncing;
mod tree;

use std::ffi::OsStr;
use std::path::PathBuf;
use std::sync::{Arc, RwLock, mpsc};
use std::time::Duration;
use std::{path, process, thread};

use egui::style::ScrollStyle;
use egui::{EventFilter, Frame, Id, Key, Rect, ScrollArea, Stroke, Vec2};
use lb::Uuid;
use lb::blocking::Lb;
use lb::model::file::File;
use lb::model::file_metadata::FileType;
use lb::service::events::broadcast::error::TryRecvError;
use lb::service::events::{self, Event};
use lb::service::import_export::ImportStatus;
use lb::subscribers::status::Status;
use tree::FilesExt;
use workspace_rs::theme::icons::Icon;
use workspace_rs::widgets::Button;
use workspace_rs::workspace::Workspace;

use crate::settings::Settings;

use self::full_doc_search::FullDocSearch;
use self::modals::*;

use self::syncing::SyncPanel;
use self::tree::FileTree;

pub struct AccountScreen {
    settings: Arc<RwLock<Settings>>,
    pub core: Lb,
    toasts: egui_notify::Toasts,

    update_tx: mpsc::Sender<AccountUpdate>,
    update_rx: mpsc::Receiver<AccountUpdate>,

    lb_rx: events::Receiver<Event>,

    tree: FileTree,
    is_new_user: bool,
    full_search_doc: FullDocSearch,
    sync: SyncPanel,
    lb_status: Status,
    workspace: Workspace,
    modals: Modals,
    shutdown: Option<AccountShutdownProgress>,
}

impl AccountScreen {
    pub fn new(
        settings: Arc<RwLock<Settings>>, core: &Lb, files: Vec<File>, ctx: &egui::Context,
        is_new_user: bool,
    ) -> Self {
        let core = core.clone();
        let (update_tx, update_rx) = mpsc::channel();

        let core_clone = core.clone();

        let toasts = egui_notify::Toasts::default()
            .with_margin(egui::vec2(20.0, 20.0))
            .with_padding(egui::vec2(10.0, 10.0));

        let mut result = Self {
            settings,
            core: core.clone(),
            toasts,
            update_tx,
            update_rx,
            is_new_user,
            tree: FileTree::new(files),
            full_search_doc: FullDocSearch::default(),
            sync: SyncPanel::new(),
            workspace: Workspace::new(&core_clone, &ctx.clone()),
            modals: Modals::default(),
            shutdown: None,
            lb_rx: core.subscribe(),
            lb_status: core.status(),
        };
        result.tree.recalc_suggested_files(&core, ctx);
        result
    }

    pub fn begin_shutdown(&mut self) {
        self.shutdown = Some(AccountShutdownProgress::default());
        self.workspace.save_all_tabs();
        // todo: wait for saves to complete
    }

    pub fn is_shutdown(&self) -> bool {
        match &self.shutdown {
            Some(s) => s.done_saving && s.done_syncing,
            None => false,
        }
    }

    pub fn update(&mut self, ctx: &egui::Context) {
        self.process_lb_updates(ctx);
        self.process_updates(ctx);
        self.process_keys(ctx);
        self.process_dropped_files(ctx);
        self.toasts.show(ctx);

        if self.shutdown.is_some() {
            egui::CentralPanel::default()
                .show(ctx, |ui| ui.centered_and_justified(|ui| ui.label("Shutting down...")));
            return Default::default();
        }

        self.show_any_modals(ctx, 0.0);

        // focus management
        let full_doc_search_id = Id::from("full_doc_search");
        let suggested_docs_id = Id::from("suggested_docs");

        let sidebar_expanded = !self.settings.read().unwrap().zen_mode;
        if ctx.input(|i| i.key_pressed(Key::F) && i.modifiers.command && i.modifiers.shift) {
            if !sidebar_expanded {
                self.update_zen_mode(false);

                ctx.memory_mut(|m| m.request_focus(full_doc_search_id));
            } else if ctx.memory(|m| m.has_focus(full_doc_search_id)) {
                self.update_zen_mode(true);

                ctx.memory_mut(|m| m.focused().map(|f| m.surrender_focus(f))); // surrender focus - editor will take it
            } else {
                ctx.memory_mut(|m| m.request_focus(full_doc_search_id));
            }
        }

        egui::SidePanel::left("sidebar_panel")
            .frame(egui::Frame::none().fill(ctx.style().visuals.extreme_bg_color))
            .min_width(300.0)
            .show_animated(ctx, sidebar_expanded, |ui| {
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
                        });

                    ui.vertical(|ui| {
                        let full_doc_search_resp = self.full_search_doc.show(ui, &self.core);
                        if let Some(file) = full_doc_search_resp.file_to_open {
                            self.workspace.open_file(file, false, true, false);
                        }
                        if full_doc_search_resp.advance_focus {
                            ctx.memory_mut(|m| m.request_focus(suggested_docs_id));
                            self.tree.cursor = Some(self.tree.suggested_docs_folder_id);
                            self.tree.selected = Some(self.tree.suggested_docs_folder_id)
                                .into_iter()
                                .collect();
                        }

                        let full_doc_search_term_empty = self
                            .full_search_doc
                            .query
                            .lock()
                            .map(|q| q.is_empty())
                            .unwrap_or(true);
                        let mut max_rect = ui.max_rect(); // used to size end-of-tree padding
                        max_rect.min.y = ui.min_rect().max.y;
                        if full_doc_search_term_empty {
                            self.show_tree(ui, max_rect);
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

                self.workspace.focused_parent = self.focused_parent();
                let wso = self.workspace.show(ui);

                if self.settings.read().unwrap().zen_mode {
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
                            self.update_zen_mode(false);
                        }
                        zen_mode_btn.on_hover_text("Show side panel");
                    });
                }

                if let Some(result) = wso.file_created {
                    self.file_created(ctx, result);
                }

                if let Some(file) = wso.selected_file {
                    if !self.tree.selected.contains(&file) {
                        self.tree.cursor = Some(file);
                        self.tree.selected.clear();
                        self.tree.selected.insert(file);
                        self.tree.reveal_selection();
                        self.tree.scroll_to_cursor = true;
                    }
                }

                for msg in wso.failure_messages {
                    self.toasts.error(msg);
                }
            });

        if self.is_new_user {
            if let Ok(metas) = self.core.list_metadatas() {
                if let Some(welcome_doc) = metas.iter().find(|meta| meta.name == "welcome.md") {
                    self.workspace.open_file(welcome_doc.id, false, true, false);
                }
            }
            self.is_new_user = false;
        }

        // whatever is focused, lock focus on it
        // while the sidebar is expanding, it isn't rendered, so its contents lose focus
        if let Some(focused) = ctx.memory(|m| m.focused()) {
            // "register" the widget id - this keeps the id and its focus from being garbage collected
            // in debug builds, this will render some errors if the id is also used elsewhere
            ctx.check_for_id_clash(focused, egui::Rect::ZERO, "");

            // focus lock filter happens to be the same for all widgets we're managing here
            let event_filter = EventFilter {
                tab: true, // we don't need to capture tab input but tab focus navigation is unimplemented
                horizontal_arrows: true, // horizontal arrows move cursor in search and navigate file tree
                vertical_arrows: true, // vertical arrows navigate file tree and only change focus at widget discretion
                escape: false, // escape releases focus which is generally grabbed by the editor
            };
            ctx.memory_mut(|m| m.set_focus_lock_filter(focused, event_filter))
        }
    }

    fn process_lb_updates(&mut self, ctx: &egui::Context) {
        match self.lb_rx.try_recv() {
            Ok(evt) => match evt {
                Event::MetadataChanged => {
                    self.refresh_tree(ctx);
                }
                Event::StatusUpdated => {
                    self.lb_status = self.core.status();
                }
                _ => {}
            },
            Err(TryRecvError::Empty) => {}
            Err(e) => eprintln!("cannot recv events from lb-rs {e:?}"),
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
                        self.modals.settings = Some(SettingsModal::new(
                            &self.core,
                            &self.settings,
                            &self.workspace.cfg,
                        ));
                    }
                },
                AccountUpdate::ShareAccepted(result) => match result {
                    Ok(_) => {
                        self.modals.file_picker = None;
                        self.workspace.tasks.queue_sync();
                        // todo: figure out how to call reveal_file after the file tree is updated with the new sync info
                    }
                    Err(msg) => self.modals.error = Some(ErrorModal::new(msg)),
                },
                AccountUpdate::FileImported(result) => match result {
                    Ok(()) => {
                        self.modals.file_picker = None;
                    }
                    Err(msg) => self.modals.error = Some(ErrorModal::new(msg)),
                },
                AccountUpdate::FileCreated(result) => self.file_created(ctx, result),
                AccountUpdate::DoneDeleting => self.modals.confirm_delete = None,
                AccountUpdate::ReloadTree(files) => {
                    self.tree.update_files(files);
                    self.tree.recalc_suggested_files(&self.core, ctx);
                }

                AccountUpdate::FinalSyncAttemptDone => {
                    if let Some(s) = &mut self.shutdown {
                        s.done_syncing = true;
                    }
                }
                AccountUpdate::FileShared(result) => match result {
                    Ok(_) => {
                        self.modals.create_share = None;
                        self.workspace.tasks.queue_sync();
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
            let current_zen_mode = self.settings.read().unwrap().zen_mode;
            self.update_zen_mode(!current_zen_mode);
        }

        // Ctrl-Space or Ctrl-O or Ctrl-L pressed while search modal is not open.
        let is_search_open = ctx.input_mut(|i| {
            i.consume_key(COMMAND, egui::Key::Space)
                || i.consume_key(COMMAND, egui::Key::O)
                || i.consume_key(COMMAND, egui::Key::L)
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
            self.modals.settings =
                Some(SettingsModal::new(&self.core, &self.settings, &self.workspace.cfg));
        }

        // Ctrl-/ to toggle the help modal.
        if ctx.input_mut(|i| i.consume_key(COMMAND, egui::Key::Slash)) {
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

    fn show_tree(&mut self, ui: &mut egui::Ui, max_rect: Rect) {
        // avoids flickering due to hover conflict with sidebar resize
        ui.style_mut().spacing.scroll = ScrollStyle::solid();
        ui.style_mut().spacing.scroll.floating = true;
        ui.style_mut().spacing.scroll.bar_width *= 2.;

        let resp = ScrollArea::vertical()
            .show(ui, |ui| {
                ui.vertical_centered_justified(|ui| {
                    Frame::canvas(ui.style())
                        .inner_margin(0.)
                        .stroke(Stroke::NONE)
                        .show(ui, |ui| {
                            ui.allocate_space(Vec2 { x: ui.available_width(), y: 0. });
                            self.tree.show(ui, max_rect, &mut self.toasts)
                        })
                })
            })
            .inner
            .inner
            .inner;

        if resp.new_file.is_some() {
            self.workspace.create_file(false);
            ui.memory_mut(|m| m.focused().map(|f| m.surrender_focus(f))); // surrender focus - editor will take it
        }

        if resp.new_drawing.is_some() {
            self.workspace.create_file(true);
        }

        if resp.clear_suggested {
            self.core.clear_suggested().unwrap();
            self.tree.recalc_suggested_files(&self.core, ui.ctx());
        }

        if let Some(id) = resp.clear_suggested_id {
            self.core.clear_suggested_id(id).unwrap();
            self.tree.recalc_suggested_files(&self.core, ui.ctx());
        }

        if resp.space_inspector_root.is_some() {
            self.workspace
                .start_space_inspector(self.core.clone(), resp.space_inspector_root);
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

        for move_req in resp.move_requests {
            self.workspace.move_file(move_req);
        }

        if let Some(rename_req) = resp.rename_request {
            self.workspace.rename_file(rename_req, true);
        }

        for id in resp.open_requests {
            self.workspace.open_file(id, false, true, false);
        }

        if !resp.delete_requests.is_empty() {
            let files = resp
                .delete_requests
                .iter()
                .map(|&id| self.tree.files.get_by_id(id))
                .cloned()
                .collect();
            self.update_tx
                .send(OpenModal::ConfirmDelete(files).into())
                .unwrap();
        }

        if let Some(id) = resp.dropped_on {
            // todo: async
            self.move_selected_files_to(ui.ctx(), id);
        }

        if let Some((f, dest)) = resp.export_file {
            self.export_file(f, dest);
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
                        .icon(&Icon::SHARED_FOLDER.badge(self.lb_status.pending_shares))
                        .show(ui);

                    if incoming_shares_btn.clicked() {
                        self.update_tx.send(OpenModal::AcceptShare.into()).unwrap();
                        ui.ctx().request_repaint();
                    };
                    incoming_shares_btn.on_hover_text("Incoming shares");

                    let zen_mode_btn = Button::default().icon(&Icon::TOGGLE_SIDEBAR).show(ui);

                    if zen_mode_btn.clicked() {
                        self.update_zen_mode(true);
                    }

                    zen_mode_btn.on_hover_text("Hide side panel");
                });
            },
        );
    }

    fn update_zen_mode(&mut self, new_value: bool) {
        if let Err(err) = self.settings.write().unwrap().write_zen_mode(new_value) {
            self.modals.error = Some(ErrorModal::new(err));
        }
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
            update_tx
                .send(AccountUpdate::ReloadTree(all_metas))
                .unwrap();
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
                .map_err(|err| format!("{err:?}"));
            update_tx.send(AccountUpdate::FileCreated(result)).unwrap();
        });
    }

    fn focused_parent(&mut self) -> Option<Uuid> {
        if let Some(cursor) = self.tree.cursor {
            if cursor != self.tree.suggested_docs_folder_id
                && self.tree.files.iter().any(|f| f.id == cursor)
            {
                let cursor = self.tree.files.get_by_id(cursor);
                return if cursor.is_folder() { Some(cursor.id) } else { Some(cursor.parent) };
            }
        }
        None
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
        // pre-check cyclic moves for atomicity
        for &file in &self.tree.selected {
            let descendents = self
                .tree
                .files
                .descendents(file)
                .into_iter()
                .map(|f| f.id)
                .collect::<Vec<_>>();
            if descendents.contains(&target) {
                // todo: show error
                println!("cannot move folder into self");
                return;
            }
        }

        // pre-check name conflicts for atomicity
        let target_children = self.tree.files.children(target);
        for &file in &self.tree.selected {
            let name = self.tree.files.get_by_id(file).name.clone();
            if target_children.iter().any(|f| f.name == name) {
                // todo: show error
                println!("cannot move file into folder containing file with same name");
                return;
            }
        }

        // move files
        for &f in &self.tree.selected {
            if self.tree.files.get_by_id(f).parent == target {
                continue;
            }
            match self.core.move_file(&f, &target) {
                Err(err) => {
                    // todo: show error
                    println!("error moving file: {err:?}");
                    return;
                }
                _ => {
                    ctx.request_repaint();
                }
            }
        }

        ctx.request_repaint();
    }

    fn export_file(&mut self, f: File, dest: PathBuf) {
        println!("export_file");
        let res = self.core.export_files(
            f.id,
            dest.clone(),
            true,
            &Some(Box::new(|info| println!("{info:?}"))),
        );
        match res {
            Ok(()) => self.toasts.success(format!(
                "Exported \"{}\" to \"{}\"",
                f.name,
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

    fn accept_share(&self, ctx: &egui::Context, target: File, parent: File) {
        let core = self.core.clone();
        let update_tx = self.update_tx.clone();
        let ctx = ctx.clone();

        thread::spawn(move || {
            let result = core
                .create_file(&target.name, &parent.id, FileType::Link { target: target.id })
                .map_err(|err| format!("{err:?}"));

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
                .map_err(|err| format!("{err:?}"))
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
                    println!("importing {count} files");
                }
                ImportStatus::StartingItem(item) => {
                    println!("starting import: {item}");
                }
                ImportStatus::FinishedItem(item) => {
                    println!("finished import of {} as lb://{}", item.name, item.id);
                }
            });

            let result = result.map_err(|err| format!("{err:?}"));

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
            if files.iter().any(|f| Some(f.id) == tab.id()) {
                tabs_to_delete.push(i);
            }
        }
        for i in tabs_to_delete {
            self.workspace.close_tab(i);
        }

        thread::spawn(move || {
            for f in &files {
                core.delete_file(&f.id).unwrap();
            }
            update_tx.send(AccountUpdate::DoneDeleting).unwrap();
            ctx.request_repaint();
        });
    }

    fn file_created(&mut self, ctx: &egui::Context, result: Result<File, String>) {
        match result {
            Ok(f) => {
                let (id, is_doc) = (f.id, f.is_document());

                // inefficient but works
                let mut files = self.tree.files.clone();
                files.push(f);
                self.tree.update_files(files);

                if is_doc {
                    self.workspace.open_file(id, true, true, true);
                }
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

    /// if a file has been imported successfully refresh the tree, otherwise show what went wrong
    FileImported(Result<(), String>),

    ShareAccepted(Result<File, String>),

    DoneDeleting,

    ReloadTree(Vec<File>),

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
