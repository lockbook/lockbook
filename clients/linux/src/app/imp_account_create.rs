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
        message.set_margin_bottom(16);

        let goto_settings = gtk::Button::with_label("Backup Now");
        goto_settings.connect_clicked({
            let info = info.clone();
            let app = self.clone();

            move |_| {
                info.close();
                app.open_settings_dialog();
            }
        });

        let goto_account_screen = gtk::Button::with_label("Skip and Backup Later");
        goto_account_screen.connect_clicked({
            let info = info.clone();
            move |_| info.close()
        });

        let btns = gtk::Grid::builder()
            .column_spacing(4)
            .column_homogeneous(true)
            .margin_top(16)
            .build();
        btns.attach(&goto_settings, 0, 0, 1, 1);
        btns.attach(&goto_account_screen, 1, 0, 1, 1);

        let cntr = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .halign(gtk::Align::Center)
            .valign(gtk::Align::Center)
            .margin_top(16)
            .margin_bottom(16)
            .margin_start(16)
            .margin_end(16)
            .build();
        cntr.append(&message);
        cntr.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        cntr.append(&btns);

        info.content_area().append(&cntr);
        info.present();
    }
}

static MESSAGE: &str = "Lockbook encrypts your notes with a key that stays on your Lockbook devices. This makes your notes unreadable to everyone except you. However, if you lose this key, your notes are not recoverable. Therefore, we recommend you make a backup in case something happens to this device.";
