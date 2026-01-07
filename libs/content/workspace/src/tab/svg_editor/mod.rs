mod background;
mod clip;
mod element;
mod eraser;
mod gesture_handler;
mod history;
mod path_builder;
mod pen;
mod renderer;
mod selection;
mod shapes;
mod toolbar;
mod util;

use std::time::Instant;

use self::history::History;
use crate::tab::ExtendedInput;
use crate::tab::svg_editor::toolbar::Toolbar;
use crate::theme::palette::ThemePalette;
use crate::workspace::WsPersistentStore;

use colors_transform::Color;
use egui::UiBuilder;
use element::PromoteBufferWeakImages;
pub use eraser::Eraser;
pub use history::{DeleteElement, Event, InsertElement};
use lb_rs::Uuid;
use lb_rs::blocking::Lb;
use lb_rs::model::file_metadata::DocumentHmac;
use lb_rs::model::svg::buffer::{Buffer, u_transform_to_bezier};
use lb_rs::model::svg::diff::DiffState;
use lb_rs::model::svg::element::{DynamicColor, Element};
pub use path_builder::PathBuilder;
pub use pen::Pen;
use pen::PenSettings;
use renderer::Renderer;
use resvg::usvg::Transform;
use serde::{Deserialize, Serialize};
pub use toolbar::Tool;
use toolbar::{ToolContext, ToolbarContext};
use tracing::{Level, info, span};

pub struct SVGEditor {
    pub buffer: Buffer,
    pub opened_content: Buffer,
    pub open_file_hmac: Option<DocumentHmac>,

    pub cfg: WsPersistentStore,

    history: History,
    pub toolbar: Toolbar,
    lb: Lb,
    pub open_file: Uuid,
    has_islands_interaction: bool,
    last_render: Instant,
    renderer: Renderer,
    painter: egui::Painter,
    has_queued_save_request: bool,
    pub viewport_settings: ViewportSettings,
    /// don't allow zooming or panning
    allow_viewport_changes: bool,
    pub settings: CanvasSettings,
    input_ctx: InputContext,

    read_only: bool,
}
#[derive(Debug, Clone, Copy)]
pub struct ViewportSettings {
    /// the drawable rect in the viewport-transformed plane.
    /// **only defined if there's a lock**
    pub bounded_rect: Option<egui::Rect>,
    /// the intersection of the bounded rect and the current screen rect
    pub working_rect: egui::Rect,
    pub viewport_transform: Option<Transform>,
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
            viewport_transform: None,
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
    pub fn new(
        bytes: &[u8], ctx: &egui::Context, lb: lb_rs::blocking::Lb, open_file: Uuid,
        hmac: Option<DocumentHmac>, cfg: &WsPersistentStore, read_only: bool,
    ) -> Self {
        let content = std::str::from_utf8(bytes).unwrap();

        let mut buffer = Buffer::new(content);
        let mut viewport_settings = ViewportSettings::from(buffer.weak_viewport_settings);

        for (_, el) in buffer.elements.iter_mut() {
            if let Element::Path(path) = el {
                path.data
                    .apply_transform(u_transform_to_bezier(&viewport_settings.master_transform));
            }
        }

        let elements_count = buffer.elements.len();

        let mut cfg = cfg.clone();
        let settings = cfg.get_canvas_settings();

        let toolbar = Toolbar::new(elements_count, &settings);

        if viewport_settings.master_transform == Transform::identity() && buffer.elements.is_empty()
        {
            viewport_settings.bottom_locked = settings.bottom_locked;
            viewport_settings.left_locked = settings.left_locked;
            viewport_settings.right_locked = settings.right_locked;
            viewport_settings.top_locked = settings.top_locked;
            viewport_settings.bounded_rect = Some(ctx.available_rect());
        }

        if read_only {
            info!(id=?open_file, "creating canvas with read only mode");
        }

        Self {
            buffer,
            opened_content: Buffer::new(content),
            open_file_hmac: hmac,
            history: History::default(),
            toolbar,
            lb,
            open_file,
            has_islands_interaction: false,
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
            settings,
            viewport_settings,
            cfg,
            read_only,
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) -> Response {
        set_style(ui);

        let span = span!(Level::TRACE, "showing canvas widget");
        let _ = span.enter();

        self.viewport_settings.container_rect = ui.available_rect_before_wrap();
        self.input_ctx.update(ui);
        self.buffer.weak_viewport_settings = self.viewport_settings.into();

        let non_empty_weak_imaegs = !self.buffer.weak_images.is_empty();
        self.buffer
            .promote_weak_images(self.viewport_settings.master_transform, &self.lb);

        let request_diff_change = self.show_toolbar(ui);

        if !request_diff_change {
            for (_, el) in &mut self.buffer.elements {
                match el {
                    Element::Path(p) => p.diff_state = DiffState::default(),
                    Element::Image(i) => i.diff_state = DiffState::default(),
                    Element::Text(_) => todo!(),
                }
            }
        }
        self.process_events(ui);

        self.painter = ui.painter_at(self.viewport_settings.working_rect);

        self.paint_background_colors(ui);
        ui.set_clip_rect(self.viewport_settings.working_rect);

        self.show_background_overlay();
        let global_diff = self.show_canvas(ui);

        if cfg!(debug_assertions) {
            self.show_debug_info(ui);
        }
        self.viewport_settings.update_working_rect(
            self.settings,
            &self.buffer,
            &global_diff,
            self.toolbar.hide_overlay,
        );

        if non_empty_weak_imaegs {
            self.has_queued_save_request = true;
        }
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

        self.has_islands_interaction = false;
        self.buffer.master_transform_changed = false;
        ui.ctx().pop_events();
        Response { request_save: !self.read_only && needs_save_and_frame_is_cheap }
    }

    fn show_toolbar(&mut self, ui: &mut egui::Ui) -> bool {
        let mut toolbar_context = ToolbarContext {
            buffer: &mut self.buffer,
            history: &mut self.history,
            settings: &mut self.settings,
            painter: &mut self.painter,
            viewport_settings: &mut self.viewport_settings,
            cfg: &mut self.cfg,
            input_ctx: &self.input_ctx,
            read_only: self.read_only,
        };

        ui.scope_builder(
            UiBuilder::new().layer_id(egui::LayerId {
                order: egui::Order::Middle,
                id: egui::Id::from("canvas_ui_overlay"),
            }),
            |ui| {
                let mut ui = ui.new_child(
                    UiBuilder::new()
                        .max_rect(ui.available_rect_before_wrap())
                        .layout(egui::Layout::default()),
                );

                self.toolbar
                    .show(&mut ui, &mut toolbar_context, &mut self.has_islands_interaction)
            },
        )
        .inner
    }

    fn process_events(&mut self, ui: &mut egui::Ui) {
        if !ui.is_enabled() {
            return;
        }

        if !self.read_only {
            if ui.input_mut(|r| {
                r.consume_key(egui::Modifiers::COMMAND.plus(egui::Modifiers::SHIFT), egui::Key::Z)
            }) {
                self.history.redo(&mut self.buffer);
            }

            if ui.input_mut(|r| r.consume_key(egui::Modifiers::COMMAND, egui::Key::Z)) {
                self.history.undo(&mut self.buffer);
            }

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
            viewport_settings: &mut self.viewport_settings,
            toolbar_has_interaction: self.has_islands_interaction,
        };

        if has_click_outside_islands && self.toolbar.has_visible_popover() {
            self.toolbar
                .close_all_popovers(tool_context.settings, &mut self.cfg);
            return;
        }

        if !self.read_only {
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
                Tool::Shapes => self.toolbar.shapes_tool.handle_input(ui, &mut tool_context),
            }
        } else {
            *tool_context.allow_viewport_changes = true;
        }

        if !self.has_islands_interaction {
            self.toolbar
                .gesture_handler
                .handle_input(ui, &mut tool_context);
        }
    }

    fn show_canvas(&mut self, ui: &mut egui::Ui) -> DiffState {
        ui.vertical(|ui| {
            self.renderer.render_svg(
                ui,
                &mut self.buffer,
                &mut self.painter,
                Default::default(),
                self.viewport_settings.master_transform,
            )
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

fn set_style(ui: &mut egui::Ui) {
    let toolbar_margin = egui::Margin::symmetric(15.0, 7.0);
    ui.visuals_mut().window_rounding = egui::Rounding::same(30.0);
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
            egui::Stroke::new(0.5, egui::Color32::from_rgb(56, 56, 56));
        ui.visuals_mut().window_fill = egui::Color32::from_rgb(30, 30, 30);
        ui.visuals_mut().window_shadow = egui::Shadow::NONE;
    } else {
        ui.visuals_mut().window_stroke =
            egui::Stroke::new(0.5, egui::Color32::from_rgb(235, 235, 235));
        ui.visuals_mut().window_shadow = egui::Shadow {
            offset: egui::vec2(1.0, 8.0),
            blur: 20.0,
            spread: 0.0,
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
