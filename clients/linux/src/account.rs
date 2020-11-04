use glib::clone;
use gtk::prelude::*;
use gtk::Orientation::{Horizontal, Vertical};
use gtk::{
    Align as GtkAlign, Box as GtkBox, Button as GtkBtn, Grid as GtkGrid, HeaderBar as GtkHeaderBar,
    Image as GtkImage, Label as GtkLabel, Paned as GtkPaned, Separator as GtkSeparator,
    Spinner as GtkSpinner, Stack as GtkStack, TextView as GtkTextView, WrapMode as GtkWrapMode,
};

use lockbook_core::model::file_metadata::FileMetadata;

use crate::backend::LbCore;
use crate::editmode::EditMode;
use crate::filetree::FileTree;
use crate::messages::{Messenger, Msg};
use crate::settings::Settings;

pub struct AccountScreen {
    pub sidebar: Sidebar,
    editor: Editor,
    pub cntr: GtkPaned,
}

impl AccountScreen {
    pub fn new(m: &Messenger, s: &Settings) -> Self {
        let sidebar = Sidebar::new(m, &s);
        let editor = Editor::new(m);

        let cntr = GtkPaned::new(Horizontal);
        cntr.add1(&sidebar.widget);
        cntr.add2(&editor.cntr);

        Self {
            sidebar,
            editor,
            cntr,
        }
    }

    pub fn fill(&self, core: &LbCore) {
        self.sidebar.fill(&core);
        self.set_sync_status(&core);
    }

    pub fn add_file(&self, b: &LbCore, f: &FileMetadata) {
        self.sidebar.tree.add(b, f);
    }

    pub fn show(&self, mode: &EditMode) {
        match mode {
            EditMode::PlainText { meta, content } => {
                self.sidebar.tree.select(&meta.id);
                self.editor.set_file(&meta, &content);
            }
            EditMode::Folder {
                path,
                meta,
                n_children,
            } => {
                self.sidebar.tree.focus();
                self.editor.show_folder_info(&path, &meta, *n_children);
            }
            EditMode::None => self.editor.clear(),
        }
    }

    pub fn text_content(&self) -> String {
        let buf = self.editor.textarea.get_buffer().unwrap();
        buf.get_text(&buf.get_start_iter(), &buf.get_end_iter(), true)
            .unwrap()
            .to_string()
    }

    pub fn set_saving(&self, is_saving: bool) {
        if is_saving {
            self.editor.show_spinner();
        } else {
            self.editor.hide_spinner();
        }
    }

    pub fn set_syncing(&self, is_syncing: bool) {
        if is_syncing {
            self.sidebar.sync.syncing();
        } else {
            self.sidebar.sync.done();
        }
    }

    pub fn set_sync_status(&self, core: &LbCore) {
        match core.get_last_synced() {
            Ok(last) => match last {
                0 => self.sidebar.sync.status.set_markup("✘  Never synced."),
                _ => match core.calculate_work() {
                    Ok(work) => {
                        let n_files = work.work_units.len();
                        let txt = match n_files {
                            0 => "✔  Synced.".to_string(),
                            1 => "<b>1</b> file not synced.".to_string(),
                            _ => format!("<b>{}</b> files not synced.", n_files),
                        };
                        self.sidebar.sync.status.set_markup(&txt);
                    }
                    Err(err) => println!("{:?}", err),
                },
            },
            Err(err) => panic!(err),
        }
    }
}

pub struct Sidebar {
    pub tree: FileTree,
    pub sync: SyncPanel,
    widget: GtkBox,
}

impl Sidebar {
    fn new(m: &Messenger, s: &Settings) -> Self {
        let tree = FileTree::new(&m, &s.hidden_tree_cols);
        let sync = SyncPanel::new(&m);

        let bx = GtkBox::new(Vertical, 0);
        bx.add(&{
            let hb = GtkHeaderBar::new();
            hb.set_title(Some("My Files:"));
            hb
        });
        bx.add(&GtkSeparator::new(Horizontal));
        bx.pack_start(&tree.tree, true, true, 0);
        bx.add(&GtkSeparator::new(Horizontal));
        bx.add(&sync.cntr);

        Self {
            tree,
            sync,
            widget: bx,
        }
    }

    pub fn fill(&self, core: &LbCore) {
        self.tree.fill(core);
    }
}

pub struct SyncPanel {
    errlbl: GtkLabel,
    status: GtkLabel,
    button: GtkBtn,
    spinner: GtkSpinner,
    center: GtkBox,
    cntr: GtkBox,
}

impl SyncPanel {
    fn new(m: &Messenger) -> Self {
        let errlbl = GtkLabel::new(None);
        errlbl.set_halign(GtkAlign::Start);

        let (status, button, spinner, center) = Self::center(&m);

        let cntr = GtkBox::new(Vertical, 0);
        cntr.set_center_widget(Some(&center));
        cntr.set_margin_start(8);
        cntr.set_margin_end(8);

        Self {
            errlbl,
            status,
            button,
            spinner,
            center,
            cntr,
        }
    }

    fn center(m: &Messenger) -> (GtkLabel, GtkBtn, GtkSpinner, GtkBox) {
        let status = GtkLabel::new(None);
        status.set_halign(GtkAlign::Start);

        let button = GtkBtn::with_label("Sync");
        button.connect_clicked(clone!(@strong m => move |_| {
            m.send(Msg::PerformSync);
        }));

        let spinner = GtkSpinner::new();
        spinner.set_margin_top(4);
        spinner.set_margin_bottom(4);
        spinner.set_size_request(20, 20);

        let center = GtkBox::new(Horizontal, 0);
        center.set_margin_top(8);
        center.set_margin_bottom(8);
        center.pack_start(&status, false, false, 0);
        center.pack_end(&button, false, false, 0);

        (status, button, spinner, center)
    }

    pub fn doing(&self, txt: &str) {
        self.status.set_text(txt);
    }

    pub fn error(&self, txt: &str) {
        self.cntr.pack_start(&self.errlbl, false, false, 0);
        self.errlbl.set_text(txt);
    }

    fn syncing(&self) {
        self.center.remove(&self.button);
        self.center.pack_end(&self.spinner, false, false, 0);
        self.spinner.show();
        self.spinner.start();
    }

    fn done(&self) {
        self.spinner.stop();
        self.center.remove(&self.spinner);
        self.center.pack_end(&self.button, false, false, 0);
        self.status.set_text("");
    }
}

struct Editor {
    header: EditorHeader,
    center: GtkStack,

    info: GtkBox,
    textarea: GtkTextView,
    spinner: GtkSpinner,

    cntr: GtkBox,
}

impl Editor {
    fn new(m: &Messenger) -> Self {
        let header = EditorHeader::new(&m);

        let empty = GtkBox::new(Vertical, 0);
        empty.set_valign(GtkAlign::Center);
        empty.add(&GtkImage::from_file("./lockbook.png"));

        let info = GtkBox::new(Vertical, 0);
        info.set_vexpand(false);
        info.set_valign(GtkAlign::Center);

        let textarea = GtkTextView::new();
        textarea.set_property_monospace(true);
        textarea.set_wrap_mode(GtkWrapMode::Word);
        textarea.set_left_margin(4);

        let center = GtkStack::new();
        center.add_named(&empty, "empty");
        center.add_named(&info, "folderinfo");
        center.add_named(&textarea, "textarea");

        let cntr = GtkBox::new(Vertical, 0);
        cntr.add(&header.cntr);
        cntr.add(&GtkSeparator::new(Horizontal));
        cntr.pack_start(&center, true, true, 0);

        Self {
            header,
            center,
            info,
            spinner: GtkSpinner::new(),
            textarea,
            cntr,
        }
    }

    fn set_file(&self, f: &FileMetadata, content: &str) {
        self.header.set_file(&f);
        self.center.set_visible_child_name("textarea");

        self.textarea.get_buffer().unwrap().set_text(content);
        self.textarea.grab_focus();
    }

    fn show_folder_info(&self, full_path: &str, f: &FileMetadata, n_children: usize) {
        self.header.cntr.set_title(Some(full_path));
        self.info.foreach(|w| {
            self.info.remove(w);
        });

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
            grid.attach(&text_right(key), 0, row as i32, 1, 1);
            grid.attach(&text_left(&val), 1, row as i32, 1, 1);
        }

        self.info.add(&name);
        self.info.add(&grid);
        self.info.show_all();
        self.center.set_visible_child_name("folderinfo");
    }

    fn show_spinner(&self) {
        self.header.cntr.pack_end(&self.spinner);
        self.header.cntr.show_all();
        self.spinner.start();
    }

    fn hide_spinner(&self) {
        self.spinner.stop();
        self.header.cntr.remove(&self.spinner);
    }

    fn clear(&self) {
        self.header.cntr.set_title(Some(""));
        self.center.set_visible_child_name("empty");
    }
}

pub fn text_right(txt: &str) -> GtkLabel {
    let l = GtkLabel::new(Some(txt));
    l.set_halign(GtkAlign::End);
    l.set_margin_end(4);
    l
}

pub fn text_left(txt: &str) -> GtkLabel {
    let l = GtkLabel::new(Some(txt));
    l.set_halign(GtkAlign::Start);
    l.set_margin_start(4);
    l
}

struct EditorHeader {
    cntr: GtkHeaderBar,
}

impl EditorHeader {
    fn new(_m: &Messenger) -> Self {
        let cntr = GtkHeaderBar::new();

        Self { cntr }
    }

    fn set_file(&self, f: &FileMetadata) {
        self.cntr.set_title(Some(&f.name));
    }
}
