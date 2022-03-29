use gtk::prelude::*;

pub fn open(win: &gtk::ApplicationWindow) {
    let logo_pixbuf = gdk_pixbuf::Pixbuf::from_read(LOGO_DATA).unwrap();
    let logo = gtk::Picture::for_pixbuf(&logo_pixbuf);

    gtk::AboutDialog::builder()
        .transient_for(win)
        .modal(true)
        .program_name("Lockbook")
        .version(env!("CARGO_PKG_VERSION"))
        .website("https://lockbook.net")
        .authors(vec!["The Lockbook Team".to_string()])
        .license(LICENSE)
        .comments(COMMENTS)
        .logo(&logo.paintable().unwrap())
        .build()
        .show();
}

static LICENSE: &str = include_str!("../../UNLICENSE");
static COMMENTS: &str = "Lockbook is a document editor that is secure, minimal, private, open source, and cross-platform.";
static LOGO_DATA: &[u8] = include_bytes!("../../lockbook.png");
