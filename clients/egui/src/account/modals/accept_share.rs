use eframe::egui;

pub struct AcceptShareModal {
    err: String,
}

impl AcceptShareModal {
    pub fn new() -> Self {
        Self { err: "hello".to_string() }
    }
}

impl super::Modal for AcceptShareModal {
    type Response = Option<()>;

    fn title(&self) -> &str {
        "Error!"
    }

    fn show(&mut self, ui: &mut egui::Ui) -> Self::Response {
        ui.add_space(10.0);

        ui.label(&self.err);

        ui.add_space(10.0);

        if ui.button("Dismiss").clicked() {
            Some(())
        } else {
            None
        }
    }
}
