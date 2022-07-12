use gtk::glib;

use crate::ui;

impl super::App {
    pub fn import_account(&self, acct_str: String) {
        self.onboard.start(ui::OnboardRoute::Import);

        // Create a channel to receive and process the result of importing the account.
        let (tx, rx) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);

        // In a separate thread, import the account and send the result down the channel.
        let core = self.core.clone();
        std::thread::spawn(move || {
            tx.send(core.import_account(&acct_str)).unwrap();
        });

        // If there is any error, it's shown on the import screen.
        // Otherwise, account syncing will start.
        let app = self.clone();
        rx.attach(None, move |result| {
            match result {
                Ok(_acct) => app.sync_for_import(),
                Err(err) => app.onboard.handle_import_error(err),
            }
            glib::Continue(false)
        });
    }

    fn sync_for_import(&self) {
        // Create a channel to receive and process any account sync progress updates.
        let (tx, rx) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);

        // In a separate thread, start syncing the account. Pass the sync channel which will be
        // used to receive progress updates as indicated above.
        let core = self.core.clone();
        std::thread::spawn(move || {
            let closure = {
                let tx = tx.clone();
                move |msg| tx.send(lb::SyncProgressReport::Update(msg)).unwrap()
            };
            let result = core
                .sync(Some(Box::new(closure)))
                .map_err(lb::SyncError::from);
            tx.send(lb::SyncProgressReport::Done(result)).unwrap();
        });

        let app = self.clone();
        rx.attach(None, move |pr: lb::SyncProgressReport| {
            match pr {
                lb::SyncProgressReport::Update(msg) => set_import_sync_progress(&app.onboard, &msg),
                lb::SyncProgressReport::Done(result) => {
                    app.onboard.stop(ui::OnboardRoute::Import);
                    match result {
                        Ok(_) => app.init_account_screen(),
                        Err(err) => import_sync_done_with_err(&app, err),
                    }
                }
            }
            glib::Continue(true)
        });
    }
}

fn set_import_sync_progress(onboard: &ui::OnboardScreen, sp: &lb::SyncProgress) {
    let name = match &sp.current_work_unit {
        lb::ClientWorkUnit::PullMetadata => "file tree updates".to_string(),
        lb::ClientWorkUnit::PushMetadata => "file tree updates".to_string(),
        lb::ClientWorkUnit::PullDocument(name) => name.clone(),
        lb::ClientWorkUnit::PushDocument(name) => name.clone(),
    };
    let caption = format!("syncing :: {} ({}/{})", name, sp.progress, sp.total);
    onboard.status.caption.set_text(&caption);
}

fn import_sync_done_with_err(app: &super::App, err: lb::SyncError) {
    match err {
        lb::SyncError::Minor(msg) => app.onboard.set_import_err_msg(&msg),
        lb::SyncError::Major(msg) => eprintln!("{}", msg), //todo: show dialog or something
    }
}
