#[cfg(feature = "debug-window")]
use lb::Uuid;

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

    let core = lb::Core::init(&lb::Config::ui_config("cli")).unwrap(); // use the cli account

    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "My egui App",
        options,
        Box::new(|cc| {
            let app = TestApp {
                editor: egui_editor::editor::Editor::new(core, Uuid::new_v4(), "", &Uuid::new_v4()),
            };
            app.editor.set_font(&cc.egui_ctx);
            Box::new(app)
        }),
    )
    .unwrap();
}

#[cfg(not(feature = "debug-window"))]
fn main() {}
