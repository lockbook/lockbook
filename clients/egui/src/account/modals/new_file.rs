use eframe::egui;

use crate::widgets::ButtonGroup;

pub struct NewFileModal {
    ftype: NewFileType,
    parent_path: String,
    new_name: String,
    name_field_needs_focus: bool,
    pub err_msg: Option<String>,
}

impl NewFileModal {
    pub fn new(parent_path: String) -> Self {
        Self {
            ftype: NewFileType::Markdown,
            parent_path,
            new_name: "".to_string(),
            name_field_needs_focus: true,
            err_msg: None,
        }
    }
}

impl super::Modal for NewFileModal {
    type Response = Option<NewFileParams>;

    fn title(&self) -> &str {
        "New File"
    }

    fn show(&mut self, ui: &mut egui::Ui) -> Self::Response {
        let mut maybe_submission = None;

        ui.add_space(10.0);

        // File type selection.
        ui.horizontal(|ui| {
            if let Some(_type_selected) = ButtonGroup::toggle_mut(&mut self.ftype)
                .btn(NewFileType::Markdown, "Markdown")
                .btn(NewFileType::Drawing, "Drawing")
                .btn(NewFileType::PlainText, "Plain Text")
                .btn(NewFileType::Folder, "Folder")
                .show(ui)
            {
                self.name_field_needs_focus = true;
            }
        });

        ui.add_space(10.0);

        egui::Grid::new("new_file_modal_content")
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
                ui.with_layout(
                    egui::Layout::left_to_right().with_cross_align(egui::Align::Center),
                    |ui| {
                        ui.set_max_width(300.0);

                        let out = egui::TextEdit::singleline(&mut self.new_name)
                            .margin(egui::vec2(8.0, 8.0))
                            .hint_text("Name...")
                            .show(ui);

                        if out.response.lost_focus() && ui.input().key_pressed(egui::Key::Enter) {
                            if self.new_name.is_empty() {
                                self.err_msg = Some("File names cannot be empty!".to_string());
                            } else {
                                let name = format!(
                                    "{}{}",
                                    self.new_name,
                                    self.ftype.ext().unwrap_or_default()
                                );
                                maybe_submission = Some(NewFileParams {
                                    ftype: self.ftype.as_lb_type(),
                                    parent_path: self.parent_path.clone(),
                                    name,
                                });
                            }
                        }

                        // If this is the first frame for the modal, or if a file type was
                        // selected, focus the name input field.
                        if self.name_field_needs_focus {
                            out.response.request_focus();
                        }

                        if let Some(ext) = self.ftype.ext() {
                            ui.label(ext);
                        }
                    },
                );

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

#[derive(Clone, Copy, PartialEq)]
enum NewFileType {
    Markdown,
    Drawing,
    PlainText,
    Folder,
}

impl NewFileType {
    fn ext(&self) -> Option<&'static str> {
        match self {
            Self::Markdown => Some(".md"),
            Self::Drawing => Some(".draw"),
            Self::PlainText => Some(".txt"),
            Self::Folder => None,
        }
    }

    fn as_lb_type(&self) -> lb::FileType {
        match self {
            Self::Folder => lb::FileType::Folder,
            _ => lb::FileType::Document,
        }
    }
}

pub struct NewFileParams {
    pub ftype: lb::FileType,
    pub parent_path: String,
    pub name: String,
}
