use std::sync::Arc;

use gtk::prelude::*;

fn main() {
    let gtk_app = lockbook_desktop_gtk::new_gtk_app(Arc::new(lb::DefaultApi::default()));
    gtk_app.run();
}
