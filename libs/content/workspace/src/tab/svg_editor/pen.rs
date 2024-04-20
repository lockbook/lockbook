use minidom::Element;
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

use super::{
    toolbar::ColorSwatch,
    util::{self, apply_transform_to_pos, d_to_subpath, deserialize_transform},
    Buffer, InsertElement,
};

pub struct Pen {
    pub active_color: Option<ColorSwatch>,
    pub active_stroke_width: u32,
    path_builder: CubicBezBuilder,
    pub current_id: usize, // todo: this should be at a higher component state, maybe in buffer
    maybe_snap_started: Option<Instant>,
}

pub enum PenResponse {
    ToggleSelection(usize),
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
        }
    }

    pub fn handle_input(
        &mut self, ui: &mut egui::Ui, inner_rect: egui::Rect, buffer: &mut Buffer,
    ) -> Option<PenResponse> {
        let event = match self.setup_events(ui, inner_rect) {
            Some(e) => e,
            None => return None,
        };

        match event {
            PathEvent::Draw(mut pos, id) => {
                apply_transform_to_pos(&mut pos, buffer);

                // for some reason the integration will send two draw events on the same pos which results in a knot.
                if let Some(last_pos) = self.path_builder.points.last() {
                    if last_pos.eq(&pos) {
                        return None;
                    }
                }
                let mut master_transform = None;
                if let Some(transform) = buffer.current.attr("transform") {
                    master_transform = Some(deserialize_transform(transform));
                }

                if self.detect_snap(pos, master_transform) {
                    let curr_id = self.current_id; // needed because end path will advance to the next id
                    self.end_path(buffer, true);
                    return Some(PenResponse::ToggleSelection(curr_id));
                } else if let Some(node) = util::node_by_id(&mut buffer.current, id.to_string()) {
                    self.path_builder.cubic_to(pos);
                    node.set_attr("d", &self.path_builder.data);

                    if let Some(color) = &self.active_color {
                        node.set_attr("stroke", format!("url(#{})", color.id));
                    } else {
                        node.set_attr("stroke", "url(#fg)");
                    }
                } else {
                    self.path_builder.cubic_to(pos);

                    let child = Element::builder("path", "")
                        .attr("stroke-width", self.active_stroke_width.to_string())
                        .attr("fill", "none")
                        .attr("stroke-linejoin", "round")
                        .attr("stroke-linecap", "round")
                        .attr("id", id)
                        .attr("d", &self.path_builder.data)
                        .build();

                    buffer.current.append_child(child);
                }
            }
            PathEvent::End => {
                self.end_path(buffer, false);
                self.maybe_snap_started = None;
            }
        }
        None
    }

    fn end_path(&mut self, buffer: &mut Buffer, is_snapped: bool) {
        if self.path_builder.points.len() < 2 {
            buffer.current.remove_child(&self.current_id.to_string());
            self.path_builder.clear();
            return;
        }

        self.path_builder.finish(is_snapped, buffer);

        if let Some(node) = util::node_by_id(&mut buffer.current, self.current_id.to_string()) {
            node.set_attr("d", &self.path_builder.data);

            let node = node.clone();

            buffer.save(super::Event::Insert(vec![InsertElement {
                id: self.current_id.to_string(),
                element: node,
            }]));
        }
        self.path_builder.clear();
        self.current_id += 1;
    }

    fn detect_snap(&mut self, current_pos: egui::Pos2, master_transform: Option<[f64; 6]>) -> bool {
        if self.path_builder.points.len() < 2 {
            return false;
        }

        if let Some(last_pos) = self.path_builder.points.last() {
            let dist_diff = last_pos.distance(current_pos).abs();

            let mut dist_to_trigger_snap = 1.5;
            if let Some(t) = master_transform {
                dist_to_trigger_snap /= t[0] as f32;
            }

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

    pub fn setup_events(&mut self, ui: &mut egui::Ui, inner_rect: egui::Rect) -> Option<PathEvent> {
        if let Some(cursor_pos) = ui.ctx().pointer_hover_pos() {
            if !ui.is_enabled() {
                return None;
            };

            if inner_rect.contains(cursor_pos) {
                ui.output_mut(|w| w.cursor_icon = egui::CursorIcon::Crosshair);
            }

            let pointer_gone_out_of_canvas =
                !self.path_builder.points.is_empty() && !inner_rect.contains(cursor_pos);
            let pointer_released_in_canvas =
                ui.input(|i| i.pointer.any_released()) && inner_rect.contains(cursor_pos);
            let pointer_pressed_in_canvas =
                ui.input(|i| i.pointer.primary_down()) && inner_rect.contains(cursor_pos);

            if pointer_gone_out_of_canvas || pointer_released_in_canvas {
                Some(PathEvent::End)
            } else if pointer_pressed_in_canvas {
                Some(PathEvent::Draw(cursor_pos, self.current_id))
            } else {
                None
            }
        } else if !self.path_builder.points.is_empty() {
            Some(PathEvent::End)
        } else {
            None
        }
    }
}

pub enum PathEvent {
    Draw(egui::Pos2, usize),
    End,
}

/// Build a cubic bézier path with Catmull-Rom smoothing and Ramer–Douglas–Peucker compression
pub struct CubicBezBuilder {
    /// store the 4 past points
    prev_points_window: VecDeque<egui::Pos2>,
    points: Vec<egui::Pos2>,
    pub data: String,
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
            points: vec![],
            data: String::new(),
        }
    }

    fn line_to(&mut self, dest: egui::Pos2) {
        let is_first_point = self.prev_points_window.is_empty();

        if is_first_point {
            self.data = format!("M {} {}", dest.x, dest.y);
        }
        self.data
            .push_str(format!(" L{} {}", dest.x, dest.y).as_str());

        self.prev_points_window.push_back(dest);
    }

    fn catmull_to(&mut self, dest: egui::Pos2, is_last_point: bool) {
        let is_first_point = self.prev_points_window.is_empty();

        if is_first_point {
            self.data = format!("M {} {}", dest.x, dest.y);

            // repeat the first pos twice to avoid later index arithmetic
            self.prev_points_window.push_back(dest);
        };

        if is_last_point {
            if let Some(last) = self.prev_points_window.back() {
                self.prev_points_window.push_back(*last);
            }
        }

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

        self.data
            .push_str(format!("C {cp1x},{cp1y},{cp2x},{cp2y},{},{}", p2.x, p2.y).as_str());

        // shift the window foreword
        self.prev_points_window.pop_front();
    }

    pub fn cubic_to(&mut self, dest: egui::Pos2) {
        self.points.push(dest);
        self.catmull_to(dest, false);
    }

    pub fn finish(&mut self, is_snapped: bool, buffer: &mut Buffer) {
        let mut tolerance = if is_snapped {
            let perim = d_to_subpath(&self.data).length(None) as f32;
            perim * 0.04
        } else {
            2.0
        };
        if let Some(transform) = buffer.current.attr("transform") {
            tolerance /= deserialize_transform(transform)[0] as f32;
        }
        self.simplify(tolerance);

        self.data.clear();
        self.prev_points_window.clear();

        self.points.clone().iter().enumerate().for_each(|(i, p)| {
            if is_snapped {
                self.line_to(*p);
            } else {
                self.catmull_to(*p, false);
                if i == self.points.len() - 1 {
                    self.catmull_to(*p, true);
                };
            }
        });
    }

    pub fn clear(&mut self) {
        self.prev_points_window.clear();
        self.data.clear();
        self.points.clear();
    }

    /// Ramer–Douglas–Peucker algorithm courtesy of @author: Michael-F-Bryan
    /// https://github.com/Michael-F-Bryan/arcs/blob/master/core/src/algorithms/line_simplification.rs
    fn simplify(&mut self, tolerance: f32) {
        if self.points.len() <= 2 {
            return;
        }

        let mut buffer = Vec::new();

        // push the first point
        buffer.push(self.points[0]);
        // then simplify every point in between the start and end
        self.simplify_points(&self.points[..], tolerance, &mut buffer);
        // and finally the last one
        buffer.push(*self.points.last().unwrap());

        self.points = buffer;
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

#[test]
fn catmull_to() {
    let points = vec![
        egui::pos2(440.6, 902.5),
        egui::pos2(431.2, 945.1),
        egui::pos2(438.6, 917.9),
        egui::pos2(455.0, 887.6),
        egui::pos2(465.4, 884.2),
        egui::pos2(466.4, 893.1),
        egui::pos2(457.9, 906.5),
        egui::pos2(454.0, 922.8),
        egui::pos2(471.8, 956.0),
    ];

    let mut path_builder =
        CubicBezBuilder { prev_points_window: VecDeque::default(), points, data: "".to_string() };

    path_builder
        .points
        .clone()
        .iter()
        .enumerate()
        .for_each(|(i, p)| {
            path_builder.catmull_to(*p, false);
            if i == path_builder.points.len() - 1 {
                path_builder.catmull_to(*p, true);
            };
        });
    println!("{}", path_builder.data);
}
