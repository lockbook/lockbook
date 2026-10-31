//! A workspace on an account at the local server, drawn frame by frame.

use egui::{Context, RawInput, Rect, pos2, vec2};
use lb_rs::Uuid;
use lb_rs::blocking::Lb;
use lb_rs::model::core_config::Config;
use lb_rs::model::file_metadata::FileType;
use web_time::{Duration, Instant};

use crate::tab::ContentState;
use crate::theme::palette_v2::{Mode, Theme, ThemeExt as _};
use crate::workspace::Workspace;

pub struct TestWs {
    pub ws: Workspace,
    pub ctx: Context,
    pub size: egui::Vec2,
}

pub fn test_lb() -> Lb {
    let core = Lb::init(Config {
        writeable_path: format!("/tmp/lb-ws-test/{}", Uuid::new_v4()),
        logs: false,
        stdout_logs: false,
        colored_logs: false,
        background_work: false,
        client_name: "tests".into(),
        build_info: None,
        client_type: lb_rs::model::core_config::ClientType::Unknown,
    })
    .unwrap();
    let url = std::env::var("API_URL").unwrap_or_else(|_| "http://localhost:8000".into());
    let name = format!("ws{}", Uuid::new_v4().simple())
        .chars()
        .take(30)
        .collect::<String>();
    core.create_account(&name, &url, false).unwrap();
    core
}

pub fn test_ctx() -> Context {
    let ctx = Context::default();
    let mut fonts = egui::FontDefinitions::default();
    crate::register_fonts(&mut fonts);
    ctx.set_fonts(fonts);
    ctx.set_lb_theme(Theme::default(Mode::Light));
    crate::register_font_system(&ctx);
    ctx
}

impl TestWs {
    pub fn new() -> Self {
        let ctx = test_ctx();
        let ws = Workspace::new(&test_lb(), &ctx, false, None);
        Self { ws, ctx, size: vec2(800.0, 600.0) }
    }

    /// A document at the root with `text`.
    pub fn doc(&mut self, name: &str, text: &str) -> Uuid {
        let root = self.ws.core.get_root().unwrap().id;
        let file = self
            .ws
            .create_file_at(root, name, FileType::Document, false)
            .unwrap();
        self.ws
            .core
            .write_document(file.id, text.as_bytes())
            .unwrap();
        file.id
    }

    pub fn frame(&mut self) -> crate::Response {
        self.frame_with(vec![])
    }

    pub fn frame_with(&mut self, events: Vec<egui::Event>) -> crate::Response {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), self.size)),
            events,
            ..Default::default()
        };
        let mut out = None;
        self.ctx
            .run_pass(input, |ctx| {
                ctx.root_ui(|ui| {
                    egui::CentralPanel::default().show(ui, |ui| out = Some(self.ws.show(ui)))
                });
            })
            .textures_delta
            .clear();
        out.unwrap()
    }

    /// Frames until no open tab is loading and a few more for first draws.
    pub fn settle(&mut self) {
        let start = Instant::now();
        loop {
            self.frame();
            let loading = self
                .ws
                .tabs
                .values()
                .any(|t| matches!(t.content, ContentState::Loading(_)));
            if !loading {
                break;
            }
            assert!(start.elapsed() < Duration::from_secs(10), "tabs never loaded");
            std::thread::sleep(Duration::from_millis(5));
        }
        for _ in 0..4 {
            self.frame();
        }
    }
}
