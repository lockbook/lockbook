use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use gdk_pixbuf::Pixbuf as GdkPixbuf;
use gspell::TextViewExt as GtkTextViewExt;
use gtk::prelude::*;
use gtk::{Align, Box as GtkBox, IconSize, Overlay};
use gtk::Orientation::{Horizontal, Vertical};
use regex::Regex;
use sourceview::prelude::*;
use sourceview::{Buffer as GtkSourceViewBuffer, LanguageManager, View as GtkSourceView};

use crate::backend::{LbCore, LbSyncMsg};
use crate::editmode::EditMode;
use crate::error::{LbErrTarget, LbError, LbResult};
use crate::filetree::FileTree;
use crate::messages::{Messenger, Msg, MsgFn};
use crate::settings::Settings;
use crate::util::{
    gui as gui_util, gui::LEFT_CLICK, gui::RIGHT_CLICK, IMAGE_TARGET_INFO, TEXT_TARGET_INFO,
    URI_TARGET_INFO,
};
use crate::{closure, get_language_specs_dir, GtkLabel, progerr, uerr, uerr_dialog};

use lockbook_models::file_metadata::DecryptedFileMetadata;
use lockbook_models::work_unit::ClientWorkUnit;

pub struct AccountScreen {
    pub sidebar: Sidebar,
    editor: Editor,
    pub cntr: gtk::Paned,
}

impl AccountScreen {
    pub fn new(m: &Messenger, s: &Settings, c: &Arc<LbCore>) -> Self {
        let sidebar = Sidebar::new(m, c, s);
        let editor = Editor::new(m);

        let cntr = gtk::Paned::new(Horizontal);
        cntr.set_position(350);
        cntr.add1(&sidebar.cntr);
        cntr.add2(&editor.cntr);

        Self {
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

    pub fn add_file(&self, b: &LbCore, f: &DecryptedFileMetadata) -> LbResult<()> {
        self.sidebar.tree.add(b, f)
    }

    pub fn show(&self, mode: &EditMode) {
        match mode {
            EditMode::PlainText { meta, content } => {
                self.sidebar.tree.select(&meta.id);
                self.editor.set_file(&meta.decrypted_name, content);
            }
            EditMode::Folder { meta, n_children } => {
                self.sidebar.tree.focus();
                self.sidebar.tree.select(&meta.id);
                self.editor.show_folder_info(meta, *n_children);
            }
            EditMode::None => {
                self.editor.show("empty");
            }
        }
    }

    pub fn get_cursor_mark(&self) -> LbResult<gtk::TextMark> {
        let svb = self
            .editor
            .textarea
            .get_buffer()
            .unwrap()
            .downcast::<GtkSourceViewBuffer>()
            .unwrap();

        svb.create_mark(
            // Since get_insert gives me the textmark of the cursor, and it is subject to change, I make my own textmark so multiple pastes won't collide
            None,
            &svb.get_iter_at_mark(
                &svb.get_insert()
                    .ok_or_else(|| uerr_dialog!("No cursor found in textview!"))?,
            ),
            true,
        )
        .ok_or_else(|| uerr_dialog!("Cannot create textmark!"))
    }

    pub fn insert_text_at_mark(&self, mark: &gtk::TextMark, txt: &str) {
        let svb = self
            .editor
            .textarea
            .get_buffer()
            .unwrap()
            .downcast::<GtkSourceViewBuffer>()
            .unwrap();

        svb.insert(&mut svb.get_iter_at_mark(mark), txt)
    }

    pub fn text_content(&self) -> String {
        let buf = self.editor.textarea.get_buffer().unwrap();
        let start = buf.get_start_iter();
        let end = buf.get_end_iter();
        buf.get_text(&start, &end, true).unwrap().to_string()
    }

    pub fn set_saving(&self, _is_saving: bool) {
        // todo: how to indicate saving now since there is no more header bar
    }

    pub fn status(&self) -> &Rc<StatusPanel> {
        &self.sidebar.status
    }
}

pub struct Sidebar {
    pub tree: FileTree,
    pub out_of_space: OutOfSpacePanel,
    status: Rc<StatusPanel>,
    cntr: GtkBox,
}

impl Sidebar {
    fn new(m: &Messenger, c: &Arc<LbCore>, s: &Settings) -> Self {
        let tree = FileTree::new(m, c, &s.hidden_tree_cols);
        let scroll = gui_util::scrollable(tree.widget());

        let sync = Rc::new(StatusPanel::new(m));

        let out_of_space = OutOfSpacePanel::new();

        let overlay = gtk::Overlay::new();
        overlay.add(&scroll);
        overlay.add_overlay(&out_of_space.cntr);

        let cntr = GtkBox::new(Vertical, 0);
        cntr.pack_start(&overlay, true, true, 0);
        cntr.add(&gtk::Separator::new(Horizontal));
        cntr.add(&sync.cntr);

        Self {
            tree,
            status: sync,
            out_of_space,
            cntr,
        }
    }

    fn fill(&self, core: &LbCore) -> LbResult<()> {
        self.tree.fill(core)
    }
}

pub struct OutOfSpacePanel {
    progress: gtk::ProgressBar,
    cntr: GtkBox
}

impl OutOfSpacePanel {
    fn new() -> OutOfSpacePanel {
        let progress = gtk::ProgressBar::new();

        progress.color

        let button = gtk::Button::from_icon_name(Some("window-close"), IconSize::Button);

        // button.set_label("Close");
        button.set_halign(Align::End);

        let cntr = GtkBox::new(Horizontal, 0);
        cntr.pack_start(&GtkLabel::new(Some("You are running out of space!")), false, false, 0);
        cntr.pack_end(&button, false, false, 0);

        cntr.set_halign(Align::Fill);
        cntr.set_margin_bottom(10);

        let cntr_main = GtkBox::new(Vertical, 0);
        cntr_main.add(&cntr);
        cntr_main.add(&progress);

        cntr_main.set_margin_bottom(20);
        cntr_main.set_margin_start(20);
        cntr_main.set_margin_end(20);

        cntr_main.set_halign(Align::Fill);
        cntr_main.set_valign(Align::End);

        cntr_main.hide();

        button.connect_clicked(closure!(cntr_main => move |_| {
            cntr_main.hide();
        }));

        OutOfSpacePanel {
            progress,
            cntr: cntr_main
        }
    }

    pub fn update(&self, usage: f64, data_cap: f64) {
        let usage_progress = usage / data_cap;

        if usage_progress > 0.8 {
            self.progress.set_fraction(usage_progress);
            self.cntr.show()
        } else {
            self.cntr.hide();
        }
    }
}

pub struct StatusPanel {
    status: gtk::Label,
    sync_button: gtk::Button,
    sync_progress: gtk::ProgressBar,
    cntr: GtkBox,
}

impl StatusPanel {
    fn new(m: &Messenger) -> Self {
        let status = gtk::Label::new(None);
        status.set_halign(gtk::Align::Start);

        let status_evbox = gtk::EventBox::new();
        status_evbox.add(&status);
        status_evbox.connect_button_press_event(closure!(m => move |_, evt| {
            if evt.get_button() == RIGHT_CLICK {
                let menu = gtk::Menu::new();
                let item_data: Vec<(&str, MsgFn)> = vec![
                    ("Refresh Sync Status", || Msg::RefreshSyncStatus),
                    ("Show Sync Details", || Msg::ShowDialogSyncDetails),
                ];
                for (name, msg) in item_data {
                    let mi = gtk::MenuItem::with_label(name);
                    mi.connect_activate(closure!(m => move |_| m.send(msg())));
                    menu.append(&mi);
                }
                menu.show_all();
                menu.popup_at_pointer(Some(evt));
            }
            gtk::Inhibit(false)
        }));

        let sync_button = gtk::Button::with_label("Sync");
        sync_button.connect_clicked(closure!(m => move |_| m.send(Msg::PerformSync)));

        let progress = gtk::ProgressBar::new();
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
        let status = match &s.work {
            ClientWorkUnit::PullMetadata => String::from("Pulling file tree updates"),
            ClientWorkUnit::PushMetadata => String::from("Pushing file tree updates"),
            ClientWorkUnit::PullDocument(name) => format!("Pulling: {}", name),
            ClientWorkUnit::PushDocument(name) => format!("Pushing: {}", name),
        };
        self.set_status(&status, None);
        self.sync_progress
            .set_fraction(s.index as f64 / s.total as f64);
    }
}

pub enum TextAreaDropPasteInfo {
    Image(Vec<u8>),
    Uris(Vec<String>),
}

struct Editor {
    info: GtkBox,
    textarea: GtkSourceView,
    highlighter: LanguageManager,
    change_sig_id: RefCell<Option<glib::SignalHandlerId>>,
    cntr: gtk::Stack,
    messenger: Messenger,
}

impl Editor {
    fn new(m: &Messenger) -> Self {
        let empty = GtkBox::new(Vertical, 0);
        empty.set_valign(gtk::Align::Center);
        empty.add(&gtk::Image::from_pixbuf(Some(
            &GdkPixbuf::from_inline(LOGO, false).unwrap(),
        )));

        let info = GtkBox::new(Vertical, 0);
        info.set_vexpand(false);
        info.set_valign(gtk::Align::Center);

        let textarea = GtkSourceView::new();
        textarea.set_property_monospace(true);
        textarea.set_wrap_mode(gtk::WrapMode::Word);
        textarea.set_left_margin(4);
        textarea.set_tab_width(4);

        let target_list = gtk::TargetList::new(&[]);

        textarea.drag_dest_set(gtk::DestDefaults::ALL, &[], gdk::DragAction::COPY);

        target_list.add_uri_targets(URI_TARGET_INFO);
        target_list.add_text_targets(TEXT_TARGET_INFO);
        target_list.add_image_targets(IMAGE_TARGET_INFO, true);

        textarea.drag_dest_set_target_list(Some(&target_list));

        textarea.connect_drag_data_received(Self::on_drag_data_received(m));
        textarea.connect_paste_clipboard(Self::on_paste_clipboard(m));
        textarea.connect_button_press_event(Self::on_button_press(m));

        let textview = textarea.upcast_ref::<gtk::TextView>();

        let gspell_view = gspell::TextView::get_from_gtk_text_view(textview).unwrap();
        gspell_view.basic_setup();

        let scroll = gui_util::scrollable(&textarea);

        let cntr = gtk::Stack::new();
        cntr.add_named(&empty, "empty");
        cntr.add_named(&info, "folderinfo");
        cntr.add_named(&scroll, "scroll");

        let highlighter = LanguageManager::get_default().unwrap_or_default();
        let lang_paths = highlighter.get_search_path();

        let mut str_paths: Vec<&str> = lang_paths.iter().map(|path| path.as_str()).collect();
        let lang_specs = get_language_specs_dir();
        str_paths.push(lang_specs.as_str());
        highlighter.set_search_path(str_paths.as_slice());

        Self {
            info,
            textarea,
            highlighter,
            change_sig_id: RefCell::new(None),
            cntr,
            messenger: m.clone(),
        }
    }

    fn on_drag_data_received(
        m: &Messenger,
    ) -> impl Fn(&GtkSourceView, &gdk::DragContext, i32, i32, &gtk::SelectionData, u32, u32) {
        closure!(m => move |_, _, _, _, s, info, _| {
            let target = match info {
                URI_TARGET_INFO => {
                    TextAreaDropPasteInfo::Uris(s.get_uris().iter().map(|uri| uri.to_string()).collect())
                }
                IMAGE_TARGET_INFO => match s.get_pixbuf() {
                    None => {
                        m.send_err_dialog("Dropping image", uerr_dialog!("Unsupported image format!"));
                        return;
                    }
                    Some(pixbuf) => match pixbuf.save_to_bufferv("jpg", &[]) {
                        Ok(bytes) => TextAreaDropPasteInfo::Image(bytes),
                        Err(err) => {
                            m.send_err_dialog("Dropping image", LbError::fmt_program_err(err));
                            return;
                        }
                    },
                },
                TEXT_TARGET_INFO => return,
                _ => {
                    m.send_err_dialog("Dropping data", progerr!("Unrecognized data format '{}'.", s.get_data_type().name()));
                    return;
                },
            };

            m.send(Msg::DropPasteInTextArea(target))
        })
    }

    fn on_paste_clipboard(m: &Messenger) -> impl Fn(&GtkSourceView) {
        closure!(m => move |w| {
            let clipboard = gtk::Clipboard::get(&gdk::SELECTION_CLIPBOARD);

            if let Some(pixbuf) = clipboard.wait_for_image() {
                match pixbuf.save_to_bufferv("jpeg", &[]) {
                    Ok(bytes) => {
                        m.send(Msg::DropPasteInTextArea(TextAreaDropPasteInfo::Image(bytes)))
                    },
                    Err(err) => {
                        m.send_err_dialog("Pasting image", LbError::fmt_program_err(err));
                    }
                }
            } else {
                let uris = clipboard.wait_for_uris();
                if !uris.is_empty() {
                    w.stop_signal_emission("paste-clipboard");

                    m.send(Msg::DropPasteInTextArea(TextAreaDropPasteInfo::Uris(uris.iter().map(|uri| uri.to_string()).collect())))
                }
            }

        })
    }

    fn on_button_press(m: &Messenger) -> impl Fn(&GtkSourceView, &gdk::EventButton) -> Inhibit {
        closure!(m => move |w, evt| {
            if evt.get_button() == LEFT_CLICK && evt.get_state() == gdk::ModifierType::CONTROL_MASK {
                let (absol_x, absol_y) = evt.get_position();
                let (x, y) = w.window_to_buffer_coords(gtk::TextWindowType::Text, absol_x as i32, absol_y as i32);
                if let Some(iter) = w.get_iter_at_location(x, y) {
                    let mut start = iter.clone();
                    let mut end = iter.clone();

                    start.backward_visible_line();
                    start.forward_visible_line();
                    end.forward_visible_line();

                    let svb = w.get_buffer().unwrap().downcast::<GtkSourceViewBuffer>().unwrap();
                    let maybe_selected = svb.get_text(&start, &end, false);
                    let index = iter.get_line_index();

                    if let Some(text) = maybe_selected {
                        let uri_regex = Regex::new(r"\[.*]\(([a-zA-z]+://)(.*)\)").unwrap();

                        for capture in uri_regex.captures_iter(text.as_str()) {
                            let whole = match capture.get(0) {
                                Some(whole) => whole,
                                None => {
                                    return Inhibit(false);
                                }
                            };
                            let loc = whole.start()..whole.end();

                            if loc.contains(&(index as usize)) {
                                let scheme = capture.get(1).map(|scheme| scheme.as_str()).unwrap();
                                let uri = capture.get(2).unwrap().as_str();

                                m.send(Msg::MarkdownLinkExec(scheme.to_string(), uri.to_string()));
                                break
                            }
                        }
                    }
                }

            }

            Inhibit(false)
        })
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

    fn show_folder_info(&self, f: &DecryptedFileMetadata, n_children: usize) {
        let name = gtk::Label::new(None);
        name.set_markup(&format!("<span><big>{}/</big></span>", f.decrypted_name));
        name.set_margin_end(64);
        name.set_margin_bottom(16);

        let grid = gtk::Grid::new();
        grid.set_halign(gtk::Align::Center);

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

const LOGO: &[u8] = include_bytes!("../res/lockbook-pixdata");
