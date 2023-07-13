use std::sync::mpsc;

use eframe::egui;
use egui_extras::{RetainedImage, Size, StripBuilder};

pub struct AccountSettings {
    update_tx: mpsc::Sender<Update>,
    update_rx: mpsc::Receiver<Update>,
    export_result: Result<String, String>,
    maybe_qr_result: Option<Result<RetainedImage, String>>,
    generating_qr: bool,
}

impl AccountSettings {
    pub fn new(export_result: Result<String, String>) -> Self {
        let (update_tx, update_rx) = mpsc::channel();

        Self { update_tx, update_rx, export_result, maybe_qr_result: None, generating_qr: false }
    }
}

enum Update {
    GenerateQr,
    OpenQrCode(Result<RetainedImage, String>),
    CloseQr,
}

impl super::SettingsModal {
    pub fn show_account_tab(&mut self, ui: &mut egui::Ui) {
        while let Ok(update) = self.account.update_rx.try_recv() {
            match update {
                Update::GenerateQr => {
                    self.account.generating_qr = true;
                    self.generate_qr(ui.ctx());
                }
                Update::OpenQrCode(result) => {
                    self.account.maybe_qr_result = Some(result);
                    self.account.generating_qr = false;
                }
                Update::CloseQr => self.account.maybe_qr_result = None,
            }
        }

        if let Some(qr_result) = &self.account.maybe_qr_result {
            ui.vertical_centered(|ui| {
                match qr_result {
                    Ok(img) => {
                        img.show_size(ui, egui::vec2(350.0, 350.0));
                    }
                    Err(err) => {
                        ui.label(err);
                    }
                }
                if ui.button("Done").clicked() {
                    self.account.update_tx.send(Update::CloseQr).unwrap();
                }
            });
        } else {
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
                                    ui.output_mut(|out| out.copied_text = key.to_owned());
                                }
                            });
                            strip.cell(|ui| {
                                let text = if self.account.generating_qr {
                                    "Generating QR Code..."
                                } else {
                                    "Show QR Code"
                                };
                                if ui.button(text).clicked() {
                                    // Can't simply call `self.generate_qr` here because of
                                    // borrowing within closure errors, so we just queue an update
                                    // for next frame.
                                    self.account.update_tx.send(Update::GenerateQr).unwrap();
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

    fn generate_qr(&self, ctx: &egui::Context) {
        let core = self.core.clone();
        let update_tx = self.account.update_tx.clone();
        let ctx = ctx.clone();

        std::thread::spawn(move || {
            let result = core
                .export_account_qr()
                .map(|png| RetainedImage::from_image_bytes("qr", &png).unwrap())
                .map_err(|err| format!("{:?}", err));
            update_tx.send(Update::OpenQrCode(result)).unwrap();
            ctx.request_repaint();
        });
    }
}

const EXPORT_DESC: &str = "\
Lockbook encrypts your data with a secret key that remains on your devices. \
Whoever has access to this key has complete knowledge and control of your data.

Do not give this key to anyone. Do not display the QR code if there are cameras around.";
