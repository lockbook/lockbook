extern crate gdk;
extern crate gdk_pixbuf;
extern crate gio;
extern crate glib;
extern crate gtk;
extern crate pango;
extern crate qrcode_generator;
extern crate sourceview;

mod account;
mod app;
mod auto_save;
mod backend;
mod editmode;
mod error;
mod filetree;
mod intro;
mod menubar;
mod messages;
mod settings;
mod util;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use gio::prelude::*;
use gtk::prelude::*;
use gtk::Application as GtkApp;
use gtk::CssProvider as GtkCssProvider;
use gtk::Dialog as GtkDialog;
use gtk::Label as GtkLabel;
use gtk::StyleContext as GtkStyleContext;

use crate::app::LbApp;
use crate::backend::LbCore;
use crate::settings::Settings;

fn main() {
    gtk::init().unwrap();

    let data_dir = get_data_dir();

    let core = match LbCore::new(&data_dir) {
        Ok(c) => Arc::new(c),
        Err(err) => launch_err("initializing db", &err.msg()),
    };

    let settings = match Settings::from_data_dir(&data_dir) {
        Ok(s) => Rc::new(RefCell::new(s)),
        Err(err) => launch_err("unable to read settings", &err.to_string()),
    };

    let gtk_app = GtkApp::new(None, Default::default()).unwrap();
    gtk_app.connect_activate(closure!(core, settings => move |app| {
        if let Err(err) = gtk_add_css_provider() {
            launch_err("adding css provider", &err);
        }

        let lb = LbApp::new(&core, &settings, &app);
        if let Err(err) = lb.show() {
            launch_err("displaying app", &err.msg());
        }
    }));
    gtk_app.connect_shutdown(closure!(settings => move |_| {
        match settings.borrow_mut().to_file() {
            Ok(_) => println!("bye!"),
            Err(err) => println!("error: {:?}", err),
        }
    }));
    gtk_app.run(&[]);
}

fn get_data_dir() -> String {
    let default = format!("{}/.lockbook", std::env::var("HOME").unwrap());
    std::env::var("LOCKBOOK_PATH").unwrap_or(default)
}

fn gtk_add_css_provider() -> Result<(), String> {
    let styling = include_bytes!("../res/app.css");
    let provider = GtkCssProvider::new();
    if let Err(err) = provider.load_from_data(styling) {
        return Err(format!("loading styling css: {}", err));
    }

    if let Some(screen) = gdk::Screen::get_default() {
        let priority = gtk::STYLE_PROVIDER_PRIORITY_APPLICATION;
        GtkStyleContext::add_provider_for_screen(&screen, &provider, priority);
        Ok(())
    } else {
        Err("no gdk default screen found".to_string())
    }
}

fn launch_err(prefix: &str, err: &str) -> ! {
    let lbl = GtkLabel::new(Some(&format!("error: {}: {}", prefix, err)));
    lbl.set_margin_top(20);
    lbl.set_margin_bottom(20);
    lbl.set_margin_start(20);
    lbl.set_margin_end(20);

    let d = GtkDialog::new();
    d.set_title("Lockbook Launch Error");
    d.set_icon_name(Some("emblem-important"));
    d.get_content_area().add(&lbl);
    d.show_all();
    d.run();

    std::process::exit(1);
}
