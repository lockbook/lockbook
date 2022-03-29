use gtk::prelude::*;

impl super::App {
    pub fn show_err_dialog(&self, txt: &str) {
        let err_lbl = gtk::Label::builder()
            .label(txt)
            .name("err")
            .halign(gtk::Align::Start)
            .margin_top(16)
            .margin_bottom(8)
            .build();
        let d = gtk::Dialog::builder()
            .transient_for(&self.window)
            .default_width(300)
            .modal(true)
            .build();
        d.content_area().append(&err_lbl);
        d.show();
    }
}
