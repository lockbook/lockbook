use eframe::egui;
use egui_editor::editor::Editor;

fn main() {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "My egui App",
        options,
        Box::new(|cc| {
            let app = TestApp::default();
            app.editor.set_font(&cc.egui_ctx);
            Box::new(app)
        }),
    );
}

#[derive(Default)]
struct TestApp {
    editor: Editor,
}

impl eframe::App for TestApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.editor.draw(ctx);
    }
}
