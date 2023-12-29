use bezier_rs::{Bezier, Subpath};
use eframe::egui;
use minidom::Element;

use super::history::ManipulatorGroupId;

pub fn node_by_id(root: &mut Element, id: String) -> Option<&mut Element> {
    root.children_mut().find(
        |e| {
            if let Some(id_attr) = e.attr("id") {
                id_attr == id
            } else {
                false
            }
        },
    )
}

pub fn pointer_interests_path(
    path: &Subpath<ManipulatorGroupId>, pos: egui::Pos2, last_pos: Option<egui::Pos2>,
    error_radius: f64,
) -> bool {
    // first pass: check if the path bounding box contain the cursor.
    // padding to account for low sampling rate scenarios and flat
    // lines with empty bounding boxes
    let padding = 50.0;
    let bb = match path.bounding_box() {
        Some(bb) => egui::Rect {
            min: egui::pos2(bb[0].x as f32, bb[0].y as f32),
            max: egui::pos2(bb[1].x as f32, bb[1].y as f32),
        }
        .expand(padding),
        None => return false,
    };
    let last_pos = last_pos.unwrap_or(pos.round());
    if !(bb.contains(pos) || bb.contains(last_pos)) {
        return false;
    }

    // second more rigorous pass
    let delete_brush = Bezier::from_linear_dvec2(
        glam::dvec2(last_pos.x as f64, last_pos.y as f64),
        glam::dvec2(pos.x as f64, pos.y as f64),
    )
    .outline(error_radius, bezier_rs::Cap::Round);

    let is_inside_delete_brush = path.is_point()
        && delete_brush.contains_point(path.manipulator_groups().get(0).unwrap().anchor);
    let intersects_delete_brush = !path
        .subpath_intersections(&delete_brush, None, None)
        .is_empty();

    intersects_delete_brush || is_inside_delete_brush
}

pub fn parse_transform(transform: &str) -> [f64; 6] {
    for segment in svgtypes::TransformListParser::from(transform) {
        let segment = match segment {
            Ok(v) => v,
            Err(_) => break,
        };
        if let svgtypes::TransformListToken::Matrix { a, b, c, d, e, f } = segment {
            return [a, b, c, d, e, f];
        }
    }
    let identity_matrix = [0, 1, 1, 0, 0, 0].map(|f| f as f64);
    identity_matrix
}
