use std::sync::Arc;

use eframe::egui;
use lb::File;

use crate::{theme::Icon, widgets::Button};

pub struct FilePicker {
    core: Arc<lb::Core>,
    file_panels: Vec<lb::File>,
}

impl FilePicker {
    pub fn new(core: Arc<lb::Core>) -> Self {
        let root = core.get_root().unwrap();

        Self { core, file_panels: vec![root] }
    }
}

impl super::Modal for FilePicker {
    type Response = Option<()>;

    fn title(&self) -> &str {
        "File Picker"
    }

    fn show(&mut self, ui: &mut egui::Ui) -> Self::Response {
        ui.set_max_width(660.0);
        egui::ScrollArea::horizontal()
            .id_source("parent")
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.set_height(300.0);
                    for (i, f) in self.file_panels.clone().iter().enumerate() {
                        let mut enabled = true;

                        if i != self.file_panels.len() - 1 {
                            enabled = false;
                        }
                        ui.add_enabled_ui(enabled, |ui| {
                            show_file_panel(ui, self, f);
                        });
                        ui.separator();
                    }
                });
            });

        ui.separator();

        //bottom bar
        ui.horizontal(|ui| {
            egui::ScrollArea::horizontal()
                .max_width(ui.available_width() - 100.0) // allow some room for the cta
                .show(ui, |ui| {
                    self.file_panels.iter().for_each(|f| {
                        if f.name != self.core.get_account().unwrap().username {
                            ui.label(f.name.clone());
                            ui.label(">");
                        }
                    });
                });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| ui.button("select"));
        });

        None
    }
}

fn show_file_panel(ui: &mut egui::Ui, file_picker: &mut FilePicker, root: &lb::File) {
    egui::ScrollArea::vertical()
        .id_source(root.name.clone())
        .show(ui, |ui| {
            ui.set_width(200.0);

            ui.vertical(|ui| {
                let children = file_picker.core.get_children(root.id).unwrap();
                let mut children: Vec<&File> = children
                    .iter()
                    .filter(|f| f.file_type == lb::FileType::Folder)
                    .collect();
                children.sort_by(|a, b| a.name.cmp(&b.name));

                for child in children {
                    ui.visuals_mut().widgets.inactive.bg_fill = egui::Color32::TRANSPARENT;
                    ui.visuals_mut().widgets.hovered.bg_fill = egui::Color32::TRANSPARENT;

                    if Button::default()
                        .text(child.name.clone().as_str())
                        .icon(&Icon::FOLDER)
                        .show(ui)
                        .clicked()
                        && file_picker.file_panels.last().unwrap() != child
                    {
                        file_picker.file_panels.push(child.clone())
                    };
                }
            });
        });
}
