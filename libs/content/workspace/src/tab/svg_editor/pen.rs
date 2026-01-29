use bezier_rs::Subpath;
use egui::{PointerButton, TouchId, TouchPhase};
use egui_animation::{animate_bool_eased, easing};
use lb_rs::Uuid;
use lb_rs::model::svg::buffer::{get_dyn_color, get_highlighter_colors, get_pen_colors};
use lb_rs::model::svg::diff::DiffState;
use lb_rs::model::svg::element::{Color, DynamicColor, Element, Path, Stroke};
use resvg::usvg::Transform;
use serde::{Deserialize, Serialize};
use tracing::{Level, event, trace};
use tracing_test::traced_test;
use web_time::{Duration, Instant};

use crate::tab::ExtendedInput;
use crate::tab::svg_editor::util::is_scroll;
use crate::theme::palette::ThemePalette;

use super::toolbar::ToolContext;
use super::util::is_multi_touch;
use super::{InsertElement, PathBuilder};

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
    hover_pos: Option<(egui::Pos2, Instant)>,
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
            pressure_alpha: if cfg!(target_os = "ios") { 0.5 } else { 0.0 },
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

#[derive(Clone)]
enum IntegrationEvent<'a> {
    Custom(&'a crate::Event),
    Native(&'a egui::Event),
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
            hover_pos: None,
            has_inf_thick: settings.has_inf_thick,
            pressure_alpha: settings.pressure_alpha,
            colors_history: [pen_colors[1], pen_colors[2]],
        }
    }

    /// returns true if a path is being built
    pub fn handle_input(&mut self, ui: &mut egui::Ui, pen_ctx: &mut ToolContext) -> bool {
        if pen_ctx.toolbar_has_interaction {
            self.cancel_path(pen_ctx);
        }

        let input_state =
            PenPointerInput { is_multi_touch: is_multi_touch(ui), is_scroll: is_scroll(ui) };
        let mut is_drawing = false;

        // clear the previous predicted touches and replace them with the actual touches
        if self.path_builder.first_predicted_mg.is_some() {
            self.handle_path_event(PathEvent::ClearPredictedTouches, pen_ctx);
        }

        // handle std egui input events
        ui.input(|r| {
            r.events.iter().for_each(|e| {
                if let Some(path_event) =
                    self.map_ui_event(IntegrationEvent::Native(e), pen_ctx, &input_state)
                {
                    trace!(?path_event, "native events");
                    self.handle_path_event(path_event, pen_ctx);
                    if matches!(path_event, PathEvent::Draw(..)) {
                        is_drawing = true;
                    }
                }
            });
        });

        // handle custom input events
        ui.ctx().read_events().iter().for_each(|e| {
            if let Some(path_event) =
                self.map_ui_event(IntegrationEvent::Custom(e), pen_ctx, &input_state)
            {
                self.handle_path_event(path_event, pen_ctx);
                if matches!(path_event, PathEvent::Draw(..)) {
                    is_drawing = true;
                }
            }
        });

        // draw hover pos
        self.show_hover_point(ui, pen_ctx);

        is_drawing
    }

    fn show_hover_point(&mut self, ui: &mut egui::Ui, pen_ctx: &mut ToolContext<'_>) {
        let old_layer = pen_ctx.painter.layer_id();

        pen_ctx.painter.set_layer_id(egui::LayerId {
            order: egui::Order::PanelResizeLine,
            id: "eraser_overlay".into(),
        });

        if let Some((pos, instant)) = self.hover_pos {
            let is_current_path_empty = if let Some(Element::Path(path)) =
                pen_ctx.buffer.elements.get_mut(&self.current_id)
            {
                path.data.is_empty()
            } else {
                true
            };
            let opacity = animate_bool_eased(
                ui.ctx(),
                "pen_hover_pos",
                Instant::now() - instant < Duration::from_millis(10)
                    && !(pen_ctx.settings.pencil_only_drawing && pen_ctx.is_locked_vw_pen_only),
                easing::cubic_in_out,
                0.5,
            );

            if is_current_path_empty
                && !(pen_ctx.settings.pencil_only_drawing && pen_ctx.is_locked_vw_pen_only)
            {
                let mut radius = self.active_stroke_width / 2.0;
                if !self.has_inf_thick {
                    radius *= pen_ctx.viewport_settings.master_transform.sx;
                }

                let pressure_adj = self.pressure_alpha * -0.5 + 1.;
                radius *= pressure_adj;

                pen_ctx.painter.circle_filled(
                    pos,
                    radius,
                    ThemePalette::resolve_dynamic_color(self.active_color, ui.visuals().dark_mode)
                        .linear_multiply(self.active_opacity * opacity),
                );
            }
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
                .save(super::Event::Insert(vec![InsertElement { id: self.current_id }]));

            self.current_id = Uuid::new_v4();
        }
    }

    /// given a path event mutate state of the current path by building it, canceling it, or ending it.
    fn handle_path_event(
        &mut self, event: PathEvent, pen_ctx: &mut ToolContext,
    ) -> Option<egui::Shape> {
        match event {
            PathEvent::Draw(payload) => {
                if payload.force.is_none() && pen_ctx.settings.pencil_only_drawing {
                    return None;
                }

                if let Some(touch_id) = payload.id {
                    if self.path_builder.first_point_touch_id.is_none() {
                        self.path_builder.first_point_touch_id = Some(touch_id);
                    }
                }

                if self.path_builder.first_point_frame.is_none() {
                    self.path_builder.first_point_frame = Some(Instant::now());
                }

                let has_same_touch_id_as_curr_path = if let Some(curr_id) = payload.id {
                    if let Some(first_point_touch_id) = self.path_builder.first_point_touch_id {
                        first_point_touch_id == curr_id
                    } else {
                        true
                    }
                } else {
                    true
                };

                if !has_same_touch_id_as_curr_path {
                    self.cancel_path(pen_ctx);
                }

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
            PathEvent::Hover(draw_payload) => {
                self.hover_pos = Some((draw_payload.pos, Instant::now()));
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

    /// converts a single ui event into a path event  
    fn map_ui_event(
        &mut self, e: IntegrationEvent, pen_ctx: &mut ToolContext<'_>,
        input_state: &PenPointerInput,
    ) -> Option<PathEvent> {
        let is_current_path_empty =
            if let Some(Element::Path(path)) = pen_ctx.buffer.elements.get_mut(&self.current_id) {
                path.data.is_empty()
            } else {
                true
            };
        let inner_rect = pen_ctx.painter.clip_rect();
        let has_same_touch_id_as_curr_path = get_event_touch_id(&e).is_some_and(|curr_id| {
            if let Some(first_point_touch_id) = self.path_builder.first_point_touch_id {
                first_point_touch_id == curr_id
            } else {
                false
            }
        });

        if input_state.is_multi_touch {
            if let Some(first_point_frame) = self.path_builder.first_point_frame {
                if Instant::now() - first_point_frame < Duration::from_millis(500) {
                    trace!("drew stroke for a bit but then shifted to a vw change");
                    *pen_ctx.allow_viewport_changes = true;
                    return Some(PathEvent::CancelStroke);
                }
            }

            if is_current_path_empty {
                trace!("path is empty on a multi touch allow zoom");
                *pen_ctx.allow_viewport_changes = true;
                return None;
            }

            if !has_same_touch_id_as_curr_path {
                *pen_ctx.allow_viewport_changes = false;
                return None;
            }
        }

        if let IntegrationEvent::Native(&egui::Event::Touch {
            device_id: _,
            id,
            phase,
            pos,
            force,
        }) = e
        {
            *pen_ctx.allow_viewport_changes = false;
            match phase {
                TouchPhase::Start => {
                    if is_current_path_empty && inner_rect.contains(pos) {
                        trace!("start path");
                        return Some(PathEvent::Draw(DrawPayload { pos, force, id: Some(id) }));
                    }
                }
                TouchPhase::Move => {
                    if inner_rect.contains(pos) && !is_current_path_empty {
                        trace!("continue draw path");
                        return Some(PathEvent::Draw(DrawPayload { pos, force, id: Some(id) }));
                    }
                }
                TouchPhase::End => {
                    if !is_current_path_empty {
                        trace!("end path");
                        return Some(PathEvent::End(DrawPayload { pos, force, id: Some(id) }));
                    }
                }
                TouchPhase::Cancel => {
                    if inner_rect.contains(pos) {
                        trace!("cancel path");
                        return Some(PathEvent::CancelStroke);
                    }
                }
            }
        }

        if let IntegrationEvent::Custom(&crate::Event::PredictedTouch { id, force, pos }) = e {
            *pen_ctx.allow_viewport_changes = false;
            if inner_rect.contains(pos) && !is_current_path_empty && has_same_touch_id_as_curr_path
            {
                trace!("draw predicted");
                return Some(PathEvent::PredictedDraw(DrawPayload { pos, force, id: Some(id) }));
            }
        }

        if pen_ctx.is_touch_frame {
            *pen_ctx.allow_viewport_changes = true;
            // shouldn't handle non touch events on touch devices to avoid breaking ipad hover.
            if let IntegrationEvent::Native(&egui::Event::PointerMoved(pos)) = e {
                if is_current_path_empty && !input_state.is_scroll {
                    return Some(PathEvent::Hover(DrawPayload { pos, force: None, id: None }));
                } else {
                    *pen_ctx.allow_viewport_changes = false;
                }
            }
            return None;
        }

        // todo figure out if there's common ground between multi touch and zoom/scroll events
        // it would to handle viewport lock for the touch and pointer events in the same spot
        if matches!(e, IntegrationEvent::Native(&egui::Event::Zoom(_)))
            || matches!(
                e,
                IntegrationEvent::Native(&egui::Event::MouseWheel {
                    unit: _,
                    delta: _,
                    modifiers: _
                })
            )
        {
            if is_current_path_empty {
                *pen_ctx.allow_viewport_changes = true;
                return None;
            } else {
                *pen_ctx.allow_viewport_changes = false;
                return None;
            }
        }

        if let IntegrationEvent::Native(&egui::Event::PointerButton {
            pos,
            button,
            pressed,
            modifiers: _,
        }) = e
        {
            *pen_ctx.allow_viewport_changes = false;
            if button != PointerButton::Primary {
                return None;
            }

            // equivalent to touch started
            if pressed {
                if is_current_path_empty && inner_rect.contains(pos) {
                    return Some(PathEvent::Draw(DrawPayload { pos, force: None, id: None }));
                }
                // equivalent to touch end
            } else if !is_current_path_empty {
                {
                    return Some(PathEvent::End(DrawPayload { pos, force: None, id: None }));
                }
            }
        }

        if let IntegrationEvent::Native(&egui::Event::PointerMoved(pos)) = e {
            *pen_ctx.allow_viewport_changes = false;
            if inner_rect.contains(pos) && !is_current_path_empty {
                return Some(PathEvent::Draw(DrawPayload { pos, force: None, id: None }));
            }
        }

        *pen_ctx.allow_viewport_changes = true;
        None
    }
}

fn get_event_touch_id(event: &IntegrationEvent) -> Option<egui::TouchId> {
    match event {
        IntegrationEvent::Custom(custom_event) => {
            if let crate::Event::PredictedTouch { id, force: _, pos: _ } = custom_event {
                Some(*id)
            } else {
                None
            }
        }
        IntegrationEvent::Native(native_event) => {
            if let egui::Event::Touch { device_id: _, id, phase: _, pos: _, force: _ } =
                native_event
            {
                Some(*id)
            } else {
                None
            }
        }
    }
}

#[derive(Clone, Copy)]
struct PenPointerInput {
    is_multi_touch: bool,
    is_scroll: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PathEvent {
    Draw(DrawPayload),
    Hover(DrawPayload),
    PredictedDraw(DrawPayload),
    ClearPredictedTouches,
    End(DrawPayload),
    CancelStroke,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DrawPayload {
    pos: egui::Pos2,
    force: Option<f32>,
    id: Option<TouchId>,
}

#[traced_test]
#[test]
fn correct_start_of_path() {
    let mut pen = Pen::new(PenSettings::default_pen());
    let mut pen_ctx = ToolContext {
        painter: &mut egui::Painter::new(
            egui::Context::default(),
            egui::LayerId::background(),
            egui::Rect::EVERYTHING,
        ),
        buffer: &mut lb_rs::model::svg::buffer::Buffer::default(),
        history: &mut crate::tab::svg_editor::history::History::default(),
        allow_viewport_changes: &mut false,
        is_touch_frame: true,
        settings: &mut crate::tab::svg_editor::CanvasSettings::default(),
        is_locked_vw_pen_only: false,
        viewport_settings: &mut Default::default(),
        toolbar_has_interaction: false,
    };

    let start_pos = egui::pos2(10.0, 10.0);
    let path_id = Uuid::new_v4();
    pen.current_id = path_id;
    let touch_id = TouchId(1);

    let events =
        vec![PathEvent::Draw(DrawPayload { pos: start_pos, force: None, id: Some(touch_id) })];

    for event in &events {
        pen.handle_path_event(*event, &mut pen_ctx);
    }
    if let Some(Element::Path(p)) = pen_ctx.buffer.elements.get(&path_id) {
        assert_eq!(p.data.len(), 2);
        // assert_eq!(pen.path_builder.original_points.len(), 1);
    }
}

#[traced_test]
#[test]
fn cancel_touch_ui_event() {
    let touch_1 = TouchId(1);
    let touch_2 = TouchId(2);
    let mut events = vec![
        egui::Event::Touch {
            device_id: egui::TouchDeviceId(1),
            id: touch_1,
            phase: TouchPhase::Start,
            pos: egui::pos2(10.0, 10.0),
            force: None,
        },
        egui::Event::Touch {
            device_id: egui::TouchDeviceId(1),
            id: touch_1,
            phase: TouchPhase::Move,
            pos: egui::pos2(11.0, 11.0),
            force: None,
        },
    ];

    let mut pen = Pen::new(PenSettings::default_pen());
    let mut pen_ctx = ToolContext {
        painter: &mut egui::Painter::new(
            egui::Context::default(),
            egui::LayerId::background(),
            egui::Rect::EVERYTHING,
        ),
        buffer: &mut lb_rs::model::svg::buffer::Buffer::default(),
        history: &mut crate::tab::svg_editor::history::History::default(),
        allow_viewport_changes: &mut false,
        is_touch_frame: true,
        settings: &mut crate::tab::svg_editor::CanvasSettings::default(),
        is_locked_vw_pen_only: false,
        viewport_settings: &mut Default::default(),
        toolbar_has_interaction: false,
    };

    let input_state = PenPointerInput { is_multi_touch: false, is_scroll: false };

    events.iter().for_each(|e| {
        if let Some(path_event) =
            pen.map_ui_event(IntegrationEvent::Native(e), &mut pen_ctx, &input_state)
        {
            pen.handle_path_event(path_event, &mut pen_ctx);
        }
    });

    events = vec![
        egui::Event::Touch {
            device_id: egui::TouchDeviceId(1),
            id: touch_1,
            phase: TouchPhase::Cancel,
            pos: egui::pos2(11.0, 11.0),
            force: None,
        },
        egui::Event::Touch {
            device_id: egui::TouchDeviceId(1),
            id: touch_2,
            phase: TouchPhase::Start,
            pos: egui::pos2(12.0, 12.0),
            force: None,
        },
        egui::Event::Touch {
            device_id: egui::TouchDeviceId(1),
            id: touch_2,
            phase: TouchPhase::Move,
            pos: egui::pos2(16.0, 16.0),
            force: None,
        },
    ];

    events.iter().for_each(|e| {
        if let Some(path_event) =
            pen.map_ui_event(IntegrationEvent::Native(e), &mut pen_ctx, &input_state)
        {
            pen.handle_path_event(path_event, &mut pen_ctx);
        }
    });
    assert_eq!(pen_ctx.buffer.elements.len(), 1);

    if let Some(Element::Path(path)) = pen_ctx.buffer.elements.get(&pen.current_id) {
        assert_eq!(path.data.len(), 3)
    }
}
