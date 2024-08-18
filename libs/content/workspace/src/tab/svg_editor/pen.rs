use bezier_rs::{Bezier, Subpath};
use resvg::usvg::Transform;
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

use crate::{tab::svg_editor::util::get_current_touch_id, theme::palette::ThemePalette};

use super::{
    history::History,
    parser::{self, DiffState, ManipulatorGroupId, Path, Stroke},
    Buffer, InsertElement,
};

pub struct Pen {
    pub active_color: Option<(egui::Color32, egui::Color32)>,
    pub active_stroke_width: u32,
    pub active_opacity: f32,
    path_builder: CubicBezBuilder,
    pub current_id: usize, // todo: this should be at a higher component state, maybe in buffer
    maybe_snap_started: Option<Instant>,
}

impl Pen {
    pub fn new(max_id: usize) -> Self {
        let default_stroke_width = 3;

        Pen {
            active_color: None,
            active_stroke_width: default_stroke_width,
            current_id: max_id,
            path_builder: CubicBezBuilder::new(),
            maybe_snap_started: None,
            active_opacity: 1.0,
        }
    }

    pub fn handle_input(
        &mut self, ui: &mut egui::Ui, inner_rect: egui::Rect, buffer: &mut parser::Buffer,
        history: &mut History,
    ) {
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

        for event in self.setup_events(ui, inner_rect) {
            match event {
                PathEvent::Draw(payload, id) => {
                    // for some reason in ipad there are  two draw events on the same pos which results in a knot.
                    if let Some(last_pos) = self.path_builder.original_points.last() {
                        if last_pos.eq(&payload.pos) && self.path_builder.path.len() > 1 {
                            return;
                        }
                    }

                    if self.detect_snap(payload.pos, buffer.master_transform) {
                        return self.end_path(buffer, history, true);
                    }

                    if let Some(parser::Element::Path(p)) = buffer.elements.get_mut(&id.to_string())
                    {
                        p.diff_state.data_changed = true;

                        if self.handle_dancing_lines(p) {
                            return;
                        }

                        self.avoid_phantom_strokes(ui);

                        self.path_builder.cubic_to(payload.pos);
                        p.data = self.path_builder.path.clone();

                        if let Some(f) = payload.force {
                            if let Some(pressure) = &mut p.pressure {
                                pressure.push(f)
                            } else {
                                p.pressure = Some(vec![f]);
                            }
                        // sometimes the force is missing from the event so just autofill it based on the last force
                        } else if let Some(pressure) = &mut p.pressure {
                            pressure.push(*pressure.last().unwrap_or(&1.0))
                        }
                    } else {
                        self.path_builder.cubic_to(payload.pos);
                        let mut stroke = Stroke::default();
                        if let Some(c) = self.active_color {
                            stroke.color = c;
                        }
                        stroke.width = self.active_stroke_width as f32;

                        let pressure = payload.force.map(|f| vec![f]);
                        self.path_builder.first_point_touch_id = get_current_touch_id(ui);

                        buffer.elements.insert(
                            id.to_string(),
                            parser::Element::Path(Path {
                                data: self.path_builder.path.clone(),
                                visibility: resvg::usvg::Visibility::Visible,
                                fill: None,
                                stroke: Some(stroke),
                                transform: Transform::identity().post_scale(
                                    buffer.master_transform.sx,
                                    buffer.master_transform.sy,
                                ),
                                opacity: self.active_opacity,
                                pressure,
                                diff_state: DiffState::default(),
                                deleted: false,
                            }),
                        );
                    }
                }
                PathEvent::End => {
                    self.end_path(buffer, history, false);

                    self.maybe_snap_started = None;
                }
            }
        }
    }

    fn avoid_phantom_strokes(&mut self, ui: &mut egui::Ui) {
        let current_touch_id = get_current_touch_id(ui);
        if !current_touch_id.eq(&self.path_builder.first_point_touch_id)
            && self.path_builder.path.len_segments().eq(&0)
        {
            self.path_builder.clear();
            self.path_builder.first_point_touch_id = current_touch_id;
        }
    }

    /// a transform occurred causing a mismatch between buffer and path builder finish path early
    fn handle_dancing_lines(&mut self, p: &mut Path) -> bool {
        if p.data != self.path_builder.path {
            self.path_builder.clear();
            self.current_id += 1;
            return true;
        }
        false
    }

    pub fn end_path(&mut self, buffer: &mut Buffer, history: &mut History, is_snapped: bool) {
        if self.path_builder.path.is_empty() {
            return;
        }

        if self.path_builder.path.len() > 2 && is_snapped {
            self.path_builder.snap(buffer);
        }

        history.save(super::Event::Insert(vec![InsertElement { id: self.current_id.to_string() }]));

        if let Some(parser::Element::Path(p)) =
            buffer.elements.get_mut(&self.current_id.to_string())
        {
            p.data = self.path_builder.path.clone();
        }

        self.path_builder.clear();

        self.current_id += 1;
    }

    fn detect_snap(&mut self, current_pos: egui::Pos2, master_transform: Transform) -> bool {
        if self.path_builder.path.len() < 2 {
            return false;
        }

        if let Some(last_pos) = self.path_builder.path.iter().last() {
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

    pub fn setup_events(&mut self, ui: &mut egui::Ui, inner_rect: egui::Rect) -> Vec<PathEvent> {
        ui.input(|r| {
            r.events
                .iter()
                .filter_map(|e| {
                    if let egui::Event::Touch { device_id: _, id: _, phase, pos, force } = *e {
                        let (should_end_path, should_draw) =
                            self.decide_event(inner_rect, pos, phase == egui::TouchPhase::End, r);

                        if should_end_path {
                            Some(PathEvent::End)
                        } else if should_draw {
                            Some(PathEvent::Draw(DrawPayload { pos, force }, self.current_id))
                        } else {
                            None
                        }
                    } else if let egui::Event::PointerButton {
                        pos,
                        button: _,
                        pressed,
                        modifiers: _,
                    } = *e
                    {
                        let (should_end_path, should_draw) =
                            self.decide_event(inner_rect, pos, pressed, r);

                        if should_end_path {
                            Some(PathEvent::End)
                        } else if should_draw {
                            Some(PathEvent::Draw(DrawPayload { pos, force: None }, self.current_id))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .collect()
        })
    }

    fn decide_event(
        &mut self, inner_rect: egui::Rect, pos: egui::Pos2, end_of_event: bool,
        r: &egui::InputState,
    ) -> (bool, bool) {
        let pointer_gone_out_of_canvas =
            !self.path_builder.path.is_empty() && !inner_rect.contains(pos);

        let pointer_released_in_canvas = end_of_event && inner_rect.contains(pos);

        let pointer_pressed_and_originated_in_canvas = inner_rect
            .contains(r.pointer.press_origin().unwrap_or_default())
            && inner_rect.contains(pos);
        (
            pointer_gone_out_of_canvas || pointer_released_in_canvas,
            pointer_pressed_and_originated_in_canvas,
        )
    }
}

#[derive(Debug)]
pub enum PathEvent {
    Draw(DrawPayload, usize),
    End,
}

#[derive(Debug)]
pub struct DrawPayload {
    pos: egui::Pos2,
    force: Option<f32>,
}

/// Build a cubic bézier path with Catmull-Rom smoothing and Ramer–Douglas–Peucker compression
#[derive(Debug)]
pub struct CubicBezBuilder {
    /// store the 4 past points
    prev_points_window: VecDeque<egui::Pos2>,
    path: Subpath<ManipulatorGroupId>,
    simplified_points: Vec<egui::Pos2>,
    original_points: Vec<egui::Pos2>,
    first_point_touch_id: Option<egui::TouchId>,
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
            path: Subpath::<ManipulatorGroupId>::from_anchors(vec![], false),
            simplified_points: vec![],
            original_points: vec![],
        }
    }

    fn line_to(&mut self, dest: egui::Pos2) {
        self.original_points.push(dest);
        if let Some(prev) = self.prev_points_window.back() {
            let bez = Bezier::from_linear_coordinates(
                prev.x.into(),
                prev.y.into(),
                dest.x.into(),
                dest.y.into(),
            );
            self.path
                .append_bezier(&bez, bezier_rs::AppendType::IgnoreStart);
        }
        self.prev_points_window.push_back(dest);
    }

    fn catmull_to(&mut self, dest: egui::Pos2) {
        let is_first_point = self.prev_points_window.is_empty();

        if is_first_point {
            // repeat the first pos twice to avoid later index arithmetic
            self.prev_points_window.push_back(dest);
            self.prev_points_window.push_back(dest);
        };

        self.prev_points_window.push_back(dest);

        if self.prev_points_window.len() < 4 {
            return;
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
        self.path
            .append_bezier(&bez, bezier_rs::AppendType::IgnoreStart);

        // shift the window foreword
        self.prev_points_window.pop_front();
    }

    pub fn cubic_to(&mut self, dest: egui::Pos2) {
        if self.prev_points_window.is_empty() {
            self.original_points.clear();
        }
        self.original_points.push(dest);
        self.catmull_to(dest);
    }

    pub fn snap(&mut self, buffer: &mut Buffer) {
        let perim = self.path.length(None) as f32;
        let mut tolerance = perim * 0.04;

        tolerance *= buffer.master_transform.sx;
        let maybe_simple_points = self.simplify(tolerance);

        self.clear();

        if let Some(simple_points) = maybe_simple_points {
            self.simplified_points = simple_points.clone();
            simple_points.iter().enumerate().for_each(|(_, p)| {
                self.line_to(*p);
            });
        }
    }

    pub fn clear(&mut self) {
        self.prev_points_window.clear();
        self.first_point_touch_id = None;
        self.path = Subpath::<ManipulatorGroupId>::from_anchors(vec![], false);
    }

    /// Ramer–Douglas–Peucker algorithm courtesy of @author: Michael-F-Bryan
    /// https://github.com/Michael-F-Bryan/arcs/blob/master/core/src/algorithms/line_simplification.rs
    fn simplify(&mut self, tolerance: f32) -> Option<Vec<egui::Pos2>> {
        let mut simplified_points = Vec::new();

        // push the first point
        let mut points = vec![];
        self.path.iter().for_each(|b| {
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
