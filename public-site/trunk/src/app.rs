use lb_rs::{Uuid, blocking::Lb, model::core_config::Config};
use workspace_rs::{
    tab::{
        markdown_editor::{Editor, MdConfig},
        svg_editor::SVGEditor,
    },
    workspace::Workspace,
};

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
pub struct LbWebApp {
    workspace: Workspace,
    editor: Option<Editor>,
    canvas: Option<SVGEditor>,
    initial_screen: InitialScreen,
}

#[derive(PartialEq)]
pub enum InitialScreen {
    Canvas,
    Editor,
}
impl LbWebApp {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>, initial_screen: InitialScreen) -> Self {
        let ctx = cc.egui_ctx.clone();

        let lb = Lb::init(Config {
            logs: false,
            colored_logs: false,
            writeable_path: "".into(),
            background_work: false,
            stdout_logs: false,
        })
        .unwrap();

        let mut fonts = egui::FontDefinitions::default();

        workspace_rs::register_fonts(&mut fonts);

        ctx.set_fonts(fonts);
        ctx.set_zoom_factor(0.9);

        ctx.set_visuals(generate_visuals());

        Self {
            workspace: Workspace::new(&lb, &ctx, false),
            editor: None,
            canvas: None,
            initial_screen,
        }
    }
}

fn generate_visuals() -> egui::Visuals {
    let mut visuals = egui::Visuals::dark();
    visuals.extreme_bg_color = egui::Color32::from_hex("#1a1a1a").unwrap();
    visuals.code_bg_color = egui::Color32::from_hex("#67e4b6").unwrap();
    visuals.faint_bg_color = egui::Color32::BLUE;
    visuals.widgets.noninteractive.bg_fill = visuals.extreme_bg_color;

    visuals
}

impl eframe::App for LbWebApp {
    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default()
            .frame(egui::Frame::default().fill(ctx.style().visuals.widgets.noninteractive.bg_fill))
            .show(ctx, |ui| {
                if self.editor.is_none() && self.initial_screen == InitialScreen::Editor {
                    self.editor = Some(Editor::new(
                        ctx.clone(),
                        self.workspace.core.clone(),
                        self.workspace.cfg.clone(),
                        include_str!("../resources/editor-demo.md"),
                        Uuid::new_v4(),
                        None,
                        MdConfig { plaintext_mode: false, readonly: false },
                    ));
                }

                if self.canvas.is_none() && self.initial_screen == InitialScreen::Canvas {
                    let svg = include_str!("../resources/canvas-demo.svg");
                    self.canvas = Some(SVGEditor::new(
                        svg.as_bytes(),
                        ui.ctx(),
                        self.workspace.core.clone(),
                        Uuid::new_v4(),
                        None,
                        &self.workspace.cfg,
                        false,
                    ))
                }
                if let Some(md) = &mut self.editor {
                    egui::Frame::default().show(ui, |ui| {
                        md.show(ui);
                    });
                }

                if let Some(svg) = &mut self.canvas {
                    egui::Frame::default().show(ui, |ui| {
                        svg.show(ui);
                    });
                }
            });
    }
}
