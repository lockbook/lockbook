use bezier_rs::{Bezier, Subpath};
use egui::{PointerButton, TouchId, TouchPhase};
use lb_rs::Uuid;
use resvg::usvg::Transform;
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};
use tracing::{event, trace, warn, Level};
use tracing_test::traced_test;

use crate::theme::palette::ThemePalette;

use super::{
    parser::{self, DiffState, ManipulatorGroupId, Path, Stroke},
    toolbar::ToolContext,
    util::{get_event_touch_id, is_multi_touch},
    InsertElement,
};

pub const DEFAULT_PEN_STROKE_WIDTH: f32 = 3.0;

#[derive(Default)]
pub struct Pen {
    pub active_color: Option<(egui::Color32, egui::Color32)>,
    pub active_stroke_width: f32,
    pub active_opacity: f32,
    path_builder: CubicBezBuilder,
    pub current_id: Uuid, // todo: this should be at a higher component state, maybe in buffer
    maybe_snap_started: Option<Instant>,
}

impl Pen {
    pub fn new() -> Self {
        Pen {
            active_color: None,
            active_stroke_width: DEFAULT_PEN_STROKE_WIDTH,
            current_id: Uuid::new_v4(),
            path_builder: CubicBezBuilder::new(),
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

        for path_event in self.get_path_events(ui, pen_ctx) {
            self.handle_path_event(path_event, pen_ctx);
            if path_event == PathEvent::Break || path_event == PathEvent::End {
                break;
            }
        }
        false
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

                    self.path_builder.cubic_to(payload.pos, &mut p.data);
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
                        self.path_builder.cubic_to(payload.pos, &mut p.data);
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
            _ => {}
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
                *pen_ctx.allow_viewport_changes = true;
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

/// Build a cubic bézier path with Catmull-Rom smoothing and Ramer–Douglas–Peucker compression
#[derive(Debug)]
pub struct CubicBezBuilder {
    /// store the 4 past points
    prev_points_window: VecDeque<egui::Pos2>,
    simplified_points: Vec<egui::Pos2>,
    original_points: Vec<egui::Pos2>,
    first_point_touch_id: Option<egui::TouchId>,
    is_canceled_path: bool,
}

impl Default for CubicBezBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl CubicBezBuilder {
    pub fn new() -> Self {
        CubicBezBuilder {
            prev_points_window: VecDeque::from(vec![]),
            first_point_touch_id: None,
            simplified_points: vec![],
            original_points: vec![],
            is_canceled_path: false,
        }
    }

    fn line_to(&mut self, dest: egui::Pos2, path: &mut Subpath<ManipulatorGroupId>) {
        self.original_points.push(dest);
        if let Some(prev) = self.prev_points_window.back() {
            let bez = Bezier::from_linear_coordinates(
                prev.x.into(),
                prev.y.into(),
                dest.x.into(),
                dest.y.into(),
            );
            path.append_bezier(&bez, bezier_rs::AppendType::IgnoreStart);
        }
        self.prev_points_window.push_back(dest);
    }

    fn catmull_to(&mut self, dest: egui::Pos2, path: &mut Subpath<ManipulatorGroupId>) {
        self.prev_points_window.push_back(dest);

        if self.prev_points_window.len() < 3 {
            return;
        }

        if self.prev_points_window.len() == 3 {
            self.prev_points_window.push_back(dest);
        }

        let (p0, p1, p2, p3) = (
            self.prev_points_window[0],
            self.prev_points_window[1],
            self.prev_points_window[2],
            self.prev_points_window[3],
        );

        let cp1x = p1.x + (p2.x - p0.x) / 10.; // * k, k is tension which is set to 1, 0 <= k <= 1
        let cp1y = p1.y + (p2.y - p0.y) / 10.;

        let cp2x = p2.x - (p3.x - p1.x) / 10.;
        let cp2y = p2.y - (p3.y - p1.y) / 10.;

        let bez = Bezier::from_cubic_coordinates(
            self.prev_points_window.back().unwrap().x.into(),
            self.prev_points_window.back().unwrap().y.into(),
            cp1x.into(),
            cp1y.into(),
            cp2x.into(),
            cp2y.into(),
            p2.x.into(),
            p2.y.into(),
        );
        path.append_bezier(&bez, bezier_rs::AppendType::IgnoreStart);

        // shift the window foreword
        self.prev_points_window.pop_front();
    }

    pub fn cubic_to(&mut self, dest: egui::Pos2, path: &mut Subpath<ManipulatorGroupId>) {
        if self.prev_points_window.is_empty() {
            self.original_points.clear();
        }
        self.original_points.push(dest);
        self.catmull_to(dest, path);
    }

    pub fn snap(&mut self, master_transform: Transform, path: &mut Subpath<ManipulatorGroupId>) {
        let perim = path.length(None) as f32;
        let mut tolerance = perim * 0.04;

        tolerance *= master_transform.sx;
        let maybe_simple_points = self.simplify(tolerance, path);

        self.clear();

        if let Some(simple_points) = maybe_simple_points {
            self.simplified_points = simple_points.clone();
            simple_points.iter().enumerate().for_each(|(_, p)| {
                self.line_to(*p, path);
            });
        }
    }

    pub fn clear(&mut self) {
        self.prev_points_window.clear();
        self.first_point_touch_id = None;
        self.is_canceled_path = false;
    }

    /// Ramer–Douglas–Peucker algorithm courtesy of @author: Michael-F-Bryan
    /// https://github.com/Michael-F-Bryan/arcs/blob/master/core/src/algorithms/line_simplification.rs
    fn simplify(
        &mut self, tolerance: f32, path: &Subpath<ManipulatorGroupId>,
    ) -> Option<Vec<egui::Pos2>> {
        let mut simplified_points = Vec::new();

        // push the first point
        let mut points = vec![];
        path.iter().for_each(|b| {
            points.push(egui::pos2(b.start().x as f32, b.start().y as f32));
            points.push(egui::pos2(b.end().x as f32, b.end().y as f32));
        });

        simplified_points.push(points[0]);

        // then simplify every point in between the start and end
        self.simplify_points(&points, tolerance, &mut simplified_points);
        // and finally the last one
        simplified_points.push(*points.last().unwrap());

        Some(simplified_points)
    }

    fn simplify_points(&self, points: &[egui::Pos2], tolerance: f32, buffer: &mut Vec<egui::Pos2>) {
        if points.len() < 2 {
            return;
        }
        let first = points.first().unwrap();
        let last = points.last().unwrap();
        let rest = &points[1..points.len() - 1];

        let line_segment = Line::new(*first, *last);

        if let Some((ix, distance)) =
            self.max_by_key(rest, |p| line_segment.perpendicular_distance_to(*p))
        {
            if distance > tolerance {
                // note: index is the index into `rest`, but we want it relative
                // to `point`
                let ix = ix + 1;

                self.simplify_points(&points[..=ix], tolerance, buffer);
                buffer.push(points[ix]);
                self.simplify_points(&points[ix..], tolerance, buffer);
            }
        }
    }

    fn max_by_key<T, F, K>(&self, items: &[T], mut key_func: F) -> Option<(usize, K)>
    where
        F: FnMut(&T) -> K,
        K: PartialOrd,
    {
        let mut best_so_far = None;

        for (i, item) in items.iter().enumerate() {
            let key = key_func(item);

            let is_better = match best_so_far {
                Some((_, ref best_key)) => key > *best_key,
                None => true,
            };

            if is_better {
                best_so_far = Some((i, key));
            }
        }
        best_so_far
    }
}

struct Line {
    start: egui::Pos2,
    end: egui::Pos2,
}

impl Line {
    fn new(start: egui::Pos2, end: egui::Pos2) -> Self {
        Line { start, end }
    }

    fn length(&self) -> f32 {
        let dx = self.end.x - self.start.x;
        let dy = self.end.y - self.start.y;

        // Using the Pythagorean theorem to calculate the length
        (dx * dx + dy * dy).sqrt()
    }

    fn perpendicular_distance_to(&self, point: egui::Pos2) -> f32 {
        const SOME_SMALL_NUMBER: f32 = std::f32::EPSILON * 100.0;

        let side_a = self.start - point;
        let side_b = self.end - point;

        let area = (side_a.x * side_b.y - side_a.y * side_b.x) / 2.0;

        // area = base * height / 2
        let base_length = self.length();

        if base_length.abs() < SOME_SMALL_NUMBER {
            side_a.length()
        } else {
            area.abs() * 2.0 / base_length
        }
    }
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
