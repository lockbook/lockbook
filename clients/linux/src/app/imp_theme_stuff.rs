use gtk::prelude::*;
use sv5::prelude::*;

use crate::ui;

impl super::App {
    pub fn listen_for_theme_changes(self) {
        self.set_source_view_scheme_name();

        gtk::Settings::default()
            .unwrap()
            .connect_gtk_theme_name_notify(move |_| self.set_source_view_scheme_name());
    }

    fn set_source_view_scheme_name(&self) {
        let scheme_name = self.scheme_name();

        if scheme_name != self.account.scheme_name.get() {
            self.account.scheme_name.set(scheme_name);

            if let Some(ref scheme) = sv5::StyleSchemeManager::default().scheme(scheme_name) {
                for i in 0..self.account.tabs.n_pages() {
                    self.account
                        .tabs
                        .nth_page(Some(i))
                        .unwrap()
                        .downcast::<ui::TextEditor>()
                        .unwrap()
                        .editor()
                        .buffer()
                        .downcast::<sv5::Buffer>()
                        .unwrap()
                        .set_style_scheme(Some(scheme));
                }
            }
        }
    }

    fn scheme_name(&self) -> &'static str {
        let ctx = self.window.style_context();
        let fg = ctx.lookup_color("theme_fg_color").unwrap();
        let bg = ctx.lookup_color("theme_bg_color").unwrap();

        // idea: https://lzone.de/blog/Detecting-a-Dark-Theme-in-GTK
        let fg_avg = (fg.red() / 256.0) + (fg.green() / 256.0) + (fg.blue() / 256.0);
        let bg_avg = (bg.red() / 256.0) + (bg.green() / 256.0) + (bg.blue() / 256.0);

        match bg_avg < fg_avg {
            true => "classic-dark",
            false => "classic",
        }
    }
}
