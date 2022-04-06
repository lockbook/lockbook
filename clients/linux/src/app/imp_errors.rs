use gtk::prelude::*;

use crate::ui::icons;

impl super::App {
    pub fn show_err_dialog(&self, txt: &str) {
        let err_lbl = gtk::Label::builder()
            .label(txt)
            .name("err")
            .halign(gtk::Align::Start)
            .margin_top(16)
            .margin_bottom(16)
            .margin_start(16)
            .margin_end(16)
            .build();

        let err_icon = gtk::Image::from_icon_name(icons::ERROR_RED);
        err_icon.set_pixel_size(24);

        let title = gtk::Label::new(None);
        title.set_markup("<b>Error</b>");

        let titlebar = gtk::HeaderBar::new();
        titlebar.set_title_widget(Some(&title));
        titlebar.pack_start(&err_icon);

        let d = gtk::Dialog::builder()
            .transient_for(&self.window)
            .default_width(300)
            .titlebar(&titlebar)
            .modal(true)
            .build();

        d.content_area().append(&err_lbl);
        d.show();
    }
}
