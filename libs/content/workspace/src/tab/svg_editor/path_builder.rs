use std::collections::VecDeque;

use bezier_rs::{Bezier, Subpath};
use resvg::usvg::Transform;

use super::parser::ManipulatorGroupId;

/// Build a cubic bézier path with Catmull-Rom smoothing and Ramer–Douglas–Peucker compression
#[derive(Debug)]
pub struct PathBuilder {
    /// store the 4 past points
    pub prev_points_window: VecDeque<egui::Pos2>,
    pub simplified_points: Vec<egui::Pos2>,
    pub original_points: Vec<egui::Pos2>,
    pub first_point_touch_id: Option<egui::TouchId>,
    pub is_canceled_path: bool,
}

impl Default for PathBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl PathBuilder {
    pub fn new() -> Self {
        PathBuilder {
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
