use egui::ScrollArea;
use workspace_rs::theme::icons::Icon;
use workspace_rs::widgets::IconButton;

impl super::SettingsModal {
    pub fn show_debug_tab(&mut self, ui: &mut egui::Ui) {
        let debug_str = self.debug.lock().unwrap().clone();

        ui.horizontal(|ui| {
            if IconButton::new(&Icon::CONTENT_COPY).show(ui).clicked() {
                ui.output_mut(|o| o.copied_text = debug_str.clone());
            }
            ui.heading("Debug");
        });

        ui.add_space(12.0);

        if !debug_str.is_empty() {
            ScrollArea::new([false, true]).show(ui, |ui| {
                ui.set_max_size(ui.available_size());
                ui.label(debug_str);
            });
        } else {
            ui.label("Loading...");
        }
    }
}
