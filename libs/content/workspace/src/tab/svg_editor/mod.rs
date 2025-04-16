mod clip;
mod element;
mod eraser;
mod gesture_handler;
mod history;
mod path_builder;
mod pen;
mod renderer;
mod selection;
mod toolbar;
mod util;

use std::time::Instant;

use self::history::History;
use crate::tab::svg_editor::toolbar::Toolbar;
use element::PromoteWeakImage;
pub use eraser::Eraser;
pub use history::DeleteElement;
pub use history::Event;
pub use history::InsertElement;
use lb_rs::blocking::Lb;
use lb_rs::model::file_metadata::DocumentHmac;
use lb_rs::model::svg::buffer::u_transform_to_bezier;
use lb_rs::model::svg::buffer::Buffer;
use lb_rs::model::svg::diff::DiffState;
use lb_rs::model::svg::element::Element;
use lb_rs::model::svg::element::Image;
use lb_rs::Uuid;
pub use path_builder::PathBuilder;
pub use pen::Pen;
use renderer::Renderer;
pub use toolbar::Tool;
use toolbar::ToolContext;
use toolbar::ToolbarContext;
use toolbar::ViewportPopover;
use tracing::span;
use tracing::Level;

pub struct SVGEditor {
    pub buffer: Buffer,
    pub opened_content: Buffer,
    pub open_file_hmac: Option<DocumentHmac>,

    history: History,
    pub toolbar: Toolbar,
    inner_rect: egui::Rect,
    lb: Lb,
    pub open_file: Uuid,
    skip_frame: bool,
    last_render: Instant,
    renderer: Renderer,
    painter: egui::Painter,
    has_queued_save_request: bool,
    /// don't allow zooming or panning
    allow_viewport_changes: bool,
    pub settings: CanvasSettings,
    input_ctx: InputContext,
}

pub struct Response {
    pub request_save: bool,
}
#[derive(Clone, Copy)]
pub struct CanvasSettings {
    pub pencil_only_drawing: bool,
    show_dot_grid: bool,
}

impl Default for CanvasSettings {
    fn default() -> Self {
        Self { pencil_only_drawing: false, show_dot_grid: true }
    }
}

#[derive(PartialEq)]
pub enum CanvasOp {
    PanOrZoom,
    BuildingPath,
    Idle,
}
impl SVGEditor {
    pub fn new(
        bytes: &[u8], ctx: &egui::Context, lb: lb_rs::blocking::Lb, open_file: Uuid,
        hmac: Option<DocumentHmac>, maybe_settings: Option<CanvasSettings>,
    ) -> Self {
        let content = std::str::from_utf8(bytes).unwrap();

        let mut buffer = Buffer::new(content);
        for (_, el) in buffer.elements.iter_mut() {
            if let Element::Path(path) = el {
                path.data
                    .apply_transform(u_transform_to_bezier(&buffer.master_transform));
            }
        }

        let elements_count = buffer.elements.len();
        let toolbar = Toolbar::new(elements_count);

        Self {
            buffer,
            opened_content: Buffer::new(content),
            open_file_hmac: hmac,
            history: History::default(),
            toolbar,
            inner_rect: egui::Rect::NOTHING,
            lb,
            open_file,
            skip_frame: false,
            last_render: Instant::now(),
            painter: egui::Painter::new(
                ctx.to_owned(),
                egui::LayerId::new(egui::Order::Background, "canvas_painter".into()),
                egui::Rect::NOTHING,
            ),
            input_ctx: InputContext::default(),
            renderer: Renderer::new(elements_count),
            has_queued_save_request: false,
            allow_viewport_changes: false,
            settings: maybe_settings.unwrap_or_default(),
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) -> Response {
        let frame = ui.ctx().frame_nr();
        let span = span!(Level::TRACE, "showing canvas widget", frame);
        let _ = span.enter();

        self.inner_rect = ui.available_rect_before_wrap();
        ui.set_clip_rect(self.inner_rect);
        self.input_ctx.update(ui);

        let non_empty_weak_imaegs = !self.buffer.weak_images.is_empty();
        self.buffer
            .weak_images
            .drain()
            .for_each(|(id, mut weak_image)| {
                weak_image.transform(self.buffer.master_transform);

                let mut image = Image::from_weak(weak_image, &self.lb);

                image.diff_state.transformed = None;

                if weak_image.z_index >= self.buffer.elements.len() {
                    self.buffer.elements.insert(id, Element::Image(image));
                } else {
                    self.buffer.elements.shift_insert(
                        weak_image.z_index,
                        id,
                        Element::Image(image),
                    );
                };
            });

        ui.painter()
            .rect_filled(self.inner_rect, 0., ui.style().visuals.extreme_bg_color);

        self.painter = ui.painter_at(self.inner_rect);

        self.show_toolbar(ui);
        self.process_events(ui);

        self.show_dot_gird(ui);

        let global_diff = self.show_canvas(ui);

        if cfg!(debug_assertions) {
            self.show_debug_info(ui);
        }

        if non_empty_weak_imaegs {
            self.has_queued_save_request = true;
        }
        if global_diff.is_dirty() {
            self.has_queued_save_request = true;
            if global_diff.transformed.is_none() {
                self.toolbar.show_tool_controls = false;
                if !matches!(self.toolbar.viewport_popover, Some(ViewportPopover::MiniMap)) {
                    self.toolbar.viewport_popover = None;
                }
            }
        }

        let needs_save_and_frame_is_cheap =
            if self.has_queued_save_request && !global_diff.is_dirty() {
                self.has_queued_save_request = false;
                true
            } else {
                false
            };

        for (_, el) in &mut self.buffer.elements {
            match el {
                Element::Path(p) => p.diff_state = DiffState::default(),
                Element::Image(i) => i.diff_state = DiffState::default(),
                Element::Text(_) => todo!(),
            }
        }
        self.buffer.master_transform_changed = false;
        Response { request_save: needs_save_and_frame_is_cheap }
    }

    fn show_toolbar(&mut self, ui: &mut egui::Ui) {
        let mut toolbar_context = ToolbarContext {
            buffer: &mut self.buffer,
            history: &mut self.history,
            settings: &mut self.settings,
            painter: &mut self.painter,
        };

        ui.with_layer_id(
            egui::LayerId { order: egui::Order::Middle, id: egui::Id::from("canvas_ui_overlay") },
            |ui| {
                let mut ui = ui.child_ui(self.inner_rect, egui::Layout::default(), None);

                self.toolbar
                    .show(&mut ui, &mut toolbar_context, &mut self.skip_frame);
            },
        );
    }

    fn show_dot_gird(&self, ui: &mut egui::Ui) {
        if !self.settings.show_dot_grid {
            return;
        }

        let distance_between_dots = (30.0 * self.buffer.master_transform.sx).max(16.0);
        let dot_radius = (1. * self.buffer.master_transform.sx).max(0.6);

        let offset = egui::vec2(
            self.buffer
                .master_transform
                .tx
                .rem_euclid(distance_between_dots),
            self.buffer
                .master_transform
                .ty
                .rem_euclid(distance_between_dots),
        );

        let end = egui::vec2(
            (ui.clip_rect().right() + distance_between_dots) / distance_between_dots,
            (ui.clip_rect().bottom() + distance_between_dots) / distance_between_dots,
        );

        let mut dot = egui::Pos2::ZERO;
        for i in 0..=(end.y.ceil() as i32) {
            dot.x = 0.0;
            for j in 0..=(end.x.ceil() as i32) {
                let dot = egui::pos2(
                    j as f32 * distance_between_dots + offset.x,
                    i as f32 * distance_between_dots + offset.y,
                );

                ui.painter().circle(
                    dot,
                    dot_radius,
                    egui::Color32::GRAY.gamma_multiply(0.4),
                    egui::Stroke::NONE,
                );
            }
        }
    }

    fn process_events(&mut self, ui: &mut egui::Ui) {
        if !ui.is_enabled() {
            return;
        }

        self.handle_clip_input(ui);

        let mut tool_context = ToolContext {
            painter: &mut self.painter,
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
            settings: &mut self.settings,
            is_locked_vw_pen_only: self.toolbar.gesture_handler.is_locked_vw_pen_only_draw(),
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
                .render_svg(ui, &mut self.buffer, &mut self.painter, Default::default())
        })
        .inner
        .diff_state
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
                if let Element::Path(p) = el {
                    anchor_count += p.data.len()
                }
            });

        let mut top = self.inner_rect.right_top();
        top.x -= 250.0;
        top.y += 10.0;

        if frame_cost.as_millis() != 0 {
            ui.painter().debug_text(
                top,
                egui::Align2::LEFT_TOP,
                egui::Color32::RED,
                format!("{} anchor | {} fps", anchor_count, 1000 / frame_cost.as_millis()),
            );
        }
    }
}

// across frame persistent state about egui's input
#[derive(Default)]
struct InputContext {
    pub last_touch: Option<egui::Pos2>,
}

impl InputContext {
    fn update(&mut self, ui: &mut egui::Ui) {
        ui.input(|r| {
            r.events.iter().for_each(|e| {
                if let egui::Event::Touch { device_id: _, id: _, phase: _, pos, force: _ } = e {
                    self.last_touch = Some(*pos);
                }
            })
        })
    }
}
