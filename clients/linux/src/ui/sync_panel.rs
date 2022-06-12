use gtk::prelude::*;

use crate::ui::icons;

#[derive(Clone)]
pub struct SyncPanel {
    status: gtk::Label,
    button: gtk::Button,
    progress: gtk::ProgressBar,
    pub cntr: gtk::Box,
}

impl SyncPanel {
    pub fn new() -> Self {
        let status = gtk::Label::builder()
            .halign(gtk::Align::Start)
            .hexpand(true)
            .build();

        let button = gtk::Button::builder()
            .action_name("app.sync")
            .icon_name(icons::SYNC)
            .halign(gtk::Align::End)
            .tooltip_text("Sync (Alt - S)")
            .build();

        let progress = gtk::ProgressBar::builder().margin_top(4).build();

        let cntr = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .margin_start(8)
            .margin_top(8)
            .margin_bottom(8)
            .margin_end(8)
            .build();
        cntr.append(&status);
        cntr.append(&button);

        Self { status, button, progress, cntr }
    }

    pub fn set_status(&self, result: Result<String, String>) {
        let markup = result.unwrap_or_else(|e| format!("<span foreground=\"red\">{}</span>", e));
        self.status.set_markup(&markup);
    }

    pub fn set_started(&self) {
        self.set_status(Ok("Syncing...".to_string()));
        self.cntr.remove(&self.button);
        self.cntr.set_orientation(gtk::Orientation::Vertical);
        self.cntr.append(&self.progress);
        self.progress.set_fraction(0.0);
        self.progress.show();
    }

    pub fn set_progress(&self, sp: &lb::SyncProgress) {
        let status = match &sp.current_work_unit {
            lb::ClientWorkUnit::PullMetadata => "Pulling file tree updates".to_string(),
            lb::ClientWorkUnit::PushMetadata => "Pushing file tree updates".to_string(),
            lb::ClientWorkUnit::PullDocument(name) => format!("Pulling: {}", name),
            lb::ClientWorkUnit::PushDocument(name) => format!("Pushing: {}", name),
        };
        self.set_status(Ok(status));
        self.progress
            .set_fraction(sp.progress as f64 / sp.total as f64);
    }

    pub fn set_done(&self, result: Result<String, String>) {
        self.cntr.remove(&self.progress);
        self.cntr.set_orientation(gtk::Orientation::Horizontal);
        self.cntr.append(&self.button);
        self.set_status(result);
    }
}
