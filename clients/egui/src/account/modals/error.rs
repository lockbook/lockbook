use eframe::egui;

pub struct ErrorModal {
    err: String,
}

impl ErrorModal {
    pub fn open(err: impl ToString) -> Option<Box<Self>> {
        Some(Box::new(Self { err: err.to_string() }))
    }
}

impl super::Modal for ErrorModal {
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
