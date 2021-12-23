use std::{cell::RefCell, fs, io, rc::Rc};

use gtk::prelude::*;
use gtk::Orientation::Vertical;
use serde::{Deserialize, Serialize};

use crate::app::LbApp;
use crate::error::LbResult;
use crate::filetree::FileTreeCol;
use crate::messages::{Messenger, Msg};

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub hidden_tree_cols: Vec<String>,
    pub window_maximize: bool,
    pub auto_save: bool,
    pub auto_sync: bool,
    #[serde(skip_serializing, skip_deserializing)]
    path: String,
}

impl Settings {
    pub fn from_data_dir(dir: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let path = format!("{}/settings.yaml", dir);
        let mut s: Self = match fs::File::open(&path) {
            Ok(f) => serde_yaml::from_reader(f)?,
            Err(err) => match err.kind() {
                io::ErrorKind::NotFound => Self::default(),
                _ => return Err(Box::new(err)),
            },
        };
        s.path = path;
        Ok(s)
    }

    pub fn to_file(&self) -> io::Result<()> {
        let content = serde_yaml::to_string(&self).ok().unwrap();
        fs::write(&self.path, &content)
    }

    pub fn toggle_tree_col(&mut self, col: String) {
        let cols = &mut self.hidden_tree_cols;
        if cols.contains(&col) {
            cols.retain(|c| !c.eq(&col));
        } else {
            cols.push(col);
        }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            hidden_tree_cols: vec!["Id".to_string(), "Type".to_string()],
            window_maximize: false,
            auto_save: true,
            auto_sync: true,
            path: "".to_string(),
        }
    }
}

pub fn show_dialog(lb: &LbApp) -> LbResult<()> {
    let m = &lb.messenger;
    let s = &lb.settings;

    let tabs = gtk::Notebook::new();
    for tab_data in vec![
        ("File Tree", filetree_tab(s, m)),
        ("Window", window_tab(s)),
        ("Editor", editor_tab(s, m)),
    ] {
        let (title, content) = tab_data;
        let tab_btn = gtk::Label::new(Some(title));
        let tab_page = content.upcast::<gtk::Widget>();
        tabs.append_page(&tab_page, Some(&tab_btn));
    }

    let d = lb.gui.new_dialog("Lockbook Settings");
    d.set_default_size(300, 400);
    d.get_content_area().add(&tabs);
    d.add_button("Ok", gtk::ResponseType::Ok);
    d.connect_response(|d, _| d.close());
    d.show_all();
    Ok(())
}

fn filetree_tab(s: &Rc<RefCell<Settings>>, m: &Messenger) -> gtk::Box {
    let chbxs = gtk::Box::new(Vertical, 0);

    for col in FileTreeCol::removable() {
        let ch = gtk::CheckButton::with_label(&col.name());
        ch.set_active(!s.borrow().hidden_tree_cols.contains(&col.name()));
        ch.connect_toggled(glib::clone!(@strong m => move |_| m.send(Msg::ToggleTreeCol(col))));
        chbxs.add(&ch);
    }

    chbxs
}

fn window_tab(s: &Rc<RefCell<Settings>>) -> gtk::Box {
    let ch = gtk::CheckButton::with_label("Maximize on startup");
    ch.set_active(s.borrow().window_maximize);
    ch.connect_toggled(glib::clone!(@strong s => move |chbox| {
        s.borrow_mut().window_maximize = chbox.get_active();
    }));

    let chbxs = gtk::Box::new(Vertical, 0);
    chbxs.add(&ch);
    chbxs
}

fn editor_tab(s: &Rc<RefCell<Settings>>, m: &Messenger) -> gtk::Box {
    let auto_save_ch = gtk::CheckButton::with_label("Auto-save ");
    auto_save_ch.set_active(s.borrow().auto_save);
    auto_save_ch.connect_toggled(glib::clone!(@strong s, @strong m => move |chbox| {
        let auto_save = chbox.get_active();

        s.borrow_mut().auto_save = auto_save;
        m.send(Msg::ToggleAutoSave(auto_save))
    }));

    let auto_sync_ch = gtk::CheckButton::with_label("Auto-sync ");
    auto_sync_ch.set_active(s.borrow().auto_sync);
    auto_sync_ch.connect_toggled(glib::clone!(@strong s, @strong m => move |chbox| {
        let auto_sync = chbox.get_active();

        s.borrow_mut().auto_sync = auto_sync;
        m.send(Msg::ToggleAutoSync(auto_sync))
    }));

    let chbxs = gtk::Box::new(Vertical, 0);
    chbxs.add(&auto_save_ch);
    chbxs.add(&auto_sync_ch);
    chbxs
}
