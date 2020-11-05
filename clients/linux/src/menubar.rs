use std::collections::HashMap;

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
                Item::Separator => submenu.append(&GtkSeparatorMenuItem::new()),
                _ => submenu.append($item_map.get($items).unwrap()),
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
    pub cntr: GtkMenuBar,
}

impl Menubar {
    pub fn new(m: &Messenger, accels: &GtkAccelGroup) -> Self {
        let items = Item::hashmap(&m, &accels);

        let file = GtkMenuItem::with_label("File");
        let edit = GtkMenuItem::with_label("Edit");
        let acct = GtkMenuItem::with_label("Account");
        let help = GtkMenuItem::with_label("Help");

        let cntr = GtkMenuBar::new();
        for menu in &[&file, &edit, &acct, &help] {
            cntr.append(*menu);
        }

        menu_set!(help, items, &Item::HelpAbout);

        Self {
            items,
            file,
            edit,
            acct,
            help,
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
                    &Item::FileNew,
                    &Item::FileOpen,
                    &Item::Separator,
                    &Item::FileClose,
                    &Item::Separator,
                    &Item::FileQuit
                );
            }
            EditMode::PlainText {
                meta: _,
                content: _,
            } => {
                menu_set!(
                    self.file,
                    self.items,
                    &Item::FileNew,
                    &Item::FileOpen,
                    &Item::Separator,
                    &Item::FileSave,
                    &Item::FileClose,
                    &Item::Separator,
                    &Item::FileQuit
                );
            }
            EditMode::None => {
                menu_set!(
                    self.file,
                    self.items,
                    &Item::FileNew,
                    &Item::FileOpen,
                    &Item::FileQuit
                );
                menu_set!(self.edit, self.items, &Item::EditPreferences);
                menu_set!(
                    self.acct,
                    self.items,
                    &Item::AccountSync,
                    &Item::AccountUsage,
                    &Item::AccountExport
                );
            }
        }
    }

    pub fn for_intro_screen(&self) {
        self.cntr.foreach(|w| {
            if *w == self.file || *w == self.edit || *w == self.acct {
                self.cntr.remove(w);
            }
        });
    }

    pub fn for_account_screen(&self) {
        self.cntr.foreach(|w| self.cntr.remove(w));
        for menu in &[&self.file, &self.edit, &self.acct, &self.help] {
            self.cntr.append(*menu);
        }
    }
}

// Each menu Item has a name and accelertor, as well as a certain Msg it sends when clicked. In
// order to avoid having to implement Clone and (more tediously) Copy on the Msg enum, a function
// is used to pass the Item's particular Msg for initialization.
type ItemData = (&'static str, &'static str, fn() -> Msg);

#[derive(Hash, Eq, PartialEq, Debug)]
enum Item {
    FileNew,
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
        for (item_key, (name, accel, msg)) in Self::data() {
            let mi = GtkMenuItem::with_label(name);

            if accel != "" {
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
    fn data() -> Vec<(Self, ItemData)> {
        vec![
            (Self::FileNew, ("New", "<Primary>N", || Msg::ShowDialogNew)),
            (Self::FileOpen, ("Open", "<Primary>O", || Msg::ShowDialogOpen)),
            (Self::FileSave, ("Save", "<Primary>S", || Msg::SaveFile)),
            (Self::FileClose, ("Close File", "<Primary>W", || Msg::CloseFile)),
            (Self::FileQuit, ("Quit", "", || Msg::Quit)),
            (Self::EditPreferences, ("Preferences", "", || Msg::ShowDialogPreferences)),
            (Self::AccountSync, ("Sync", "", || Msg::PerformSync)),
            (Self::AccountUsage, ("Usage", "", || Msg::ShowDialogUsage)),
            (Self::AccountExport, ("Export", "", || Msg::ExportAccount)),
            (Self::HelpAbout, ("About", "", || Msg::ShowDialogAbout)),
        ]
    }
}
