use eframe::egui;

use crate::model::DocType;
use crate::splash::SuggestedFile;

pub struct SuggestedDocs {
    files: Vec<SuggestedFile>,
}

impl SuggestedDocs {
    pub fn new(files: Vec<SuggestedFile>) -> Self {
        Self { files }
    }

    pub fn show(&self, ui: &mut egui::Ui) -> Option<lb::Uuid> {
        egui::ScrollArea::horizontal()
            .id_source("suggested_documents")
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.add_space(5.0);
                    for f in self.files.iter() {
                        let r = egui::Frame::default()
                            .outer_margin(egui::Margin::symmetric(10.0, 20.0))
                            .show(ui, |ui| Self::suggested_card(ui, f));
                        if r.inner.is_some() {
                            return r.inner;
                        }
                    }
                    None
                })
            })
            .inner
            .inner
    }

    fn suggested_card(ui: &mut egui::Ui, f: &SuggestedFile) -> Option<lb::Uuid> {
        let response = egui::Frame::default()
            .inner_margin(egui::Margin::symmetric(10.0, 20.0))
            .rounding(egui::Rounding::same(5.0))
            .fill(ui.visuals().code_bg_color)
            .show(ui, |ui| {
                ui.set_min_width(130.0);
                ui.set_max_width(170.0);
                ui.vertical(|ui| {
                    DocType::from_name(&f.name).to_icon().show(ui);
                    ui.horizontal_wrapped(|ui| {
                        let mut job = egui::text::LayoutJob::single_section(
                            f.name.clone(),
                            egui::TextFormat::simple(
                                egui::FontId::proportional(20.0),
                                ui.visuals().text_color(),
                            ),
                        );

                        job.wrap = egui::epaint::text::TextWrapping {
                            overflow_character: Some('…'),
                            max_rows: 1,
                            break_anywhere: true,
                            ..Default::default()
                        };
                        ui.label(job);
                    });

                    let path_parent_index = f.path.rfind('/').unwrap_or_default();
                    let path: String = f.path.chars().take(path_parent_index).collect();

                    ui.horizontal_wrapped(|ui| {
                        let mut job = egui::text::LayoutJob::single_section(
                            path,
                            egui::TextFormat::simple(
                                egui::FontId::proportional(15.0),
                                egui::Color32::GRAY,
                            ),
                        );

                        job.wrap = egui::epaint::text::TextWrapping {
                            overflow_character: Some('…'),
                            max_rows: 1,
                            break_anywhere: true,
                            ..Default::default()
                        };
                        ui.label(job);
                    });
                });
            })
            .response;

        let response = ui.interact(
            response.rect,
            egui::Id::from(format!("suggested_card_{}", f.path)),
            egui::Sense::click(),
        );
        if response.hovered() {
            ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
        }

        if response.clicked() {
            return Some(f.id);
        }
        None
    }
}
