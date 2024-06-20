use eframe::egui;
use egui_extras::{Column, TableBuilder};

#[derive(Default)]
pub struct HelpModal;

impl HelpModal {
    pub fn show(&mut self, ui: &mut egui::Ui) {
        let is_mac = ui.ctx().os() == egui::os::OperatingSystem::Mac;
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
                for (shortcut, shortcut_mac, description) in [
                    ("Ctrl-N", "Cmd-N", "Open the New File prompt"),
                    ("Ctrl-Space, Ctrl-L", "Cmd-Space, Cmd-L", "Open the search prompt"),
                    ("Ctrl-S", "Cmd-S", "Save the active document"),
                    ("Ctrl-W", "Cmd-W", "Close an opened document"),
                    ("Alt-{1-9}", "Alt-{1-9}", "Navigate tabs (9 will always go to the last one)"),
                    ("Alt-H", "Alt-H", "Toggle this help window"),
                ] {
                    body.row(30.0, |mut row| {
                        row.col(|ui| {
                            let shortcut = if is_mac { shortcut_mac } else { shortcut };
                            ui.label(shortcut);
                        });
                        row.col(|ui| {
                            ui.label(description);
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
