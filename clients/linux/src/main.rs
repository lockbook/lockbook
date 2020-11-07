extern crate gdk;
extern crate gdk_pixbuf;
extern crate gio;
extern crate glib;
extern crate gtk;
extern crate pango;

mod account;
mod backend;
mod editmode;
mod filetree;
mod gui;
mod intro;
mod menubar;
mod messages;
mod settings;

use std::env;
use std::sync::Arc;

use crate::backend::LbCore;
use crate::gui::run_gtk;
use crate::settings::Settings;

fn lockbook_path() -> String {
    match env::var("LOCKBOOK_PATH") {
        Ok(path) => path,
        Err(_) => format!("{}/.lockbook", env::var("HOME").unwrap()),
    }
}

fn main() {
    let datadir = lockbook_path();

    let core = LbCore::new(&datadir);
    match core.init_db() {
        Ok(_) => {}
        Err(err) => panic!("{}", err),
    }

    let settings_file = format!("{}/settings.yaml", datadir);
    let sr = Settings::new_rc(match Settings::from_file(&settings_file) {
        Ok(s) => s,
        Err(err) => panic!("unable to read settings: {}", err),
    });

    run_gtk(sr, Arc::new(core));
}
