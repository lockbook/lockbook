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
                let any_folders = self.file_ids.iter().any(|f| f.is_folder());
                let all_folders = self.file_ids.iter().all(|f| f.is_folder());

                match (how_many, any_folders, all_folders) {
                    (1, true, _) => ui.label("Are you sure you want to delete this folder and everything in it?"),
                    (1, false, _) => ui.label("Are you sure you want to delete this document?"),
                    (_, _, true) => ui.label(format!("Are you sure you want to delete these {how_many} folders and everything in them?")),
                    (_, true, _) => ui.label(format!("Are you sure you want to delete these {how_many} files? Contents of folders will also be deleted.")),
                    (_, _, false) => ui.label(format!("Are you sure you want to delete these {how_many} files?")),
                };

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
