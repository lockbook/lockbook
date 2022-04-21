use gtk::glib;
use gtk::prelude::*;

impl super::App {
    pub fn create_account(&self, uname: String, url: String) {
        self.onboard.start("Creating account...");

        let (tx, rx) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
        std::thread::spawn({
            let api = self.api.clone();
            let uname = uname.clone();

            move || {
                let result = api.create_account(&uname, &url);
                tx.send(result).unwrap();
            }
        });

        let app = self.clone();
        rx.attach(None, move |create_acct_result| {
            app.onboard.stop("create");

            match create_acct_result {
                Ok(_acct) => app.prompt_backup(),
                Err(err) => app.onboard.handle_create_error(err, &uname),
            }

            glib::Continue(true)
        });
    }

    fn prompt_backup(&self) {
        self.init_account_screen();

        let info = gtk::Dialog::builder()
            .title("Welcome to Lockbook!")
            .transient_for(&self.window)
            .modal(true)
            .build();
        info.set_default_size(350, -1);

        let message = gtk::Label::new(Some(MESSAGE));
        message.set_wrap(true);

        let learn_more_e2e_btn = gtk::LinkButton::builder()
            .uri("https://en.wikipedia.org/wiki/End-to-end_encryption")
            .label("Learn more")
            .build();
        learn_more_e2e_btn.add_css_class("onboard-link");
        let learn_more_e2e = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .build();
        learn_more_e2e.append(&learn_more_e2e_btn);
        learn_more_e2e.append(&gtk::Label::new(Some(" about end-to-end encryption.")));

        let goto_settings = icon_button("security-high", "Backup Now");
        goto_settings.connect_clicked({
            let info = info.clone();
            let app = self.clone();

            move |_| {
                info.close();
                app.open_settings_dialog();
            }
        });

        let goto_account_screen = icon_button("security-low", "Backup Later");
        goto_account_screen.connect_clicked({
            let info = info.clone();
            move |_| info.close()
        });

        let btns = gtk::Grid::builder()
            .column_spacing(4)
            .column_homogeneous(true)
            .build();
        btns.attach(&goto_settings, 0, 0, 1, 1);
        btns.attach(&goto_account_screen, 1, 0, 1, 1);

        let cntr = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(20)
            .halign(gtk::Align::Center)
            .valign(gtk::Align::Center)
            .margin_top(20)
            .margin_bottom(20)
            .margin_start(20)
            .margin_end(20)
            .build();
        cntr.append(&message);
        cntr.append(&learn_more_e2e);
        cntr.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        cntr.append(&btns);

        let info_icon = gtk::Image::from_icon_name("emblem-important");
        info_icon.set_pixel_size(32);

        let info_hbar = gtk::HeaderBar::new();
        info_hbar.pack_start(&info_icon);

        info.set_titlebar(Some(&info_hbar));
        info.content_area().append(&cntr);
        info.present();
        goto_settings.grab_focus();
    }
}

fn icon_button(icon_name: &str, label: &str) -> gtk::Button {
    let icon = gtk::Image::from_icon_name(icon_name);
    icon.set_pixel_size(24);

    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .halign(gtk::Align::Center)
        .margin_top(4)
        .margin_bottom(4)
        .spacing(8)
        .build();
    content.append(&icon);
    content.append(&gtk::Label::new(Some(label)));

    gtk::Button::builder().child(&content).build()
}

static MESSAGE: &str = "Lockbook encrypts your notes with a key that stays on your Lockbook devices. This makes your notes unreadable to everyone except you. However, if you lose this key, your notes are not recoverable. Therefore, we recommend you make a backup in case something happens to this device.";
