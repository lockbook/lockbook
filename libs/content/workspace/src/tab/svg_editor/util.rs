use bezier_rs::{Bezier, Subpath};
use glam::DVec2;
use minidom::Element;

use super::parser::{self, ManipulatorGroupId};

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
    if path.len_segments() == 0 {
        return false;
    }
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

    let is_inside_delete_brush = path
        .manipulator_groups()
        .iter()
        .all(|m| delete_brush.contains_point(m.anchor));

    let intersects_delete_brush = !path
        .subpath_intersections(&delete_brush, None, None)
        .is_empty();

    intersects_delete_brush || is_inside_delete_brush
}

pub fn deserialize_transform(transform: &str) -> [f64; 6] {
    for segment in svgtypes::TransformListParser::from(transform) {
        let segment = match segment {
            Ok(v) => v,
            Err(_) => break,
        };
        if let svgtypes::TransformListToken::Matrix { a, b, c, d, e, f } = segment {
            return [a, b, c, d, e, f];
        }
    }
    [1, 0, 0, 1, 0, 0].map(|f| f as f64)
}

pub fn serialize_transform(matrix: &[f64]) -> String {
    format!(
        "matrix({},{},{},{},{},{} )",
        matrix[0], matrix[1], matrix[2], matrix[3], matrix[4], matrix[5]
    )
}

pub fn apply_transform_to_pos(pos: &mut egui::Pos2, buffer: &mut parser::Buffer) {
    pos.x -= buffer.master_transform.tx;
    pos.y -= buffer.master_transform.ty;

    pos.x /= buffer.master_transform.sx;
    pos.y /= buffer.master_transform.sy;
}

pub fn bb_to_rect(bb: [DVec2; 2]) -> egui::Rect {
    egui::Rect {
        min: egui::pos2(bb[0].x as f32, bb[0].y as f32),
        max: egui::pos2(bb[1].x as f32, bb[1].y as f32),
    }
}

pub fn d_to_subpath(data: &str) -> Subpath<ManipulatorGroupId> {
    let mut start = (0.0, 0.0);
    let mut subpath: Subpath<ManipulatorGroupId> = Subpath::new(vec![], false);

    for segment in svgtypes::SimplifyingPathParser::from(data) {
        let segment = match segment {
            Ok(v) => v,
            Err(_) => break,
        };

        match segment {
            svgtypes::SimplePathSegment::MoveTo { x, y } => {
                start = (x, y);
            }
            svgtypes::SimplePathSegment::CurveTo { x1, y1, x2, y2, x, y } => {
                let bez = Bezier::from_cubic_coordinates(start.0, start.1, x1, y1, x2, y2, x, y);
                subpath.append_bezier(&bez, bezier_rs::AppendType::IgnoreStart);
                start = (x, y)
            }
            svgtypes::SimplePathSegment::LineTo { x, y } => {
                let bez = Bezier::from_linear_coordinates(start.0, start.1, x, y);
                subpath.append_bezier(&bez, bezier_rs::AppendType::IgnoreStart);

                start = (x, y)
            }
            _ => {}
        }
    }
    subpath
}
