use std::cell::RefCell;
use std::rc::Rc;

use gdk_pixbuf::Pixbuf as GdkPixbuf;
use glib::SignalHandlerId;
use gtk::prelude::*;
use gtk::Orientation::{Horizontal, Vertical};
use gtk::{
    Adjustment as GtkAdjustment, Align as GtkAlign, Box as GtkBox, Button as GtkBtn,
    Entry as GtkEntry, EntryCompletion as GtkEntryCompletion,
    EntryIconPosition as GtkEntryIconPosition, Grid as GtkGrid, Image as GtkImage,
    Label as GtkLabel, Menu as GtkMenu, MenuItem as GtkMenuItem, Paned as GtkPaned,
    ProgressBar as GtkProgressBar, ScrolledWindow as GtkScrolledWindow, Separator as GtkSeparator,
    Spinner as GtkSpinner, Stack as GtkStack, TextWindowType, WrapMode as GtkWrapMode,
};
use sourceview::prelude::*;
use sourceview::View as GtkSourceView;
use sourceview::{Buffer as GtkSourceViewBuffer, LanguageManager};

use crate::backend::{LbCore, LbSyncMsg};
use crate::editmode::EditMode;
use crate::error::{LbErrTarget, LbError, LbResult};
use crate::filetree::FileTree;
use crate::messages::{Messenger, Msg, MsgFn};
use crate::settings::Settings;
use crate::util::gui::LEFT_CLICK;
use crate::util::{gui as gui_util, gui::RIGHT_CLICK};
use crate::{closure, get_data_dir};
use gdk::ModifierType;
use gspell::TextViewExt as GtkTextViewExt;
use lockbook_core::model::client_conversion::{ClientFileMetadata, ClientWorkUnit};
use regex::Regex;
use std::path::Path;
use std::sync::Arc;

pub struct AccountScreen {
    header: Header,
    pub sidebar: Sidebar,
    editor: Editor,
    pub cntr: GtkBox,
}

impl AccountScreen {
    pub fn new(m: &Messenger, s: &Settings, c: &Arc<LbCore>) -> Self {
        let header = Header::new(m);
        let sidebar = Sidebar::new(m, c, s);
        let editor = Editor::new(m);

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

    pub fn fill(&self, core: &LbCore, m: &Messenger) -> LbResult<()> {
        self.sidebar.fill(core)?;
        m.send(Msg::RefreshSyncStatus);
        Ok(())
    }

    pub fn add_file(&self, b: &LbCore, f: &ClientFileMetadata) -> LbResult<()> {
        self.sidebar.tree.add(b, f)
    }

    pub fn show(&self, mode: &EditMode) {
        match mode {
            EditMode::PlainText {
                path,
                meta,
                content,
            } => {
                self.header.set_file(path);
                self.sidebar.tree.select(&meta.id);
                self.editor.set_file(&meta.name, content);
            }
            EditMode::Folder {
                path,
                meta,
                n_children,
            } => {
                self.header.set_file(path);
                self.sidebar.tree.focus();
                self.sidebar.tree.select(&meta.id);
                self.editor.show_folder_info(meta, *n_children);
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

    pub fn status(&self) -> &Rc<StatusPanel> {
        &self.sidebar.status
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
}

struct Header {
    search: GtkEntry,
    spinner: GtkSpinner,
    cntr: GtkBox,
}

impl Header {
    fn new(m: &Messenger) -> Self {
        let search = Self::new_search_field(m);

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
    pub tree: FileTree,
    status: Rc<StatusPanel>,
    cntr: GtkBox,
}

impl Sidebar {
    fn new(m: &Messenger, c: &Arc<LbCore>, s: &Settings) -> Self {
        let scroll = GtkScrolledWindow::new::<GtkAdjustment, GtkAdjustment>(None, None);
        let tree = FileTree::new(m, c, &s.hidden_tree_cols);
        let sync = Rc::new(StatusPanel::new(m));
        scroll.add(tree.widget());

        let cntr = GtkBox::new(Vertical, 0);
        cntr.pack_start(&scroll, true, true, 0);
        cntr.add(&GtkSeparator::new(Horizontal));
        cntr.add(&sync.cntr);

        Self {
            tree,
            status: sync,
            cntr,
        }
    }

    fn fill(&self, core: &LbCore) -> LbResult<()> {
        self.tree.fill(core)
    }
}

pub struct StatusPanel {
    status: GtkLabel,
    sync_button: GtkBtn,
    sync_progress: GtkProgressBar,
    cntr: GtkBox,
}

impl StatusPanel {
    fn new(m: &Messenger) -> Self {
        let status = GtkLabel::new(None);
        status.set_halign(GtkAlign::Start);

        let status_evbox = gtk::EventBox::new();
        status_evbox.add(&status);
        status_evbox.connect_button_press_event(closure!(m => move |_, evt| {
            if evt.get_button() == RIGHT_CLICK {
                let menu = GtkMenu::new();
                let item_data: Vec<(&str, MsgFn)> = vec![
                    ("Refresh Sync Status", || Msg::RefreshSyncStatus),
                    ("Show Sync Details", || Msg::ShowDialogSyncDetails),
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

        let sync_button = GtkBtn::with_label("Sync");
        sync_button.connect_clicked(closure!(m => move |_| m.send(Msg::PerformSync)));

        let progress = GtkProgressBar::new();
        progress.set_margin_top(3);

        let cntr = GtkBox::new(Horizontal, 0);
        gui_util::set_margin(&cntr, 8);
        cntr.pack_start(&status_evbox, false, false, 0);
        cntr.pack_end(&sync_button, false, false, 0);

        Self {
            status,
            sync_button,
            sync_progress: progress,
            cntr,
        }
    }

    pub fn set_syncing(&self, is_syncing: bool) {
        if is_syncing {
            self.set_status("Syncing...", None);
            self.cntr.remove(&self.sync_button);
            self.cntr.set_orientation(Vertical);
            self.cntr.pack_end(&self.sync_progress, true, true, 0);
            self.sync_progress.show();
            self.sync_progress.set_fraction(0.0);
        } else {
            self.cntr.remove(&self.sync_progress);
            self.cntr.set_orientation(Horizontal);
            self.cntr.pack_end(&self.sync_button, false, false, 0);
            self.status.set_text("");
        }
    }

    pub fn set_status(&self, txt: &str, tool_tip_txt: Option<&str>) {
        self.status.set_markup(txt);
        self.status.set_tooltip_text(tool_tip_txt)
    }

    pub fn set_sync_progress(&self, s: &LbSyncMsg) {
        let prefix = match s.work {
            ClientWorkUnit::Server(_) | ClientWorkUnit::ServerUnknownName(_) => "Pulling",
            ClientWorkUnit::Local(_) => "Pushing",
        };
        self.set_status(&format!("{}: {}", prefix, s.name), None);
        self.sync_progress
            .set_fraction(s.index as f64 / s.total as f64);
    }
}

struct Editor {
    info: GtkBox,
    textarea: GtkSourceView,
    highlighter: LanguageManager,
    change_sig_id: RefCell<Option<SignalHandlerId>>,
    cntr: GtkStack,
    messenger: Messenger,
}

impl Editor {
    fn new(m: &Messenger) -> Self {
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

        let textview = textarea.upcast_ref::<gtk::TextView>();
        let gspell_view = gspell::TextView::get_from_gtk_text_view(textview).unwrap();
        gspell_view.basic_setup();

        textarea.connect_button_press_event(closure!(m => move |w, evt| {
            if evt.get_button() == LEFT_CLICK && evt.get_state() == ModifierType::CONTROL_MASK {
                let (absol_x, absol_y) = evt.get_position();
                let (x, y) = w.window_to_buffer_coords(TextWindowType::Text, absol_x as i32, absol_y as i32);
                if let Some(iter) = w.get_iter_at_location(x, y) {
                    let mut end = iter.clone();
                    let mut start = iter.clone();

                    start.backward_line();
                    end.forward_line();

                    let maybe_selected = w.get_buffer().unwrap().downcast::<GtkSourceViewBuffer>().unwrap().get_text(&start, &end, false);

                    if let Some(text) = maybe_selected {
                        let uri_regex = Regex::new(r"\[.*]\(([a-zA-z]+://)(.*)\)").unwrap();

                        if let Some(uri_capture) = uri_regex.captures(text.as_str()) {
                            let scheme = uri_capture.get(1).map(|scheme| scheme.as_str()).unwrap();
                            let uri = uri_capture.get(2).unwrap().as_str();

                            m.send(Msg::MarkdownLinkExec(scheme.to_string(), uri.to_string()))
                        }
                    }
                }

            }

            Inhibit(false)
        }));

        let scroll = GtkScrolledWindow::new(None::<&GtkAdjustment>, None::<&GtkAdjustment>);
        scroll.add(&textarea);

        let cntr = GtkStack::new();
        cntr.add_named(&empty, "empty");
        cntr.add_named(&info, "folderinfo");
        cntr.add_named(&scroll, "scroll");

        let highlighter = LanguageManager::get_default().unwrap_or(LanguageManager::new());

        match Self::language_specs_in_data_dir() {
            Ok(path) => {
                let paths = highlighter.get_search_path();
                let mut str_paths: Vec<&str> = paths.iter().map(|path| path.as_str()).collect();
                str_paths.push(path.as_str());
                highlighter.set_search_path(str_paths.as_slice());
            }
            Err(err) => match err.target() {
                LbErrTarget::Dialog => m.send_err_dialog("language specs", err),
                LbErrTarget::StatusPanel => m.send_err_status_panel(err.msg()),
            },
        }

        Self {
            info,
            textarea,
            highlighter,
            change_sig_id: RefCell::new(None),
            cntr,
            messenger: m.clone(),
        }
    }

    fn language_specs_in_data_dir() -> LbResult<String> {
        let language_specs = format!("{}/language-specs", get_data_dir());
        let language = format!("{}/custom.lang", language_specs);

        if !Path::new(&language_specs).exists() {
            std::fs::create_dir(&language_specs).map_err(LbError::fmt_program_err)?;
            std::fs::write(language, CUSTOM_LANG).map_err(LbError::fmt_program_err)?;
        }

        Ok(language_specs)
    }

    fn set_file(&self, name: &str, content: &str) {
        let tvb = self.textarea.get_buffer().unwrap();

        // Stop listening for changes so that document load doesn't emit FileEdited
        if let Some(id) = self.change_sig_id.take() {
            tvb.disconnect(id)
        }

        let svb = tvb.downcast::<GtkSourceViewBuffer>().unwrap();
        svb.begin_not_undoable_action();
        svb.set_text(content);

        let guess = if name.ends_with(".md") {
            self.highlighter.get_language("lbmd")
        } else {
            self.highlighter.guess_language(Some(name), None)
        };

        svb.set_language(guess.as_ref());
        svb.end_not_undoable_action();

        self.change_sig_id.replace(Some(svb.connect_changed(
            closure!(self.messenger as m => move |_| m.send(Msg::FileEdited)),
        )));

        self.show("scroll");
        self.textarea.grab_focus();
    }

    fn show_folder_info(&self, f: &ClientFileMetadata, n_children: usize) {
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
        self.cntr.set_visible_child_name(name);
    }
}

fn entry_set_primary_icon(entry: &GtkEntry, name: &str) {
    entry.set_icon_from_icon_name(GtkEntryIconPosition::Primary, Some(name));
}

fn entry_set_primary_icon_tooltip(entry: &GtkEntry, tooltip: Option<&str>) {
    entry.set_icon_tooltip_text(GtkEntryIconPosition::Primary, tooltip);
}

const LOGO: &[u8] = include_bytes!("../res/lockbook-pixdata");
const CUSTOM_LANG: &[u8] = include_bytes!("../res/custom.lang");

const ESC: u16 = 9;
const ARROW_UP: u16 = 111;
const ARROW_DOWN: u16 = 116;
