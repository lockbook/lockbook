use lb::model::file_metadata::FileType;

pub struct NewFileParams {
    pub ftype: FileType,
    pub parent_path: String,
    pub name: String,
}

pub struct NewFolderModal {
    parent_path: String,
    new_name: String,
    name_field_needs_focus: bool,
    pub err_msg: Option<String>,
}

impl NewFolderModal {
    pub fn new(parent_path: String) -> Self {
        Self { parent_path, new_name: "".to_string(), name_field_needs_focus: true, err_msg: None }
    }
}

impl super::Modal for NewFolderModal {
    type Response = Option<NewFileParams>;

    fn title(&self) -> &str {
        "New Folder"
    }

    fn show(&mut self, ui: &mut egui::Ui) -> Self::Response {
        let mut maybe_submission = None;

        ui.add_space(10.0);

        egui::Grid::new("new_folder_modal_content")
            .spacing(egui::vec2(10.0, 10.0))
            .show(ui, |ui| {
                ui.label("Parent:");

                // The path of the parent folder.
                ui.add_sized(
                    ui.available_size_before_wrap(),
                    egui::TextEdit::singleline(&mut self.parent_path)
                        .margin(egui::vec2(8.0, 8.0))
                        .hint_text("Parent...")
                        .interactive(false),
                );

                ui.end_row();

                ui.label("Name:");

                // The new file's name and extension.
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.set_max_width(300.0);

                    let out = egui::TextEdit::singleline(&mut self.new_name)
                        .margin(egui::vec2(8.0, 8.0))
                        .hint_text("Name...")
                        .show(ui);

                    if out.response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        if self.new_name.is_empty() {
                            self.err_msg = Some("File names cannot be empty!".to_string());
                        } else {
                            maybe_submission = Some(NewFileParams {
                                ftype: FileType::Folder,
                                parent_path: self.parent_path.clone(),
                                name: self.new_name.clone(),
                            });
                        }
                    }

                    // If this is the first frame for the modal, or if a file type was
                    // selected, focus the name input field.
                    if self.name_field_needs_focus {
                        out.response.request_focus();
                    }
                });

                ui.end_row();
            });

        ui.add_space(10.0);

        if let Some(msg) = &self.err_msg {
            ui.label(egui::RichText::new(msg).color(egui::Color32::RED));
            ui.add_space(10.0);
        }

        maybe_submission
    }
}
