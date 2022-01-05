use std::collections::HashMap;

use gtk::prelude::*;
use gtk::AccelGroup as GtkAccelGroup;
use gtk::Menu as GtkMenu;
use gtk::MenuBar as GtkMenuBar;
use gtk::MenuItem as GtkMenuItem;
use gtk::SeparatorMenuItem as GtkSeparatorMenuItem;

use crate::editmode::EditMode;
use crate::messages::{Messenger, Msg};

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
        let items = Item::hashmap(m, accels);

        let file = GtkMenuItem::with_label("File");
        let edit = GtkMenuItem::with_label("Edit");
        let acct = GtkMenuItem::with_label("Account");
        let help = GtkMenuItem::with_label("Help");

        let mbar = GtkMenuBar::new();
        for menu in &[&file, &edit, &acct, &help] {
            mbar.append(*menu);
        }

        let lb_mbar = Self {
            items,
            file,
            edit,
            acct,
            help,
            mbar,
        };
        lb_mbar.set_menu(&lb_mbar.help, &[Item::HelpAbout]);
        lb_mbar
    }

    pub fn widget(&self) -> &GtkMenuBar {
        &self.mbar
    }

    pub fn set(&self, mode: &EditMode) {
        match mode {
            EditMode::Folder {
                meta: _,
                n_children: _,
            } => {
                self.set_menu(
                    &self.file,
                    &[
                        Item::FileOpen,
                        Item::Separator,
                        Item::FileClose,
                        Item::Separator,
                        Item::FileQuit,
                    ],
                );
            }
            EditMode::PlainText {
                meta: _,
                content: _,
            } => {
                self.set_menu(
                    &self.file,
                    &[
                        Item::FileOpen,
                        Item::Separator,
                        Item::FileSave,
                        Item::FileClose,
                        Item::Separator,
                        Item::FileQuit,
                    ],
                );
            }
            EditMode::None => {
                self.set_menu(&self.file, &[Item::FileOpen, Item::FileQuit]);
                self.set_menu(&self.edit, &[Item::EditPreferences]);
                self.set_menu(
                    &self.acct,
                    &[Item::AccountSync, Item::AccountUsage, Item::AccountExport],
                );
            }
        }
    }

    pub fn for_onboarding_screen(&self) {
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

    // set_menu clears out the existing submenu (or creates and sets a new one if none exists) and
    // appends the given items.
    //
    // There are two more concise methods that did not work:
    // 1) Simply creating and setting a new submenu does not work because the widgets in the existing
    //    submenu cannot belong to two parents at the same time.
    // 2) Setting the submenu to None, then creating and setting a new submenu resulted in a segfault
    //    that I could not get to the bottom of.
    fn set_menu(&self, menu: &GtkMenuItem, items: &[Item]) {
        let submenu = {
            if let Some(m) = menu.get_submenu() {
                let m = m.downcast::<GtkMenu>().unwrap();
                m.foreach(|child| m.remove(child));
                m
            } else {
                let m = GtkMenu::new();
                menu.set_submenu(Some(&m));
                m
            }
        };
        for item in items {
            match item {
                Item::Separator => submenu.append(&GtkSeparatorMenuItem::new()),
                _ => submenu.append(self.items.get(item).unwrap()),
            }
        }
        menu.show_all();
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
        for (item_key, (name, accel, msg)) in Self::data() {
            let mi = GtkMenuItem::with_label(name);

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
    fn data() -> Vec<(Self, ItemData)> {
        vec![
            (Self::FileOpen, ("Open", "<Primary>L", || Msg::PromptSearch)),
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
