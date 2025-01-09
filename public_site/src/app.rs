use lb_rs::{blocking::Lb, model::core_config::Config};
use workspace_rs::workspace::{Workspace, WsConfig};

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
pub struct LbWebApp {
    workspace: Workspace,
}

impl LbWebApp {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let ctx = cc.egui_ctx.clone();

        let config = WsConfig::new("/".into(), false, false, true);
        let lb = Lb::init(Config {
            logs: false,
            colored_logs: false,
            writeable_path: "".into(),
            background_work: false,
        })
        .unwrap();

        Self { workspace: Workspace::new(config, &lb, &ctx) }
    }
}

impl eframe::App for LbWebApp {
    /// Called by the frame work to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        // maybe persist the demo document here.
        // eframe::set_value(storage, eframe::APP_KEY, self);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            self.workspace.show(ui);
        });
    }
}
