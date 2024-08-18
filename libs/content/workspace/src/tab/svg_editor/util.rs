use bezier_rs::{Bezier, Subpath};
use glam::DVec2;

use super::parser::{Element, ManipulatorGroupId};

pub fn pointer_intersects_element(
    el: &Element, pos: egui::Pos2, last_pos: Option<egui::Pos2>, error_radius: f64,
) -> bool {
    match el {
        Element::Path(p) => pointer_intersects_outline(&p.data, pos, last_pos, error_radius),
        Element::Image(img) => {
            let rect = img.bounding_box().expand(error_radius as f32);

            let last_pos = last_pos.unwrap_or(pos.round());

            rect.contains(pos) || rect.contains(last_pos)
        }
        Element::Text(_) => todo!(),
    }
}

pub fn pointer_intersects_outline(
    path: &Subpath<ManipulatorGroupId>, pos: egui::Pos2, last_pos: Option<egui::Pos2>,
    error_radius: f64,
) -> bool {
    if path.len_segments() == 0 {
        return false;
    }
    // first pass: check if the path bounding box contain the cursor.
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

pub fn bb_to_rect(bb: [DVec2; 2]) -> egui::Rect {
    egui::Rect {
        min: egui::pos2(bb[0].x as f32, bb[0].y as f32),
        max: egui::pos2(bb[1].x as f32, bb[1].y as f32),
    }
}

pub fn get_current_touch_id(ui: &mut egui::Ui) -> Option<egui::TouchId> {
    ui.input(|r| {
        r.events.iter().find_map(move |event| {
            if let egui::Event::Touch { device_id: _, id, phase: _, pos: _, force: _ } = event {
                Some(*id)
            } else {
                None
            }
        })
    })
}
