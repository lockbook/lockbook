use std::rc::Rc;

use gdk_pixbuf::Pixbuf as GdkPixbuf;
use gtk::prelude::*;
use gtk::Orientation::{Horizontal, Vertical};
use gtk::{
    Adjustment as GtkAdjustment, Align as GtkAlign, Box as GtkBox, Button as GtkBtn,
    Entry as GtkEntry, EntryCompletion as GtkEntryCompletion,
    EntryIconPosition as GtkEntryIconPosition, Grid as GtkGrid, Image as GtkImage,
    Label as GtkLabel, Menu as GtkMenu, MenuItem as GtkMenuItem, Paned as GtkPaned,
    ScrolledWindow as GtkScrolledWindow, Separator as GtkSeparator, Spinner as GtkSpinner,
    Stack as GtkStack, WrapMode as GtkWrapMode,
};
use sourceview::prelude::*;
use sourceview::View as GtkSourceView;
use sourceview::{Buffer as GtkSourceViewBuffer, LanguageManager};

use lockbook_models::file_metadata::FileMetadata;
use lockbook_models::work_unit::WorkUnit;

use crate::backend::{LbCore, LbSyncMsg};
use crate::closure;
use crate::editmode::EditMode;
use crate::error::LbResult;
use crate::filetree::FileTree;
use crate::messages::{Messenger, Msg, MsgFn};
use crate::settings::Settings;
use crate::util::{gui as gui_util, gui::RIGHT_CLICK};

pub struct AccountScreen {
    header: Header,
    sidebar: Sidebar,
    editor: Editor,
    pub cntr: GtkBox,
}

impl AccountScreen {
    pub fn new(m: &Messenger, s: &Settings) -> Self {
        let header = Header::new(&m);
        let sidebar = Sidebar::new(&m, &s);
        let editor = Editor::new();

        let paned = GtkPaned::new(Horizontal);
        paned.set_position(350);
        paned.add1(&sidebar.cntr);
        paned.add2(&editor.cntr);

        let cntr = GtkBox::new(Vertical, 0);
        cntr.add(&header.cntr);
        cntr.add(&GtkSeparator::new(Horizontal));
        cntr.pack_start(&paned, true, true, 0);

        Self {
            header,
            sidebar,
            editor,
            cntr,
        }
    }

    pub fn fill(&self, core: &LbCore) -> LbResult<()> {
        self.sidebar.fill(&core)?;
        self.sidebar.sync.set_status(&core.sync_status()?);
        Ok(())
    }

    pub fn add_file(&self, b: &LbCore, f: &FileMetadata) -> LbResult<()> {
        self.sidebar.tree.add(b, f)
    }

    pub fn show(&self, mode: &EditMode) {
        match mode {
            EditMode::PlainText {
                path,
                meta,
                content,
            } => {
                self.header.set_file(&path);
                self.sidebar.tree.select(&meta.id);
                self.editor.set_file(&meta.name, &content);
            }
            EditMode::Folder {
                path,
                meta,
                n_children,
            } => {
                self.header.set_file(path);
                self.sidebar.tree.focus();
                self.editor.show_folder_info(&meta, *n_children);
            }
            EditMode::None => {
                self.header.set_file("");
                self.editor.show("empty");
            }
        }
    }

    pub fn text_content(&self) -> String {
        let buf = self.editor.textarea.get_buffer().unwrap();
        let start = buf.get_start_iter();
        let end = buf.get_end_iter();
        buf.get_text(&start, &end, true).unwrap().to_string()
    }

    pub fn set_saving(&self, is_saving: bool) {
        if is_saving {
            self.header.show_spinner();
        } else {
            self.header.hide_spinner();
        }
    }

    pub fn sync(&self) -> &Rc<SyncPanel> {
        &self.sidebar.sync
    }

    pub fn get_search_field_text(&self) -> String {
        self.header.search.get_text().to_string()
    }

    pub fn set_search_field_text(&self, txt: &str) {
        self.header.search.set_text(txt);
    }

    pub fn set_search_field_icon(&self, icon_name: &str, tooltip: Option<&str>) {
        entry_set_primary_icon(&self.header.search, icon_name);
        entry_set_primary_icon_tooltip(&self.header.search, tooltip);
    }

    pub fn set_search_field_completion(&self, comp: &GtkEntryCompletion) {
        self.header.search.set_completion(Some(comp));
        self.header.search.grab_focus();
    }

    pub fn deselect_search_field(&self) {
        self.header.search.select_region(0, 0);
    }

    pub fn focus_editor(&self) {
        self.editor.textarea.grab_focus();
    }

    pub fn tree(&self) -> &FileTree {
        &self.sidebar.tree
    }
}

struct Header {
    search: GtkEntry,
    spinner: GtkSpinner,
    cntr: GtkBox,
}

impl Header {
    fn new(m: &Messenger) -> Self {
        let search = Self::new_search_field(&m);

        let spinner = GtkSpinner::new();
        spinner.set_margin_start(6);
        spinner.set_margin_end(3);

        let cntr = GtkBox::new(Horizontal, 0);
        cntr.set_margin_top(6);
        cntr.set_margin_bottom(6);
        cntr.set_margin_start(3);
        cntr.set_margin_end(3);
        cntr.pack_start(&search, true, true, 0);

        Self {
            search,
            spinner,
            cntr,
        }
    }

    fn new_search_field(m: &Messenger) -> GtkEntry {
        let search = GtkEntry::new();
        entry_set_primary_icon(&search, "edit-find-symbolic");
        search.set_placeholder_text(Some("Enter a file location..."));

        search.connect_focus_out_event(closure!(m => move |_, _| {
            m.send(Msg::SearchFieldBlur(false));
            gtk::Inhibit(false)
        }));

        search.connect_key_press_event(closure!(m => move |_, key| {
            if key.get_hardware_keycode() == ESC {
                m.send(Msg::SearchFieldBlur(true));
            }
            gtk::Inhibit(false)
        }));

        search.connect_key_release_event(closure!(m => move |_, key| {
            let k = key.get_hardware_keycode();
            if k != ARROW_UP && k != ARROW_DOWN {
                m.send(Msg::SearchFieldUpdate);
            }
            gtk::Inhibit(false)
        }));

        search.connect_changed(closure!(m => move |_| m.send(Msg::SearchFieldUpdateIcon)));
        search.connect_activate(closure!(m => move |_| m.send(Msg::SearchFieldExec(None))));
        search
    }

    fn set_file(&self, path: &str) {
        self.search.set_text(path);
    }

    fn show_spinner(&self) {
        self.cntr.pack_end(&self.spinner, false, false, 0);
        self.cntr.show_all();
        self.spinner.start();
    }

    fn hide_spinner(&self) {
        self.spinner.stop();
        self.cntr.remove(&self.spinner);
    }
}

pub struct Sidebar {
    tree: FileTree,
    sync: Rc<SyncPanel>,
    cntr: GtkBox,
}

impl Sidebar {
    fn new(m: &Messenger, s: &Settings) -> Self {
        let tree = FileTree::new(&m, &s.hidden_tree_cols);
        let sync = Rc::new(SyncPanel::new(&m));

        let cntr = GtkBox::new(Vertical, 0);
        cntr.pack_start(tree.widget(), true, true, 0);
        cntr.add(&GtkSeparator::new(Horizontal));
        cntr.add(&sync.cntr);

        Self { tree, sync, cntr }
    }

    fn fill(&self, core: &LbCore) -> LbResult<()> {
        self.tree.fill(core)
    }
}

pub struct SyncPanel {
    status: GtkLabel,
    button: GtkBtn,
    spinner: GtkSpinner,
    cntr: GtkBox,
}

impl SyncPanel {
    fn new(m: &Messenger) -> Self {
        let status = GtkLabel::new(None);
        status.set_halign(GtkAlign::Start);

        let status_evbox = gtk::EventBox::new();
        status_evbox.add(&status);
        status_evbox.connect_button_press_event(closure!(m => move |_, evt| {
            if evt.get_button() == RIGHT_CLICK {
                let menu = GtkMenu::new();
                let item_data: Vec<(&str, MsgFn)> = vec![
                    ("Refresh", || Msg::RefreshSyncStatus),
                    ("Details", || Msg::ShowDialogSyncDetails),
                ];
                for (name, msg) in item_data {
                    let mi = GtkMenuItem::with_label(name);
                    mi.connect_activate(closure!(m => move |_| m.send(msg())));
                    menu.append(&mi);
                }
                menu.show_all();
                menu.popup_at_pointer(Some(evt));
            }
            gtk::Inhibit(false)
        }));

        let button = GtkBtn::with_label("Sync");
        button.connect_clicked(closure!(m => move |_| m.send(Msg::PerformSync)));

        let spinner = GtkSpinner::new();
        spinner.set_margin_top(4);
        spinner.set_margin_bottom(4);
        spinner.set_size_request(20, 20);

        let cntr = GtkBox::new(Horizontal, 0);
        gui_util::set_margin(&cntr, 8);
        cntr.pack_start(&status_evbox, false, false, 0);
        cntr.pack_end(&button, false, false, 0);

        Self {
            status,
            button,
            spinner,
            cntr,
        }
    }

    pub fn set_syncing(&self, is_syncing: bool) {
        if is_syncing {
            self.cntr.remove(&self.button);
            self.cntr.pack_end(&self.spinner, false, false, 0);
            self.spinner.show();
            self.spinner.start();
        } else {
            self.cntr.remove(&self.spinner);
            self.cntr.pack_end(&self.button, false, false, 0);
            self.spinner.stop();
            self.status.set_text("");
        }
    }

    pub fn set_status(&self, txt: &str) {
        self.status.set_markup(&txt);
    }

    pub fn sync_progress(&self, s: &LbSyncMsg) {
        let prefix = match s.work {
            WorkUnit::LocalChange { metadata: _ } => "Pushing",
            WorkUnit::ServerChange { metadata: _ } => "Pulling",
        };
        self.set_status(&format!("{}: {} ({}/{})", prefix, s.path, s.index, s.total));
    }
}

struct Editor {
    info: GtkBox,
    textarea: GtkSourceView,
    highlighter: LanguageManager,
    stack: GtkStack,
    cntr: GtkScrolledWindow,
}

impl Editor {
    fn new() -> Self {
        let empty = GtkBox::new(Vertical, 0);
        empty.set_valign(GtkAlign::Center);
        empty.add(&GtkImage::from_pixbuf(Some(
            &GdkPixbuf::from_inline(LOGO, false).unwrap(),
        )));

        let info = GtkBox::new(Vertical, 0);
        info.set_vexpand(false);
        info.set_valign(GtkAlign::Center);

        let textarea = GtkSourceView::new();
        textarea.set_property_monospace(true);
        textarea.set_wrap_mode(GtkWrapMode::Word);
        textarea.set_left_margin(4);
        textarea.set_tab_width(4);

        let stack = GtkStack::new();
        stack.add_named(&empty, "empty");
        stack.add_named(&info, "folderinfo");
        stack.add_named(&textarea, "textarea");

        let cntr = GtkScrolledWindow::new(None::<&GtkAdjustment>, None::<&GtkAdjustment>);
        cntr.add(&stack);

        Self {
            info,
            textarea,
            highlighter: LanguageManager::new(),
            stack,
            cntr,
        }
    }

    fn set_file(&self, name: &str, content: &str) {
        let tvb = self.textarea.get_buffer().unwrap();
        let svb = tvb.downcast::<GtkSourceViewBuffer>().unwrap();
        svb.begin_not_undoable_action();
        svb.set_text(content);
        svb.set_language(self.highlighter.guess_language(Some(name), None).as_ref());
        svb.end_not_undoable_action();

        self.show("textarea");
        self.textarea.grab_focus();
    }

    fn show_folder_info(&self, f: &FileMetadata, n_children: usize) {
        let name = GtkLabel::new(None);
        name.set_markup(&format!("<span><big>{}/</big></span>", f.name));
        name.set_margin_end(64);
        name.set_margin_bottom(16);

        let grid = GtkGrid::new();
        grid.set_halign(GtkAlign::Center);

        let rows = vec![
            ("ID", f.id.to_string()),
            ("Owner", f.owner.clone()),
            ("Children", n_children.to_string()),
        ];
        for (row, (key, val)) in rows.into_iter().enumerate() {
            grid.attach(&gui_util::text_right(key), 0, row as i32, 1, 1);
            grid.attach(&gui_util::text_left(&val), 1, row as i32, 1, 1);
        }

        self.info.foreach(|w| self.info.remove(w));
        self.info.add(&name);
        self.info.add(&grid);
        self.info.show_all();
        self.show("folderinfo");
    }

    fn show(&self, name: &str) {
        self.stack.set_visible_child_name(name);
    }
}

fn entry_set_primary_icon(entry: &GtkEntry, name: &str) {
    entry.set_icon_from_icon_name(GtkEntryIconPosition::Primary, Some(name));
}

fn entry_set_primary_icon_tooltip(entry: &GtkEntry, tooltip: Option<&str>) {
    entry.set_icon_tooltip_text(GtkEntryIconPosition::Primary, tooltip);
}

const LOGO: &[u8] = include_bytes!("../res/lockbook-pixdata");

const ESC: u16 = 9;
const ARROW_UP: u16 = 111;
const ARROW_DOWN: u16 = 116;
