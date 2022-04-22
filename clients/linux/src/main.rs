use std::sync::Arc;

use gtk::prelude::*;

fn main() {
    let api = match lb::DefaultApi::new() {
        Ok(api) => Arc::new(api),
        Err(err) => panic!("{}", err),
    };
    let gtk_app = lockbook_desktop_gtk::new_gtk_app(api);
    gtk_app.run();
}
