mod account_screen;
mod onboard_screen;

mod filetree;
mod menu_item;
mod sync_panel;
mod text_editor;

pub use account_screen::AccountOp;
pub use account_screen::AccountScreen;
pub use onboard_screen::OnboardOp;
pub use onboard_screen::OnboardScreen;

pub use filetree::FileTree;
pub use filetree::FileTreeCol;
pub use menu_item::menu_separator;
pub use menu_item::MenuItemBuilder;
pub use sync_panel::SyncPanel;
pub use text_editor::TextEditor;

pub mod about_dialog;
pub mod header_bar;

use gtk::glib;
use gtk::prelude::*;

pub fn id_from_tpath(model: &impl IsA<gtk::TreeModel>, tpath: &gtk::TreePath) -> lb::Uuid {
    let col = filetree::FileTreeCol::Id.as_tree_store_index();
    let iter = model.iter(tpath).unwrap();
    let iter_id = model
        .get_value(&iter, col)
        .get::<String>()
        .unwrap_or_else(|_| panic!("getting treeview string for uuid: column id {}", col));
    lb::Uuid::parse_str(&iter_id).unwrap()
}

pub fn clipboard_btn(label: &str, to_copy: &str) -> gtk::Button {
    let btn = gtk::Button::with_label(label);
    let to_copy = to_copy.to_string();
    let label = label.to_string();
    btn.connect_clicked(move |btn| {
        gtk::gdk::Display::default()
            .unwrap()
            .clipboard()
            .set_text(&to_copy);
        btn.set_label("Copied!");

        let btn = btn.clone();
        let label = label.clone();
        glib::timeout_add_seconds_local(3, move || {
            btn.set_label(&label);
            glib::Continue(false)
        });
    });
    btn
}

pub fn unexpected_error(msg: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(msg)
        .margin_top(20)
        .margin_bottom(20)
        .margin_start(20)
        .margin_end(20)
        .build()
}

pub fn document_icon_from_name(fname: &str) -> String {
    let image_suffixes =
        vec![".jpg", ".jpeg", ".png", ".pnm", ".tga", ".farbfeld", ".bmp", ".draw"];
    let script_suffixes = vec![".sh", ".bash", ".zsh"];
    if image_suffixes.iter().any(|suffix| fname.ends_with(suffix)) {
        "image-x-generic".to_string()
    } else if script_suffixes.iter().any(|suffix| fname.ends_with(suffix)) {
        "text-x-script".to_string()
    } else {
        "text-x-generic".to_string()
    }
}

pub mod icons {
    pub const ABOUT: &str = "help-about-symbolic";
    pub const ACCOUNT: &str = "avatar-default-symbolic";
    pub const APP: &str = "video-display-symbolic";
    pub const CHECK_MARK: &str = "emblem-ok-symbolic";
    pub const DELETE: &str = "edit-delete-symbolic";
    pub const ERROR_RED: &str = "dialog-error-symbolic";
    pub const EXPORT: &str = "document-save-symbolic";
    pub const NEW_DOC: &str = "document-new-symbolic";
    pub const NEW_FOLDER: &str = "folder-new-symbolic";
    pub const RENAME: &str = "go-jump-symbolic";
    //pub const SEARCH: &str = "system-search-symbolic";
    pub const SETTINGS: &str = "preferences-system-symbolic";
    pub const SYNC: &str = "emblem-synchronizing-symbolic";
    pub const USAGE: &str = "utilities-system-monitor-symbolic";
}
