use gtk::glib;

impl super::App {
    pub fn create_account(&self, uname: String, url: String) {
        self.onboard.start("Creating account...");

        let (tx, rx) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
        {
            let api = self.api.clone();
            let uname = uname.clone();
            std::thread::spawn(move || {
                let result = api.create_account(&uname, &url);
                tx.send(result).unwrap();
            });
        }

        let app = self.clone();
        rx.attach(None, move |create_acct_result| {
            app.onboard.stop("create");

            match create_acct_result {
                Ok(_acct) => app.init_account_screen(),
                Err(err) => app.onboard.handle_create_error(err, &uname),
            }

            glib::Continue(true)
        });
    }
}
