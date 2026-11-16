use bezier_rs::{Bezier, Subpath};
use glam::DVec2;

use lb_rs::model::svg::element::ManipulatorGroupId;

/// Builds a sampled path, filtering redundant points and preserving tap geometry.
#[derive(Debug)]
pub struct PathBuilder {
    pub first_predicted_mg: Option<usize>,
    /// Screen sampling threshold expressed in document units.
    pub tolerance: f64,
}

impl Default for PathBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl PathBuilder {
    pub fn new() -> Self {
        PathBuilder { first_predicted_mg: None, tolerance: 0.5 }
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
            // A tap starts as two coincident anchors. Its provisional endpoint
            // is replaced by the first real movement, rather than kept as a kink.
            if path.len() == 2
                && path.manipulator_groups()[0].anchor == path.manipulator_groups()[1].anchor
            {
                path.remove_manipulator_group(1);
            }
            path.append_bezier(&bez, bezier_rs::AppendType::IgnoreStart);
            Some(path.manipulator_groups().len() - 1)
        } else {
            // A zero-length round-capped segment is a dot at the actual touch.
            path.append_bezier(
                &Bezier::from_linear_dvec2(
                    DVec2 { x: dest.x.into(), y: dest.y.into() },
                    DVec2 { x: dest.x.into(), y: dest.y.into() },
                ),
                bezier_rs::AppendType::IgnoreStart,
            );
            Some(path.manipulator_groups().len() - 1)
        }
    }

    pub fn clear(&mut self) {
        self.first_predicted_mg = None;
    }

    fn is_redundant_point(&self, path: &Subpath<ManipulatorGroupId>, point: egui::Pos2) -> bool {
        let last_point = match path.manipulator_groups().last() {
            Some(val) => val,
            None => return false,
        };

        let distance = last_point
            .anchor
            .distance(glam::DVec2 { x: point.x as f64, y: point.y as f64 });
        distance < self.tolerance
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::pos2;

    #[test]
    fn stroke_starts_with_real_samples_without_an_offset_point() {
        let mut builder = PathBuilder::new();
        let mut path: Subpath<ManipulatorGroupId> = Subpath::new(vec![], false);
        let points =
            [pos2(552.23254, 425.79202), pos2(554.72473, 426.4561), pos2(556.2169, 427.2061)];
        for point in points {
            builder.line_to(point, &mut path);
        }
        let anchors: Vec<_> = path
            .manipulator_groups()
            .iter()
            .map(|group| pos2(group.anchor.x as f32, group.anchor.y as f32))
            .collect();
        assert_eq!(anchors, points);
    }
}
