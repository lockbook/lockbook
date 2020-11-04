use std::collections::HashMap;

use glib::clone;
use gtk::prelude::*;
use gtk::{
    AccelGroup as GtkAccelGroup, Menu as GtkMenu, MenuBar as GtkMenuBar, MenuItem as GtkMenuItem,
    SeparatorMenuItem as GtkSeparatorMenuItem,
};

use crate::editmode::EditMode;
use crate::messages::{Messenger, Msg};

// menu_set! clears out the existing submenu (or creates and sets a new one if none exists) and
// append the given items.
//
// There are two more concise methods that did not work:
// 1) Simply creating and setting a new submenu does not work because the widgets in the existing
//    submenu cannot belong to two parents at the same time.
// 2) Setting the submenu to None, then creating and setting a new submenu resulted in a segfault
//    that I could not get to the bottom of.
macro_rules! menu_set {
    ($menu:expr, $item_map:expr, $( $items:expr ),*) => {
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
            match $items {
                Items::Separator => submenu.append(&GtkSeparatorMenuItem::new()),
                _ => submenu.append($item_map.get($items).unwrap()),
            }
        )*
        $menu.show_all();
    };
}

pub struct Menubar {
    items: HashMap<Items, GtkMenuItem>,
    file: GtkMenuItem,
    edit: GtkMenuItem,
    acct: GtkMenuItem,
    pub cntr: GtkMenuBar,
}

impl Menubar {
    pub fn new(m: &Messenger, accels: &GtkAccelGroup) -> Self {
        let file = GtkMenuItem::with_label("File");
        let edit = GtkMenuItem::with_label("Edit");
        let acct = GtkMenuItem::with_label("Account");

        let cntr = GtkMenuBar::new();
        for menu in &[&file, &edit, &acct] {
            cntr.append(*menu);
        }

        Self {
            items: Items::hashmap(&m, &accels),
            file,
            edit,
            acct,
            cntr,
        }
    }

    pub fn set(&self, mode: &EditMode) {
        match mode {
            EditMode::Folder {
                path: _,
                meta: _,
                n_children: _,
            } => {
                menu_set!(
                    self.file,
                    self.items,
                    &Items::FileNew,
                    &Items::FileOpen,
                    &Items::Separator,
                    &Items::FileClose,
                    &Items::Separator,
                    &Items::FileQuit
                );
            }
            EditMode::PlainText {
                meta: _,
                content: _,
            } => {
                menu_set!(
                    self.file,
                    self.items,
                    &Items::FileNew,
                    &Items::FileOpen,
                    &Items::Separator,
                    &Items::FileSave,
                    &Items::FileClose,
                    &Items::Separator,
                    &Items::FileQuit
                );
            }
            EditMode::None => {
                menu_set!(
                    self.file,
                    self.items,
                    &Items::FileNew,
                    &Items::FileOpen,
                    &Items::Separator,
                    &Items::FileQuit
                );
                menu_set!(self.edit, self.items, &Items::EditPreferences);
                menu_set!(self.acct, self.items, &Items::AccountExport);
            }
        }
    }
}

macro_rules! menuitem_accel {
    ($txt:literal, $accel:literal, $accelgrp:expr, $action:expr) => {
        let (key, modifier) = gtk::accelerator_parse($accel);

        let mi = GtkMenuItem::with_label($txt);
        mi.add_accelerator(
            "activate",
            $accelgrp,
            key,
            modifier,
            gtk::AccelFlags::VISIBLE,
        );
        mi.connect_activate($action);
        return mi;
    };
}

#[derive(Hash, Eq, PartialEq, Debug)]
enum Items {
    FileNew,
    FileOpen,
    FileSave,
    FileClose,
    FileQuit,

    EditPreferences,

    AccountExport,

    Separator,
}

impl Items {
    fn hashmap(m: &Messenger, accels: &GtkAccelGroup) -> HashMap<Self, GtkMenuItem> {
        let mut items = HashMap::new();
        for item in Items::all() {
            let make = item.constructor();
            items.insert(item, make(&m, &accels));
        }
        items
    }

    fn all() -> Vec<Self> {
        vec![
            Self::FileNew,
            Self::FileOpen,
            Self::FileSave,
            Self::FileClose,
            Self::FileQuit,
            Self::EditPreferences,
            Self::AccountExport,
        ]
    }

    fn constructor(&self) -> fn(&Messenger, &GtkAccelGroup) -> GtkMenuItem {
        match self {
            Items::FileNew => Self::file_new,
            Items::FileOpen => Self::file_open,
            Items::FileSave => Self::file_save,
            Items::FileClose => Self::file_close,
            Items::FileQuit => Self::file_quit,

            Items::EditPreferences => Self::edit_preferences,

            Items::AccountExport => Self::acct_export,

            _ => panic!("Trying to make '{:?}' menu item", self),
        }
    }

    fn file_new(m: &Messenger, accels: &GtkAccelGroup) -> GtkMenuItem {
        menuitem_accel!(
            "New",
            "<Primary>N",
            accels,
            clone!(@strong m => move |_| {
                m.send(Msg::ShowDialogNew);
            })
        );
    }

    fn file_open(m: &Messenger, accels: &GtkAccelGroup) -> GtkMenuItem {
        menuitem_accel!(
            "Open",
            "<Primary>O",
            accels,
            clone!(@strong m => move |_| {
                m.send(Msg::ShowDialogOpen);
            })
        );
    }

    fn file_save(m: &Messenger, accels: &GtkAccelGroup) -> GtkMenuItem {
        menuitem_accel!(
            "Save",
            "<Primary>S",
            accels,
            clone!(@strong m => move |_| {
                m.send(Msg::SaveFile);
            })
        );
    }

    fn file_close(m: &Messenger, accels: &GtkAccelGroup) -> GtkMenuItem {
        menuitem_accel!(
            "Close File",
            "<Primary>W",
            accels,
            clone!(@strong m => move |_| {
                m.send(Msg::CloseFile);
            })
        );
    }

    fn file_quit(m: &Messenger, _: &GtkAccelGroup) -> GtkMenuItem {
        let mi = GtkMenuItem::with_label("Quit");
        mi.connect_activate(clone!(@strong m => move |_| {
            m.send(Msg::Quit);
        }));
        mi
    }

    fn edit_preferences(m: &Messenger, _: &GtkAccelGroup) -> GtkMenuItem {
        let mi = GtkMenuItem::with_label("Preferences");
        mi.connect_activate(clone!(@strong m => move |_| {
            m.send(Msg::ShowDialogPreferences);
        }));
        mi
    }

    fn acct_export(m: &Messenger, accels: &GtkAccelGroup) -> GtkMenuItem {
        menuitem_accel!(
            "Export...",
            "<Primary>E",
            accels,
            clone!(@strong m => move |_| m.send(Msg::ExportAccount))
        );
    }
}
