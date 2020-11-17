use std::cell::RefCell;
use std::cmp::Ordering;
use std::rc::Rc;
use std::sync::Arc;

use uuid::Uuid;

use gio::prelude::*;
use gtk::prelude::*;
use gtk::Orientation::Vertical;
use gtk::{
    AboutDialog as GtkAboutDialog, AccelGroup as GtkAccelGroup, Application as GtkApp,
    ApplicationWindow as GtkAppWindow, Box as GtkBox, CheckButton as GtkCheckBox,
    Dialog as GtkDialog, Entry as GtkEntry, EntryCompletion as GtkEntryCompletion,
    Expander as GtkExpander, Label as GtkLabel, ListStore as GtkListStore, Notebook as GtkNotebook,
    ProgressBar as GtkProgressBar, ResponseType as GtkResponseType, SortColumn as GtkSortColumn,
    SortType as GtkSortType, Stack as GtkStack, TreeIter as GtkTreeIter, TreeModel as GtkTreeModel,
    TreeModelSort as GtkTreeModelSort, Widget as GtkWidget, WidgetExt as GtkWidgetExt,
    WindowPosition as GtkWindowPosition,
};

use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;

use lockbook_core::model::file_metadata::{FileMetadata, FileType};

use crate::account::AccountScreen;
use crate::backend::{LbCore, LbSyncMsg};
use crate::editmode::EditMode;
use crate::filetree::FileTreeCol;
use crate::intro::IntroScreen;
use crate::menubar::Menubar;
use crate::messages::{Messenger, Msg, MsgReceiver};
use crate::settings::Settings;
use crate::tree_iter_value;
use crate::util::{Util, KILOBYTE};

pub fn run_gtk(sr: Rc<RefCell<Settings>>, core: Arc<LbCore>) {
    let gtkapp = GtkApp::new(None, Default::default()).unwrap();

    let sr1 = sr.clone();
    gtkapp.connect_activate(move |a| {
        gtk_add_css_provider();

        let (sender, receiver) = glib::MainContext::channel::<Msg>(glib::PRIORITY_DEFAULT);
        let m = Messenger::new(sender);

        let lb = LockbookApp::new(&a, core.clone(), m, sr1.clone());
        lb.attach_events(receiver);
        lb.show();
    });

    gtkapp.connect_shutdown(move |_| match sr.borrow_mut().to_file() {
        Ok(_) => println!("bye!"),
        Err(err) => println!("error: {:?}", err),
    });

    gtkapp.run(&[]);
}

fn gtk_add_css_provider() {
    let provider = gtk::CssProvider::new();
    provider
        .load_from_data(STYLE.as_bytes())
        .expect("Failed to load CSS");

    gtk::StyleContext::add_provider_for_screen(
        &gdk::Screen::get_default().expect("Error initializing gtk css provider."),
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}

fn make_glib_chan<T, F: FnMut(T) -> glib::Continue + 'static>(func: F) -> glib::Sender<T> {
    let (s, r) = glib::MainContext::channel::<T>(glib::PRIORITY_DEFAULT);
    r.attach(None, func);
    s
}

#[derive(Clone)]
struct LockbookApp {
    settings: Rc<RefCell<Settings>>,
    state: Rc<RefCell<LbState>>,
    core: Arc<LbCore>,
    gui: Rc<Gui>,
    messenger: Messenger,
}

impl LockbookApp {
    fn new(a: &GtkApp, core: Arc<LbCore>, m: Messenger, s: Rc<RefCell<Settings>>) -> Self {
        let gui = Gui::new(&a, &m, &s.borrow());

        Self {
            settings: s,
            state: Rc::new(RefCell::new(LbState::default())),
            core,
            gui: Rc::new(gui),
            messenger: m,
        }
    }

    fn attach_events(&self, r: MsgReceiver) {
        let lb = self.clone();
        r.attach(None, move |msg| {
            match msg {
                Msg::CreateAccount(username) => lb.create_account(username),
                Msg::ImportAccount(privkey) => lb.import_account(privkey),
                Msg::ExportAccount => lb.export_account(),
                Msg::PerformSync => lb.perform_sync(),
                Msg::Quit => lb.quit(),

                Msg::NewFile(path) => lb.new_file(path),
                Msg::OpenFile(id) => lb.open_file(id),
                Msg::SaveFile => lb.save(),
                Msg::CloseFile => lb.close(),
                Msg::DeleteFile(id) => lb.delete_file(id),

                Msg::ToggleTreeCol(col) => lb.toggle_tree_col(col),

                Msg::SearchFieldFocus => lb.search_field_focus(),
                Msg::SearchFieldBlur => lb.search_field_blur(),
                Msg::SearchFieldUpdate => lb.search_field_update(),
                Msg::SearchFieldUpdateIcon => lb.search_field_update_icon(),
                Msg::SearchFieldExec(vopt) => lb.search_field_exec(vopt),

                Msg::ShowDialogNew => lb.show_dialog_new(),
                Msg::ShowDialogPreferences => lb.show_dialog_preferences(),
                Msg::ShowDialogUsage => lb.show_dialog_usage(),
                Msg::ShowDialogAbout => lb.show_dialog_about(),

                Msg::UnexpectedErr(desc, deets) => lb.show_unexpected_err(&desc, &deets),
            }
            glib::Continue(true)
        });
    }

    fn show(&self) {
        self.gui.show(&self.core, &self.messenger);
    }

    fn create_account(&self, name: String) {
        self.gui.intro.doing("Creating account...");

        let gui = self.gui.clone();
        let c = self.core.clone();

        let ch = make_glib_chan(move |result: Result<(), String>| {
            match result {
                Ok(_) => gui.show_account_screen(&c),
                Err(err) => gui.intro.error_create(&err),
            }
            glib::Continue(false)
        });

        let c = self.core.clone();
        std::thread::spawn(move || ch.send(c.create_account(&name)).unwrap());
    }

    fn import_account(&self, privkey: String) {
        self.gui.intro.doing("Importing account...");

        let gui = self.gui.clone();
        let c = self.core.clone();

        let import_chan = make_glib_chan(move |result: Result<(), String>| {
            match result {
                Ok(_) => {
                    let gui = gui.clone();
                    let cc = c.clone();

                    let sync_chan = make_glib_chan(move |msg| {
                        match msg {
                            LbSyncMsg::Doing(work) => gui.intro.doing_status(&work),
                            LbSyncMsg::Done => {
                                gui.show_account_screen(&cc);
                                gui.account.sync().set_status(&cc);
                            }
                            _ => {}
                        }
                        glib::Continue(true)
                    });

                    let c = c.clone();
                    std::thread::spawn(move || c.sync(&sync_chan));
                }
                Err(err) => gui.intro.error_import(&err),
            }
            glib::Continue(false)
        });

        let c = self.core.clone();
        std::thread::spawn(move || import_chan.send(c.import_account(&privkey)).unwrap());
    }

    fn export_account(&self) {
        match self.core.export_account() {
            Ok(privkey) => self.show_dialog_export_account(&privkey),
            Err(err) => self.show_unexpected_err("Unable to export account", &err),
        }
    }

    fn perform_sync(&self) {
        let core = self.core.clone();
        let acctscr = self.gui.account.clone();
        acctscr.sync().set_syncing(true);

        let ch = make_glib_chan(move |msg| {
            let sync_ui = acctscr.sync();
            match msg {
                LbSyncMsg::Doing(work) => sync_ui.doing(&work),
                LbSyncMsg::Error(err) => sync_ui.error(&format!("error: {}", err)),
                LbSyncMsg::Done => {
                    sync_ui.set_syncing(false);
                    sync_ui.set_status(&core);
                }
            }
            glib::Continue(true)
        });

        let c = self.core.clone();
        std::thread::spawn(move || c.sync(&ch));
    }

    fn quit(&self) {
        self.gui.win.close();
    }

    fn new_file(&self, path: String) {
        match self.core.create_file_at_path(&path) {
            Ok(file) => {
                self.gui.account.add_file(&self.core, &file);
                self.gui.account.sync().set_status(&self.core);
                self.open_file(file.id);
            }
            Err(err) => println!("error creating '{}': {}", path, err),
        }
    }

    fn open_file(&self, id: Uuid) {
        match self.core.file_by_id(id) {
            Ok(meta) => {
                self.state.borrow_mut().set_opened_file(Some(meta.clone()));
                match meta.file_type {
                    FileType::Document => self.open_document(meta.id),
                    FileType::Folder => self.open_folder(&meta),
                }
            }
            Err(err) => println!("error opening '{}': {}", id, err),
        }
    }

    fn open_document(&self, id: Uuid) {
        match self.core.open(&id) {
            Ok((meta, content)) => {
                let path = self.core.full_path_for(&meta);
                self.edit(&EditMode::PlainText {
                    path,
                    meta,
                    content,
                })
            }
            Err(err) => println!("error opening '{}': {}", id, err),
        }
    }

    fn open_folder(&self, f: &FileMetadata) {
        match self.core.children(&f) {
            Ok(children) => self.edit(&EditMode::Folder {
                path: self.core.full_path_for(&f),
                meta: f.clone(),
                n_children: children.len(),
            }),
            Err(err) => println!("error getting children for '{}': {}", f.id, err),
        }
    }

    fn edit(&self, mode: &EditMode) {
        self.gui.menubar.set(&mode);
        self.gui.account.show(&mode);
    }

    fn save(&self) {
        if let Some(f) = &self.state.borrow().get_opened_file() {
            if f.file_type == FileType::Document {
                let acctscr = self.gui.account.clone();
                acctscr.set_saving(true);

                let id = f.id;
                let content = acctscr.text_content();
                let core = self.core.clone();

                let ch = make_glib_chan(move |result: Result<(), String>| {
                    match result {
                        Ok(_) => {
                            acctscr.set_saving(false);
                            acctscr.sync().set_status(&core);
                        }
                        Err(err) => {
                            println!("error saving: {}", err);
                        }
                    }
                    glib::Continue(false)
                });

                let c = self.core.clone();
                std::thread::spawn(move || ch.send(c.save(id, content)).unwrap());
            }
        }
    }

    fn close(&self) {
        if self.state.borrow().get_opened_file().is_some() {
            self.edit(&EditMode::None);
        }
    }

    fn delete_file(&self, id: Uuid) {
        let meta = self.core.file_by_id(id).ok().unwrap();
        let path = self.core.full_path_for(&meta);
        let mut msg = format!("Are you sure you want to delete '{}'?", path);

        if meta.file_type == FileType::Folder {
            let children = self.core.get_children_recursively(meta.id).ok().unwrap();
            msg = format!("{} ({} files)", msg, children.len());
        }

        let d = self.gui.new_dialog("Confirm Delete");
        d.get_content_area().add(&GtkLabel::new(Some(&msg)));
        d.get_content_area().show_all();
        d.add_button("Cancel", GtkResponseType::Cancel);
        d.add_button("Delete", GtkResponseType::Yes);

        if d.run() == GtkResponseType::Yes {
            match self.core.delete(id) {
                Ok(_) => self.gui.account.tree().remove(&meta.id),
                Err(err) => println!("{}", err),
            }
        }

        d.close();
    }

    fn toggle_tree_col(&self, c: FileTreeCol) {
        self.gui.account.tree().toggle_col(&c);
        self.settings.borrow_mut().toggle_tree_col(c.name());
    }

    fn show_dialog_new(&self) {
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
    }

    fn search_field_focus(&self) {
        let search = SearchComponents::new(&self.core);

        let comp = GtkEntryCompletion::new();
        comp.set_model(Some(&search.sort_model));
        comp.set_popup_completion(true);
        comp.set_inline_selection(true);
        comp.set_text_column(1);
        comp.set_match_func(|_, _, _| true);

        let m = self.messenger.clone();
        comp.connect_match_selected(move |_, model, iter| {
            let iter_val = tree_iter_value!(model, iter, 1, String);
            m.send(Msg::SearchFieldExec(Some(iter_val)));
            gtk::Inhibit(false)
        });

        self.gui.account.set_search_field_completion(&comp);
        self.state.borrow_mut().set_search_components(search);
    }

    fn search_field_update(&self) {
        if let Some(search) = self.state.borrow().search_ref() {
            let input = self.gui.account.get_search_field_text();
            search.update_for(&input);
        }
    }

    fn search_field_update_icon(&self) {
        let input = self.gui.account.get_search_field_text();
        let icon_name = if input.ends_with(".md") || input.ends_with(".txt") {
            "text-x-generic-symbolic"
        } else if input.ends_with('/') {
            "folder-symbolic"
        } else {
            "edit-find-symbolic"
        };
        self.gui.account.set_search_field_icon(icon_name, None);
    }

    fn search_field_blur(&self) {
        let path = match self.state.borrow().get_opened_file() {
            Some(meta) => self.core.full_path_for(meta),
            None => "".to_string(),
        };
        self.gui.account.set_search_field_text(&path);
        self.gui.account.deselect_search_field();
        self.gui.account.tree().focus();
    }

    fn search_field_exec(&self, explicit: Option<String>) {
        let entry_text = self.gui.account.get_search_field_text();
        let best_match = self.state.borrow().get_first_search_match();
        let path = explicit.unwrap_or_else(|| best_match.unwrap_or(entry_text));

        match self.core.file_by_path(&path) {
            Ok(meta) => self.messenger.send(Msg::OpenFile(meta.id)),
            Err(_) => self.gui.account.set_search_field_icon(
                "dialog-error-symbolic",
                Some(&format!("The file '{}' does not exist", path)),
            ),
        }
    }

    fn show_dialog_preferences(&self) {
        let tabs = GuiUtil::settings(&self.settings, &self.messenger);

        let d = self.gui.new_dialog("Lockbook Settings");
        d.set_default_size(300, 400);
        d.get_content_area().add(&tabs);
        d.add_button("Ok", GtkResponseType::Ok);
        d.connect_response(|d, resp| {
            if let GtkResponseType::Ok = resp {
                d.close();
            }
        });
        d.show_all();
    }

    fn show_dialog_about(&self) {
        let logo = gdk_pixbuf::Pixbuf::from_file("./lockbook-intro.png").unwrap();

        let d = GtkAboutDialog::new();
        d.set_transient_for(Some(&self.gui.win));
        d.set_logo(Some(&logo));
        d.set_program_name("Lockbook");
        d.set_version(Some(VERSION));
        d.set_website(Some("https://lockbook.app"));
        d.set_authors(&["The Lockbook Team"]);
        d.set_license(Some(LICENSE));
        d.set_comments(Some(COMMENTS));
        d.connect_response(|d, resp| {
            if let GtkResponseType::DeleteEvent = resp {
                d.close();
            }
        });
        d.show_all();
    }

    fn show_dialog_usage(&self) {
        match self.core.usage() {
            Ok(n_bytes) => {
                let usage = GuiUtil::usage(n_bytes);
                let d = self.gui.new_dialog("My Lockbook Usage");
                d.get_content_area().add(&usage);
                d.show_all();
            }
            Err(err) => self.show_unexpected_err("Unable to get usage", &err),
        };
    }

    fn show_dialog_export_account(&self, privkey: &str) {
        let bx = GtkBox::new(Vertical, 0);
        bx.add(&GuiUtil::selectable_label(&privkey));
        bx.add(&GtkLabel::new(Some("(Click the key above to highlight)")));

        let d = self.gui.new_dialog("My Lockbook Private Key");
        d.get_content_area().add(&bx);
        d.set_resizable(false);
        d.show_all();
    }

    fn show_unexpected_err(&self, desc: &str, deets: &str) {
        let lbl = GtkLabel::new(Some(&format!("ERROR: {}", desc)));
        GtkWidgetExt::set_widget_name(&lbl, "unexpected_err_lbl");

        let details = GuiUtil::selectable_label(&deets);
        details.set_margin_top(16);

        let content = GtkExpander::new(None);
        content.set_label_widget(Some(&lbl));
        content.set_expanded(true);
        content.add(&details);

        let d = self.gui.new_dialog("Lockbook Error");
        d.get_content_area().add(&content);
        d.show_all();
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
        let root = core.account().unwrap().unwrap().username;
        let root_len = root.len();
        let mut possibs = Vec::new();
        for mut p in core.list_paths().ok().unwrap() {
            if p.starts_with(&root) {
                p.replace_range(..root_len, "");
            }
            possibs.push(p);
        }

        let list_store = GtkListStore::new(&[glib::Type::I64, glib::Type::String]);
        let sort_model = GtkTreeModelSort::new(&list_store);
        sort_model.set_sort_column_id(GtkSortColumn::Index(0), GtkSortType::Descending);
        sort_model.set_sort_func(GtkSortColumn::Index(0), Self::compare_possibs);

        Self {
            possibs,
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
                if text2 == "" {
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

struct LbState {
    search: Option<SearchComponents>,
    opened_file: Option<FileMetadata>,
}

impl LbState {
    fn default() -> Self {
        Self {
            search: None,
            opened_file: None,
        }
    }

    fn set_search_components(&mut self, search: SearchComponents) {
        self.search = Some(search);
    }

    fn search_ref(&self) -> Option<&SearchComponents> {
        self.search.as_ref()
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

    fn get_opened_file(&self) -> Option<&FileMetadata> {
        match &self.opened_file {
            Some(f) => Some(f),
            None => None,
        }
    }

    fn set_opened_file(&mut self, f: Option<FileMetadata>) {
        self.opened_file = f;
        if self.opened_file.is_some() {
            self.search = None;
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

        // Window.
        let w = GtkAppWindow::new(app);
        w.set_title("Lockbook");
        w.add_accel_group(&accels);
        w.set_default_size(1300, 700);
        if s.window_maximize {
            w.maximize();
        }
        w.add(&{
            let base = GtkBox::new(Vertical, 0);
            base.add(&menubar.cntr);
            base.pack_start(&screens, true, true, 0);
            base
        });

        Self {
            win: w,
            menubar,
            screens,
            intro,
            account: Rc::new(account),
        }
    }

    fn show(&self, core: &LbCore, m: &Messenger) {
        self.win.show_all();
        match core.account() {
            Ok(acct) => match acct {
                Some(_) => self.show_account_screen(&core),
                None => self.show_intro_screen(),
            },
            Err(err) => m.send(Msg::UnexpectedErr(
                "Unable to load account".to_string(),
                err,
            )),
        }
    }

    fn show_intro_screen(&self) {
        self.menubar.for_intro_screen();
        self.intro.cntr.show_all();
        self.screens.set_visible_child_name("intro");
    }

    fn show_account_screen(&self, core: &LbCore) {
        self.menubar.for_account_screen();
        self.account.cntr.show_all();
        self.account.fill(&core);
        self.account.tree().focus();
        self.screens.set_visible_child_name("account");
    }

    fn new_dialog(&self, title: &str) -> GtkDialog {
        let d = GtkDialog::new();
        d.set_transient_for(Some(&self.win));
        d.set_position(GtkWindowPosition::CenterOnParent);
        d.set_title(&title);
        d
    }
}

struct GuiUtil;
impl GuiUtil {
    fn settings(s: &Rc<RefCell<Settings>>, m: &Messenger) -> GtkNotebook {
        let tabs = GtkNotebook::new();
        for tab_data in vec![
            ("File Tree", settings_filetree(&s, &m)),
            ("Window", settings_window(&s)),
        ] {
            let (title, content) = tab_data;
            let tab_btn = GtkLabel::new(Some(title));
            let tab_page = content.upcast::<GtkWidget>();
            tabs.append_page(&tab_page, Some(&tab_btn));
        }
        tabs
    }

    fn selectable_label(txt: &str) -> GtkLabel {
        let lbl = GtkLabel::new(Some(txt));
        lbl.set_selectable(true);
        lbl.set_max_width_chars(80);
        lbl.set_line_wrap(true);
        lbl.set_line_wrap_mode(pango::WrapMode::Char);
        lbl.connect_button_release_event(|lbl, _| {
            lbl.select_region(0, -1);
            gtk::Inhibit(false)
        });
        lbl
    }

    fn usage(usage: u64) -> GtkBox {
        let limit = KILOBYTE as f64 * 20.0;

        let pbar = GtkProgressBar::new();
        pbar.set_size_request(300, -1);
        pbar.set_margin_start(16);
        pbar.set_margin_end(16);
        pbar.set_fraction(usage as f64 / limit);

        let human_limit = Util::human_readable_bytes(limit as u64);
        let human_usage = Util::human_readable_bytes(usage);

        let lbl = GtkLabel::new(Some(&format!("{} / {}", human_usage, human_limit)));
        lbl.set_margin_bottom(24);

        let cntr = GtkBox::new(Vertical, 0);
        cntr.set_margin_top(32);
        cntr.set_margin_bottom(36);
        cntr.add(&lbl);
        cntr.add(&pbar);
        cntr
    }
}

fn settings_filetree(s: &Rc<RefCell<Settings>>, m: &Messenger) -> GtkBox {
    let s = s.borrow();
    let chbxs = GtkBox::new(Vertical, 0);

    for col in FileTreeCol::removable() {
        let m = m.clone();

        let ch = GtkCheckBox::with_label(&col.name());
        ch.set_active(!s.hidden_tree_cols.contains(&col.name()));
        ch.connect_toggled(move |_| m.send(Msg::ToggleTreeCol(col)));
        chbxs.add(&ch);
    }

    chbxs
}

fn settings_window(s: &Rc<RefCell<Settings>>) -> GtkBox {
    let s = s.clone();

    let ch = GtkCheckBox::with_label("Maximize on startup");
    ch.set_active(s.borrow().window_maximize);
    ch.connect_toggled(move |chbox| {
        s.borrow_mut().window_maximize = chbox.get_active();
    });

    let chbxs = GtkBox::new(Vertical, 0);
    chbxs.add(&ch);
    chbxs
}

const STYLE: &str = "
#intro_heading {
    font-size: 64px;
    opacity: 0.75;
}
#intro_hr {
    background: rgba(100, 100, 100, 0.35);
}
#unexpected_err_lbl,
#intro_error {
    color: red;
}
#unexpected_err_lbl {
    font-size: 14px;
}
#intro_error {
    font-weight: bold;
}
#intro_doing_caption {
    font-size: 20px;
}
";

const VERSION: &str = env!("CARGO_PKG_VERSION");

const COMMENTS: &str = "
Lockbook is a document editor that is secure, minimal, private, open source, and cross-platform.
";

const LICENSE: &str = "
This is free and unencumbered software released into the public domain.

Anyone is free to copy, modify, publish, use, compile, sell, or
distribute this software, either in source code form or as a compiled
binary, for any purpose, commercial or non-commercial, and by any
means.

In jurisdictions that recognize copyright laws, the author or authors
of this software dedicate any and all copyright interest in the
software to the public domain. We make this dedication for the benefit
of the public at large and to the detriment of our heirs and
successors. We intend this dedication to be an overt act of
relinquishment in perpetuity of all present and future rights to this
software under copyright law.

THE SOFTWARE IS PROVIDED \"AS IS\", WITHOUT WARRANTY OF ANY KIND,
EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.
IN NO EVENT SHALL THE AUTHORS BE LIABLE FOR ANY CLAIM, DAMAGES OR
OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE,
ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR
OTHER DEALINGS IN THE SOFTWARE.

For more information, please refer to <http://unlicense.org/>";
