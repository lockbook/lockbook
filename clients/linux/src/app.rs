use std::cell::RefCell;
use std::cmp::Ordering;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};
use gdk_pixbuf::Pixbuf as GdkPixbuf;
use gio::prelude::*;
use gtk::prelude::*;
use gtk::Orientation::{Horizontal, Vertical};
use gtk::{
    AboutDialog as GtkAboutDialog, AccelGroup as GtkAccelGroup, Align as GtkAlign,
    Application as GtkApp, ApplicationWindow as GtkAppWindow, Box as GtkBox, Button,
    CellRendererText as GtkCellRendererText, CheckButton as GtkCheckBox, Dialog as GtkDialog,
    Entry as GtkEntry, EntryCompletion as GtkEntryCompletion, Image as GtkImage, Label as GtkLabel,
    ListStore as GtkListStore, Notebook as GtkNotebook, ResponseType as GtkResponseType,
    SelectionMode as GtkSelectionMode, SortColumn as GtkSortColumn, SortType as GtkSortType,
    Spinner as GtkSpinner, Stack as GtkStack, TreeIter as GtkTreeIter, TreeModel as GtkTreeModel,
    TreeModelSort as GtkTreeModelSort, TreeStore as GtkTreeStore, TreeView as GtkTreeView,
    TreeViewColumn as GtkTreeViewColumn, Widget as GtkWidget, WidgetExt as GtkWidgetExt,
    WindowPosition as GtkWindowPosition,
};
use uuid::Uuid;

use lockbook_models::file_metadata::{FileMetadata, FileType};
use lockbook_models::work_unit::WorkUnit;

use crate::account::AccountScreen;
use crate::auto_save::AutoSaveState;
use crate::backend::{LbCore, LbSyncMsg};
use crate::editmode::EditMode;
use crate::error::{
    LbErrKind::{Program as ProgErr, User as UserErr},
    LbError, LbResult,
};
use crate::filetree::FileTreeCol;
use crate::intro::{IntroScreen, LOGO_INTRO};
use crate::menubar::Menubar;
use crate::messages::{Messenger, Msg};
use crate::settings::Settings;
use crate::util;
use crate::{closure, progerr, tree_iter_value, uerr};
use std::thread;

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
    core: Arc<LbCore>,
    settings: Rc<RefCell<Settings>>,
    state: Rc<RefCell<LbState>>,
    gui: Rc<Gui>,
    messenger: Messenger,
}

impl LbApp {
    pub fn new(c: &Arc<LbCore>, s: &Rc<RefCell<Settings>>, a: &GtkApp) -> Self {
        let (sender, receiver) = glib::MainContext::channel::<Msg>(glib::PRIORITY_DEFAULT);
        let m = Messenger::new(sender);

        let gui = Gui::new(&a, &m, &s.borrow());

        let lb_app = Self {
            core: c.clone(),
            settings: s.clone(),
            state: Rc::new(RefCell::new(LbState::default(&m))),
            gui: Rc::new(gui),
            messenger: m,
        };

        if s.borrow().auto_save {
            let auto_save_state = lb_app.state.borrow().auto_save_state.clone();
            thread::spawn(move || {
                AutoSaveState::auto_save_loop(auto_save_state);
            });
        }

        let lb = lb_app.clone();
        receiver.attach(None, move |msg| {
            let maybe_err = match msg {
                Msg::CreateAccount(username) => lb.create_account(username),
                Msg::ImportAccount(privkey) => lb.import_account(privkey),
                Msg::ExportAccount => lb.export_account(),
                Msg::PerformSync => lb.perform_sync(),
                Msg::RefreshSyncStatus => lb.refresh_sync_status(),
                Msg::Quit => lb.quit(),

                Msg::NewFile(path) => lb.new_file(path),
                Msg::OpenFile(id) => lb.open_file(id),
                Msg::FileEdited => lb.file_edited(),
                Msg::SaveFile => lb.save(),
                Msg::CloseFile => lb.close_file(),
                Msg::DeleteFiles => lb.delete_files(),
                Msg::RenameFile => lb.rename_file(),

                Msg::ToggleTreeCol(col) => lb.toggle_tree_col(col),

                Msg::SearchFieldFocus => lb.search_field_focus(),
                Msg::SearchFieldBlur(escaped) => lb.search_field_blur(escaped),
                Msg::SearchFieldUpdate => lb.search_field_update(),
                Msg::SearchFieldUpdateIcon => lb.search_field_update_icon(),
                Msg::SearchFieldExec(vopt) => lb.search_field_exec(vopt),

                Msg::ShowDialogNew => lb.show_dialog_new(),
                Msg::ShowDialogSyncDetails => lb.show_dialog_sync_details(),
                Msg::ShowDialogPreferences => lb.show_dialog_preferences(),
                Msg::ShowDialogUsage => lb.show_dialog_usage(),
                Msg::ShowDialogAbout => lb.show_dialog_about(),

                Msg::Error(title, err) => {
                    lb.err(&title, &err);
                    Ok(())
                }
            };
            if let Err(err) = maybe_err {
                lb.err("", &err);
            }
            glib::Continue(true)
        });

        lb_app
    }

    pub fn show(&self) -> LbResult<()> {
        self.gui.show(&self.core)
    }

    fn create_account(&self, name: String) -> LbResult<()> {
        self.gui.intro.doing("Creating account...");

        let ch = make_glib_chan!(self as lb => move |result: LbResult<()>| {
            match result {
                Ok(_) => {
                    if let Err(err) = lb.gui.show_account_screen(&lb.core) {
                        lb.messenger.send_err("showing account screen", err);
                    }
                }
                Err(err) => match err.kind() {
                    UserErr => lb.gui.intro.error_create(&err.msg()),
                    ProgErr => lb.messenger.send_err("creating account", err),
                },
            }
            glib::Continue(false)
        });

        spawn!(self.core as c => move || ch.send(c.create_account(&name)).unwrap());
        Ok(())
    }

    fn import_account(&self, privkey: String) -> LbResult<()> {
        self.gui.intro.doing("Importing account...");

        // Create a channel to receive and process the result of importing the account.
        let ch = make_glib_chan!(self as lb => move |result: LbResult<()>| {
            // Show any error on the import screen. Otherwise, account syncing will start.
            match result {
                Ok(_) => lb.import_account_sync(),
                Err(err) => lb.gui.intro.error_import(err.msg()),
            }
            glib::Continue(false)
        });

        // In a separate thread, import the account and send the result down the channel.
        spawn!(self.core as c, self.messenger as m => move || {
            if let Err(err) = ch.send(c.import_account(&privkey)) {
                m.send_err("sending import result", LbError::fmt_program_err(err));
            }
        });

        Ok(())
    }

    fn import_account_sync(&self) {
        // Create a channel to receive and process any account sync progress updates.
        let sync_chan = make_glib_chan!(self as lb => move |msgopt: Option<LbSyncMsg>| {
            // If there is some message, show it. If not, syncing is done, so try to show the
            // account screen. If the account screen is successfully shown, get the account's
            // sync status.
            if let Some(msg) = msgopt {
                lb.gui.intro.sync_progress(&msg)
            } else if let Err(err) = lb.gui.show_account_screen(&lb.core) {
                lb.messenger.send_err("showing account screen", err);
            } else {
                lb.messenger.send(Msg::RefreshSyncStatus);
            }
            glib::Continue(true)
        });

        // In a separate thread, start syncing the account. Pass the sync channel which will be
        // used to receive progress updates as indicated above.
        spawn!(self.core as c, self.messenger as m => move || {
            if let Err(err) = c.sync(&sync_chan) {
                m.send_err("syncing", err);
            }
        });
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
            Ok(privkey) => {
                let btn_cntr = GtkBox::new(Horizontal, 0);
                btn_cntr.set_center_widget(Some(&util::gui::clipboard_btn(&privkey)));
                btn_cntr.set_margin_bottom(8);

                let d = self.gui.new_dialog("Lockbook Private Key");
                d.get_content_area().pack_start(&image_cntr, true, true, 8);
                d.get_content_area().add(&btn_cntr);
                d.set_resizable(false);
                d.show_all();
            }
            Err(err) => self.err("unable to export account", &err),
        }

        let ch = make_glib_chan!(self.messenger as m => move |res: LbResult<String>| {
            match res {
                Ok(path) => {
                    let qr_image = GtkImage::from_file(&path);
                    image_cntr.set_center_widget(Some(&qr_image));
                    image_cntr.show_all();
                }
                Err(err) => m.send_err("generating qr code", err),
            }
            glib::Continue(false)
        });

        spawn!(self.core as c => move || ch.send(c.account_qrcode()).unwrap());
        Ok(())
    }

    fn perform_sync(&self) -> LbResult<()> {
        let sync_ui = self.gui.account.sync().clone();
        sync_ui.set_syncing(true);

        let ch = make_glib_chan!(self as lb => move |msgopt: Option<LbSyncMsg>| {
            if let Some(msg) = msgopt {
                sync_ui.sync_progress(&msg);
            } else {
                sync_ui.set_syncing(false);
                lb.messenger.send(Msg::RefreshSyncStatus);
            }
            glib::Continue(true)
        });

        spawn!(self.core as c, self.messenger as m => move || {
            if let Err(err) = c.sync(&ch) {
                m.send_err("syncing", err);
            }
        });

        Ok(())
    }

    fn refresh_sync_status(&self) -> LbResult<()> {
        let s = self.core.sync_status()?;
        self.gui.account.sync().set_status(&s);
        Ok(())
    }

    fn quit(&self) -> LbResult<()> {
        self.gui.win.close();
        Ok(())
    }

    fn new_file(&self, path: String) -> LbResult<()> {
        let file = self.core.create_file_at_path(&path)?;
        self.gui.account.add_file(&self.core, &file)?;

        self.refresh_sync_status()?;
        self.open_file(Some(file.id))
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
            self.gui.win.set_title(&meta.name);
            self.state.borrow_mut().set_opened_file(Some(meta.clone()));

            match meta.file_type {
                FileType::Document => self.open_document(&meta.id),
                FileType::Folder => self.open_folder(&meta),
            }
        } else {
            Ok(())
        }
    }

    fn save_file_with_dialog(&self, open_file: &FileMetadata) -> bool {
        let file_dealt_with = Rc::new(RefCell::new(false));

        let msg = format!("{} has unsaved changes.", open_file.name);
        let lbl = GtkLabel::new(Some(&msg));
        util::gui::set_marginx(&lbl, 16);
        lbl.set_margin_top(16);

        let d = self.gui.new_dialog(&open_file.name);

        let save = Button::with_label("Save");
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
                        Err(err) => m.send_err("saving file", err),
                    };
                    glib::Continue(false)
                });

                let content = account.text_content();
                spawn!(core, open_file => move || {
                    ch.send(core.save(open_file.id, content)).unwrap()
                });
            }
        ));

        let discard = Button::with_label("Discard");
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
        let (meta, content) = self.core.open(&id)?;
        self.state.borrow_mut().open_file_dirty = false;
        self.edit(&EditMode::PlainText {
            path: self.core.full_path_for(&meta),
            meta,
            content,
        })
    }

    fn open_folder(&self, f: &FileMetadata) -> LbResult<()> {
        let children = self.core.children(&f)?;
        self.edit(&EditMode::Folder {
            path: self.core.full_path_for(&f),
            meta: f.clone(),
            n_children: children.len(),
        })
    }

    fn edit(&self, mode: &EditMode) -> LbResult<()> {
        self.gui.menubar.set(&mode);
        self.gui.account.show(&mode);
        Ok(())
    }

    fn file_edited(&self) -> LbResult<()> {
        let open_file = self.state.borrow().opened_file.clone();
        if let Some(f) = open_file {
            self.gui.win.set_title(&format!("{}*", f.name));
            self.state.borrow_mut().open_file_dirty = true;

            self.state
                .borrow()
                .auto_save_state
                .lock()
                .unwrap()
                .file_changed();
        }
        Ok(())
    }

    fn save(&self) -> LbResult<()> {
        let open_file = self.state.borrow().opened_file.clone();

        if let Some(f) = open_file {
            if f.file_type == FileType::Document {
                self.gui.win.set_title(&f.name);
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
                        Err(err) => m.send_err("saving file", err),
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
            return Err(uerr!("No file tree items are selected to delete!"));
        }

        let mut file_data: Vec<(String, Uuid, String)> = Vec::new();
        for tpath in selected_files {
            let iter = tmodel.get_iter(&tpath).unwrap();
            let id = tree_iter_value!(tmodel, &iter, 1, String);
            let uuid = Uuid::parse_str(&id).unwrap();

            let meta = self.core.file_by_id(uuid)?;
            let path = self.core.full_path_for(&meta);

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
            c.set_title(&name);
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
        let lbl = GtkLabel::new(Some(&msg));
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
        self.refresh_sync_status()
    }

    fn rename_file(&self) -> LbResult<()> {
        // Get the iterator for the selected tree item.
        let (selected_tpaths, tmodel) = self.gui.account.sidebar.tree.selected_rows();
        let tpath = selected_tpaths.get(0).ok_or_else(|| {
            progerr!("No file tree items selected! At least one file tree item must be selected.")
        })?;
        let iter = tmodel
            .get_iter(&tpath)
            .ok_or_else(|| progerr!("Unable to get the tree iterator for tree path: {}", tpath))?;

        // Get the FileMetadata from the iterator.
        let id = tree_iter_value!(tmodel, &iter, 1, String);
        let uuid = Uuid::parse_str(&id).map_err(LbError::fmt_program_err)?;
        let meta = self.core.file_by_id(uuid)?;

        let lbl = util::gui::text_left("Enter the new name:");
        lbl.set_margin_top(12);

        let entry = GtkEntry::new();
        util::gui::set_marginy(&entry, 16);
        entry.set_margin_start(8);
        entry.set_activates_default(true);

        let errlbl = util::gui::text_left("");
        util::gui::set_widget_name(&errlbl, "err");
        errlbl.set_margin_start(8);
        errlbl.set_margin_bottom(8);

        let d = self.gui.new_dialog(&format!("Rename '{}'", meta.name));
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
                    let acctscr = &lb.gui.account;
                    acctscr.sidebar.tree.set_name(&id, &name);
                    lb.gui.win.set_title(&name);

                    match lb.core.file_by_id(id) {
                        Ok(f) => {
                            acctscr.set_search_field_text(&lb.core.full_path_for(&f));
                            lb.messenger.send(Msg::RefreshSyncStatus);
                        }
                        Err(err) => lb.messenger.send_err("getting renamed file", err)
                    }
                }
                Err(err) => match err.kind() {
                    UserErr => {
                        util::gui::add(&d.get_content_area(), &errlbl);
                        errlbl.set_text(&err.msg());
                        errlbl.show();
                    }
                    ProgErr => {
                        d.close();
                        lb.messenger.send_err("renaming file", err);
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

    fn show_dialog_new(&self) -> LbResult<()> {
        let entry = GtkEntry::new();
        entry.set_activates_default(true);

        let d = self.gui.new_dialog("New...");
        d.get_content_area().add(&entry);
        d.add_button("Ok", GtkResponseType::Ok);
        d.set_default_response(GtkResponseType::Ok);
        d.show_all();

        if d.run() == GtkResponseType::Ok {
            let path = entry.get_buffer().get_text();
            self.messenger.send(Msg::NewFile(path));
            d.close();
        }
        Ok(())
    }

    fn search_field_focus(&self) -> LbResult<()> {
        let search = SearchComponents::new(&self.core);

        let comp = GtkEntryCompletion::new();
        comp.set_model(Some(&search.sort_model));
        comp.set_popup_completion(true);
        comp.set_inline_selection(true);
        comp.set_text_column(1);
        comp.set_match_func(|_, _, _| true);

        comp.connect_match_selected(closure!(self.messenger as m => move |_, model, iter| {
            let iter_val = tree_iter_value!(model, iter, 1, String);
            m.send(Msg::SearchFieldExec(Some(iter_val)));
            gtk::Inhibit(false)
        }));

        self.gui.account.set_search_field_completion(&comp);
        self.state.borrow_mut().search = Some(search);
        Ok(())
    }

    fn search_field_update(&self) -> LbResult<()> {
        if let Some(search) = self.state.borrow().search.as_ref() {
            let input = self.gui.account.get_search_field_text();
            search.update_for(&input);
        }
        Ok(())
    }

    fn search_field_update_icon(&self) -> LbResult<()> {
        let input = self.gui.account.get_search_field_text();
        let icon_name = if input.ends_with(".md") || input.ends_with(".txt") {
            "text-x-generic-symbolic"
        } else if input.ends_with('/') {
            "folder-symbolic"
        } else {
            "edit-find-symbolic"
        };
        self.gui.account.set_search_field_icon(icon_name, None);
        Ok(())
    }

    fn search_field_blur(&self, escaped: bool) -> LbResult<()> {
        let state = self.state.borrow();
        let opened_file = state.opened_file.as_ref();

        if escaped {
            match opened_file {
                Some(_) => self.gui.account.focus_editor(),
                None => self.gui.account.sidebar.tree.focus(),
            }
        }

        let txt = opened_file.map_or("".to_string(), |f| self.core.full_path_for(f));
        self.gui.account.deselect_search_field();
        self.gui.account.set_search_field_text(&txt);
        Ok(())
    }

    fn search_field_exec(&self, explicit: Option<String>) -> LbResult<()> {
        let entry_text = self.gui.account.get_search_field_text();
        let best_match = self.state.borrow().get_first_search_match();
        let path = explicit.unwrap_or_else(|| best_match.unwrap_or(entry_text));

        match self.core.file_by_path(&path) {
            Ok(meta) => self.messenger.send(Msg::OpenFile(Some(meta.id))),
            Err(_) => self.gui.account.set_search_field_icon(
                "dialog-error-symbolic",
                Some(&format!("The file '{}' does not exist", path)),
            ),
        }
        Ok(())
    }

    fn show_dialog_sync_details(&self) -> LbResult<()> {
        const RESP_REFRESH: u16 = 1;

        let details = sync_details(&self.core)?;

        let d = self.gui.new_dialog("Sync Details");
        d.get_content_area().set_center_widget(Some(&details));
        d.add_button("Refresh", GtkResponseType::Other(RESP_REFRESH));
        d.add_button("Close", GtkResponseType::Close);
        d.connect_response(closure!(self as lb => move |d, r| match r {
            GtkResponseType::Other(RESP_REFRESH) => match sync_details(&lb.core) {
                Ok(details) => {
                    lb.messenger.send(Msg::RefreshSyncStatus);
                    d.get_content_area().set_center_widget(Some(&details));
                    d.get_content_area().show_all();
                    d.set_position(GtkWindowPosition::CenterAlways);
                }
                Err(err) => lb.messenger.send_err("building sync details ui", err),
            },
            _ => d.close(),
        }));
        d.show_all();

        Ok(())
    }

    fn show_dialog_preferences(&self) -> LbResult<()> {
        let tabs = SettingsUi::create(&self.settings, &self.messenger);

        let d = self.gui.new_dialog("Lockbook Settings");
        d.set_default_size(300, 400);
        d.get_content_area().add(&tabs);
        d.add_button("Ok", GtkResponseType::Ok);
        d.connect_response(|d, _| d.close());
        d.show_all();
        Ok(())
    }

    fn show_dialog_about(&self) -> LbResult<()> {
        let d = GtkAboutDialog::new();
        d.set_transient_for(Some(&self.gui.win));
        d.set_logo(Some(&GdkPixbuf::from_inline(LOGO_INTRO, false).unwrap()));
        d.set_program_name("Lockbook");
        d.set_version(Some(VERSION));
        d.set_website(Some("https://lockbook.app"));
        d.set_authors(&["The Lockbook Team"]);
        d.set_license(Some(LICENSE));
        d.set_comments(Some(COMMENTS));
        d.show_all();
        if d.run() == GtkResponseType::DeleteEvent {
            d.close();
        }
        Ok(())
    }

    fn show_dialog_usage(&self) -> LbResult<()> {
        let usage_string = self.core.usage_human_string()?;
        let usage = usage(usage_string)?;
        let d = self.gui.new_dialog("My Lockbook Usage");
        d.get_content_area().add(&usage);
        d.show_all();
        Ok(())
    }

    fn err(&self, title: &str, err: &LbError) {
        let details = util::gui::scrollable(&GtkLabel::new(Some(&err.msg())));
        util::gui::set_margin(&details, 16);

        let copy = GtkBox::new(Horizontal, 0);
        copy.set_center_widget(Some(&util::gui::clipboard_btn(&err.msg())));
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

struct LbState {
    search: Option<SearchComponents>,
    opened_file: Option<FileMetadata>,
    open_file_dirty: bool,
    auto_save_state: Arc<Mutex<AutoSaveState>>,
}

impl LbState {
    fn default(m: &Messenger) -> Self {
        Self {
            search: None,
            opened_file: None,
            open_file_dirty: false,
            auto_save_state: Arc::new(Mutex::new(AutoSaveState::default(&m))),
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

    fn set_opened_file(&mut self, f: Option<FileMetadata>) {
        self.opened_file = f;
        if self.opened_file.is_some() {
            self.search = None;
        }
    }
}

struct SearchComponents {
    possibs: Vec<String>,
    list_store: GtkListStore,
    sort_model: GtkTreeModelSort,
    matcher: SkimMatcherV2,
}

impl SearchComponents {
    fn new(core: &LbCore) -> Self {
        let list_store = GtkListStore::new(&[glib::Type::I64, glib::Type::String]);
        let sort_model = GtkTreeModelSort::new(&list_store);
        sort_model.set_sort_column_id(GtkSortColumn::Index(0), GtkSortType::Descending);
        sort_model.set_sort_func(GtkSortColumn::Index(0), Self::compare_possibs);

        Self {
            possibs: core.list_paths_without_root().unwrap_or_default(),
            list_store,
            sort_model,
            matcher: SkimMatcherV2::default(),
        }
    }

    fn compare_possibs(model: &GtkTreeModel, it1: &GtkTreeIter, it2: &GtkTreeIter) -> Ordering {
        let score1 = tree_iter_value!(model, it1, 0, i64);
        let score2 = tree_iter_value!(model, it2, 0, i64);

        match score1.cmp(&score2) {
            Ordering::Greater => Ordering::Greater,
            Ordering::Less => Ordering::Less,
            Ordering::Equal => {
                let text1 = tree_iter_value!(model, it1, 1, String);
                let text2 = model
                    .get_value(&it2, 1)
                    .get::<String>()
                    .unwrap_or_default()
                    .unwrap_or_default();
                if text2.is_empty() {
                    return Ordering::Less;
                }

                let chars1: Vec<char> = text1.chars().collect();
                let chars2: Vec<char> = text2.chars().collect();

                let n_chars1 = chars1.len();
                let n_chars2 = chars2.len();

                for i in 0..std::cmp::min(n_chars1, n_chars2) {
                    let ord = chars1[i].cmp(&chars2[i]);
                    if ord != Ordering::Equal {
                        return ord.reverse();
                    }
                }

                n_chars1.cmp(&n_chars2)
            }
        }
    }

    fn update_for(&self, pattern: &str) {
        let list = &self.list_store;
        list.clear();

        for p in &self.possibs {
            if let Some(score) = self.matcher.fuzzy_match(&p, &pattern) {
                let values: [&dyn ToValue; 2] = [&score, &p];
                list.set(&list.append(), &[0, 1], &values);
            }
        }
    }
}

struct Gui {
    win: GtkAppWindow,
    menubar: Menubar,
    screens: GtkStack,
    intro: IntroScreen,
    account: Rc<AccountScreen>,
}

impl Gui {
    fn new(app: &GtkApp, m: &Messenger, s: &Settings) -> Self {
        // Menubar.
        let accels = GtkAccelGroup::new();
        let menubar = Menubar::new(m, &accels);
        menubar.set(&EditMode::None);

        // Screens.
        let intro = IntroScreen::new(m);
        let account = AccountScreen::new(m, &s);
        let screens = GtkStack::new();
        screens.add_named(&intro.cntr, "intro");
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
            intro,
            account: Rc::new(account),
        }
    }

    fn show(&self, core: &LbCore) -> LbResult<()> {
        self.win.show_all();
        if core.has_account()? {
            self.show_account_screen(&core)
        } else {
            self.show_intro_screen()
        }
    }

    fn show_intro_screen(&self) -> LbResult<()> {
        self.menubar.for_intro_screen();
        self.intro.cntr.show_all();
        self.screens.set_visible_child_name("intro");
        Ok(())
    }

    fn show_account_screen(&self, core: &LbCore) -> LbResult<()> {
        self.menubar.for_account_screen();
        self.account.cntr.show_all();
        self.account.fill(&core)?;
        self.account.sidebar.tree.focus();
        self.screens.set_visible_child_name("account");
        Ok(())
    }

    fn new_dialog(&self, title: &str) -> GtkDialog {
        let d = GtkDialog::new();
        d.set_transient_for(Some(&self.win));
        d.set_position(GtkWindowPosition::CenterOnParent);
        d.set_title(&title);
        d
    }
}

struct SettingsUi;
impl SettingsUi {
    fn create(s: &Rc<RefCell<Settings>>, m: &Messenger) -> GtkNotebook {
        let tabs = GtkNotebook::new();
        for tab_data in vec![
            ("File Tree", Self::filetree(&s, &m)),
            ("Window", Self::window(&s)),
            ("Editor", Self::editor(&s)),
        ] {
            let (title, content) = tab_data;
            let tab_btn = GtkLabel::new(Some(title));
            let tab_page = content.upcast::<GtkWidget>();
            tabs.append_page(&tab_page, Some(&tab_btn));
        }
        tabs
    }

    fn filetree(s: &Rc<RefCell<Settings>>, m: &Messenger) -> GtkBox {
        let chbxs = GtkBox::new(Vertical, 0);

        for col in FileTreeCol::removable() {
            let ch = GtkCheckBox::with_label(&col.name());
            ch.set_active(!s.borrow().hidden_tree_cols.contains(&col.name()));
            ch.connect_toggled(closure!(m => move |_| m.send(Msg::ToggleTreeCol(col))));
            chbxs.add(&ch);
        }

        chbxs
    }

    fn window(s: &Rc<RefCell<Settings>>) -> GtkBox {
        let ch = GtkCheckBox::with_label("Maximize on startup");
        ch.set_active(s.borrow().window_maximize);
        ch.connect_toggled(closure!(s => move |chbox| {
            s.borrow_mut().window_maximize = chbox.get_active();
        }));

        let chbxs = GtkBox::new(Vertical, 0);
        chbxs.add(&ch);
        chbxs
    }

    fn editor(s: &Rc<RefCell<Settings>>) -> GtkBox {
        let ch = GtkCheckBox::with_label("Auto-save ");
        ch.set_active(s.borrow().auto_save);
        ch.connect_toggled(closure!(s => move |chbox| {
            s.borrow_mut().auto_save = chbox.get_active();
        }));

        let chbxs = GtkBox::new(Vertical, 0);
        chbxs.add(&ch);
        chbxs
    }
}

fn sync_details(c: &Arc<LbCore>) -> LbResult<GtkBox> {
    let work = c.calculate_work()?;
    let n_units = work.work_units.len();

    let cntr = GtkBox::new(Vertical, 0);
    cntr.set_hexpand(true);
    if n_units == 0 {
        let lbl = GtkLabel::new(Some("All synced up!"));
        lbl.set_margin_top(12);
        lbl.set_margin_bottom(16);
        cntr.add(&lbl);
    } else {
        let desc = util::gui::text_left(&format!(
            "The following {} to sync:",
            if n_units > 1 {
                format!("{} changes need", n_units)
            } else {
                "change needs".to_string()
            }
        ));
        desc.set_margin_start(12);
        desc.set_margin_top(12);

        let tree_add_col = |tree: &GtkTreeView, name: &str, id| {
            let cell = GtkCellRendererText::new();
            cell.set_padding(12, 4);

            let c = GtkTreeViewColumn::new();
            c.set_title(&name);
            c.pack_start(&cell, true);
            c.add_attribute(&cell, "text", id);
            tree.append_column(&c);
        };

        let model = GtkTreeStore::new(&[glib::Type::String, glib::Type::String]);
        let tree = GtkTreeView::with_model(&model);
        tree.get_selection().set_mode(GtkSelectionMode::None);
        tree.set_enable_search(false);
        tree.set_can_focus(false);
        tree_add_col(&tree, "Name", 0);
        tree_add_col(&tree, "Origin", 1);
        for wu in &work.work_units {
            let path = c.full_path_for(&wu.get_metadata());
            let change = match &wu {
                WorkUnit::LocalChange { metadata: _ } => "Local",
                WorkUnit::ServerChange { metadata: _ } => "Remote",
            };
            model.insert_with_values(None, None, &[0, 1], &[&path, &change]);
        }

        let scrolled = util::gui::scrollable(&tree);
        util::gui::set_margin(&scrolled, 16);
        scrolled.set_size_request(450, 300);

        cntr.add(&desc);
        cntr.pack_start(&scrolled, true, true, 0);
    }
    Ok(cntr)
}

fn usage(usage_string: String) -> LbResult<GtkBox> {
    let title = GtkLabel::new(Some("Total Usage"));
    let attr_list = pango::AttrList::new();
    let attr = pango::Attribute::new_weight(pango::Weight::Bold)
        .ok_or_else(|| progerr!("Unable to apply bold attribute to title."))?;

    attr_list.change(attr);
    title.set_attributes(Some(&attr_list));
    title.set_margin_bottom(20);

    let lbl = GtkLabel::new(Some(&usage_string));
    lbl.set_margin_bottom(24);

    let cntr = GtkBox::new(Vertical, 0);
    util::gui::set_marginy(&cntr, 36);
    util::gui::set_marginx(&cntr, 100);
    cntr.add(&title);
    cntr.add(&lbl);
    Ok(cntr)
}

const DEFAULT_WIN_TITLE: &str = "Lockbook";
const VERSION: &str = env!("CARGO_PKG_VERSION");
const LICENSE: &str = include_str!("../res/UNLICENSE");
const COMMENTS: &str = "Lockbook is a document editor that is secure, minimal, private, open source, and cross-platform.";
const WINDOW_ICON: &[u8] = include_bytes!("../res/lockbook-window-icon-pixdata");
