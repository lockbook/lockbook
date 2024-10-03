use bezier_rs::Subpath;
use egui::{PointerButton, TouchId, TouchPhase};
use lb_rs::Uuid;
use resvg::usvg::Transform;
use std::time::{Duration, Instant};
use tracing::{event, trace, warn, Level};
use tracing_test::traced_test;

use crate::{tab::ExtendedInput, theme::palette::ThemePalette};

use super::{
    parser::{self, DiffState, ManipulatorGroupId, Path, Stroke},
    toolbar::ToolContext,
    util::{get_event_touch_id, is_multi_touch},
    InsertElement, PathBuilder,
};

pub const DEFAULT_PEN_STROKE_WIDTH: f32 = 3.0;

#[derive(Default)]
pub struct Pen {
    pub active_color: Option<(egui::Color32, egui::Color32)>,
    pub active_stroke_width: f32,
    pub active_opacity: f32,
    path_builder: PathBuilder,
    pub current_id: Uuid, // todo: this should be at a higher component state, maybe in buffer
    maybe_snap_started: Option<Instant>,
}

impl Pen {
    pub fn new() -> Self {
        Pen {
            active_color: None,
            active_stroke_width: DEFAULT_PEN_STROKE_WIDTH,
            current_id: Uuid::new_v4(),
            path_builder: PathBuilder::new(),
            maybe_snap_started: None,
            active_opacity: 1.0,
        }
    }

    /// returns true if a path is being built
    pub fn handle_input(&mut self, ui: &mut egui::Ui, pen_ctx: &mut ToolContext) -> bool {
        if self.active_color.is_none() {
            self.active_color = Some(ThemePalette::get_fg_color());
        }

        if ui.input(|r| r.key_down(egui::Key::F2)) {
            self.path_builder.original_points.iter().for_each(|p| {
                ui.painter()
                    .circle(*p, 2.0, egui::Color32::RED, egui::Stroke::NONE);
            });
        } else if ui.input(|r| r.key_down(egui::Key::F3)) {
            self.path_builder.simplified_points.iter().for_each(|p| {
                ui.painter()
                    .circle(*p, 2.0, egui::Color32::BLUE, egui::Stroke::NONE);
            });
        }

        self.handle_path_event(PathEvent::ClearPredictedTouches, pen_ctx);

        for path_event in self.get_path_events(ui, pen_ctx) {
            self.handle_path_event(path_event, pen_ctx);

            if path_event == PathEvent::Break {
                return false;
            }
        }

        ui.ctx().pop_events().iter().for_each(|event| {
            if let crate::Event::PredictedTouch { id, force, pos } = *event {
                self.handle_path_event(
                    PathEvent::PredictedDraw(DrawPayload { pos, force, id: Some(id) }),
                    pen_ctx,
                );
            }
        });

        true
    }

    pub fn end_path(&mut self, pen_ctx: &mut ToolContext, is_snapped: bool) {
        if let Some(parser::Element::Path(path)) = pen_ctx.buffer.elements.get_mut(&self.current_id)
        {
            trace!("found path to end");
            self.path_builder.clear();

            let path = &mut path.data;
            if path.is_empty() {
                return;
            }

            if path.len() > 2 && is_snapped {
                self.path_builder
                    .snap(pen_ctx.buffer.master_transform, path);
            }

            pen_ctx
                .history
                .save(super::Event::Insert(vec![InsertElement { id: self.current_id }]));

            self.current_id = Uuid::new_v4();
        }
    }

    /// given a path event mutate state of the current path by building it, canceling it, or ending it.
    fn handle_path_event(&mut self, event: PathEvent, pen_ctx: &mut ToolContext) {
        match event {
            PathEvent::Draw(payload) => {
                if let Some(parser::Element::Path(p)) =
                    pen_ctx.buffer.elements.get_mut(&self.current_id)
                {
                    if let Some(touch_id) = payload.id {
                        if self.path_builder.first_point_touch_id.is_none() {
                            self.path_builder.first_point_touch_id = Some(touch_id);
                        }
                    }
                    // for some reason in ipad there are  two draw events on the same pos which results in a knot.
                    if let Some(last_pos) = self.path_builder.original_points.last() {
                        if last_pos.eq(&payload.pos) {
                            event!(Level::TRACE, ?payload.pos, "draw event canceled because it's pos is equal to the last pos on the path");
                            return;
                        }
                    }

                    // todo: snaping is broken, bring it back when the pen tool is stable
                    if self.detect_snap(&p.data, payload.pos, pen_ctx.buffer.master_transform) {
                        self.end_path(pen_ctx, true);
                        return;
                    }

                    p.diff_state.data_changed = true;

                    self.path_builder.cubic_to(
                        payload.pos,
                        &mut p.data,
                        pen_ctx.buffer.master_transform.sx,
                    );
                    event!(Level::TRACE, "drawing");
                } else {
                    let mut stroke = Stroke::default();
                    if let Some(c) = self.active_color {
                        stroke.color = c;
                    }

                    stroke.width = self.active_stroke_width;

                    self.path_builder.first_point_touch_id = payload.id;

                    event!(Level::TRACE, "starting a new path");

                    // pen_ctx.buffer.elements.insert(
                    let el = parser::Element::Path(Path {
                        data: Subpath::new(vec![], false),
                        visibility: resvg::usvg::Visibility::Visible,
                        fill: None,
                        stroke: Some(stroke),
                        transform: Transform::identity().post_scale(
                            pen_ctx.buffer.master_transform.sx,
                            pen_ctx.buffer.master_transform.sy,
                        ),
                        opacity: self.active_opacity,
                        diff_state: DiffState::default(),
                        deleted: false,
                    });
                    // );

                    // this is a highlighter insert at top z-index
                    if self.active_opacity < 1.0 {
                        pen_ctx
                            .buffer
                            .elements
                            .insert_before(0, self.current_id, el);
                    } else {
                        pen_ctx.buffer.elements.insert(self.current_id, el);
                    }
                    if let Some(parser::Element::Path(p)) =
                        pen_ctx.buffer.elements.get_mut(&self.current_id)
                    {
                        self.path_builder.cubic_to(
                            payload.pos,
                            &mut p.data,
                            pen_ctx.buffer.master_transform.sx,
                        );
                    }
                }
            }
            PathEvent::End => {
                self.end_path(pen_ctx, false);

                self.maybe_snap_started = None;
            }
            PathEvent::CancelStroke() => {
                trace!("canceling stroke");
                if let Some(parser::Element::Path(path)) =
                    pen_ctx.buffer.elements.get_mut(&self.current_id)
                {
                    self.path_builder.clear();
                    self.path_builder.is_canceled_path = true;
                    path.diff_state.data_changed = true;
                    path.data = Subpath::new(vec![], false);
                }
            }
            PathEvent::PredictedDraw(payload) => {
                if let Some(parser::Element::Path(p)) =
                    pen_ctx.buffer.elements.get_mut(&self.current_id)
                {
                    let maybe_new_mg = self.path_builder.cubic_to(
                        payload.pos,
                        &mut p.data,
                        pen_ctx.buffer.master_transform.sx,
                    );
                    trace!(maybe_new_mg, "adding predicted touch to the path at");

                    if self.path_builder.first_predicted_mg.is_none() && maybe_new_mg.is_some() {
                        self.path_builder.first_predicted_mg = maybe_new_mg;
                        trace!(maybe_new_mg, "setting start of mg");
                    }
                } else {
                    warn!("predicting touches on an empty path")
                }
            }
            PathEvent::ClearPredictedTouches => {
                if let Some(first_predicted_mg) = self.path_builder.first_predicted_mg {
                    if let Some(parser::Element::Path(p)) =
                        pen_ctx.buffer.elements.get_mut(&self.current_id)
                    {
                        for n in (first_predicted_mg..p.data.manipulator_groups().len()).rev() {
                            trace!(n, "removing predicted touch at ");
                            p.data.remove_manipulator_group(n);
                        }
                        self.path_builder.first_predicted_mg = None;
                    } else {
                        trace!("no path found ");
                    }
                }
            }
            PathEvent::Break => {}
        }
    }

    /// convert egui events into path events
    pub fn get_path_events(
        &mut self, ui: &mut egui::Ui, pen_ctx: &mut ToolContext,
    ) -> Vec<PathEvent> {
        let input_state = PenPointerInput { is_multi_touch: is_multi_touch(ui) };

        ui.input(|r| {
            r.events
                .iter()
                .filter_map(|e| self.map_ui_event(e, pen_ctx, &input_state))
                .rev()
                .collect()
        })
    }

    fn detect_snap(
        &mut self, path: &Subpath<ManipulatorGroupId>, current_pos: egui::Pos2,
        master_transform: Transform,
    ) -> bool {
        if path.len() < 2 {
            return false;
        }

        if let Some(last_pos) = path.iter().last() {
            let last_pos = last_pos.end();
            let last_pos = egui::pos2(last_pos.x as f32, last_pos.y as f32);

            let dist_diff = last_pos.distance(current_pos).abs();

            let mut dist_to_trigger_snap = 1.5;

            dist_to_trigger_snap /= master_transform.sx;

            let time_to_trigger_snap = Duration::from_secs(1);

            if dist_diff < dist_to_trigger_snap {
                if let Some(snap_start) = self.maybe_snap_started {
                    if Instant::now() - snap_start > time_to_trigger_snap {
                        self.maybe_snap_started = None;
                        return true;
                    }
                } else {
                    self.maybe_snap_started = Some(Instant::now());
                }
            } else {
                self.maybe_snap_started = Some(Instant::now());
            }
        }
        false
    }

    /// converts a single ui event into a path event  
    fn map_ui_event(
        &mut self, e: &egui::Event, pen_ctx: &mut ToolContext<'_>, input_state: &PenPointerInput,
    ) -> Option<PathEvent> {
        let is_current_path_empty = if let Some(parser::Element::Path(path)) =
            pen_ctx.buffer.elements.get_mut(&self.current_id)
        {
            path.data.is_empty()
        } else {
            true
        };
        let inner_rect = pen_ctx.painter.clip_rect();

        // todo: should i wait for input start
        if input_state.is_multi_touch {
            if is_current_path_empty {
                *pen_ctx.allow_viewport_changes = true;
                return Some(PathEvent::Break);
            }
            if !get_event_touch_id(e).eq(&self.path_builder.first_point_touch_id) {
                return None;
            }
        }

        *pen_ctx.allow_viewport_changes = false;

        if let egui::Event::Touch { device_id: _, id, phase, pos, force } = *e {
            if phase == TouchPhase::Cancel {
                trace!("sending cancel stroke");
                return Some(PathEvent::CancelStroke());
            }
            if phase == TouchPhase::End
                && inner_rect.contains(pos)
                && id.eq(&self.path_builder.first_point_touch_id.unwrap_or(TouchId(0)))
            {
                trace!("sending end path");
                return Some(PathEvent::End);
            }

            if !inner_rect.contains(pos) && !is_current_path_empty {
                trace!("ending path because it's out of canvas clip rect");
                return Some(PathEvent::End);
            }

            if phase != TouchPhase::End && inner_rect.contains(pos) {
                if self
                    .path_builder
                    .first_point_touch_id
                    .is_some_and(|first_touch| !first_touch.eq(&id))
                {
                    warn!("phantom stroke about to happen");
                    return Some(PathEvent::CancelStroke());
                }
                if phase == TouchPhase::Move
                    && self.path_builder.prev_points_window.is_empty()
                    && !self.path_builder.is_canceled_path
                {
                    trace!("probably lifted a finger off from a zoom.");
                    return None;
                }

                return Some(PathEvent::Draw(DrawPayload { pos, force, id: Some(id) }));
            }
        }
        if pen_ctx.is_touch_frame {
            *pen_ctx.allow_viewport_changes = true;
            return None;
        }

        if let egui::Event::PointerMoved(pos) = *e {
            if self.path_builder.original_points.is_empty() {
                return None;
            }

            if !inner_rect.contains(pos) {
                return Some(PathEvent::End);
            } else if pen_ctx.buffer.elements.contains_key(&self.current_id) {
                return Some(PathEvent::Draw(DrawPayload { pos, force: None, id: None }));
            }
        }
        if let egui::Event::PointerButton { pos, button, pressed, modifiers: _ } = *e {
            if !inner_rect.contains(pos) || button != PointerButton::Primary {
                return None;
            }

            if pressed {
                return Some(PathEvent::Draw(DrawPayload { pos, force: None, id: None }));
            } else {
                return Some(PathEvent::End);
            }
        }
        *pen_ctx.allow_viewport_changes = true;
        None
    }
}

struct PenPointerInput {
    is_multi_touch: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PathEvent {
    Draw(DrawPayload),
    PredictedDraw(DrawPayload),
    ClearPredictedTouches,
    End,
    CancelStroke(),
    Break,
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
    let mut pen = Pen::new();
    let mut pen_ctx = ToolContext {
        painter: &egui::Painter::new(
            egui::Context::default(),
            egui::LayerId::background(),
            egui::Rect::EVERYTHING,
        ),
        buffer: &mut parser::Buffer::default(),
        history: &mut crate::tab::svg_editor::history::History::default(),
        allow_viewport_changes: &mut false,
        is_touch_frame: true,
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
    if let Some(parser::Element::Path(p)) = pen_ctx.buffer.elements.get(&path_id) {
        assert!(p.data.is_empty());
        assert_eq!(pen.path_builder.original_points.len(), 1);
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

    let mut pen = Pen::new();
    let mut pen_ctx = ToolContext {
        painter: &egui::Painter::new(
            egui::Context::default(),
            egui::LayerId::background(),
            egui::Rect::EVERYTHING,
        ),
        buffer: &mut parser::Buffer::default(),
        history: &mut crate::tab::svg_editor::history::History::default(),
        allow_viewport_changes: &mut false,
        is_touch_frame: true,
    };

    let input_state = PenPointerInput { is_multi_touch: false };

    events.iter().for_each(|e| {
        if let Some(path_event) = pen.map_ui_event(e, &mut pen_ctx, &input_state) {
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
            pos: egui::pos2(13.0, 13.0),
            force: None,
        },
    ];

    events.iter().for_each(|e| {
        if let Some(path_event) = pen.map_ui_event(e, &mut pen_ctx, &input_state) {
            pen.handle_path_event(path_event, &mut pen_ctx);
        }
    });

    assert_eq!(pen_ctx.buffer.elements.len(), 1);
    assert_eq!(pen.path_builder.original_points.len(), 2) // the cancel touch doesn't count
}
