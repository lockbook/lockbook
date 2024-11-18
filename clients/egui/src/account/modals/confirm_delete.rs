use lb::model::file::File;

enum State {
    WaitingForAnswer,
    Deleting,
    Done,
}

pub struct ConfirmDeleteModal {
    state: State,
    file_ids: Vec<File>,
}

impl ConfirmDeleteModal {
    pub fn new(file_ids: Vec<File>) -> Self {
        Self { state: State::WaitingForAnswer, file_ids }
    }
}

impl super::Modal for ConfirmDeleteModal {
    type Response = Option<(bool, Vec<File>)>;

    fn title(&self) -> &str {
        "Confirm Delete"
    }

    fn show(&mut self, ui: &mut egui::Ui) -> Self::Response {
        let mut resp = None;

        match &self.state {
            State::WaitingForAnswer => {
                let how_many = self.file_ids.len();

                let desc = if how_many == 1 {
                    "this 1 file".to_string()
                } else {
                    format!("these {} files", how_many)
                };
                ui.label(format!("Are you sure you want to delete {}?", desc));

                ui.horizontal(|ui| {
                    if ui.button("Yes, I'm Sure").clicked() {
                        self.state = State::Deleting;
                        resp = Some((true, std::mem::take(&mut self.file_ids)));
                    }
                    if ui.button("No, Cancel").clicked() {
                        self.state = State::Done;
                        resp = Some((false, Vec::new()));
                    }
                });
            }
            State::Deleting => {
                ui.spinner();
            }
            State::Done => {
                ui.label("Done.");
            }
        }

        resp
    }
}
