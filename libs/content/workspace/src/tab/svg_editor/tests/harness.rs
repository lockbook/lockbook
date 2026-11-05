//! Headless canvas driver. The canvas runs whole: input controller, tools,
//! history, viewport, renderer. What it depends on is spelled out and kept
//! off disk and network:
//!
//! - the drawing's bytes, given at open and on reload
//! - egui input per frame: pointer, keys, wheel, zoom, at a fixed screen size and clock
//! - toolbar controls, as the `CanvasAction`s they submit
//! - images: [`TestImages`], an in-memory store
//! - canvas settings: [`WsPersistentStore::in_memory`]
//! - fonts: the shared test context's, as the markdown harness uses (on
//!   Apple platforms those are read from the system once per process)
//!
//! What it produces: the buffer, the bytes a save writes, and the meshes it paints.

use std::collections::HashMap;
use std::sync::Mutex;

use egui::{Event, Key, Modifiers, PointerButton, Pos2, RawInput, Rect, Vec2};
use lb_rs::Uuid;
use lb_rs::model::errors::{LbErrKind, LbResult};

use super::super::keyboard_shortcuts::CanvasAction;
use super::super::{CanvasImages, SVGEditor, Tool};
use crate::tab::ExtendedInput as _;
use crate::tab::TabContent;
use crate::theme::palette_v2::{Mode, Theme, ThemeExt as _};
use crate::workspace::WsPersistentStore;

pub const SCREEN: Vec2 = Vec2::new(800., 600.);

/// Images kept in memory: what a paste stores, a promote reads back.
#[derive(Default)]
pub struct TestImages(Mutex<HashMap<Uuid, Vec<u8>>>);

impl CanvasImages for TestImages {
    fn read(&self, id: Uuid) -> LbResult<Vec<u8>> {
        self.0
            .lock()
            .unwrap()
            .get(&id)
            .cloned()
            .ok_or(LbErrKind::FileNonexistent.into())
    }

    fn import(&self, _drawing: Uuid, _name: Option<&str>, data: &[u8]) -> Result<Uuid, String> {
        let id = Uuid::new_v4();
        self.0.lock().unwrap().insert(id, data.to_vec());
        Ok(id)
    }
}

pub struct TestCanvas {
    pub tab: TabContent,
    ctx: egui::Context,
    time: f64,
    /// What the last frame painted.
    pub shapes: Vec<egui::epaint::ClippedShape>,
    /// The tool a pen stroke is drawn with.
    pub drawing_with: Tool,
}

impl TestCanvas {
    pub fn open(bytes: &str) -> Self {
        let ctx = crate::tab::markdown_editor::test_egui_ctx();
        let mut editor = SVGEditor::new(
            bytes.as_bytes(),
            &ctx,
            Box::new(TestImages::default()),
            Uuid::new_v4(),
            None,
            &WsPersistentStore::in_memory(),
            false,
        );
        // the minimap paints the strokes again, smaller
        editor.settings.show_mini_map = false;
        let mut canvas = Self {
            tab: TabContent::Svg(editor),
            ctx,
            time: 0.,
            shapes: vec![],
            drawing_with: Tool::Pen,
        };
        // the first frame lays the canvas out, the second paints it
        canvas.frame(vec![]);
        canvas.frame(vec![]);
        canvas
    }

    pub fn svg(&mut self) -> &mut SVGEditor {
        let TabContent::Svg(svg) = &mut self.tab else { unreachable!() };
        svg
    }

    pub fn svg_ref(&self) -> &SVGEditor {
        let TabContent::Svg(svg) = &self.tab else { unreachable!() };
        svg
    }

    pub fn frame(&mut self, events: Vec<Event>) {
        self.time += 1. / 60.;
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SCREEN)),
            time: Some(self.time),
            events,
            ..Default::default()
        };
        let ctx = self.ctx.clone();
        let mut out = ctx.run_pass(input, |ctx| {
            ctx.set_lb_theme(Theme::default(Mode::Dark));
            crate::register_font_system(ctx);
            ctx.root_ui(|ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    self.svg().show(ui);
                })
            });
        });
        out.textures_delta.clear();
        self.shapes = out.shapes;
    }

    pub fn key(&mut self, key: Key, modifiers: Modifiers) {
        self.frame(vec![
            Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers },
            Event::Key { key, physical_key: None, pressed: false, repeat: false, modifiers },
        ]);
    }

    /// Submits what a toolbar control submits.
    pub fn act(&mut self, action: CanvasAction) {
        self.ctx.push_event(crate::tab::Event::Canvas(action));
        self.frame(vec![]);
    }

    /// Picks a tool by its key, or its button when it has none. Choosing the
    /// active tool again opens its popover, so this only switches.
    pub fn tool(&mut self, tool: Tool) {
        if self.svg_ref().toolbar.active_tool == tool {
            return;
        }
        let key = match tool {
            Tool::Pen => Key::B,
            Tool::Eraser => Key::E,
            Tool::Selection => Key::S,
            other => return self.act(CanvasAction::SetTool(other)),
        };
        self.key(key, Modifiers::NONE);
    }

    /// Press at the first point, move through the rest, release at the last.
    pub fn drag(&mut self, pts: &[Pos2]) {
        let press = |pos, pressed| Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        self.frame(vec![Event::PointerMoved(pts[0])]);
        self.frame(vec![press(pts[0], true)]);
        for p in &pts[1..] {
            self.frame(vec![Event::PointerMoved(*p)]);
        }
        self.frame(vec![press(*pts.last().unwrap(), false)]);
        self.frame(vec![]);
    }

    /// Pinch-zoom by `factor` around `at`.
    pub fn zoom(&mut self, factor: f32, at: Pos2) {
        self.frame(vec![Event::PointerMoved(at)]);
        self.frame(vec![Event::Zoom(factor)]);
        self.frame(vec![]);
    }

    /// Two-finger scroll.
    pub fn pan(&mut self, delta: Vec2) {
        self.frame(vec![Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta,
            modifiers: Modifiers::NONE,
            phase: egui::TouchPhase::Move,
            source: egui::MouseWheelSource::Unknown,
        }]);
        self.frame(vec![]);
    }

    /// The bytes a save writes (tab/mod.rs clone_content → into_bytes).
    pub fn file(&self) -> String {
        self.svg_ref().buffer.serialize()
    }

    /// A save completing: workspace.rs stores what was saved as the base
    /// the next reload merges against.
    pub fn save(&mut self) -> String {
        let bytes = self.file();
        // CANVAS_PROPS_PARSED_BASE: the base as canvas-remote-style sets it
        let saved = if std::env::var("CANVAS_PROPS_PARSED_BASE").is_ok() {
            lb_rs::model::svg::buffer::Buffer::new(&bytes)
        } else {
            self.svg_ref().buffer.clone()
        };
        self.svg().opened_content = saved;
        bytes
    }

    /// A newer version arriving (open.rs TabContent::reload).
    pub fn reload(&mut self, bytes: &str) {
        self.tab.reload(None, bytes.as_bytes());
        self.frame(vec![]);
    }

    pub fn master(&self) -> resvg::usvg::Transform {
        self.svg_ref().viewport_settings.master_transform
    }
}
