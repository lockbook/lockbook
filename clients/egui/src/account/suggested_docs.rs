use eframe::egui;

use crate::model::DocType;
use chrono::{DateTime, NaiveDateTime, Utc};

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
                        egui::Frame::default()
                            .outer_margin(egui::Margin::symmetric(10.0, 20.0))
                            .show(ui, |ui| Self::suggested_card(ui, f));
                    });
                });
            });
    }

    fn suggested_card(ui: &mut egui::Ui, f: &lb::File) {
        let naive_time = NaiveDateTime::from_timestamp_millis(f.last_modified as i64).unwrap();
        let utc_time: DateTime<Utc> = DateTime::from_utc(naive_time, Utc);
        let time = chrono_humanize::HumanTime::from(utc_time).to_string();

        let response = egui::Frame::default()
            .inner_margin(egui::Margin::symmetric(10.0, 20.0))
            .rounding(egui::Rounding::same(5.0))
            .fill(ui.visuals().faint_bg_color)
            .show(ui, |ui| {
                ui.set_max_width(150.0);
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        DocType::from_name(&f.name).to_icon().size(40.0).show(ui);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                            ui.label(
                                egui::RichText::new(time)
                                    .size(15.0)
                                    .color(egui::Color32::GRAY),
                            );
                        });
                    });

                    let mut job = egui::text::LayoutJob::single_section(
                        f.name.clone(),
                        egui::TextFormat::simple(
                            egui::FontId::proportional(17.0),
                            ui.visuals().text_color(),
                        ),
                    );

                    job.wrap = egui::epaint::text::TextWrapping {
                        overflow_character: Some('…'),
                        max_rows: 1,
                        ..Default::default()
                    };
                    ui.add_space(5.0);
                    ui.label(job);
                    // ui.label(&f.name);
                });
            })
            .response;

        let response = ui.interact(
            response.rect,
            egui::Id::from(format!("suggested_card_{}", f.name)),
            egui::Sense::click(),
        );

        if response.clicked() {
            println!("open tab of file {}", f.name);
        }
    }
}
