use bezier_rs::Subpath;
use lb_rs::Uuid;
use lb_rs::model::svg::buffer::{get_dyn_color, get_highlighter_colors, get_pen_colors};
use lb_rs::model::svg::diff::DiffState;
use lb_rs::model::svg::element::{Color, DynamicColor, Element, Path, Stroke};
use resvg::usvg::Transform;
use serde::{Deserialize, Serialize};
use tracing::{Level, event, trace};
use web_time::Instant;

use crate::tab::svg_editor::InsertElement;
use crate::tab::svg_editor::roger::{RogerEvent, ToolPayload};
use crate::tab::svg_editor::toolbar::ToolContext;
use crate::tab::svg_editor::tools::path_builder::PathBuilder;
use crate::theme::palette::ThemePalette;

pub const DEFAULT_PEN_STROKE_WIDTH: f32 = 1.0;
pub const DEFAULT_HIGHLIGHTER_STROKE_WIDTH: f32 = 15.0;

#[derive(Default)]
pub struct Pen {
    pub active_color: DynamicColor,
    pub colors_history: [DynamicColor; 2],
    pub active_stroke_width: f32,
    pub active_opacity: f32,
    pub pressure_alpha: f32,
    pub has_inf_thick: bool,
    path_builder: PathBuilder,
    pub current_id: Uuid, // todo: this should be at a higher component state, maybe in buffer
    maybe_snap_started: Option<Instant>,
}

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct PenSettings {
    pub color: egui::Color32,
    pub width: f32,
    pub opacity: f32,
    pub pressure_alpha: f32,
    pub has_inf_thick: bool,
}

impl Default for PenSettings {
    fn default() -> Self {
        PenSettings::default_pen()
    }
}

impl PenSettings {
    pub fn default_pen() -> Self {
        let color = get_pen_colors()[0].dark;

        Self {
            color: egui::Color32::from_rgb(color.red, color.green, color.blue),
            width: DEFAULT_PEN_STROKE_WIDTH,
            opacity: 1.0,
            pressure_alpha: if cfg!(target_os = "ios") || cfg!(target_os = "android") {
                0.5
            } else {
                0.0
            },
            has_inf_thick: false,
        }
    }
    pub fn default_highlighter() -> Self {
        let color = get_highlighter_colors()[0].dark;

        Self {
            color: egui::Color32::from_rgb(color.red, color.green, color.blue),
            width: DEFAULT_HIGHLIGHTER_STROKE_WIDTH,
            opacity: 0.1,
            pressure_alpha: 0.0,
            has_inf_thick: false,
        }
    }
}

impl Pen {
    pub fn new(settings: PenSettings) -> Self {
        let active_color = get_dyn_color(Color {
            red: settings.color.r(),
            green: settings.color.g(),
            blue: settings.color.b(),
        });

        let pen_colors = get_pen_colors();

        Pen {
            active_color,
            active_stroke_width: settings.width,
            current_id: Uuid::new_v4(),
            path_builder: PathBuilder::new(),
            maybe_snap_started: None,
            active_opacity: settings.opacity,
            has_inf_thick: settings.has_inf_thick,
            pressure_alpha: settings.pressure_alpha,
            colors_history: [pen_colors[1], pen_colors[2]],
        }
    }

    pub fn show_hover_point(
        &self, ui: &mut egui::Ui, pos: egui::Pos2, pen_ctx: &mut ToolContext<'_>,
    ) {
        let old_layer = pen_ctx.painter.layer_id();

        pen_ctx.painter.set_layer_id(egui::LayerId {
            order: egui::Order::PanelResizeLine,
            id: "pen_overlay".into(),
        });

        let is_current_path_empty =
            if let Some(Element::Path(path)) = pen_ctx.buffer.elements.get_mut(&self.current_id) {
                path.data.is_empty()
            } else {
                true
            };

        if is_current_path_empty {
            let mut radius = self.active_stroke_width / 2.0;
            if !self.has_inf_thick {
                radius *= pen_ctx.viewport_settings.master_transform.sx;
            }

            let pressure_adj = self.pressure_alpha * -0.5 + 1.;
            radius *= pressure_adj;

            pen_ctx.painter.circle_filled(
                pos,
                radius,
                ThemePalette::resolve_dynamic_color(self.active_color, ui.visuals().dark_mode),
            );
        }

        pen_ctx.painter.set_layer_id(old_layer);
    }

    pub fn end_path(&mut self, pen_ctx: &mut ToolContext, is_snapped: bool) {
        if let Some(Element::Path(path)) = pen_ctx.buffer.elements.get_mut(&self.current_id) {
            trace!("found path to end");
            self.path_builder.clear();

            let path = &mut path.data;
            if path.is_empty() {
                return;
            }

            if path.len() > 2 && is_snapped {
                self.path_builder
                    .snap(pen_ctx.viewport_settings.master_transform, path);
            }

            pen_ctx
                .history
                .save(crate::tab::svg_editor::Event::Insert(vec![InsertElement {
                    id: self.current_id,
                }]));

            self.current_id = Uuid::new_v4();
        }
    }

    /// given a path event mutate state of the current path by building it, canceling it, or ending it.
    pub fn handle_path_event(
        &mut self, ui: &mut egui::Ui, event: PathEvent, pen_ctx: &mut ToolContext,
    ) -> Option<egui::Shape> {
        match event {
            PathEvent::Draw(payload) => {
                let mut path_stroke = Stroke {
                    color: self.active_color,
                    opacity: self.active_opacity,
                    width: self.active_stroke_width,
                };

                if self.has_inf_thick {
                    path_stroke.width /= pen_ctx.viewport_settings.master_transform.sx;
                };

                if let Some(Element::Path(p)) = pen_ctx.buffer.elements.get_mut(&self.current_id) {
                    p.diff_state.data_changed = true;
                    p.stroke = Some(path_stroke);

                    let new_seg_i = self.path_builder.line_to(payload.pos, &mut p.data);

                    if new_seg_i.is_some() {
                        if let Some(force) = payload.force {
                            if let Some(forces) =
                                pen_ctx.buffer.weak_path_pressures.get_mut(&self.current_id)
                            {
                                forces.push((force * 2. - 1.) * self.pressure_alpha);
                            }
                        }
                    }
                } else {
                    event!(Level::TRACE, "starting a new path");
                    if let Some(force) = payload.force {
                        pen_ctx
                            .buffer
                            .weak_path_pressures
                            .insert(self.current_id, vec![(force * 2. - 1.) * self.pressure_alpha]);
                    }

                    let el = Element::Path(Path {
                        data: Subpath::new(vec![], false),
                        visibility: resvg::usvg::Visibility::Visible,
                        fill: None,
                        stroke: Some(path_stroke),
                        transform: Transform::identity(),
                        opacity: 1.0,
                        diff_state: DiffState::default(),
                        deleted: false,
                    });

                    pen_ctx
                        .buffer
                        .elements
                        .insert_before(0, self.current_id, el);

                    if let Some(Element::Path(p)) =
                        pen_ctx.buffer.elements.get_mut(&self.current_id)
                    {
                        self.path_builder.line_to(payload.pos, &mut p.data);
                    }
                }
            }
            PathEvent::End(payload) => {
                if let Some(Element::Path(p)) = pen_ctx.buffer.elements.get_mut(&self.current_id) {
                    p.diff_state.data_changed = true;

                    self.path_builder.line_to(payload.pos, &mut p.data);
                }
                self.end_path(pen_ctx, false);

                self.maybe_snap_started = None;
            }
            PathEvent::CancelStroke => {
                trace!("canceling stroke");
                self.cancel_path(pen_ctx);
            }
            PathEvent::PredictedDraw(payload) => {
                if let Some(Element::Path(p)) = pen_ctx.buffer.elements.get_mut(&self.current_id) {
                    let maybe_new_mg = self.path_builder.line_to(payload.pos, &mut p.data);
                    trace!(maybe_new_mg, "adding predicted touch to the path at");

                    // let's repeat the last known force for predicted touches
                    if maybe_new_mg.is_some() {
                        if let Some(forces) =
                            pen_ctx.buffer.weak_path_pressures.get_mut(&self.current_id)
                        {
                            if let Some(last_force) = forces.last() {
                                forces.push(*last_force);
                            }
                        }
                    }

                    if self.path_builder.first_predicted_mg.is_none() && maybe_new_mg.is_some() {
                        self.path_builder.first_predicted_mg = maybe_new_mg;
                        trace!(maybe_new_mg, "setting start of mg");
                    }
                }
            }
            PathEvent::ClearPredictedTouches => {
                if let Some(first_predicted_mg) = self.path_builder.first_predicted_mg {
                    if let Some(Element::Path(p)) =
                        pen_ctx.buffer.elements.get_mut(&self.current_id)
                    {
                        for n in (first_predicted_mg..p.data.manipulator_groups().len()).rev() {
                            trace!(n, "removing predicted touch at ");
                            p.data.remove_manipulator_group(n);

                            if let Some(forces) =
                                pen_ctx.buffer.weak_path_pressures.get_mut(&self.current_id)
                            {
                                forces.pop();
                            }
                        }
                        self.path_builder.first_predicted_mg = None;
                    } else {
                        trace!("no path found ");
                    }
                }
            }
        }
        None
    }

    fn cancel_path(&mut self, pen_ctx: &mut ToolContext<'_>) {
        if let Some(Element::Path(path)) = pen_ctx.buffer.elements.get_mut(&self.current_id) {
            self.path_builder.clear();
            self.path_builder.is_canceled_path = true;
            path.diff_state.data_changed = true;
            path.data = Subpath::new(vec![], false);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PathEvent {
    Draw(ToolPayload),
    PredictedDraw(ToolPayload),
    ClearPredictedTouches,
    End(ToolPayload),
    CancelStroke,
}

pub fn from_roger_to_pen_event(event: RogerEvent) -> Option<PathEvent> {
    match event {
        RogerEvent::ToolStart(tool_payload) => Some(PathEvent::Draw(tool_payload)),
        RogerEvent::ToolRun(tool_payload) => Some(PathEvent::Draw(tool_payload)),
        RogerEvent::ToolEnd(tool_payload) => Some(PathEvent::End(tool_payload)),
        RogerEvent::ToolCancel => Some(PathEvent::CancelStroke),
        RogerEvent::ToolHover(_) => None,
        RogerEvent::ViewportChange(_) => None,
        RogerEvent::Gesture(_) => None,
        RogerEvent::ViewportChangeWithToolCancel => Some(PathEvent::CancelStroke),
    }
}
