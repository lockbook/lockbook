mod background;
mod clip;
mod coordinates;
mod element;
mod history;
mod keyboard_shortcuts;
#[cfg(test)]
mod tests;
mod viewport;

mod renderer;
pub(super) mod toolbar;
pub(super) mod tools;
mod util;

use egui::UiBuilder;
use web_time::Instant;

use self::history::History;
use crate::tab::ExtendedInput;
use crate::tab::input_controller::{
    InputController, InputControllerConfig, InputControllerEvent, LayoutContext,
};
use crate::tab::svg_editor::toolbar::Toolbar;
use crate::tab::svg_editor::viewport::transform_canvas;
use crate::theme::palette::ThemePalette;
use crate::workspace::WsPersistentStore;

use colors_transform::Color;
pub use element::CanvasImages;
use element::PromoteBufferWeakImages;
pub use history::{DeleteElement, Event, InsertElement};
use lb_rs::Uuid;
use lb_rs::model::file_metadata::DocumentHmac;
use lb_rs::model::svg::buffer::Buffer;
use lb_rs::model::svg::diff::DiffState;
use lb_rs::model::svg::element::{DynamicColor, EditableElement};
use renderer::Renderer;
use resvg::usvg::Transform;
use serde::{Deserialize, Serialize};
pub use toolbar::Tool;
use toolbar::{ToolContext, ToolbarContext};
use tools::InputControllerTool;
use tools::pen::PenSettings;
pub use tools::{
    eraser::EraseEvent, pen::PathEvent, selection::SelectionEvent, shapes::ShapeEvent,
};
use tracing::info;

pub struct SVGEditor {
    /// Drawn alone inside another surface: no toolbar, and the context's
    /// events are left for the host.
    pub embedded: bool,
    pub buffer: Buffer,
    pub opened_content: Buffer,
    pub open_file_hmac: Option<DocumentHmac>,

    pub cfg: WsPersistentStore,

    history: History,
    pub toolbar: Toolbar,

    input_controller: InputController<CanvasAction>,
    images: Box<dyn CanvasImages>,
    pub open_file: Uuid,
    last_render: Instant,
    renderer: Renderer,
    painter: egui::Painter,
    has_queued_save_request: bool,
    pub viewport_settings: ViewportSettings,
    pub settings: CanvasSettings,
    input_ctx: InputContext,

    read_only: bool,
    viewport_initialized: bool,
    pending_viewport: Option<Transform>,
    viewport_write_pending: Option<Instant>,
}
#[derive(Debug, Clone, Copy)]
pub struct ViewportSettings {
    /// The drawable bounds in document coordinates.
    /// **only defined if there's a lock**
    pub bounded_rect: Option<egui::Rect>,
    /// the intersection of the bounded rect and the current screen rect
    pub working_rect: egui::Rect,
    pub master_transform: Transform,
    container_rect: egui::Rect,
    pub left_locked: bool,
    pub right_locked: bool,
    pub bottom_locked: bool,
    pub top_locked: bool,
}

impl Default for ViewportSettings {
    fn default() -> Self {
        Self {
            bounded_rect: None,
            working_rect: egui::Rect::ZERO,
            master_transform: Transform::identity(),
            left_locked: false,
            right_locked: false,
            bottom_locked: false,
            top_locked: false,
            container_rect: egui::Rect::ZERO,
        }
    }
}

pub struct Response {
    pub request_save: bool,
}
#[derive(Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct CanvasSettings {
    pub pencil_only_drawing: bool,
    background_type: BackgroundOverlay,
    background_color: DynamicColor,
    pub left_locked: bool,
    pub right_locked: bool,
    pub bottom_locked: bool,
    pub top_locked: bool,
    pub pen: PenSettings,
    pub show_mini_map: bool,
}

impl Default for CanvasSettings {
    fn default() -> Self {
        Self {
            pencil_only_drawing: false,
            left_locked: false,
            right_locked: false,
            bottom_locked: false,
            top_locked: false,
            pen: PenSettings::default_pen(),
            background_type: BackgroundOverlay::default(),
            show_mini_map: true,
            background_color: DynamicColor {
                light: lb_rs::model::svg::element::Color::white(),
                dark: lb_rs::model::svg::element::Color::black(),
            },
        }
    }
}

#[derive(PartialEq)]
pub enum CanvasOp {
    PanOrZoom,
    BuildingPath,
    Idle,
}

#[derive(Default, Clone, Copy, Serialize, Deserialize, PartialEq)]
enum BackgroundOverlay {
    #[default]
    Dots,
    Lines,
    Grid,
    Blank,
}

impl SVGEditor {
    pub(crate) fn reload_document(&mut self, content: &str, hmac: Option<DocumentHmac>) {
        let remote = Buffer::new(content);
        Buffer::reload(
            &mut self.buffer.elements,
            &mut self.buffer.weak_images,
            &mut self.buffer.weak_path_pressures,
            &self.opened_content,
            &remote,
        );
        self.buffer.promote_weak_images(&*self.images);
        self.viewport_settings.update_working_rect(
            self.settings,
            &self.buffer,
            &DiffState::new(),
            self.toolbar.hide_overlay,
        );
        self.renderer.request_rerender = true;
        self.toolbar.invalidate_document();
        self.opened_content = remote;
        self.open_file_hmac = hmac;
    }

    fn persist_viewport(&mut self, ctx: &egui::Context) {
        if !self.viewport_initialized {
            return;
        }
        let transform = self.viewport_settings.viewport_transform();
        if !viewport::is_valid_transform(&transform) {
            return;
        }
        {
            let transform = transform.into();
            let mut data = self.cfg.data.write().unwrap();
            if data.canvas_transforms.get(&self.open_file) != Some(&transform) {
                data.canvas_transforms.insert(self.open_file, transform);
                self.viewport_write_pending = Some(Instant::now());
            }
        }
        if let Some(changed) = self.viewport_write_pending {
            let debounce = std::time::Duration::from_millis(150);
            if changed.elapsed() >= debounce {
                self.flush_viewport();
            } else {
                ctx.request_repaint_after(debounce.saturating_sub(changed.elapsed()));
            }
        }
    }

    pub(crate) fn flush_viewport(&mut self) {
        if self.viewport_write_pending.take().is_some() {
            self.cfg.write_to_file();
        }
    }

    pub fn new(
        bytes: &[u8], ctx: &egui::Context, images: Box<dyn CanvasImages>, open_file: Uuid,
        hmac: Option<DocumentHmac>, cfg: &WsPersistentStore, read_only: bool,
    ) -> Self {
        let content = std::str::from_utf8(bytes).unwrap();

        let buffer = Buffer::new(content);
        let mut viewport_settings = ViewportSettings::default();

        let elements_count = buffer.elements.len();

        let mut cfg = cfg.clone();
        let settings = cfg.get_canvas_settings();
        let pending_viewport = cfg
            .data
            .read()
            .unwrap()
            .canvas_transforms
            .get(&open_file)
            .copied()
            .map(Transform::from)
            .filter(viewport::is_valid_transform);

        let toolbar = Toolbar::new(elements_count, &settings);

        viewport_settings.bottom_locked = settings.bottom_locked;
        viewport_settings.left_locked = settings.left_locked;
        viewport_settings.right_locked = settings.right_locked;
        viewport_settings.top_locked = settings.top_locked;

        if read_only {
            info!(id=?open_file, "creating canvas with read only mode");
        }

        Self {
            embedded: false,
            buffer,
            opened_content: Buffer::new(content),
            open_file_hmac: hmac,
            history: History::default(),
            toolbar,
            images,
            open_file,
            last_render: Instant::now(),
            painter: egui::Painter::new(
                ctx.to_owned(),
                egui::LayerId::new(egui::Order::Background, egui::Id::unique("canvas_painter")),
                egui::Rect::NOTHING,
            ),
            input_ctx: InputContext::default(),
            renderer: Renderer::new(elements_count),
            has_queued_save_request: false,
            settings,
            viewport_settings,
            cfg,
            read_only,
            input_controller: InputController::new(InputControllerConfig::new(
                settings.pencil_only_drawing,
                read_only,
                keyboard_shortcuts::resolve,
            )),
            viewport_initialized: false,
            pending_viewport,
            viewport_write_pending: None,
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) -> Response {
        set_style(ui);

        self.buffer.promote_weak_images(&*self.images);
        let container = ui.available_rect_before_wrap();
        if container.is_finite() && container.is_positive() {
            if !self.viewport_initialized {
                self.viewport_settings.initialize(
                    container,
                    &self.buffer,
                    self.pending_viewport.take(),
                );
                self.viewport_initialized = true;
            } else {
                self.viewport_settings.update_container(container);
            }
        }
        self.input_ctx.update(ui);
        if !ui.is_enabled() {
            self.toolbar.selection.cancel_opacity_preview();
            self.toolbar.eraser.cancel_preview();
        }

        self.viewport_settings.update_working_rect(
            self.settings,
            &self.buffer,
            &DiffState::default(),
            self.toolbar.hide_overlay,
        );

        for el in self.buffer.elements.values_mut() {
            match el {
                EditableElement::Path(p) => p.diff_state = DiffState::default(),
                EditableElement::Image(i) => i.diff_state = DiffState::default(),
                EditableElement::Text(_) => todo!(),
            }
        }
        let previous_brush_size = self.toolbar.active_brush_size();
        if !self.embedded {
            self.show_toolbar(ui);
        }
        self.input_controller.sync_canvas_settings(&self.settings);
        // an embed's host scrolls by the wheel
        self.input_controller.pan_on_wheel = !self.embedded;

        self.process_events(ui, previous_brush_size);

        self.viewport_settings.update_working_rect(
            self.settings,
            &self.buffer,
            &DiffState::default(),
            self.toolbar.hide_overlay,
        );

        self.painter = ui.painter_at(self.viewport_settings.working_rect);

        self.paint_background_colors(ui);
        ui.set_clip_rect(
            self.viewport_settings
                .working_rect
                .intersect(ui.clip_rect()),
        );

        self.show_background_overlay();
        let global_diff = self.show_canvas(ui);

        if cfg!(debug_assertions) {
            self.show_debug_info(ui);
        }
        self.viewport_settings.update_working_rect(
            self.settings,
            &self.buffer,
            &global_diff,
            self.toolbar.hide_overlay || self.embedded,
        );

        if global_diff.is_dirty() {
            // The minimap paints before tool input. Keep it invalidated for
            // the next frame, when these edit flags have been cleared.
            self.toolbar.invalidate_document();
            ui.ctx().request_repaint();
            self.has_queued_save_request = true;
        }

        let needs_save_and_frame_is_cheap =
            if self.has_queued_save_request && !global_diff.is_dirty() {
                self.has_queued_save_request = false;
                true
            } else {
                false
            };

        self.persist_viewport(ui.ctx());
        if !self.embedded {
            ui.ctx().pop_events();
        }

        Response { request_save: !self.read_only && needs_save_and_frame_is_cheap }
    }

    fn show_toolbar(&mut self, ui: &mut egui::Ui) -> bool {
        let controls_rect =
            crate::safe_area::inset_rect(ui.ctx(), self.viewport_settings.container_rect);
        let mut toolbar_context = ToolbarContext {
            controls_rect,
            buffer: &mut self.buffer,
            history: &mut self.history,
            settings: &mut self.settings,
            painter: &mut self.painter,
            viewport_settings: &mut self.viewport_settings,
            cfg: &mut self.cfg,
            input_ctx: &self.input_ctx,
            read_only: self.read_only,
        };

        let overlay_layer =
            egui::LayerId::new(egui::Order::Middle, egui::Id::unique("canvas_ui_overlay"));
        ui.scope_builder(UiBuilder::new().layer_id(overlay_layer), |ui| {
            let mut ui = ui.new_child(
                UiBuilder::new()
                    .max_rect(controls_rect)
                    .layout(egui::Layout::default()),
            );
            self.toolbar.show(&mut ui, &mut toolbar_context)
        })
        .inner
    }

    fn process_events(&mut self, ui: &mut egui::Ui, previous_brush_size: Option<f32>) {
        if !ui.is_enabled() {
            return;
        }

        if self.input_controller.keyboard_enabled(ui) {
            self.handle_clip_input(ui);
        }

        let has_click_outside_islands = ui.input(|r| {
            r.events.iter().any(|e| match *e {
                egui::Event::PointerButton { pos, button: _, pressed, modifiers: _ } => {
                    pressed && !self.detect_islands_interaction(pos)
                }
                egui::Event::Touch { device_id: _, id: _, phase, pos, force: _ } => {
                    phase == egui::TouchPhase::Start && !self.detect_islands_interaction(pos)
                }
                _ => false,
            })
        });

        let mut tool_context = ToolContext {
            painter: &mut self.painter,
            buffer: &mut self.buffer,
            history: &mut self.history,
            settings: &mut self.settings,
            viewport_settings: &mut self.viewport_settings,
        };

        if has_click_outside_islands && self.toolbar.has_visible_popover() {
            self.toolbar
                .close_all_popovers(tool_context.settings, &mut self.cfg);
            return;
        }

        let layout_ctx = LayoutContext::new(
            tool_context.viewport_settings.working_rect,
            self.toolbar.get_rects(),
        );

        self.toolbar
            .active_tool_mut()
            .show_tool_ui(ui, &mut tool_context);

        let events = self.input_controller.process(ui, &layout_ctx);

        for event in events {
            let action = match event {
                InputControllerEvent::Action(action) => Some(action),
                InputControllerEvent::ViewportChange(transform) => {
                    Some(CanvasAction::TransformViewport(transform))
                }
                InputControllerEvent::Gesture(2) => Some(CanvasAction::Undo),
                InputControllerEvent::Gesture(3) => Some(CanvasAction::Redo),
                InputControllerEvent::Gesture(_) => None,
                _ => self
                    .toolbar
                    .active_tool_mut()
                    .controller_event_to_tool_event(event),
            };
            if let Some(action) = action {
                execute_action(ui, action, &mut self.toolbar, &mut tool_context, &mut self.cfg);
            }
        }

        // Toolbar controls already produce canvas commands; execute them here
        // after input events, alongside commands from keyboard shortcuts.
        for event in ui.ctx().read_events() {
            if let crate::tab::Event::Canvas(action) = event {
                if !self.read_only || matches!(action, CanvasAction::TransformViewport(_)) {
                    execute_action(ui, action, &mut self.toolbar, &mut tool_context, &mut self.cfg);
                }
            }
        }

        if previous_brush_size != self.toolbar.active_brush_size() {
            self.input_controller
                .refresh_hover_indicator(ui.ctx(), &layout_ctx);
        }
        self.input_controller.show_hover_indicator(
            ui,
            &mut tool_context,
            self.toolbar.active_tool_mut(),
        );

        self.toolbar.input_controller_interrupt = self.input_controller.should_hide_overlay();
    }

    fn show_canvas(&mut self, ui: &mut egui::Ui) -> DiffState {
        ui.vertical(|ui| {
            self.renderer.render_svg(
                ui,
                coordinates::CanvasView::new(&self.buffer, self.viewport_settings.coordinates()),
                &mut self.painter,
                |id, opacity| {
                    self.toolbar.eraser.preview_opacity(
                        id,
                        self.toolbar
                            .selection
                            .preview_opacity(id)
                            .unwrap_or(opacity),
                    )
                },
            )
        })
        .inner
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
                if let EditableElement::Path(p) = el {
                    anchor_count += p.data.len()
                }
            });

        let mut top = self.viewport_settings.container_rect.right_top();
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

/// A command or tool interaction executed by the canvas.
#[derive(Clone, Debug, PartialEq)]
pub enum CanvasAction {
    PasteImages {
        images: Vec<(Uuid, std::sync::Arc<Vec<u8>>)>,
        position: egui::Pos2,
    },
    Pen(PathEvent),
    Highlighter(PathEvent),
    Erase(EraseEvent),
    Shape(ShapeEvent),
    Selection(SelectionEvent),
    TransformViewport(Transform),
    DeleteSelection,
    CutSelection,
    CopySelection,
    SelectAll,
    Undo,
    Redo,
    SetTool(Tool),
    ToggleToolPopover,
    SetSelectionColor(DynamicColor),
    SetSelectionWidth(f32),
    SetSelectionOpacity(f32),
    MoveSelectionLayer(LayerMove),
    /// for the pen or highlighter, whichever is active
    SetPenColor(DynamicColor),
    SetPenWidth(f32),
    /// Step the active pen/highlighter width or eraser radius.
    AdjustBrushSize(f32),
    SetPenOpacity(f32),
}

/// Index 0 is drawn on top.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayerMove {
    ToBack,
    Backward,
    Forward,
    ToFront,
}

fn execute_action(
    ui: &mut egui::Ui, action: CanvasAction, toolbar: &mut Toolbar, ctx: &mut ToolContext,
    cfg: &mut WsPersistentStore,
) {
    use CanvasAction as A;
    if matches!(action, A::Undo | A::Redo) {
        toolbar.selection.cancel_opacity_preview();
        toolbar.eraser.cancel_preview();
    }
    match action {
        A::PasteImages { images, position } => {
            clip::paste_images(ui, images, position, toolbar, ctx)
        }
        A::Pen(event) => toolbar.pen.handle_tool_event(ui, event, ctx),
        A::Highlighter(event) => toolbar.highlighter.handle_tool_event(ui, event, ctx),
        A::Erase(event) => toolbar.eraser.handle_tool_event(ui, event, ctx),
        A::Shape(event) => toolbar.shapes_tool.handle_tool_event(ui, event, ctx),
        A::Selection(event) => toolbar.selection.handle_tool_event(ui, event, ctx),
        A::TransformViewport(transform) => {
            if ui.is_enabled() {
                transform_canvas(ctx.viewport_settings, transform);
            }
        }
        A::Undo => ctx.history.undo(ctx.buffer),
        A::Redo => ctx.history.redo(ctx.buffer),
        A::SetTool(tool) => toolbar.set_tool(tool, ctx.settings, cfg),
        A::ToggleToolPopover => toolbar.toggle_at_cursor_tool_popover(),
        A::DeleteSelection => toolbar
            .selection
            .handle_tool_event(ui, SelectionEvent::Delete, ctx),
        A::CopySelection => toolbar
            .selection
            .handle_tool_event(ui, SelectionEvent::Copy, ctx),
        A::CutSelection => toolbar
            .selection
            .handle_tool_event(ui, SelectionEvent::Cut, ctx),
        A::SelectAll => {
            if toolbar.active_tool != Tool::Selection {
                toolbar.set_tool(Tool::Selection, ctx.settings, cfg);
            }
            toolbar
                .selection
                .handle_tool_event(ui, SelectionEvent::SelectAll, ctx);
        }
        A::SetSelectionColor(color) => toolbar.selection.set_color(ctx, color),
        A::SetSelectionWidth(width) => toolbar.selection.set_width(ctx, width),
        A::SetSelectionOpacity(opacity) => toolbar.selection.set_opacity(ctx, opacity),
        A::MoveSelectionLayer(layer) => toolbar.selection.move_layer(ctx, layer),
        A::SetPenColor(color) => toolbar.active_pen().change_color(color),
        A::SetPenWidth(width) => toolbar.active_pen().active_stroke_width = width,
        A::AdjustBrushSize(steps) => {
            toolbar.adjust_brush_size(steps, ctx.viewport_settings.coordinates().zoom());
        }
        A::SetPenOpacity(opacity) => toolbar.active_pen().active_opacity = opacity,
    }
}

impl Drop for SVGEditor {
    fn drop(&mut self) {
        self.flush_viewport();
    }
}

fn set_style(ui: &mut egui::Ui) {
    let toolbar_margin = egui::Margin::symmetric(15, 7);
    ui.visuals_mut().window_corner_radius = egui::CornerRadius::same(30);
    ui.style_mut().spacing.window_margin = toolbar_margin;
    ui.style_mut()
        .text_styles
        .insert(egui::TextStyle::Body, egui::FontId::new(13.0, egui::FontFamily::Proportional));
    ui.style_mut()
        .text_styles
        .insert(egui::TextStyle::Button, egui::FontId::new(13.0, egui::FontFamily::Proportional));

    ui.visuals_mut().widgets.active.bg_fill =
        ui.visuals_mut().widgets.active.bg_fill.linear_multiply(0.7);

    if ui.visuals().dark_mode {
        ui.visuals_mut().window_stroke =
            egui::Stroke::new(0.5_f32, egui::Color32::from_rgb(56, 56, 56));
        ui.visuals_mut().window_fill = egui::Color32::from_rgb(30, 30, 30);
        ui.visuals_mut().window_shadow = egui::Shadow::NONE;
    } else {
        ui.visuals_mut().window_stroke =
            egui::Stroke::new(0.5_f32, egui::Color32::from_rgb(235, 235, 235));
        ui.visuals_mut().window_shadow = egui::Shadow {
            offset: [1, 8],
            blur: 20,
            spread: 0,
            color: egui::Color32::from_black_alpha(10),
        };
        ui.visuals_mut().window_fill = ui.visuals().extreme_bg_color;
    }
}

// across frame persistent state about egui's input
#[derive(Default)]
pub struct InputContext {
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

impl CanvasSettings {
    fn get_secondary_background_color(&self, ui: &mut egui::Ui) -> egui::Color32 {
        let color =
            ThemePalette::resolve_dynamic_color(self.background_color, ui.visuals().dark_mode);

        get_secondary_color(color)
    }
}

pub fn get_secondary_color(color: egui::Color32) -> egui::Color32 {
    let mut secondary =
        colors_transform::Rgb::from(color.r().into(), color.g().into(), color.b().into());

    let lightness = secondary.get_lightness();
    if lightness < 50.0 {
        secondary = secondary.set_lightness(lightness + 20.0)
    } else {
        secondary = secondary.set_lightness(lightness - 20.0)
    };
    egui::Color32::from_rgb(
        secondary.get_red() as u8,
        secondary.get_green() as u8,
        secondary.get_blue() as u8,
    )
}
