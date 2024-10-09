use std::{collections::VecDeque, time::Instant};

use bezier_rs::{Bezier, Identifier, ManipulatorGroup, Subpath};
use glam::DVec2;
use resvg::usvg::Transform;
use serde::de::IntoDeserializer;
use tracing::{trace, warn};

use super::parser::ManipulatorGroupId;

/// Build a cubic bézier path with Catmull-Rom smoothing and Ramer–Douglas–Peucker compression
#[derive(Debug)]
pub struct PathBuilder {
    /// store the 4 past points
    pub prev_points_window: VecDeque<egui::Pos2>,
    pub simplified_points: Vec<egui::Pos2>,
    pub original_points: Vec<egui::Pos2>,
    pub first_point_touch_id: Option<egui::TouchId>,
    pub first_point_frame: Option<Instant>,
    pub is_canceled_path: bool,
    pub first_predicted_mg: Option<usize>,
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
            first_predicted_mg: None,
            first_point_frame: None,
        }
    }

    pub fn line_to(
        &mut self, dest: egui::Pos2, path: &mut Subpath<ManipulatorGroupId>,
    ) -> Option<usize> {
        if let Some(last_mg) = path.manipulator_groups().last() {
            if self.is_redundant_point(path, dest) {
                return None;
            }
            let bez = Bezier::from_linear_coordinates(
                last_mg.anchor.x,
                last_mg.anchor.y,
                dest.x.into(),
                dest.y.into(),
            );
            path.append_bezier(&bez, bezier_rs::AppendType::IgnoreStart);
            Some(path.manipulator_groups().len() - 1)
        } else {
            path.append_bezier(
                &Bezier::from_linear_dvec2(
                    DVec2 { x: dest.x.into(), y: dest.y.into() },
                    DVec2 { x: dest.x as f64 + 1.0, y: dest.y as f64 + 1.0 },
                ),
                bezier_rs::AppendType::IgnoreStart,
            );
            Some(path.manipulator_groups().len() - 1)
        }
    }

    fn kochanek_bartels_to_bezier(
        &mut self, p0: DVec2, p1: DVec2, p2: DVec2, dest: DVec2,
    ) -> Bezier {
        let tension = 0.0; // Adjust to control tightness
        let continuity = 0.0; // Adjust to control smoothness between segments
        let bias = 0.0; // Adjust to control curvature near points

        let p3 = dest;

        // Calculate tangents using the Kochanek-Bartels spline formula
        let d1 = (1.0 - tension) * (1.0 + continuity) * (1.0 + bias) * (p1.x - p0.x) / 2.0
            + (1.0 - tension) * (1.0 - continuity) * (1.0 - bias) * (p2.x - p1.x) / 2.0;
        let d2 = (1.0 - tension) * (1.0 - continuity) * (1.0 + bias) * (p2.x - p1.x) / 2.0
            + (1.0 - tension) * (1.0 + continuity) * (1.0 - bias) * (p3.x - p2.x) / 2.0;

        let tangent1 = DVec2 {
            x: d1,
            y: (1.0 - tension) * (1.0 + continuity) * (1.0 + bias) * (p1.y - p0.y) / 2.0
                + (1.0 - tension) * (1.0 - continuity) * (1.0 - bias) * (p2.y - p1.y) / 2.0,
        };
        let tangent2 = DVec2 {
            x: d2,
            y: (1.0 - tension) * (1.0 - continuity) * (1.0 + bias) * (p2.y - p1.y) / 2.0
                + (1.0 - tension) * (1.0 + continuity) * (1.0 - bias) * (p3.y - p2.y) / 2.0,
        };

        // Define the control points for the cubic Bézier curve
        let p0_bez = p1;
        let p1_bez = DVec2 { x: p1.x + tangent1.x / 3.0, y: p1.y + tangent1.y / 3.0 };
        let p2_bez = DVec2 { x: p2.x - tangent2.x / 3.0, y: p2.y - tangent2.y / 3.0 };
        let p3_bez = p2;

        Bezier::from_cubic_coordinates(
            p0_bez.x, p0_bez.y, p1_bez.x, p1_bez.y, p2_bez.x, p2_bez.y, p3_bez.x, p3_bez.y,
        )
    }

    pub fn cubic_to(
        &mut self, dest: egui::Pos2, path: &mut Subpath<ManipulatorGroupId>,
    ) -> Option<usize> {
        if self.is_redundant_point(path, dest) {
            return None;
        }
        let bez = match path.manipulator_groups().len() {
            0 => Bezier::from_linear_dvec2(
                DVec2 { x: dest.x.into(), y: dest.y.into() },
                DVec2 { x: dest.x as f64 + 1.0, y: dest.y as f64 + 1.0 },
            ),
            1 | 2 => {
                let p0 = path
                    .manipulator_groups()
                    .get(path.manipulator_groups().len() - 1)
                    .unwrap();
                Bezier::from_linear_coordinates(
                    p0.anchor.x,
                    p0.anchor.y,
                    dest.x.into(),
                    dest.y.into(),
                )
            }
            _ => {
                let p2 = path
                    .manipulator_groups()
                    .get(path.manipulator_groups().len() - 1)
                    .unwrap()
                    .anchor;
                let p1 = path
                    .manipulator_groups()
                    .get(path.manipulator_groups().len() - 2)
                    .unwrap()
                    .anchor;

                let p0 = path
                    .manipulator_groups()
                    .get(path.manipulator_groups().len() - 3)
                    .unwrap()
                    .anchor;

                self.kochanek_bartels_to_bezier(
                    p0,
                    p1,
                    p2,
                    DVec2 { x: dest.x.into(), y: dest.y.into() },
                )
            }
        };
        path.append_bezier(&bez, bezier_rs::AppendType::IgnoreStart);
        Some(path.manipulator_groups().len() - 1)
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
        self.first_point_frame = None;
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

    fn is_redundant_point(&self, path: &Subpath<ManipulatorGroupId>, point: egui::Pos2) -> bool {
        let last_point = match path.manipulator_groups().last() {
            Some(val) => val,
            None => return false,
        };

        let distance = last_point
            .anchor
            .distance(glam::DVec2 { x: point.x as f64, y: point.y as f64 });
        let tolerance = 1.0;
        warn!(?distance, "distance");
        distance < tolerance
    }
}

pub fn get_anchor_avg_displacement(
    path: &Subpath<ManipulatorGroupId>, master_transform: f32,
) -> f64 {
    let mut displacement_sum = 0.0;
    for (i, mg) in path.manipulator_groups().iter().enumerate() {
        if let Some(next_mg) = path.manipulator_groups().get(i + 1) {
            displacement_sum += next_mg.anchor.distance(mg.anchor) / master_transform as f64;
        }
    }
    displacement_sum / path.manipulator_groups().len() as f64
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
