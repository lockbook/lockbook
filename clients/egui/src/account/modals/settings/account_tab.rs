use eframe::egui;
use egui_extras::{Size, StripBuilder};
use egui_winit::clipboard::Clipboard;

pub struct AccountSettings {
    pub export_result: Result<String, String>,
}

impl super::SettingsModal {
    pub fn show_account_tab(&mut self, ui: &mut egui::Ui) {
        match &self.account.export_result {
            Ok(key) => {
                ui.heading("Export Account");
                ui.add_space(12.0);

                ui.label(EXPORT_DESC);
                ui.add_space(12.0);

                StripBuilder::new(ui)
                    .size(Size::remainder())
                    .size(Size::remainder())
                    .horizontal(|mut strip| {
                        strip.cell(|ui| {
                            if ui.button("Copy to Clipboard").clicked() {
                                Clipboard::default().set(key.to_owned());
                            }
                        });
                        strip.cell(|ui| {
                            if ui.button("Show QR Code").clicked() {
                                println!("show qr");
                            }
                        });
                    });
            }
            Err(err) => {
                ui.label(err);
            }
        }
    }
}

const EXPORT_DESC: &str = "\
Lockbook encrypts your data with a secret key that remains on your devices. \
Whoever has access to this key has complete knowledge and control of your data.

Do not give this key to anyone. Do not display the QR code if there are cameras around.";
