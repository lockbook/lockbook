use std::collections::HashMap;

use gtk::prelude::*;
use gtk::Menu as GtkMenu;
use gtk::MenuBar as GtkMenuBar;
use gtk::MenuItem as GtkMenuItem;
use gtk::SeparatorMenuItem as GtkSeparatorMenuItem;
use gtk::{AccelGroup as GtkAccelGroup, IconSize, Image, Label};

use crate::editmode::EditMode;
use crate::messages::{Messenger, Msg};

// menu_set! clears out the existing submenu (or creates and sets a new one if none exists) and
// appends the given items.
//
// There are two more concise methods that did not work:
// 1) Simply creating and setting a new submenu does not work because the widgets in the existing
//    submenu cannot belong to two parents at the same time.
// 2) Setting the submenu to None, then creating and setting a new submenu resulted in a segfault
//    that I could not get to the bottom of.
macro_rules! menu_set {
    ($menu:expr, $item_map:expr, $( $items:ident ),*) => {
        let submenu = {
            if let Some(m) = $menu.get_submenu() {
                let m = m.downcast::<GtkMenu>().unwrap();
                m.foreach(|child| m.remove(child));
                m
            } else {
                let m = GtkMenu::new();
                $menu.set_submenu(Some(&m));
                m
            }
        };
        $(
            match Item::$items {
                Item::Separator => submenu.append(&GtkSeparatorMenuItem::new()),
                _ => submenu.append($item_map.get(&Item::$items).unwrap()),
            }
        )*
        $menu.show_all();
    };
}

pub struct Menubar {
    items: HashMap<Item, GtkMenuItem>,
    file: GtkMenuItem,
    edit: GtkMenuItem,
    acct: GtkMenuItem,
    help: GtkMenuItem,
    mbar: GtkMenuBar,
}

impl Menubar {
    pub fn new(m: &Messenger, accels: &GtkAccelGroup) -> Self {
        let items = Item::hashmap(&m, &accels);

        let file = GtkMenuItem::with_label("File");
        let edit = GtkMenuItem::with_label("Edit");
        let acct = GtkMenuItem::with_label("Account");
        let help = GtkMenuItem::with_label("Help");

        let mbar = GtkMenuBar::new();
        for menu in &[&file, &edit, &acct, &help] {
            mbar.append(*menu);
        }

        menu_set!(help, items, HelpAbout);

        Self {
            items,
            file,
            edit,
            acct,
            help,
            mbar,
        }
    }

    pub fn widget(&self) -> &GtkMenuBar {
        &self.mbar
    }

    pub fn set(&self, mode: &EditMode) {
        match mode {
            EditMode::Folder {
                path: _,
                meta: _,
                n_children: _,
            } => {
                menu_set!(
                    self.file, self.items, FileOpen, Separator, FileClose, Separator, FileQuit
                );
            }
            EditMode::PlainText {
                path: _,
                meta: _,
                content: _,
            } => {
                menu_set!(
                    self.file, self.items, FileOpen, Separator, FileSave, FileClose, Separator,
                    FileQuit
                );
            }
            EditMode::None => {
                menu_set!(self.file, self.items, FileOpen, FileQuit);
                menu_set!(self.edit, self.items, EditPreferences);
                menu_set!(
                    self.acct,
                    self.items,
                    AccountSync,
                    AccountUsage,
                    AccountExport
                );
            }
        }
    }

    pub fn for_intro_screen(&self) {
        self.mbar.foreach(|w| {
            if *w == self.file || *w == self.edit || *w == self.acct {
                self.mbar.remove(w);
            }
        });
    }

    pub fn for_account_screen(&self) {
        self.mbar.foreach(|w| self.mbar.remove(w));
        for menu in &[&self.file, &self.edit, &self.acct, &self.help] {
            self.mbar.append(*menu);
        }
    }
}

// Each menu Item has a name and optional accelerator, as well as a Msg it sends when activated.
type ItemName = &'static str;
type ItemAccel = &'static str;
type ItemData = (ItemName, ItemAccel, fn() -> Msg);

#[derive(Hash, Eq, PartialEq, Debug)]
enum Item {
    FileOpen,
    FileSave,
    FileClose,
    FileQuit,

    EditPreferences,

    AccountSync,
    AccountUsage,
    AccountExport,

    HelpAbout,

    Separator,
}

impl Item {
    fn hashmap(m: &Messenger, accels: &GtkAccelGroup) -> HashMap<Self, GtkMenuItem> {
        let mut items = HashMap::new();
        for (item_key, icon_name, (name, accel, msg)) in Self::data() {
            let mi = match icon_name {
                None => GtkMenuItem::with_label(name),
                Some(_) => {
                    let cntr = gtk::Box::new(gtk::Orientation::Horizontal, 0);
                    cntr.pack_start(
                        &Image::from_icon_name(icon_name.as_deref(), IconSize::Menu),
                        false,
                        false,
                        0,
                    );
                    cntr.pack_start(&Label::new(Some(name)), false, false, 10);

                    let mi = GtkMenuItem::new();
                    mi.add(&cntr);
                    mi
                }
            };

            if !accel.is_empty() {
                let (key, modifier) = gtk::accelerator_parse(accel);
                mi.add_accelerator("activate", accels, key, modifier, gtk::AccelFlags::VISIBLE);
            }

            let m = m.clone();
            mi.connect_activate(move |_| m.send(msg()));

            items.insert(item_key, mi);
        }
        items
    }

    #[rustfmt::skip]
    fn data() -> Vec<(Self, Option<String>, ItemData)> {
        vec![
            (Self::FileOpen, Some("document-open".to_string()), ("Open", "<Primary>L", || Msg::SearchFieldFocus)),
            (Self::FileSave, Some("document-save".to_string()), ("Save", "<Primary>S", || Msg::SaveFile)),
            (Self::FileClose, Some("window-close".to_string()), ("Close File", "<Primary>W", || Msg::CloseFile)),
            (Self::FileQuit, Some("application-exit".to_string()), ("Quit", "", || Msg::Quit)),
            (Self::EditPreferences, Some("preferences-desktop".to_string()), ("Preferences", "", || Msg::ShowDialogPreferences)),
            (Self::AccountSync, None, ("Sync", "", || Msg::PerformSync)),
            (Self::AccountUsage, Some("network-receive".to_string()), ("Usage", "", || Msg::ShowDialogUsage)),
            (Self::AccountExport, Some("emblem-shared".to_string()), ("Export", "", || Msg::ExportAccount)),
            (Self::HelpAbout, Some("help-about".to_string()), ("About", "", || Msg::ShowDialogAbout)),
        ]
    }
}
