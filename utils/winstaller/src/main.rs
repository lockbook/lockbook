#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(feature = "build-winstaller")]
fn main() {
    use std::env;
    use std::fs;
    use std::io;
    use std::sync::mpsc;

    use eframe::egui;
    use egui_extras::RetainedImage;
    use mslnk::ShellLink;

    enum Stage {
        Prompting,
        Installing,
        Done(Result<(), String>),
    }

    struct Winstaller {
        update_rx: mpsc::Receiver<Result<(), String>>,
        update_tx: mpsc::Sender<Result<(), String>>,
        install_dir: String,
        lnk_dir: String,
        logo: RetainedImage,
        stage: Stage,
    }

    impl Winstaller {
        fn new() -> Self {
            let (update_tx, update_rx) = mpsc::channel();

            let appdata = env::var("appdata").unwrap();
            let local_appdata = env::var("localappdata").unwrap();

            Self {
                update_rx,
                update_tx,
                install_dir: format!(r"{}\Lockbook", local_appdata),
                lnk_dir: format!(r"{}\Microsoft\Windows\Start Menu\Programs", appdata),
                logo: RetainedImage::from_image_bytes("logo", include_bytes!("../lockbook.png"))
                    .unwrap(),
                stage: Stage::Prompting,
            }
        }

        fn show_prompting(&mut self, ui: &mut egui::Ui) {
            ui.label("Installation folder:");
            ui.add(egui::TextEdit::singleline(&mut self.install_dir).interactive(false));
            ui.add_space(10.0);

            if ui.button("Install").clicked() {
                self.stage = Stage::Installing;
                self.install(ui.ctx());
            }
        }

        fn show_installing(&mut self, ui: &mut egui::Ui) {
            ui.spinner();
        }

        fn show_done(&self, ui: &mut egui::Ui, result: &Result<(), String>) {
            match result {
                Ok(()) => {
                    ui.label("Done!");
                }
                Err(msg) => {
                    ui.label("Error:");
                    ui.label(msg);
                }
            }
        }

        fn install(&self, ctx: &egui::Context) {
            let update_tx = self.update_tx.clone();
            let ctx = ctx.clone();

            let install_dir = self.install_dir.clone();
            let lnk_dir = self.lnk_dir.clone();

            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(1));

                if let Err(err) = fs::create_dir(&install_dir) {
                    match err.kind() {
                        io::ErrorKind::AlreadyExists => {}
                        _ => {
                            update_tx.send(Err(format!("{:?}", err))).unwrap();
                            return;
                        }
                    }
                }

                let exe_file = format!(r"{}\Lockbook.exe", install_dir);
                let exe_bytes = include_bytes!(concat!(
                    "../../../target/",
                    env!("LB_TARGET"),
                    "/release/lockbook-windows.exe"
                ));
                if let Err(err) = fs::write(&exe_file, exe_bytes) {
                    update_tx.send(Err(format!("{:?}", err))).unwrap();
                    return;
                }

                if let Err(err) = fs::create_dir(&lnk_dir) {
                    match err.kind() {
                        io::ErrorKind::AlreadyExists => {}
                        _ => {
                            update_tx.send(Err(format!("{:?}", err))).unwrap();
                            return;
                        }
                    }
                }

                let sl = ShellLink::new(exe_file).unwrap();
                sl.create_lnk(format!(r"{}\Lockbook.lnk", lnk_dir)).unwrap();

                update_tx.send(Ok(())).unwrap();
                ctx.request_repaint();
            });
        }
    }

    impl eframe::App for Winstaller {
        fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
            while let Ok(result) = self.update_rx.try_recv() {
                self.stage = Stage::Done(result);
            }

            egui::SidePanel::left("side-panel")
                .resizable(false)
                .show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(25.0);
                        self.logo.show(ui);
                    });
                });

            egui::CentralPanel::default().show(ctx, |ui| {
                ui.add_space(25.0);
                ui.label(egui::RichText::new("Lockbook").size(48.0));
                ui.separator();
                ui.add_space(10.0);

                match &self.stage {
                    Stage::Prompting => self.show_prompting(ui),
                    Stage::Installing => self.show_installing(ui),
                    Stage::Done(result) => self.show_done(ui, result),
                }
            });
        }
    }

    eframe::run_native(
        "Lockbook Installer",
        eframe::NativeOptions {
            initial_window_size: Some(egui::vec2(600.0, 250.0)),
            ..Default::default()
        },
        Box::new(|_cc: &eframe::CreationContext| Box::new(Winstaller::new())),
    )
    .unwrap()
}

#[cfg(not(feature = "build-winstaller"))]
fn main() {}
