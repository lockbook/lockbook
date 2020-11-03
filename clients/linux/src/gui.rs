use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use uuid::Uuid;

use gio::prelude::*;
use glib::clone;
use gtk::prelude::*;
use gtk::Orientation::Vertical;
use gtk::{
    AboutDialog as GtkAboutDialog, AccelGroup as GtkAccelGroup, Application as GtkApp,
    ApplicationWindow as GtkAppWindow, Box as GtkBox, CheckButton as GtkCheckBox,
    Dialog as GtkDialog, Entry as GtkEntry, EntryCompletion as GtkEntryCompletion,
    Expander as GtkExpander, Label as GtkLabel, ListStore as GtkListStore, Notebook as GtkNotebook,
    ResponseType as GtkResponseType, Widget as GtkWidget, WidgetExt as GtkWidgetExt,
    WindowPosition as GtkWindowPosition,
};

use lockbook_core::model::file_metadata::{FileMetadata, FileType};

use crate::backend::{LbCore, LbSyncMsg};
use crate::editmode::EditMode;
use crate::filetree::FileTreeCol;
use crate::menubar::Menubar;
use crate::messages::{Messenger, Msg, MsgReceiver};
use crate::screens::Screens;
use crate::settings::Settings;
use crate::tree_iter_value;

macro_rules! widgetize {
    ($w:expr) => {
        $w.upcast::<GtkWidget>()
    };
}

macro_rules! lblopt {
    ($txt:expr) => {
        Some(&GtkLabel::new(Some($txt)))
    };
}

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
    model: Rc<RefCell<Model>>,
    core: Arc<LbCore>,
    gui: Rc<Gui>,
    messenger: Messenger,
}

impl LockbookApp {
    fn new(a: &GtkApp, core: Arc<LbCore>, m: Messenger, s: Rc<RefCell<Settings>>) -> Self {
        let gui = Gui::new(&a, &m, &s.borrow());

        Self {
            core,
            gui: Rc::new(gui),
            model: Rc::new(RefCell::new(Model::default())),
            messenger: m,
            settings: s,
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

                Msg::ToggleTreeCol(col) => lb.toggle_tree_col(col),

                Msg::ShowDialogNew => lb.show_dialog_new(),
                Msg::ShowDialogOpen => lb.show_dialog_open(),
                Msg::ShowDialogPreferences => lb.show_dialog_preferences(),
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
        self.gui.screens.intro.doing("Creating account...");

        let gui = self.gui.clone();
        let c = self.core.clone();

        let ch = make_glib_chan(move |result: Result<(), String>| {
            match result {
                Ok(_) => gui.screens.show_account(&c),
                Err(err) => gui.screens.intro.error_create(&err),
            }
            glib::Continue(false)
        });

        let c = self.core.clone();
        std::thread::spawn(move || ch.send(c.create_account(&name)).unwrap());
    }

    fn import_account(&self, privkey: String) {
        self.gui.screens.intro.doing("Importing account...");

        let gui = self.gui.clone();
        let c = self.core.clone();

        let import_chan = make_glib_chan(move |result: Result<(), String>| {
            match result {
                Ok(_) => {
                    let gui = gui.clone();
                    let cc = c.clone();

                    let sync_chan = make_glib_chan(move |msg| {
                        match msg {
                            LbSyncMsg::Doing(work) => gui.screens.intro.doing_status(&work),
                            LbSyncMsg::Done => {
                                gui.screens.show_account(&cc);
                                gui.screens.account.set_sync_status(&cc);
                            }
                            _ => {}
                        }
                        glib::Continue(true)
                    });

                    let c = c.clone();
                    std::thread::spawn(move || c.sync(&sync_chan));
                }
                Err(err) => gui.screens.intro.error_import(&err),
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
        let acctscr = self.gui.screens.account.clone();
        acctscr.set_syncing(true);

        let core = self.core.clone();

        let ch = make_glib_chan(move |msg| {
            match msg {
                LbSyncMsg::Doing(work) => acctscr.sidebar.sync.doing(&work),
                LbSyncMsg::Error(err) => acctscr.sidebar.sync.error(&format!("error: {}", err)),
                LbSyncMsg::Done => {
                    acctscr.set_syncing(false);
                    acctscr.set_sync_status(&core);
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
                self.gui.screens.account.add_file(&self.core, &file);
                self.gui.screens.account.set_sync_status(&self.core);
                self.open_file(file.id);
            }
            Err(err) => println!("error creating '{}': {}", path, err),
        }
    }

    fn open_file(&self, id: Uuid) {
        match self.core.file_by_id(id) {
            Ok(meta) => {
                self.model.borrow_mut().set_opened_file(Some(meta.clone()));
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
            Ok((meta, content)) => self.edit(&EditMode::PlainText { meta, content }),
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
        self.gui.screens.account.show(&mode);
    }

    fn save(&self) {
        if let Some(f) = &self.model.borrow().get_opened_file() {
            if f.file_type == FileType::Document {
                let acctscr = self.gui.screens.account.clone();
                acctscr.set_saving(true);

                let id = f.id;
                let content = acctscr.text_content();
                let core = self.core.clone();

                let ch = make_glib_chan(move |result: Result<(), String>| {
                    match result {
                        Ok(_) => {
                            acctscr.set_saving(false);
                            acctscr.set_sync_status(&core);
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
        if self.model.borrow().get_opened_file().is_some() {
            self.edit(&EditMode::None);
        }
    }

    fn toggle_tree_col(&self, c: FileTreeCol) {
        self.gui.screens.account.sidebar.tree.toggle_col(&c);
        self.settings.borrow_mut().toggle_tree_col(c.name());
    }

    fn show_dialog_new(&self) {
        let d = self.gui.new_dialog("New...");

        let path_entry = GtkEntry::new();
        path_entry.connect_activate(clone!(@strong d => move |_| {
            d.response(GtkResponseType::Ok);
        }));

        d.get_content_area().add(&path_entry);
        d.add_button("Ok", GtkResponseType::Ok);
        d.connect_response(
            clone!(@strong self.messenger as m, @strong path_entry => move |d, resp| {
                if let GtkResponseType::Ok = resp {
                    let path = path_entry.get_buffer().get_text();
                    m.send(Msg::NewFile(path));
                    d.close();
                }
            }),
        );
        d.show_all();
    }

    fn show_dialog_open(&self) {
        let d = self.gui.new_dialog("Open");
        let entry = GtkEntry::new();

        let completion = GtkEntryCompletion::new();
        completion.set_model(Some(&list_model_for_open(&self.core)));
        completion.set_text_column(0);
        completion.set_popup_completion(true);
        completion.set_match_func(|this, val, iter| {
            let iter_val = tree_iter_value!(this.get_model().unwrap(), iter, 0, String);
            iter_val.contains(&val)
        });
        completion.connect_match_selected(
            clone!(@strong d, @strong entry => move |_, model, iter| {
                let iter_val = tree_iter_value!(model, iter, 0, String);
                entry.set_text(&iter_val);
                d.response(GtkResponseType::Ok);
                gtk::Inhibit(false)
            }),
        );

        entry.set_completion(Some(&completion));
        entry.connect_activate(clone!(@strong d => move |_| {
            d.response(GtkResponseType::Ok);
        }));

        let core = self.core.clone();
        let m = self.messenger.clone();
        d.connect_response(clone!(@strong entry => move |d, resp| {
            if let GtkResponseType::Ok = resp {
                let path = entry.get_buffer().get_text();
                match core.file_by_path(&path) {
                    Ok(meta) => m.send(Msg::OpenFile(meta.id)),
                    Err(err) => println!("{}", err),
                }
                d.close();
            }
        }));
        d.get_content_area().add(&entry);
        d.show_all();
    }

    fn show_dialog_preferences(&self) {
        let tabs = GuiUtil::settings(&self.settings, &self.messenger);

        let d = self.gui.new_dialog("Lockbook Settings");
        d.set_default_size(300, 400);
        d.get_content_area().add(&tabs);
        d.add_button("Ok", GtkResponseType::Ok);
        d.connect_response(move |d, resp| {
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
        d.connect_response(move |d, resp| {
            if let GtkResponseType::DeleteEvent = resp {
                d.close();
            }
        });
        d.show_all();
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

struct Model {
    opened_file: Option<FileMetadata>,
}

impl Model {
    fn default() -> Self {
        Self { opened_file: None }
    }

    fn get_opened_file(&self) -> Option<&FileMetadata> {
        match &self.opened_file {
            Some(f) => Some(f),
            None => None,
        }
    }

    fn set_opened_file(&mut self, f: Option<FileMetadata>) {
        self.opened_file = f;
    }
}

struct Gui {
    win: GtkAppWindow,
    screens: Screens,
    menubar: Menubar,
}

impl Gui {
    fn new(app: &GtkApp, m: &Messenger, s: &Settings) -> Self {
        let accels = GtkAccelGroup::new();
        let screens = Screens::new(m, &s);

        let menubar = Menubar::new(m, &accels);
        menubar.set(&EditMode::None);

        let w = GtkAppWindow::new(app);
        w.set_title("Lockbook");
        w.set_default_size(1300, 700);
        w.add_accel_group(&accels);
        if s.window_maximize {
            w.maximize();
        }
        w.add(&{
            let base = GtkBox::new(Vertical, 0);
            base.add(&menubar.cntr);
            base.pack_start(&screens.cntr, true, true, 0);
            base
        });

        Self {
            win: w,
            screens: screens,
            menubar: menubar,
        }
    }

    fn show(&self, core: &LbCore, m: &Messenger) {
        self.win.show_all();
        self.screens.init(&core, &m);
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
        for tab in vec![
            ("File Tree", settings_filetree(&s, &m)),
            ("Window", settings_window(&s)),
        ] {
            let (name, page) = tab;
            tabs.append_page(&page, lblopt!(name));
        }
        tabs
    }

    fn selectable_label(txt: &str) -> GtkLabel {
        let lbl = GtkLabel::new(Some(txt));
        lbl.set_selectable(true);
        lbl.set_max_width_chars(80);
        lbl.set_line_wrap(true);
        lbl.set_line_wrap_mode(pango::WrapMode::Char);
        lbl.connect_button_release_event(|this, _| {
            this.select_region(0, -1);
            gtk::Inhibit(false)
        });
        lbl
    }
}

fn settings_filetree(s: &Rc<RefCell<Settings>>, m: &Messenger) -> GtkWidget {
    let s = s.borrow();
    let chbxs = GtkBox::new(Vertical, 0);

    for col in FileTreeCol::removable() {
        let ch = GtkCheckBox::with_label(&col.name());
        ch.set_active(!s.hidden_tree_cols.contains(&col.name()));
        ch.connect_toggled(clone!(@strong m => move |_| {
            m.send(Msg::ToggleTreeCol(col));
        }));
        chbxs.add(&ch);
    }

    widgetize!(chbxs)
}

fn settings_window(s: &Rc<RefCell<Settings>>) -> GtkWidget {
    let s = s.clone();

    let ch = GtkCheckBox::with_label("Maximize on startup");
    ch.set_active(s.borrow().window_maximize);
    ch.connect_toggled(move |this| {
        s.borrow_mut().window_maximize = this.get_active();
    });

    let chbxs = GtkBox::new(Vertical, 0);
    chbxs.add(&ch);
    widgetize!(chbxs)
}

fn list_model_for_open(b: &LbCore) -> GtkListStore {
    let paths = b.list_paths().ok().unwrap();

    let store = GtkListStore::new(&[glib::Type::String]);
    for mut p in paths {
        let uname = b.account().unwrap().unwrap().username;
        if p.contains(&uname) {
            p.replace_range(..uname.len(), "");
        }

        let values: [&dyn ToValue; 1] = [&p];
        store.set(&store.append(), &[0], &values);
    }
    store
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
