mod clip;
mod eraser;
mod history;
mod parser;
mod pen;
mod renderer;
mod selection;
mod toolbar;
mod util;
mod zoom;

use std::time::Instant;

use self::history::History;
use self::zoom::handle_zoom_input;
use crate::tab::svg_editor::toolbar::Toolbar;
pub use eraser::Eraser;
pub use history::DeleteElement;
pub use history::Event;
pub use history::InsertElement;
use lb_rs::Uuid;
pub use parser::Buffer;
use parser::DiffState;
pub use pen::CubicBezBuilder;
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
    buffer: parser::Buffer,
    history: History,
    pub toolbar: Toolbar,
    inner_rect: egui::Rect,
    core: lb_rs::Core,
    open_file: Uuid,
    skip_frame: bool,
    last_render: Instant,
    renderer: Renderer,
    painter: egui::Painter,
    has_queued_save_request: bool,
    /// don't allow zooming or panning
    allow_viewport_changes: bool,
    is_viewport_changing: bool,
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
    pub fn new(bytes: &[u8], ctx: &egui::Context, core: lb_rs::Core, open_file: Uuid) -> Self {
        let content = std::str::from_utf8(bytes).unwrap();

        let buffer = parser::Buffer::new(content, &core, open_file);

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
            last_render: Instant::now(),
            painter: egui::Painter::new(
                ctx.to_owned(),
                egui::LayerId::new(egui::Order::Background, "canvas_painter".into()),
                egui::Rect::NOTHING,
            ),
            renderer: Renderer::new(elements_count),
            has_queued_save_request: false,
            allow_viewport_changes: false,
            is_viewport_changing: false,
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) -> Response {
        let frame = ui.ctx().frame_nr();
        let span = span!(Level::TRACE, "showing canvas widget", frame);
        let _ = span.enter();

        self.process_events(ui);

        self.show_canvas(ui);

        let global_diff = self.get_and_reset_diff_state();

        if global_diff.is_dirty() {
            self.has_queued_save_request = true;
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

    fn get_and_reset_diff_state(&mut self) -> DiffState {
        let mut global_diff_state = DiffState::default();
        self.buffer.elements.iter_mut().for_each(|(_, element)| {
            if element.data_changed() {
                global_diff_state.data_changed = true;
            }
            if element.delete_changed() {
                global_diff_state.delete_changed = true;
            }
            if element.opacity_changed() {
                global_diff_state.opacity_changed = true;
            }
            if element.transformed().is_some() {
                global_diff_state.transformed = element.transformed();
            }

            match element {
                parser::Element::Path(p) => p.diff_state = DiffState::default(),
                parser::Element::Image(i) => i.diff_state = DiffState::default(),
                parser::Element::Text(_) => todo!(),
            }
        });
        global_diff_state
    }

    fn process_events(&mut self, ui: &mut egui::Ui) {
        // todo: toggle debug print before merge
        // if ui.input(|r| r.key_down(egui::Key::D)) {
        self.show_debug_info(ui);
        // }

        if !ui.is_enabled() {
            return;
        }

        if self.skip_frame {
            self.skip_frame = false;
            return;
        }

        let mut tool_context = ToolContext {
            painter: &self.painter,
            buffer: &mut self.buffer,
            history: &mut self.history,
            allow_viewport_changes: &mut self.allow_viewport_changes,
            is_viewport_changing: &mut self.is_viewport_changing,
            is_touch_frame: &mut false,
        };

        match self.toolbar.active_tool {
            Tool::Pen | Tool::Brush | Tool::Highlighter => {
                self.toolbar.pen.handle_input(ui, &mut tool_context);
            }
            Tool::Eraser => {
                self.toolbar.eraser.handle_input(ui, tool_context);
            }
            Tool::Selection => {
                self.toolbar.selection.handle_input(ui, tool_context);
            }
        }

        self.is_viewport_changing = if self.allow_viewport_changes
            && handle_zoom_input(ui, self.inner_rect, &mut self.buffer)
        {
            true
        } else {
            false
        };

        self.handle_clip_input(ui);
    }

    fn show_canvas(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            egui::Frame::default().show(ui, |ui| {
                self.toolbar.show(
                    ui,
                    &mut self.buffer,
                    &mut self.history,
                    &mut self.skip_frame,
                    self.inner_rect,
                );

                self.inner_rect = ui.available_rect_before_wrap();
                self.painter = ui
                    .allocate_painter(
                        ui.available_rect_before_wrap().size(),
                        egui::Sense::click_and_drag(),
                    )
                    .1;

                self.renderer
                    .render_svg(ui, &mut self.buffer, &mut self.painter);
            });
        });
    }

    fn show_debug_info(&mut self, ui: &mut egui::Ui) {
        let frame_cost = Instant::now() - self.last_render;
        self.last_render = Instant::now();
        let mut anchor_count = 0;
        self.buffer
            .elements
            .iter()
            .filter(|(_, el)| !el.deleted())
            .for_each(|(_, el)| {
                if let parser::Element::Path(p) = el {
                    anchor_count += p.data.len()
                }
            });

        let mut top = self.inner_rect.right_top();
        top.x -= 150.0;
        if frame_cost.as_millis() != 0 {
            ui.painter().debug_text(
                top,
                egui::Align2::LEFT_TOP,
                egui::Color32::RED,
                format!("{} anchor | {}fps", anchor_count, 1000 / frame_cost.as_millis()),
            );
        }
    }
}
