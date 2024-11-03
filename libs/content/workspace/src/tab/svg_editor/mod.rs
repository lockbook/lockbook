mod clip;
mod eraser;
mod gesture_handler;
mod history;
mod element;
mod path_builder;
mod pen;
mod renderer;
mod selection;
mod toolbar;
mod util;

use self::history::History;
use crate::tab::svg_editor::toolbar::Toolbar;
pub use eraser::Eraser;
pub use history::DeleteElement;
pub use history::Event;
pub use history::InsertElement;
use lb_rs::svg::buffer::Buffer;
use lb_rs::svg::diff::DiffState;
use lb_rs::DocumentHmac;
use lb_rs::Uuid;
pub use path_builder::PathBuilder;
pub use pen::Pen;
use renderer::Renderer;
use resvg::usvg::ImageKind;
pub use toolbar::Tool;
use toolbar::ToolContext;
use tracing::span;
use tracing::Level;
use usvg_parser::Options;

/// A shorthand for [ImageHrefResolver]'s string function.
pub type ImageHrefStringResolverFn = Box<dyn Fn(&str, &Options) -> Option<ImageKind> + Send + Sync>;

pub struct SVGEditor {
    pub buffer: Buffer,
    history: History,
    pub toolbar: Toolbar,
    inner_rect: egui::Rect,
    core: lb_rs::Core,
    open_file: Uuid,
    skip_frame: bool,
    // last_render: Instant,
    renderer: Renderer,
    painter: egui::Painter,
    has_queued_save_request: bool,
    /// don't allow zooming or panning
    allow_viewport_changes: bool,
}

pub struct Response {
    pub request_save: bool,
}

#[derive(PartialEq)]
pub enum CanvasOp {
    PanOrZoom,
    BuildingPath,
    Idle,
}
impl SVGEditor {
    pub fn new(
        bytes: &[u8], ctx: &egui::Context, core: lb_rs::Core, open_file: Uuid,
        hmac: Option<DocumentHmac>,
    ) -> Self {
        let content = std::str::from_utf8(bytes).unwrap();

        let buffer = Buffer::new(content, Some(&core), hmac);

        let toolbar = Toolbar::new();

        let elements_count = buffer.elements.len();

        Self {
            buffer,
            history: History::default(),
            toolbar,
            inner_rect: egui::Rect::NOTHING,
            core,
            open_file,
            skip_frame: false,
            // last_render: Instant::now(),
            painter: egui::Painter::new(
                ctx.to_owned(),
                egui::LayerId::new(egui::Order::Background, "canvas_painter".into()),
                egui::Rect::NOTHING,
            ),
            renderer: Renderer::new(elements_count),
            has_queued_save_request: false,
            allow_viewport_changes: false,
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) -> Response {
        let frame = ui.ctx().frame_nr();
        let span = span!(Level::TRACE, "showing canvas widget", frame);
        let _ = span.enter();

        self.inner_rect = ui.available_rect_before_wrap();

        ui.painter()
            .rect_filled(self.inner_rect, 0., ui.style().visuals.extreme_bg_color);

        self.painter = ui.painter_at(self.inner_rect);

        ui.with_layer_id(
            egui::LayerId { order: egui::Order::Middle, id: egui::Id::from("canvas_ui_overlay") },
            |ui| {
                let mut ui = ui.child_ui(ui.painter().clip_rect(), egui::Layout::default(), None);
                self.toolbar.show(
                    &mut ui,
                    &mut self.buffer,
                    &mut self.history,
                    &mut self.skip_frame,
                    self.inner_rect,
                );
            },
        );
        self.process_events(ui);

        let global_diff = self.show_canvas(ui);

        if global_diff.is_dirty() {
            self.has_queued_save_request = true;
            if global_diff.transformed.is_none() {
                self.toolbar.show_tool_controls = false;
                self.toolbar.show_viewport_popover = false;
            }
        }

        let needs_save_and_frame_is_cheap =
            if self.has_queued_save_request && !global_diff.is_dirty() {
                self.has_queued_save_request = false;
                true
            } else {
                false
            };

        Response { request_save: needs_save_and_frame_is_cheap }
    }

    fn process_events(&mut self, ui: &mut egui::Ui) {
        // self.show_debug_info(ui);

        if !ui.is_enabled() {
            return;
        }

        let mut tool_context = ToolContext {
            painter: &self.painter,
            buffer: &mut self.buffer,
            history: &mut self.history,
            allow_viewport_changes: &mut self.allow_viewport_changes,
            is_touch_frame: ui.input(|r| {
                r.events.iter().any(|e| {
                    matches!(
                        e,
                        egui::Event::Touch { device_id: _, id: _, phase: _, pos: _, force: _ }
                    )
                })
            }) || cfg!(target_os = "ios"),
        };

        if self.skip_frame {
            self.skip_frame = false;
            self.toolbar.pen.end_path(&mut tool_context, false);
            return;
        }

        match self.toolbar.active_tool {
            Tool::Pen => {
                self.toolbar.pen.handle_input(ui, &mut tool_context);
            }
            Tool::Highlighter => {
                self.toolbar.highlighter.handle_input(ui, &mut tool_context);
            }
            Tool::Eraser => {
                self.toolbar.eraser.handle_input(ui, &mut tool_context);
            }
            Tool::Selection => {
                self.toolbar.selection.handle_input(ui, &mut tool_context);
            }
        }

        self.toolbar
            .gesture_handler
            .handle_input(ui, &mut tool_context);
    }

    fn show_canvas(&mut self, ui: &mut egui::Ui) -> DiffState {
        ui.vertical(|ui| {
            self.renderer
                .render_svg(ui, &mut self.buffer, &mut self.painter)
        })
        .inner
    }

    // fn show_debug_info(&mut self, ui: &mut egui::Ui) {
    //     let frame_cost = Instant::now() - self.last_render;
    //     self.last_render = Instant::now();
    //     let mut anchor_count = 0;
    //     self.buffer
    //         .elements
    //         .iter()
    //         .filter(|(_, el)| !el.deleted())
    //         .for_each(|(_, el)| {
    //             if let parser::Element::Path(p) = el {
    //                 anchor_count += p.data.len()
    //             }
    //         });

    //     let mut top = self.inner_rect.right_top();
    //     top.x -= 150.0;
    //     if frame_cost.as_millis() != 0 {
    //         ui.painter().debug_text(
    //             top,
    //             egui::Align2::LEFT_TOP,
    //             egui::Color32::RED,
    //             format!("{} anchor | {}fps", anchor_count, 1000 / frame_cost.as_millis()),
    //         );
    //     }
    // }
}
