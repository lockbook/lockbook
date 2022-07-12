use gtk::glib;

impl super::App {
    pub fn update_sync_status(&self) {
        self.account
            .sync
            .set_status(Ok("Updating status...".to_string()));

        let (tx, rx) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);

        let core = self.core.clone();
        std::thread::spawn(move || {
            let result = sync_status(&core);
            tx.send(result).unwrap();
        });

        let app = self.clone();
        rx.attach(None, move |result: Result<String, String>| {
            app.account.sync.set_status(result);
            glib::Continue(true)
        });
    }
}

fn sync_status(core: &lb::Core) -> Result<String, String> {
    use lb::CalculateWorkError::*;

    match core.get_last_synced().map_err(|err| err.0)? {
        0 => Ok("✘  Never synced.".to_string()),
        _ => {
            let work = core.calculate_work().map_err(|err| match err {
                lb::Error::UiError(err) => match err {
                    CouldNotReachServer => "Offline.",
                    ClientUpdateRequired => "Client upgrade required.",
                }
                .to_string(),
                lb::Error::Unexpected(msg) => msg,
            })?;
            let n_files = work.work_units.len();
            Ok(match n_files {
                0 => "✔  Synced.".to_string(),
                1 => "<b>1</b>  file not synced.".to_string(),
                _ => format!("<b>{}</b>  files not synced.", n_files),
            })
        }
    }
}
