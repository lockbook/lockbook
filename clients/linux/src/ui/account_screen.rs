use std::cell::Cell;
use std::rc::Rc;

use gtk::glib;
use gtk::prelude::*;

use crate::ui;

pub enum AccountOp {
    NewDocument,
    NewFolder,
    OpenFile(lb::Uuid),
    RenameFile,
    DeleteFiles,
    ExportFiles,
    CutFiles,
    CopyFiles,
    PasteFiles,
    TreeReceiveDrop(glib::Value, f64, f64),
    TabSwitched(ui::TextEditor),
    AllTabsClosed,
}

#[derive(Clone)]
pub struct AccountScreen {
    pub tree: ui::FileTree,
    pub sync: ui::SyncPanel,
    pub lang_mngr: sv5::LanguageManager,
    pub scheme_name: Rc<Cell<&'static str>>,
    pub tabs: gtk::Notebook,
    pub cntr: gtk::Paned,
}

impl AccountScreen {
    pub fn new(
        account_op_tx: glib::Sender<AccountOp>, lang_mngr: sv5::LanguageManager,
        hidden_cols: &[String],
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
            let account_op_tx = account_op_tx.clone();
            let stack = stack.clone();

            move |tabs, _, _| {
                let n_tabs = tabs.n_pages();
                tabs.set_show_tabs(n_tabs > 1);
                if n_tabs == 0 {
                    account_op_tx.send(AccountOp::AllTabsClosed).unwrap();
                    stack.set_visible_child_name("logo");
                }
            }
        });
        tabs.connect_switch_page({
            let account_op_tx = account_op_tx.clone();

            move |_, w, _| {
                let tab = w.downcast_ref::<ui::TextEditor>().unwrap().clone();
                account_op_tx.send(AccountOp::TabSwitched(tab)).unwrap();
            }
        });

        let tree = ui::FileTree::new(account_op_tx, hidden_cols);
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
        cntr.set_position(350);
        cntr.set_start_child(&sidebar);
        cntr.set_end_child(&stack);

        let scheme_name = Rc::new(Cell::new("classic"));

        Self { tree, sync, lang_mngr, scheme_name, tabs, cntr }
    }

    pub fn tab_by_id(&self, id: lb::Uuid) -> Option<ui::TextEditor> {
        for i in 0..self.tabs.n_pages() {
            let w = self.tabs.nth_page(Some(i)).unwrap();
            let tab = w.downcast::<ui::TextEditor>().unwrap();
            if tab.id().eq(&id) {
                return Some(tab);
            }
        }
        None
    }

    pub fn current_tab(&self) -> Option<ui::TextEditor> {
        self.tabs
            .nth_page(self.tabs.current_page())
            .map(|w| w.downcast::<ui::TextEditor>().unwrap())
    }

    pub fn focus_tab_by_id(&self, id: lb::Uuid) -> bool {
        for i in 0..self.tabs.n_pages() {
            let w = self.tabs.nth_page(Some(i)).unwrap();
            let tab = w.downcast::<ui::TextEditor>().unwrap();
            if tab.id().eq(&id) {
                self.tabs.set_current_page(Some(i));
                tab.editor().grab_focus();
                return true;
            }
        }
        false
    }
}
