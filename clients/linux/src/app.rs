use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::thread;

use gdk::WindowExt;
use gdk_pixbuf::Pixbuf as GdkPixbuf;
use gio::prelude::*;
use gtk::prelude::*;
use gtk::Orientation::{Horizontal, Vertical};
use gtk::{
    Align as GtkAlign, Application as GtkApp, ApplicationWindow as GtkAppWindow, Box as GtkBox,
    CellRendererText as GtkCellRendererText, Dialog as GtkDialog, Image as GtkImage,
    Label as GtkLabel, ProgressBar as GtkProgressBar, ResponseType as GtkResponseType,
    SelectionMode as GtkSelectionMode, Spinner as GtkSpinner, Stack as GtkStack,
    TreeStore as GtkTreeStore, TreeView as GtkTreeView, TreeViewColumn as GtkTreeViewColumn,
    WindowPosition as GtkWindowPosition,
};

use lockbook_models::file_metadata::{DecryptedFileMetadata, FileType};
use uuid::Uuid;

use crate::account::{AccountScreen, TextAreaDropPasteInfo};
use crate::backend::LbCore;
use crate::background_work::BackgroundWork;
use crate::editmode::EditMode;
use crate::error::{
    LbErrKind::{Program as ProgErr, User as UserErr},
    LbErrTarget, LbError, LbResult,
};
use crate::filetree::FileTreeCol;
use crate::lbsearch::LbSearch;
use crate::menubar::Menubar;
use crate::messages::{Messenger, Msg};
use crate::onboarding;
use crate::syncing;
use crate::util;
use crate::{closure, progerr, tree_iter_value, uerr, uerr_dialog};
use crate::{settings, settings::Settings};

use lockbook_core::service::import_export_service::ImportExportFileInfo;

macro_rules! make_glib_chan {
    ($( $( $vars:ident ).+ $( as $aliases:ident )* ),+ => move |$param:ident :$param_type:ty| $fn:block) => {{
        let (s, r) = glib::MainContext::channel::<$param_type>(glib::PRIORITY_DEFAULT);
        r.attach(None, closure!($( $( $vars ).+ $( as $aliases )* ),+ => move |$param: $param_type| $fn));
        s
    }};
}

macro_rules! spawn {
    ($( $( $vars:ident ).+ $( as $aliases:ident )* ),+ => $fn:expr) => {
        std::thread::spawn(closure!($( $( $vars ).+ $( as $aliases )* ),+ => $fn));
    };
}

#[derive(Clone)]
pub struct LbApp {
    pub core: Arc<LbCore>,
    pub settings: Rc<RefCell<Settings>>,
    pub state: Rc<RefCell<LbState>>,
    pub gui: Rc<Gui>,
    pub messenger: Messenger,
}

impl LbApp {
    pub fn new(c: &Arc<LbCore>, s: &Rc<RefCell<Settings>>, a: &GtkApp) -> Self {
        let (sender, receiver) = glib::MainContext::channel::<Msg>(glib::PRIORITY_DEFAULT);
        let m = Messenger::new(sender);

        let gui = Gui::new(a, &m, &s.borrow(), c);

        let lb_app = Self {
            core: c.clone(),
            settings: s.clone(),
            state: Rc::new(RefCell::new(LbState::default(&m))),
            gui: Rc::new(gui),
            messenger: m,
        };

        lb_app
            .messenger
            .send(Msg::ToggleAutoSave(s.borrow().auto_save));
        lb_app
            .messenger
            .send(Msg::ToggleAutoSync(s.borrow().auto_sync));

        let lb = lb_app.clone();
        receiver.attach(None, move |msg| {
            let maybe_err = match msg {
                Msg::CreateAccount(username) => onboarding::create(&lb, username),
                Msg::ImportAccount(acct_str) => onboarding::import(&lb, acct_str),
                Msg::ExportAccount => lb.export_account(),
                Msg::PerformSync => syncing::perform_sync(&lb),
                Msg::RefreshSyncStatus => syncing::refresh_status(&lb),
                Msg::RefreshUsageStatus => lb.refresh_usage_status(),
                Msg::Quit => lb.quit(),

                Msg::AccountScreenShown => lb.account_screen_shown(),
                Msg::MarkdownLinkExec(scheme, uri) => lb.markdown_lb_link_exec(&scheme, &uri),

                Msg::NewFile(file_type) => lb.new_file(file_type),
                Msg::OpenFile(id) => lb.open_file(id),
                Msg::FileEdited => lb.file_edited(),
                Msg::SaveFile => lb.save(),
                Msg::CloseFile => lb.close_file(),
                Msg::DeleteFiles => lb.delete_files(),
                Msg::RenameFile => lb.rename_file(),

                Msg::ToggleTreeCol(col) => lb.toggle_tree_col(col),
                Msg::RefreshTree => lb.refresh_tree(),

                Msg::PromptSearch => lb.prompt_search(),
                Msg::SearchFieldExec(vopt) => lb.search_field_exec(vopt),

                Msg::ShowDialogSyncDetails => syncing::show_details_dialog(&lb),
                Msg::ShowDialogPreferences => settings::show_dialog(&lb),
                Msg::ShowDialogUsage => lb.show_dialog_usage(),
                Msg::ShowDialogAbout => lb.show_dialog_about(),
                Msg::ShowDialogImportFile(parent, uris, finish_ch) => {
                    lb.show_dialog_import_file(parent, uris, finish_ch)
                }
                Msg::ShowDialogExportFile => lb.show_dialog_export_file(),

                Msg::DropPasteInTextArea(info) => lb.paste_in_text_area(info),

                Msg::ToggleAutoSave(auto_save) => lb.toggle_auto_save(auto_save),
                Msg::ToggleAutoSync(auto_sync) => lb.toggle_auto_sync(auto_sync),

                Msg::ErrorDialog(title, err) => {
                    lb.err_dialog(&title, &err);
                    Ok(())
                }
                Msg::SetStatus(txt, tool_tip_txt) => {
                    lb.gui
                        .account
                        .status()
                        .set_status(txt.as_str(), tool_tip_txt.as_deref());
                    Ok(())
                }
            };
            if let Err(err) = maybe_err {
                match err.target() {
                    LbErrTarget::Dialog => lb.err_dialog("", &err),
                    LbErrTarget::StatusPanel => lb.gui.account.status().set_status(err.msg(), None),
                }
            }
            glib::Continue(true)
        });

        let prompt_search = gio::SimpleAction::new("prompt_search", None);
        prompt_search.connect_activate(glib::clone!(@strong lb_app => move |_, _| {
            if let Err(err) = lb_app.prompt_search() {
                lb_app.err_dialog("opening search", &err)
            }
        }));
        a.add_action(&prompt_search);
        a.set_accels_for_action("app.prompt_search", &["<Ctrl>space"]);

        lb_app
    }

    pub fn show(&self) -> LbResult<()> {
        self.gui.show(&self.core)
    }

    fn export_account(&self) -> LbResult<()> {
        let spinner = GtkSpinner::new();
        spinner.set_margin_end(8);
        spinner.show();
        spinner.start();

        let placeholder = GtkBox::new(Horizontal, 0);
        util::gui::set_marginy(&placeholder, 200);
        util::gui::set_marginx(&placeholder, 125);
        placeholder.set_valign(GtkAlign::Center);
        placeholder.add(&spinner);
        placeholder.add(&GtkLabel::new(Some("Generating QR code...")));

        let image_cntr = GtkBox::new(Horizontal, 0);
        util::gui::set_marginx(&image_cntr, 8);
        image_cntr.set_center_widget(Some(&placeholder));

        match self.core.export_account() {
            Ok(acct_str) => {
                let btn_cntr = GtkBox::new(Horizontal, 0);
                btn_cntr.set_center_widget(Some(&util::gui::clipboard_btn(&acct_str)));
                btn_cntr.set_margin_bottom(8);

                let d = self.gui.new_dialog("Lockbook Private Key");
                d.get_content_area().pack_start(&image_cntr, true, true, 8);
                d.get_content_area().add(&btn_cntr);
                d.set_resizable(false);
                d.show_all();
            }
            Err(err) => self.err_dialog("unable to export account", &err),
        }

        let ch = make_glib_chan!(self.messenger as m => move |res: LbResult<String>| {
            match res {
                Ok(path) => {
                    let qr_image = GtkImage::from_file(&path);
                    image_cntr.set_center_widget(Some(&qr_image));
                    image_cntr.show_all();
                }
                Err(err) => m.send_err_dialog("generating qr code", err),
            }
            glib::Continue(false)
        });

        spawn!(self.core as c => move || ch.send(c.account_qrcode()).unwrap());
        Ok(())
    }

    fn refresh_usage_status(&self) -> LbResult<()> {
        spawn!(self.core as c, self.messenger as m => move || {
            match c.usage_status() {
                Ok(status) => if let (Some(txt), _) = status {
                    m.send(Msg::SetStatus(txt, status.1));
                }
                Err(err) => match err.target() {
                    LbErrTarget::Dialog => m.send_err_dialog("getting usage status", err),
                    LbErrTarget::StatusPanel => m.send_err_status_panel(err.msg()),
                }
            }
        });

        Ok(())
    }

    fn quit(&self) -> LbResult<()> {
        self.gui.win.close();
        Ok(())
    }

    fn account_screen_shown(&self) -> LbResult<()> {
        let background_work = self.state.borrow().background_work.clone();

        thread::spawn(move || BackgroundWork::init_background_work(background_work));

        Ok(())
    }

    fn markdown_lb_link_exec(&self, scheme: &str, uri: &str) -> LbResult<()> {
        if scheme == "lb://" {
            let f = self.core.file_by_path(uri)?;
            self.messenger.send(Msg::OpenFile(Some(f.id)));
        } else if gtk::show_uri_on_window(
            Some(&self.gui.win),
            &format!("{}{}", scheme, uri),
            gtk::get_current_event_time(),
        )
        .is_err()
        {
            uerr_dialog!("Failed to open link.");
        }

        Ok(())
    }

    fn new_file(&self, file_type: FileType) -> LbResult<()> {
        let file_type_string = match file_type {
            FileType::Document => "Document",
            FileType::Folder => "Folder",
        };

        let lbl = util::gui::text_left(&format!("Enter {} name:", file_type_string.to_lowercase()));
        lbl.set_margin_top(12);

        let errlbl = util::gui::text_left("");
        util::gui::set_widget_name(&errlbl, "err");
        errlbl.set_margin_start(8);
        errlbl.set_margin_bottom(8);

        let entry = gtk::Entry::new();
        util::gui::set_marginy(&entry, 16);
        entry.set_margin_start(8);
        entry.set_activates_default(true);

        let d = self.gui.new_dialog(&format!("New {}", file_type_string));
        d.set_default_size(300, -1);
        d.get_content_area().add(&lbl);
        d.get_content_area().add(&entry);
        d.add_button("Ok", GtkResponseType::Ok);
        d.set_default_response(GtkResponseType::Ok);

        let parent = match self.gui.account.sidebar.tree.get_selected_uuid() {
            Some(id) => {
                let file = self.core.file_by_id(id)?;
                Ok(match file.file_type {
                    FileType::Document => file.parent,
                    FileType::Folder => file.id,
                })
            }
            None => Err(uerr_dialog!("No destination is selected to create from!")),
        }?;

        d.connect_response(closure!(self as lb => move |d, resp| {
            if resp != GtkResponseType::Ok {
                d.close();
                return;
            }

            let name = entry.get_buffer().get_text();

            match lb.core.create_file(&name, parent, file_type) {
                Ok(file) => {
                    d.close();

                    match lb.gui.account.add_file(&lb.core, &file) {
                        Ok(_) => {
                            lb.messenger.send(Msg::RefreshSyncStatus);
                            lb.messenger.send(Msg::OpenFile(Some(file.id)));
                        }
                        Err(err) => lb.messenger.send_err_dialog("adding file to file tree", err)
                    }
                }
                Err(err) => match err.kind() {
                    UserErr => {
                        util::gui::add(&d.get_content_area(), &errlbl);
                        errlbl.set_text(err.msg());
                        errlbl.show();
                    }
                    ProgErr => {
                        d.close();
                        lb.messenger.send_err_dialog("creating file", err);
                    }
                },
            }
        }));

        d.show_all();
        Ok(())
    }

    fn open_file(&self, maybe_id: Option<Uuid>) -> LbResult<()> {
        // Ask the user how the want to deal with unsaved changes
        if self.state.borrow().open_file_dirty {
            if let Some(open_file) = self.state.borrow().opened_file.clone() {
                if !self.save_file_with_dialog(&open_file) {
                    // File was not dealt with (dialog was closed) return early or we will lose unsaved work
                    // Re-select the file that was open to make it clear the user's action was cancelled
                    self.gui.account.sidebar.tree.select(&open_file.id);
                    return Ok(());
                }
            }
        }

        let selected = self.gui.account.sidebar.tree.get_selected_uuid();

        if let Some(id) = maybe_id.or(selected) {
            let meta = self.core.file_by_id(id)?;
            self.gui.win.set_title(&meta.decrypted_name);
            self.state.borrow_mut().set_opened_file(Some(meta.clone()));

            match meta.file_type {
                FileType::Document => self.open_document(&meta.id),
                FileType::Folder => self.open_folder(&meta),
            }
        } else {
            Ok(())
        }
    }

    fn save_file_with_dialog(&self, open_file: &DecryptedFileMetadata) -> bool {
        let file_dealt_with = Rc::new(RefCell::new(false));

        let msg = format!("{} has unsaved changes.", open_file.decrypted_name);
        let lbl = GtkLabel::new(Some(&msg));
        util::gui::set_marginx(&lbl, 16);
        lbl.set_margin_top(16);

        let d = self.gui.new_dialog(&open_file.decrypted_name);

        let save = gtk::Button::with_label("Save");
        save.connect_clicked(closure!(
            self.core as core, // to save
            self.gui.account as account, // to get text
            self.messenger as m, // to propagate errors
            save, // to keep the user informed about the operation
            d, // to dismiss the dialog
            file_dealt_with, // to detect if the operation is cancelled
            open_file // what file are we saving

            => move |_| {
                save.set_label("Saving...");
                save.set_sensitive(true);
                file_dealt_with.replace(true);

                let ch = make_glib_chan!(m, d => move |result: LbResult<()>| {
                    match result {
                        Ok(_) => d.close(),
                        Err(err) => m.send_err_dialog("saving file", err),
                    };
                    glib::Continue(false)
                });

                let content = account.text_content();
                spawn!(core, open_file => move || {
                    ch.send(core.save(open_file.id, content)).unwrap()
                });
            }
        ));

        let discard = gtk::Button::with_label("Discard");
        discard.connect_clicked(closure!(d, file_dealt_with => move |_| {
            file_dealt_with.replace(true);
            d.close();
        }));

        let buttons = GtkBox::new(Horizontal, 16);
        buttons.set_halign(GtkAlign::Center);
        buttons.add(&discard);
        buttons.add(&save);

        d.get_content_area().add(&lbl);
        d.get_content_area().add(&buttons);
        d.show_all();
        d.run();

        unsafe {
            // This is the idiomatic way to dismiss a dialog programmatically (without default buttons)
            // The default buttons don't allow you to do an async operation like save before closing the dialog
            // (they call destroy under the hood)
            d.destroy();
        }

        let file_dealt_with = *file_dealt_with.borrow();
        file_dealt_with
    }

    fn open_document(&self, id: &Uuid) -> LbResult<()> {
        // Check for file dirtiness here
        let (meta, content) = self.core.open(id)?;
        self.state.borrow_mut().open_file_dirty = false;
        self.edit(&EditMode::PlainText { meta, content })
    }

    fn open_folder(&self, f: &DecryptedFileMetadata) -> LbResult<()> {
        let children = self.core.children(f)?;
        self.edit(&EditMode::Folder {
            meta: f.clone(),
            n_children: children.len(),
        })
    }

    fn edit(&self, mode: &EditMode) -> LbResult<()> {
        self.gui.menubar.set(mode);
        self.gui.account.show(mode);
        Ok(())
    }

    fn file_edited(&self) -> LbResult<()> {
        let open_file = self.state.borrow().opened_file.clone();
        if let Some(f) = open_file {
            self.gui.win.set_title(&format!("{}*", f.decrypted_name));
            self.state.borrow_mut().open_file_dirty = true;

            self.state
                .borrow()
                .background_work
                .lock()
                .unwrap()
                .auto_save_state
                .file_changed();
        }
        Ok(())
    }

    fn save(&self) -> LbResult<()> {
        let open_file = self.state.borrow().opened_file.clone();

        if let Some(f) = open_file {
            if f.file_type == FileType::Document {
                self.gui.win.set_title(&f.decrypted_name);
                self.state.borrow_mut().open_file_dirty = false;
                let acctscr = self.gui.account.clone();
                acctscr.set_saving(true);

                let id = f.id;
                let content = acctscr.text_content();

                let ch = make_glib_chan!(self.messenger as m => move |result: LbResult<()>| {
                    match result {
                        Ok(_) => {
                            acctscr.set_saving(false);
                            m.send(Msg::RefreshSyncStatus);
                        }
                        Err(err) => m.send_err_dialog("saving file", err),
                    }
                    glib::Continue(false)
                });

                spawn!(self.core as c => move || ch.send(c.save(id, content)).unwrap());
            }
        }
        Ok(())
    }

    fn close_file(&self) -> LbResult<()> {
        self.gui.win.set_title(DEFAULT_WIN_TITLE);
        let mut state = self.state.borrow_mut();
        if state.opened_file.as_ref().is_some() {
            self.edit(&EditMode::None)?;
            state.set_opened_file(None);
        }
        Ok(())
    }

    fn delete_files(&self) -> LbResult<()> {
        let (selected_files, tmodel) = self.gui.account.sidebar.tree.selected_rows();
        if selected_files.is_empty() {
            return Err(uerr_dialog!("No file tree items are selected to delete!"));
        }

        let mut file_data: Vec<(String, Uuid, String)> = Vec::new();
        for tpath in selected_files {
            let iter = tmodel.get_iter(&tpath).unwrap();
            let id = tree_iter_value!(tmodel, &iter, 2, String);
            let uuid = Uuid::parse_str(&id).unwrap();

            let meta = self.core.file_by_id(uuid)?;
            let path = self.core.full_path_for(&meta.id)?;

            let n_children = if meta.file_type == FileType::Folder {
                let children = self.core.get_children_recursively(meta.id).ok().unwrap();
                (children.len() - 1).to_string()
            } else {
                "".to_string()
            };

            file_data.push((path, meta.id, n_children));
        }

        let tree_add_col = |tree: &GtkTreeView, name: &str, id| {
            let cell = GtkCellRendererText::new();
            cell.set_padding(12, 4);

            let c = GtkTreeViewColumn::new();
            c.set_title(name);
            c.pack_start(&cell, true);
            c.add_attribute(&cell, "text", id);
            tree.append_column(&c);
        };

        let model = GtkTreeStore::new(&[glib::Type::String, glib::Type::String]);
        let tree = GtkTreeView::with_model(&model);
        util::gui::set_margin(&tree, 16);
        tree.get_selection().set_mode(GtkSelectionMode::None);
        tree.set_enable_search(false);
        tree.set_can_focus(false);
        tree_add_col(&tree, "Name", 0);
        tree_add_col(&tree, "Children", 1);
        for f in &file_data {
            let (path, _, n_children) = f;
            model.insert_with_values(None, None, &[0, 1], &[&path, &n_children]);
        }

        let msg = "Are you absolutely sure you want to delete the following files?";
        let lbl = GtkLabel::new(Some(msg));
        util::gui::set_marginx(&lbl, 16);
        lbl.set_margin_top(16);

        let d = self.gui.new_dialog("Confirm Delete");
        d.get_content_area().add(&lbl);
        d.get_content_area().add(&tree);
        d.get_content_area().show_all();
        d.set_default_response(GtkResponseType::Cancel);
        d.add_button("No", GtkResponseType::Cancel);
        d.add_button("I'm Sure", GtkResponseType::Yes);

        if d.run() == GtkResponseType::Yes {
            for f in &file_data {
                let (_, id, _) = f;
                self.core.delete(id)?;
                self.gui.account.sidebar.tree.remove(id);

                if let Some(file) = self.state.borrow().opened_file.clone() {
                    if file.id == *id {
                        self.messenger.send(Msg::CloseFile);
                    }
                }
            }
        }

        d.close();
        self.messenger.send(Msg::RefreshSyncStatus);
        Ok(())
    }

    fn rename_file(&self) -> LbResult<()> {
        // Get the iterator for the selected tree item.
        let (selected_tpaths, tmodel) = self.gui.account.sidebar.tree.selected_rows();
        let tpath = selected_tpaths.get(0).ok_or_else(|| {
            progerr!("No file tree items selected! At least one file tree item must be selected.")
        })?;
        let iter = tmodel
            .get_iter(tpath)
            .ok_or_else(|| progerr!("Unable to get the tree iterator for tree path: {}", tpath))?;

        // Get the FileMetadata from the iterator.
        let id = tree_iter_value!(tmodel, &iter, 2, String);
        let uuid = Uuid::parse_str(&id).map_err(LbError::fmt_program_err)?;
        let meta = self.core.file_by_id(uuid)?;

        let lbl = util::gui::text_left("Enter the new name:");
        lbl.set_margin_top(12);

        let entry = gtk::Entry::new();
        util::gui::set_marginy(&entry, 16);
        entry.set_margin_start(8);
        entry.set_activates_default(true);

        let errlbl = util::gui::text_left("");
        util::gui::set_widget_name(&errlbl, "err");
        errlbl.set_margin_start(8);
        errlbl.set_margin_bottom(8);

        let d = self
            .gui
            .new_dialog(&format!("Rename '{}'", meta.decrypted_name));
        util::gui::set_marginx(&d.get_content_area(), 16);
        d.set_default_size(300, -1);
        d.get_content_area().add(&lbl);
        d.get_content_area().add(&entry);
        d.add_button("Ok", GtkResponseType::Ok);
        d.set_default_response(GtkResponseType::Ok);

        d.connect_response(closure!(self as lb => move |d, resp| {
            if resp != GtkResponseType::Ok {
                d.close();
                return;
            }

            let (id, name) = (meta.id, entry.get_text());
            match lb.core.rename(&id, &name) {
                Ok(_) => {
                    d.close();
                    lb.gui.account.sidebar.tree.set_name(&id, &name);
                    if let Some(meta) = &lb.state.borrow().opened_file {
                        if meta.id == id {
                            lb.gui.win.set_title(&name);
                        }
                    }
                    lb.messenger.send(Msg::RefreshSyncStatus);
                }
                Err(err) => match err.kind() {
                    UserErr => {
                        util::gui::add(&d.get_content_area(), &errlbl);
                        errlbl.set_text(err.msg());
                        errlbl.show();
                    }
                    ProgErr => {
                        d.close();
                        lb.messenger.send_err_dialog("renaming file", err);
                    }
                },
            }
        }));

        d.show_all();
        Ok(())
    }

    fn toggle_tree_col(&self, c: FileTreeCol) -> LbResult<()> {
        self.gui.account.sidebar.tree.toggle_col(&c);
        self.settings.borrow_mut().toggle_tree_col(c.name());
        Ok(())
    }

    fn refresh_tree(&self) -> LbResult<()> {
        let root = self.core.root()?;
        let metadatas = self.core.get_all_files()?;
        self.gui.account.sidebar.tree.refresh(&root, &metadatas);
        Ok(())
    }

    fn prompt_search(&self) -> LbResult<()> {
        let possibs = self.core.list_paths().unwrap_or_default();
        let search = LbSearch::new(possibs);
        let d = self.gui.new_dialog("Search");

        let comp = gtk::EntryCompletion::new();
        comp.set_model(Some(&search.sort_model));
        comp.set_popup_completion(true);
        comp.set_inline_selection(true);
        comp.set_text_column(1);
        comp.set_match_func(|_, _, _| true);
        comp.connect_match_selected(glib::clone!(
            @strong self.messenger as m,
            @strong d
            => move |_, model, iter| {
                d.close();
                let iter_val = tree_iter_value!(model, iter, 1, String);
                m.send(Msg::SearchFieldExec(Some(iter_val)));
                gtk::Inhibit(false)
            }
        ));

        self.state.borrow_mut().search = Some(search);

        let search_entry = gtk::Entry::new();
        util::gui::set_entry_icon(&search_entry, "edit-find-symbolic");
        util::gui::set_marginx(&search_entry, 16);
        search_entry.set_margin_top(16);
        search_entry.set_placeholder_text(Some("Start typing..."));
        search_entry.set_completion(Some(&comp));

        search_entry.connect_key_release_event(closure!(self as lb => move |entry, key| {
            let k = key.get_hardware_keycode();
            if k != util::gui::KEY_ARROW_UP && k != util::gui::KEY_ARROW_DOWN {
                if let Some(search) = lb.state.borrow().search.as_ref() {
                    let input = entry.get_text().to_string();
                    search.update_for(&input);
                }
            }
            gtk::Inhibit(false)
        }));

        search_entry.connect_changed(move |entry| {
            let input = entry.get_text().to_string();
            let icon_name = if input.ends_with(".md") || input.ends_with(".txt") {
                "text-x-generic-symbolic"
            } else if input.ends_with('/') {
                "folder-symbolic"
            } else {
                "edit-find-symbolic"
            };
            util::gui::set_entry_icon(entry, icon_name);
        });

        search_entry.connect_activate(
            glib::clone!(@strong self.messenger as m, @strong d => move |entry| {
                d.close();
                if !entry.get_text().eq("") {
                    m.send(Msg::SearchFieldExec(None));
                }
            }),
        );

        d.set_size_request(400, -1);
        d.get_content_area().add(&search_entry);
        d.get_content_area().set_margin_bottom(16);
        d.show_all();
        Ok(())
    }

    fn search_field_exec(&self, maybe_input: Option<String>) -> LbResult<()> {
        if let Some(path) = maybe_input.or_else(|| self.state.borrow().get_first_search_match()) {
            match self.core.file_by_path(&path) {
                Ok(meta) => self.messenger.send(Msg::OpenFile(Some(meta.id))),
                Err(err) => self
                    .messenger
                    .send_err_dialog("opening file from search field", err),
            }
        }
        Ok(())
    }

    fn show_dialog_about(&self) -> LbResult<()> {
        let d = gtk::AboutDialog::new();
        d.set_transient_for(Some(&self.gui.win));
        d.set_logo(Some(
            &GdkPixbuf::from_inline(onboarding::LOGO, false).unwrap(),
        ));
        d.set_program_name("Lockbook");
        d.set_version(Some(VERSION));
        d.set_website(Some("https://lockbook.net"));
        d.set_authors(&["The Lockbook Team"]);
        d.set_license(Some(LICENSE));
        d.set_comments(Some(COMMENTS));
        d.show_all();
        if d.run() == GtkResponseType::DeleteEvent {
            d.close();
        }
        Ok(())
    }

    fn show_dialog_import_file(
        &self,
        parent: Uuid,
        uris: Vec<String>,
        finish_ch: Option<glib::Sender<Vec<String>>>,
    ) -> LbResult<()> {
        let (d, disk_lbl, lb_lbl, prog_lbl, pbar) = self.gui.new_import_export_dialog(true);

        const FILE_SCHEME: &str = "file://";

        let mut total = 0;
        let progress = Rc::new(RefCell::new(-1));

        let mut paths = Vec::new();

        for uri in &uris {
            if let Some(path) = uri.strip_prefix(FILE_SCHEME) {
                let escaped_uri = glib::uri_unescape_string(path, None)
                    .ok_or_else(|| uerr_dialog!("Unable to escape uri!"))?
                    .to_string();
                total += util::io::get_children_count(PathBuf::from(&escaped_uri))?;
                paths.push(escaped_uri);
            } else {
                return Err(uerr_dialog!("Unsupported uri!"));
            }
        }

        d.show_all();

        let ch = make_glib_chan!(self.messenger as m, progress => move |maybe_info: Option<ImportExportFileInfo>| {
            *progress.borrow_mut() += 1;
            pbar.set_fraction(*progress.borrow() as f64 / total as f64);

            match maybe_info {
                None => {
                    d.close();
                    m.send(Msg::RefreshTree);
                }
                Some(info) => {
                    lb_lbl.set_text(&info.lockbook_path);
                    disk_lbl.set_text(&format!("{}", info.disk_path.display()));
                    prog_lbl.set_text(&format!("{}/{}", *progress.borrow(), total));
                }
            }
            glib::Continue(true)
        });

        let import_progress = closure!(ch => move |progress: ImportExportFileInfo| {
            ch.send(Some(progress)).unwrap();
        });

        spawn!(self.core as c, self.messenger as m => move || {
            for path in &paths {
                if let Err(err) = c.import_file(parent, path, Some(Box::new(import_progress.clone()))) {
                    m.send_err_dialog("Import files", err);
                    break;
                };
            }

            ch.send(None).unwrap();

            if let Some(finish_ch) = finish_ch {
                let mut new_dests = Vec::new();
                let parent_path = match c.full_path_for(&parent) {
                    Ok(path) => path,
                    Err(err) => {
                        m.send_err_dialog("getting parent path", err);
                        return;
                    }
                };

                for path in paths {
                    let name = match PathBuf::from(path).file_name() {
                        None => {
                            m.send_err_dialog("getting disk file name", uerr_dialog!("Unable to get disk file's name"));
                            return;
                        }
                        Some(os_name) => os_name.to_string_lossy().into_owned(),
                    };

                    new_dests.push(format!("{}{}", parent_path, name));
                }

                finish_ch.send(new_dests).unwrap();
            }
        });

        Ok(())
    }

    fn show_dialog_export_file(&self) -> LbResult<()> {
        let (selected_tpaths, model) = self.gui.account.sidebar.tree.selected_rows();

        let ids = selected_tpaths
            .iter()
            .map(|selected| {
                Uuid::parse_str(&tree_iter_value!(
                    model,
                    &model.get_iter(selected).unwrap(),
                    2,
                    String
                ))
                .unwrap()
            })
            .collect::<Vec<Uuid>>();

        let d = gtk::FileChooserDialog::new(
            None,
            Some(&self.gui.win),
            gtk::FileChooserAction::SelectFolder,
        );

        d.add_buttons(&[
            ("Cancel", GtkResponseType::Cancel),
            ("Select", GtkResponseType::Ok),
        ]);

        let resp = d.run();
        d.close();

        if resp == GtkResponseType::Ok {
            let dest = d.get_filename().unwrap().to_string_lossy().into_owned();

            let (load_d, lb_lbl, disk_lbl, prog_lbl, pbar) =
                self.gui.new_import_export_dialog(false);

            load_d.show_all();

            let mut total = 0;
            let progress = Rc::new(RefCell::new(-1));

            for id in &ids {
                let meta = self.core.file_by_id(*id)?;

                total += match meta.file_type {
                    FileType::Document => 1,
                    FileType::Folder => self.core.get_children_recursively(*id)?.len(),
                }
            }

            let ch = make_glib_chan!(load_d, progress => move |maybe_info: Option<ImportExportFileInfo>| {
                *progress.borrow_mut() += 1;
                pbar.set_fraction(*progress.borrow() as f64 / total as f64);

                match maybe_info {
                    None => load_d.close(),
                    Some(info) => {
                        lb_lbl.set_text(&info.lockbook_path);
                        disk_lbl.set_text(&format!("{}", info.disk_path.display()));
                        prog_lbl.set_text(&format!("{}/{}", *progress.borrow(), total));
                    }
                }
                glib::Continue(true)
            });

            let export_progress = closure!(ch => move |progress: ImportExportFileInfo| {
                ch.send(Some(progress)).unwrap();
            });

            spawn!(self.core as c, self.messenger as m, dest, ch => move || {
                for id in ids {
                    if let Err(err) = c.export_file(id, &dest, Some(Box::new(export_progress.clone()))) {
                        m.send_err_dialog("Exporting file", err);
                        break;
                    };
                }

                ch.send(None).unwrap();
            });
        }

        Ok(())
    }

    fn show_dialog_usage(&self) -> LbResult<()> {
        let usage = usage_dialog(&self.core)?;
        let d = self.gui.new_dialog("My Lockbook Usage");
        d.get_content_area().add(&usage);
        d.show_all();
        Ok(())
    }

    fn paste_in_text_area(&self, info: TextAreaDropPasteInfo) -> LbResult<()> {
        let mark = self.gui.account.get_cursor_mark()?;

        let opened_file = self
            .state
            .borrow()
            .opened_file
            .clone()
            .ok_or_else(|| uerr_dialog!("Open a file before pasting!"))?;

        match info {
            TextAreaDropPasteInfo::Image(bytes) => {
                let parent_path = self.core.full_path_for(&opened_file.parent)?;

                let gdk_win = self.gui.account.cntr.get_window().unwrap();
                gdk_win.set_cursor(gdk::Cursor::from_name(&gdk_win.get_display(), "wait").as_ref());

                let image_name = format!("img-{}.{}", Uuid::new_v4(), "png");

                let ch = make_glib_chan!(self.gui.account as a, image_name, mark => move |is_successful: bool| {
                    if is_successful {
                        let link = format!("[](lb://{}{})\n", parent_path, image_name);
                        a.insert_text_at_mark(&mark, &link);
                    }

                    gdk_win.set_cursor(None);

                    glib::Continue(true)
                });

                spawn!(self.core as c, self.messenger as m => move || {
                    let is_successful = match c.create_file(&image_name, opened_file.parent, FileType::Document) {
                        Ok(metadata) => {
                            let is_successful = match c.write(metadata.id, bytes.as_slice()) {
                                Ok(_) => true,
                                Err(err) => {
                                    m.send_err_dialog("writing image", err);
                                    false
                                }
                            };
                            m.send(Msg::RefreshTree);
                            is_successful
                        }
                        Err(err) => {
                            m.send_err_dialog("creating image", err);
                            false
                        }
                    };

                    ch.send(is_successful).unwrap();
                });
            }
            TextAreaDropPasteInfo::Uris(uris) => {
                let finish_ch = make_glib_chan!(self.gui.account as a, mark => move |dests: Vec<String>| {

                    for dest in dests {
                        a.insert_text_at_mark(&mark, &format!("[](lb://{})\n", dest));
                    }

                    glib::Continue(true)
                });

                self.messenger.send(Msg::ShowDialogImportFile(
                    opened_file.parent,
                    uris,
                    Some(finish_ch),
                ));
            }
        }

        Ok(())
    }

    fn toggle_auto_sync(&self, auto_sync: bool) -> LbResult<()> {
        self.state
            .borrow()
            .background_work
            .lock()
            .unwrap()
            .auto_sync_state
            .is_active = auto_sync;

        Ok(())
    }

    fn toggle_auto_save(&self, auto_save: bool) -> LbResult<()> {
        self.state
            .borrow()
            .background_work
            .lock()
            .unwrap()
            .auto_save_state
            .is_active = auto_save;

        Ok(())
    }

    fn err_dialog(&self, title: &str, err: &LbError) {
        let details = util::gui::scrollable(&GtkLabel::new(Some(err.msg())));
        util::gui::set_margin(&details, 16);

        let copy = GtkBox::new(Horizontal, 0);
        copy.set_center_widget(Some(&util::gui::clipboard_btn(err.msg())));
        copy.set_margin_bottom(16);

        let d = self.gui.new_dialog(&format!("Error: {}", title));
        d.set_default_size(500, -1);
        d.get_content_area().add(&details);
        if err.is_prog() {
            d.get_content_area().add(&copy);
        }
        d.show_all();
    }
}

pub struct LbState {
    search: Option<LbSearch>,
    opened_file: Option<DecryptedFileMetadata>,
    open_file_dirty: bool,
    pub background_work: Arc<Mutex<BackgroundWork>>,
}

impl LbState {
    fn default(m: &Messenger) -> Self {
        Self {
            search: None,
            opened_file: None,
            open_file_dirty: false,
            background_work: Arc::new(Mutex::new(BackgroundWork::default(m))),
        }
    }

    fn get_first_search_match(&self) -> Option<String> {
        if let Some(search) = self.search.as_ref() {
            let model = &search.sort_model;
            if let Some(iter) = model.get_iter_first() {
                return Some(tree_iter_value!(model, &iter, 1, String));
            }
        }
        None
    }

    fn set_opened_file(&mut self, f: Option<DecryptedFileMetadata>) {
        self.opened_file = f;
        if self.opened_file.is_some() {
            self.search = None;
        }
    }
}

pub struct Gui {
    win: GtkAppWindow,
    menubar: Menubar,
    screens: GtkStack,
    pub onboarding: onboarding::Screen,
    pub account: Rc<AccountScreen>,
    messenger: Messenger,
}

impl Gui {
    fn new(app: &GtkApp, m: &Messenger, s: &Settings, c: &Arc<LbCore>) -> Self {
        // Menubar.
        let accels = gtk::AccelGroup::new();
        let menubar = Menubar::new(m, &accels);
        menubar.set(&EditMode::None);

        // Screens.
        let onboarding = onboarding::Screen::new(m);
        let account = AccountScreen::new(m, s, c);
        let screens = GtkStack::new();
        screens.add_named(&onboarding.cntr, "onboarding");
        screens.add_named(&account.cntr, "account");

        let icon = GdkPixbuf::from_inline(WINDOW_ICON, false).unwrap();

        // Window.
        let win = GtkAppWindow::new(app);
        win.set_title(DEFAULT_WIN_TITLE);
        win.set_icon(Some(&icon));
        win.add_accel_group(&accels);
        win.set_default_size(1300, 700);
        if s.window_maximize {
            win.maximize();
        }
        win.add(&{
            let base = GtkBox::new(Vertical, 0);
            base.add(menubar.widget());
            base.pack_start(&screens, true, true, 0);
            base
        });

        Self {
            win,
            menubar,
            screens,
            onboarding,
            account: Rc::new(account),
            messenger: m.clone(),
        }
    }

    fn show(&self, core: &LbCore) -> LbResult<()> {
        self.win.show_all();
        if core.has_account()? {
            self.show_account_screen();
        } else {
            self.show_onboarding_screen();
        }
        Ok(())
    }

    fn show_onboarding_screen(&self) {
        self.menubar.for_onboarding_screen();
        self.onboarding.cntr.show_all();
        self.screens.set_visible_child_name("onboarding");
    }

    pub fn show_account_screen(&self) {
        self.menubar.for_account_screen();
        self.account.cntr.show_all();

        self.messenger.send(Msg::RefreshTree);
        self.messenger.send(Msg::RefreshSyncStatus);

        self.account.sidebar.tree.focus();
        self.screens.set_visible_child_name("account");
        self.messenger.send(Msg::AccountScreenShown);
    }

    pub fn new_dialog(&self, title: &str) -> GtkDialog {
        let d = GtkDialog::new();
        d.set_transient_for(Some(&self.win));
        d.set_position(GtkWindowPosition::CenterOnParent);
        d.set_title(title);
        d
    }

    fn new_import_export_dialog(
        &self,
        is_import: bool,
    ) -> (GtkDialog, GtkLabel, GtkLabel, GtkLabel, GtkProgressBar) {
        let title = if is_import {
            "Import Files"
        } else {
            "Export Files"
        };

        let load_d = self.new_dialog(title);
        util::gui::set_marginy(&load_d, 36);
        util::gui::set_marginx(&load_d, 100);

        let path_lbl_1 = GtkLabel::new(None);
        util::gui::set_marginx(&path_lbl_1, 16);
        path_lbl_1.set_margin_top(5);

        let to_lbl = GtkLabel::new(Some("to"));
        util::gui::set_marginy(&to_lbl, 6);

        let path_lbl_2 = GtkLabel::new(None);
        util::gui::set_marginx(&path_lbl_2, 16);

        let pbar = GtkProgressBar::new();
        util::gui::set_marginx(&pbar, 16);
        util::gui::set_marginy(&pbar, 16);
        pbar.set_size_request(300, -1);

        let prog_lbl = GtkLabel::new(None);
        util::gui::set_marginy(&prog_lbl, 4);

        load_d.get_content_area().add(&path_lbl_1);
        load_d.get_content_area().add(&to_lbl);
        load_d.get_content_area().add(&path_lbl_2);
        load_d.get_content_area().add(&pbar);
        load_d.get_content_area().add(&prog_lbl);

        (load_d, path_lbl_1, path_lbl_2, prog_lbl, pbar)
    }
}

fn usage_dialog(c: &Arc<LbCore>) -> LbResult<GtkBox> {
    let usage = c.get_usage()?;

    let title = GtkLabel::new(Some("Total Usage"));
    let attr_list = pango::AttrList::new();
    let attr = pango::Attribute::new_weight(pango::Weight::Bold)
        .ok_or_else(|| progerr!("Unable to apply bold attribute to title."))?;

    attr_list.change(attr);
    title.set_attributes(Some(&attr_list));
    title.set_margin_bottom(20);

    let lbl = GtkLabel::new(Some(&format!(
        "{} / {}",
        usage.server_usage.readable, usage.data_cap.readable
    )));
    lbl.set_margin_bottom(24);

    let pbar = GtkProgressBar::new();
    util::gui::set_marginx(&pbar, 16);
    pbar.set_size_request(300, -1);
    pbar.set_fraction(usage.server_usage.exact as f64 / usage.data_cap.exact as f64);

    let cntr = GtkBox::new(Vertical, 0);
    util::gui::set_marginy(&cntr, 36);
    util::gui::set_marginx(&cntr, 100);
    cntr.add(&title);
    cntr.add(&lbl);
    cntr.add(&pbar);
    Ok(cntr)
}

const DEFAULT_WIN_TITLE: &str = "Lockbook";
const VERSION: &str = env!("CARGO_PKG_VERSION");
const LICENSE: &str = include_str!("../res/UNLICENSE");
const COMMENTS: &str = "Lockbook is a document editor that is secure, minimal, private, open source, and cross-platform.";
const WINDOW_ICON: &[u8] = include_bytes!("../res/lockbook-window-icon-pixdata");
