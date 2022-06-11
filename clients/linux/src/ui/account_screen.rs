use std::cell::Cell;
use std::rc::Rc;

use gtk::gdk;
use gtk::glib;
use gtk::prelude::*;

use crate::ui;

pub enum AccountOp {
    NewFile,
    OpenFile(lb::Uuid),
    RenameFile,
    DeleteFiles,
    ExportFiles,

    CutFiles,
    CopyFiles,
    PasteFiles,

    TreeReceiveDrop(glib::Value, f64, f64),
    TabSwitched(ui::Tab),
    AllTabsClosed,

    SviewCtrlClick { click: gtk::GestureClick, x: i32, y: i32, sview: sv5::View },
    SviewInsertFileList { id: lb::Uuid, buf: sv5::Buffer, flist: gdk::FileList },
    SviewInsertTexture { id: lb::Uuid, buf: sv5::Buffer, texture: gdk::Texture },
}

#[derive(Clone)]
pub struct AccountScreen {
    pub op_chan: glib::Sender<AccountOp>,
    pub tree: ui::FileTree,
    pub sync: ui::SyncPanel,
    pub lang_mngr: sv5::LanguageManager,
    pub scheme_name: Rc<Cell<&'static str>>,
    pub tabs: gtk::Notebook,
    pub cntr: gtk::Paned,
}

impl AccountScreen {
    pub fn new(
        op_chan: glib::Sender<AccountOp>, lang_mngr: sv5::LanguageManager, hidden_cols: &[String],
    ) -> Self {
        let stack = gtk::Stack::new();

        let tabs = gtk::Notebook::new();
        tabs.connect_page_added({
            let stack = stack.clone();

            move |tabs, _, i| {
                tabs.set_show_tabs(tabs.n_pages() > 1);
                tabs.set_page(i as i32);
                stack.set_visible_child_name("tabs");
            }
        });
        tabs.connect_page_removed({
            let op_chan = op_chan.clone();
            let stack = stack.clone();

            move |tabs, _, _| {
                let n_tabs = tabs.n_pages();
                tabs.set_show_tabs(n_tabs > 1);
                if n_tabs == 0 {
                    op_chan.send(AccountOp::AllTabsClosed).unwrap();
                    stack.set_visible_child_name("logo");
                }
            }
        });
        tabs.connect_switch_page({
            let op_chan = op_chan.clone();

            move |_, w, _| {
                let tab = w.clone().downcast::<ui::Tab>().unwrap();
                op_chan.send(AccountOp::TabSwitched(tab)).unwrap();
            }
        });

        let tree = ui::FileTree::new(&op_chan, hidden_cols);
        let tree_scroll = gtk::ScrolledWindow::new();
        tree_scroll.set_child(Some(&tree.overlay));

        let sync = ui::SyncPanel::new();

        let sidebar = gtk::Box::new(gtk::Orientation::Vertical, 0);
        sidebar.append(&tree_scroll);
        sidebar.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        sidebar.append(&sync.cntr);

        stack.add_named(&super::logo(400), Some("logo"));
        stack.add_named(&tabs, Some("tabs"));

        let cntr = gtk::Paned::new(gtk::Orientation::Horizontal);
        cntr.set_position(325);
        cntr.set_start_child(Some(&sidebar));
        cntr.set_end_child(Some(&stack));

        let scheme_name = Rc::new(Cell::new("classic"));

        Self { op_chan, tree, sync, lang_mngr, scheme_name, tabs, cntr }
    }

    pub fn tab_by_id(&self, id: lb::Uuid) -> Option<ui::Tab> {
        for i in 0..self.tabs.n_pages() {
            let w = self.tabs.nth_page(Some(i)).unwrap();
            let tab = w.downcast::<ui::Tab>().unwrap();
            if tab.id().eq(&id) {
                return Some(tab);
            }
        }
        None
    }

    pub fn current_tab(&self) -> Option<ui::Tab> {
        self.tabs
            .nth_page(self.tabs.current_page())
            .map(|w| w.downcast::<ui::Tab>().unwrap())
    }

    pub fn focus_tab_by_id(&self, id: lb::Uuid) -> bool {
        for i in 0..self.tabs.n_pages() {
            let w = self.tabs.nth_page(Some(i)).unwrap();
            if let Some(tab) = w.downcast_ref::<ui::Tab>() {
                if tab.id().eq(&id) {
                    self.tabs.set_current_page(Some(i));
                    if let Some(txt_ed) = tab.content::<ui::TextEditor>() {
                        txt_ed.editor().grab_focus();
                    }
                    return true;
                }
            }
        }
        false
    }
}
