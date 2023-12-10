use eframe::egui;
use workspace::theme::icons::Icon;
use workspace::widgets::Button;

pub struct AccountBackup;

pub enum AccountBackupParams {
    Backup,
    DeferBackup,
}

impl super::Modal for AccountBackup {
    type Response = Option<AccountBackupParams>;

    fn title(&self) -> &str {
        "Before you begin..."
    }

    fn show(&mut self, ui: &mut egui::Ui) -> Self::Response {
        egui::Frame::default()
            .inner_margin(egui::Margin::same(10.0))
            .show(ui, |ui| {
                ui.vertical_centered(|ui|{
                    Icon::WARNING.color(egui::Color32::GRAY).size(60.0).show(ui);
                });

                ui.add_space(30.0);

                ui.label("Lockbook encrypts your notes with a key that stays on your Lockbook devices. This makes your notes unreadable to anyone but you.");
                ui.add_space(5.0);
                ui.label("If you lose the key, your notes are not recoverable, so we recommend you make a backup in case something happens to this device.");

                ui.add_space(40.0);

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                    if Button::default()
                        .text("Backup now")
                        .frame(true)
                        .show(ui)
                        .clicked()
                    {
                        return Some(AccountBackupParams::Backup);
                    };
                    if Button::default()
                        .text("I'll do this later")
                        .show(ui)
                        .clicked()
                    {
                        return Some(AccountBackupParams::DeferBackup);
                    };
                    None
                })
            })
            .inner
            .inner
    }
}
