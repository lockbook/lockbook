mod app;
mod bg;
mod lbutil;
mod settings;
mod ui;

use std::sync::Arc;

use gtk::prelude::*;

fn main() {
    gtk::init().expect("unable to initialize gtk");
    sv5::init();

    let writeable_path = format!("{}/linux", lbutil::data_dir());

    let cfg = lb::Config { logs: true, colored_logs: true, writeable_path };
    let core = match lb::Core::init(&cfg) {
        Ok(core) => Arc::new(core),
        Err(err) => panic!("unable to init core: {}", err.0),
    };

    let a = gtk::Application::new(None, Default::default());
    a.connect_startup(|_| {
        // Load the CSS on startup.
        let provider = gtk::CssProvider::new();
        provider.load_from_data(include_bytes!("../style.css"));
        gtk::StyleContext::add_provider_for_display(
            &gtk::gdk::Display::default().expect("Could not connect to a display."),
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    });
    a.connect_activate(move |a| app::App::activate(core.clone(), a));
    a.run();
}
