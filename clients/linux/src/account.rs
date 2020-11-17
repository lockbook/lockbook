use glib::clone;
use gtk::prelude::*;
use gtk::Orientation::{Horizontal, Vertical};
use gtk::{
    Adjustment as GtkAdjustment, Align as GtkAlign, Box as GtkBox, Button as GtkBtn,
    Grid as GtkGrid, HeaderBar as GtkHeaderBar, Image as GtkImage, Label as GtkLabel,
    Paned as GtkPaned, ScrolledWindow as GtkScrolledWindow, Separator as GtkSeparator,
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
        let editor = Editor::new();

        let cntr = GtkPaned::new(Horizontal);
        cntr.add1(&sidebar.cntr);
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
        let buf = self.editor.workspace.textarea.get_buffer().unwrap();
        buf.get_text(&buf.get_start_iter(), &buf.get_end_iter(), true)
            .unwrap()
            .to_string()
    }

    pub fn set_saving(&self, is_saving: bool) {
        if is_saving {
            self.editor.headerbar.show_spinner();
        } else {
            self.editor.headerbar.hide_spinner();
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
                            1 => "<b>1</b>  file not synced.".to_string(),
                            _ => format!("<b>{}</b>  files not synced.", n_files),
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
    cntr: GtkBox,
}

impl Sidebar {
    fn new(m: &Messenger, s: &Settings) -> Self {
        let tree = FileTree::new(&m, &s.hidden_tree_cols);
        let sync = SyncPanel::new(&m);

        let cntr = GtkBox::new(Vertical, 0);
        cntr.add(&{
            let hb = GtkHeaderBar::new();
            hb.set_title(Some("My Files:"));
            hb
        });
        cntr.add(&GtkSeparator::new(Horizontal));
        cntr.pack_start(&tree.tree, true, true, 0);
        cntr.add(&GtkSeparator::new(Horizontal));
        cntr.add(&sync.cntr);

        Self { tree, sync, cntr }
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
    headerbar: EditorHeaderBar,
    workspace: EditorWorkSpace,
    cntr: GtkBox,
}

impl Editor {
    fn new() -> Self {
        let headerbar = EditorHeaderBar::new();
        let workspace = EditorWorkSpace::new();

        let cntr = GtkBox::new(Vertical, 0);
        cntr.add(&headerbar.cntr);
        cntr.add(&GtkSeparator::new(Horizontal));
        cntr.pack_start(&workspace.cntr, true, true, 0);

        Self {
            headerbar,
            workspace,
            cntr,
        }
    }

    fn set_file(&self, f: &FileMetadata, content: &str) {
        self.headerbar.set_file(&f);

        let ws = &self.workspace;
        ws.show("textarea");
        ws.textarea.grab_focus();
        ws.textarea.get_buffer().unwrap().set_text(content);
    }

    fn show_folder_info(&self, full_path: &str, f: &FileMetadata, n_children: usize) {
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

        let info = &self.workspace.info;
        info.foreach(|w| info.remove(w));
        info.add(&name);
        info.add(&grid);
        info.show_all();

        self.headerbar.cntr.set_title(Some(full_path));
        self.workspace.show("folderinfo");
    }

    fn clear(&self) {
        self.headerbar.cntr.set_title(Some(""));
        self.workspace.show("empty");
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

struct EditorHeaderBar {
    spinner: GtkSpinner,
    cntr: GtkHeaderBar,
}

impl EditorHeaderBar {
    fn new() -> Self {
        Self {
            spinner: GtkSpinner::new(),
            cntr: GtkHeaderBar::new(),
        }
    }

    fn set_file(&self, f: &FileMetadata) {
        self.cntr.set_title(Some(&f.name));
    }

    fn show_spinner(&self) {
        self.cntr.pack_end(&self.spinner);
        self.cntr.show_all();
        self.spinner.start();
    }

    fn hide_spinner(&self) {
        self.spinner.stop();
        self.cntr.remove(&self.spinner);
    }
}

struct EditorWorkSpace {
    info: GtkBox,
    textarea: GtkTextView,
    stack: GtkStack,
    cntr: GtkScrolledWindow,
}

impl EditorWorkSpace {
    fn new() -> Self {
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

        let stack = GtkStack::new();
        stack.add_named(&empty, "empty");
        stack.add_named(&info, "folderinfo");
        stack.add_named(&textarea, "textarea");

        let cntr = GtkScrolledWindow::new(None::<&GtkAdjustment>, None::<&GtkAdjustment>);
        cntr.add(&stack);

        Self {
            info,
            textarea,
            stack,
            cntr,
        }
    }

    fn show(&self, name: &str) {
        self.stack.set_visible_child_name(name);
    }
}
