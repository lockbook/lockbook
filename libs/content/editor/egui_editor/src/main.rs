#[cfg(feature = "debug-window")]
fn main() {
    struct TestApp {
        editor: egui_editor::editor::Editor,
    }

    impl eframe::App for TestApp {
        fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
            self.editor.draw(ctx);
        }
    }

    let core = lb::Core::init(&lb::Config {
        logs: false,
        colored_logs: false,
        writeable_path: format!("{}/.lockbook/cli", std::env::var("HOME").unwrap()),
    })
    .unwrap();

    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "My egui App",
        options,
        Box::new(|cc| {
            let app = TestApp { editor: egui_editor::editor::Editor::new(core) };
            app.editor.set_font(&cc.egui_ctx);
            Box::new(app)
        }),
    )
    .unwrap();
}

#[cfg(not(feature = "debug-window"))]
fn main() {}
