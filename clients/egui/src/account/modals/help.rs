use eframe::egui;
use egui_extras::{Column, TableBuilder};

#[derive(Default)]
pub struct HelpModal;

impl HelpModal {
    pub fn show(&mut self, ui: &mut egui::Ui) {
        TableBuilder::new(ui)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .striped(true)
            .column(Column::exact(120.0))
            .column(Column::remainder().at_least(60.0))
            .header(30.0, |mut header| {
                header.col(|ui| {
                    ui.label(egui::RichText::new("Shortcut").strong().underline());
                });
                header.col(|ui| {
                    ui.label(egui::RichText::new("Action").strong().underline());
                });
            })
            .body(|mut body| {
                for (k, v) in [
                    ("Ctrl-N", "Open the New File prompt"),
                    ("Ctrl-Space, Ctrl-L", "Open the search prompt"),
                    ("Ctrl-S", "Save the active document"),
                    ("Ctrl-W", "Close an opened document"),
                    ("Alt-{1-9}", "Navigate tabs (9 will always go to the last one)"),
                    ("Alt-H", "Toggle this help window"),
                ] {
                    body.row(30.0, |mut row| {
                        row.col(|ui| {
                            ui.label(k);
                        });
                        row.col(|ui| {
                            ui.label(v);
                        });
                    });
                }
            });
    }
}

impl super::Modal for HelpModal {
    const ANCHOR: egui::Align2 = egui::Align2::CENTER_CENTER;
    const Y_OFFSET: f32 = 0.0;

    type Response = ();

    fn title(&self) -> &str {
        "Help"
    }

    fn show(&mut self, ui: &mut egui::Ui) {
        self.show(ui)
    }
}
