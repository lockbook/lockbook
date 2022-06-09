use crate::ui;

impl super::App {
    pub fn show_err_dialog(&self, txt: &str) {
        ui::show_err_dialog(&self.window, txt);
    }
}
