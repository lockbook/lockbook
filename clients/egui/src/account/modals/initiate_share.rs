use crate::widgets::switch;
use eframe::egui;
use lb::{Share, ShareMode, Uuid};

pub struct InitiateShareParams {
    pub id: Uuid,
    pub username: String,
    pub mode: lb::ShareMode,
}
pub struct InitiateShareModal {
    file: lb::File,
    sharee_username: String,
    is_editor: bool,
}

impl InitiateShareModal {
    pub fn new(err: lb::File) -> Self {
        Self { file: err, sharee_username: "".to_string(), is_editor: false }
    }
}

impl super::Modal for InitiateShareModal {
    type Response = Option<InitiateShareParams>;

    fn title(&self) -> &str {
        "Share"
    }

    fn show(&mut self, ui: &mut egui::Ui) -> Self::Response {
        let mut maybe_submission = None;

        egui::Frame::default()
            .inner_margin(egui::Margin::same(10.0))
            .show(ui, |ui| {
                let is_folder = if self.file.is_folder() { "Folder" } else { "" };
                ui.heading(format!("Share \"{}\" {}", self.file.name, is_folder));
                ui.separator();
                ui.add_space(15.0);

                egui::Grid::new("new_folder_modal_content")
                    .spacing(egui::vec2(10.0, 10.0))
                    .show(ui, |ui| {
                        ui.label("Username:");

                        ui.add_sized(
                            ui.available_size_before_wrap(),
                            egui::TextEdit::singleline(&mut self.sharee_username)
                                .margin(egui::vec2(8.0, 8.0))
                                .hint_text("Username..."),
                        );

                        ui.end_row();

                        ui.label("Grant Edit Access:");

                        // The new file's name and extension.
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.set_width(300.0);
                            ui.add_space(2.0);
                            switch(ui, &mut self.is_editor);
                        });

                        ui.end_row();
                    });

                if !self.file.shares.is_empty() {
                    ui.add_space(20.0);
                    ui.collapsing("People with access", |ui| {
                        self.file.shares.iter().for_each(|f| {
                            sharee_info(ui, f);
                            ui.add_space(5.0);
                        });
                    });
                }
                ui.spacing_mut().button_padding = egui::vec2(20.0, 5.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                    if ui.button("Share").clicked() {
                        maybe_submission = Some(InitiateShareParams {
                            id: self.file.id,
                            username: self.sharee_username.clone(),
                            mode: if self.is_editor { ShareMode::Write } else { ShareMode::Read },
                        });
                    }
                });
            });

        maybe_submission
    }
}
fn sharee_info(ui: &mut egui::Ui, share: &Share) {
    egui::Frame::default()
        .fill(ui.style().visuals.faint_bg_color)
        .stroke(egui::Stroke { width: 0.1, color: ui.style().visuals.text_color() })
        .inner_margin(egui::Margin::same(10.0))
        .rounding(egui::Rounding::same(5.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(share.shared_with.to_string());
                ui.add_space(60.0);
                let mode = match share.mode {
                    lb::ShareMode::Write => "Editor",
                    lb::ShareMode::Read => "Viewer",
                };
                ui.label(
                    egui::RichText::new(mode)
                        .size(15.0)
                        .color(egui::Color32::GRAY),
                );
            })
        });
}
