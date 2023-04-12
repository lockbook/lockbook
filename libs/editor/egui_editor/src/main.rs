#[cfg(feature = "debug-window")]
fn main() {
    #[derive(Default)]
    struct TestApp {
        editor: egui_editor::editor::Editor,
    }

    impl eframe::App for TestApp {
        fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
            self.editor.draw(ctx);
        }
    }

    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "My egui App",
        options,
        Box::new(|cc| {
            let app = TestApp::default();
            app.editor.set_font(&cc.egui_ctx);
            Box::new(app)
        }),
    ).unwrap();
}

#[cfg(not(feature = "debug-window"))]
fn main() {}
