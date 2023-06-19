use eframe::egui;

pub struct InitiateShareModal {
    err: String,
}

impl InitiateShareModal {
    pub fn new(err: lb::File) -> Self {
        Self { err: err.name }
    }
}

impl super::Modal for InitiateShareModal {
    type Response = Option<()>;

    fn title(&self) -> &str {
        "Share"
    }

    fn show(&mut self, ui: &mut egui::Ui) -> Self::Response {
        ui.add_space(10.0);

        ui.label(&self.err);

        ui.add_space(10.0);

        if ui.button(format!("share {}", self.err)).clicked() {
            Some(())
        } else {
            None
        }
    }
}
