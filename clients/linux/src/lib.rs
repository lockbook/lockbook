mod app;
mod bg;
mod lbutil;
mod settings;
mod ui;

use std::sync::Arc;

use gtk::prelude::*;

pub fn new_gtk_app(api: Arc<dyn lb::Api>) -> gtk::Application {
    gtk::init().expect("unable to initialize gtk");
    sv5::init();

    let a = gtk::Application::new(None, Default::default());
    a.connect_startup(|_| load_css());
    a.connect_activate(move |a| app::App::activate(api.clone(), a));
    a
}

fn load_css() {
    // Load the CSS file and add it to the provider
    let provider = gtk::CssProvider::new();
    provider.load_from_data(include_bytes!("../style.css"));
    // Add the provider to the default screen
    gtk::StyleContext::add_provider_for_display(
        &gtk::gdk::Display::default().expect("Could not connect to a display."),
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}
