use eframe::egui;

use crate::model::DocType;

pub struct SuggestedDocs {
    files: Vec<lb::File>,
}
impl SuggestedDocs {
    pub fn new(files: Vec<lb::File>) -> Self {
        Self { files }
    }

    pub fn show(&self, ui: &mut egui::Ui) {
        egui::ScrollArea::horizontal()
            .id_source("suggested_documents")
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    self.files.iter().for_each(|f| {
                        Self::suggested_card(ui, f);
                    });
                });
            });
    }

    fn suggested_card(ui: &mut egui::Ui, f: &lb::File) {
        egui::Frame::default()
            .inner_margin(egui::Margin::symmetric(10.0, 20.0))
            .outer_margin(egui::Margin::symmetric(10.0, 20.0))
            .rounding(egui::Rounding::same(5.0))
            .fill(ui.visuals().faint_bg_color)
            .show(ui, |ui| {
                ui.set_min_width(200.0);
                ui.vertical(|ui| {
                    DocType::from_name(&f.name).to_icon().size(40.0).show(ui);
                    ui.label(&f.name);

                    ui.label(
                        egui::RichText::new(f.last_modified.to_string())
                            .size(15.0)
                            .color(egui::Color32::GRAY),
                    );
                });
            });
    }
}
