use std::collections::VecDeque;

use bezier_rs::{Bezier, Subpath};
use glam::DVec2;

use lb_rs::model::svg::element::ManipulatorGroupId;

/// Build a cubic bézier path with Catmull-Rom smoothing and Ramer–Douglas–Peucker compression
#[derive(Debug)]
pub struct PathBuilder {
    /// store the 4 past points
    pub prev_points_window: VecDeque<egui::Pos2>,
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
            is_canceled_path: false,
            first_predicted_mg: None,
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

    pub fn clear(&mut self) {
        self.prev_points_window.clear();
        self.is_canceled_path = false;
    }

    fn is_redundant_point(&self, path: &Subpath<ManipulatorGroupId>, point: egui::Pos2) -> bool {
        let last_point = match path.manipulator_groups().last() {
            Some(val) => val,
            None => return false,
        };

        let distance = last_point
            .anchor
            .distance(glam::DVec2 { x: point.x as f64, y: point.y as f64 });
        let tolerance = 0.5;
        distance < tolerance
    }
}
