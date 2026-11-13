//! A workspace on an account at the local server, drawn frame by frame.

use egui::{Context, RawInput, Rect, pos2, vec2};
use lb_rs::Uuid;
use lb_rs::blocking::Lb;
use lb_rs::model::file_metadata::FileType;
use web_time::{Duration, Instant};

use crate::theme::palette_v2::{Mode, Theme, ThemeExt as _};
use crate::workspace::Workspace;

pub struct TestWs {
    pub ws: Workspace,
    pub ctx: Context,
    pub size: egui::Vec2,
}

pub fn test_lb() -> Lb {
    let core = crate::test_utils::in_memory_core();
    let name = format!("ws{}", Uuid::new_v4().simple())
        .chars()
        .take(30)
        .collect::<String>();
    core.create_account(&name, &api_url(), false).unwrap();
    core
}

fn api_url() -> String {
    std::env::var("API_URL").unwrap_or_else(|_| "http://localhost:8000".into())
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

    /// Another device on this workspace's account, synced.
    pub fn other_device(&self) -> Lb {
        let other = crate::test_utils::in_memory_core();
        let key = self.ws.core.export_account_private_key().unwrap();
        other.import_account(&key, Some(&api_url())).unwrap();
        other.sync().unwrap();
        other
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

    /// Frames until no load or save of `id` is pending.
    pub fn idle(&mut self, id: Uuid) {
        let start = Instant::now();
        self.frame();
        while self.ws.tasks.busy(id) {
            assert!(start.elapsed() < Duration::from_secs(10), "{id} never went idle");
            std::thread::sleep(Duration::from_millis(2));
            self.frame();
        }
    }

    /// Frames until the current tab is settled.
    pub fn settle(&mut self) {
        let start = Instant::now();
        loop {
            self.frame();
            if self.ws.current_tab().is_none_or(|t| t.settled()) {
                break;
            }
            assert!(start.elapsed() < Duration::from_secs(10), "tab never settled");
            std::thread::sleep(Duration::from_millis(2));
        }
    }
}
