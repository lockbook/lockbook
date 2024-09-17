use std::collections::HashSet;

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
    let bb = match path.bounding_box() {
        Some(bb) => egui::Rect {
            min: egui::pos2(bb[0].x as f32, bb[0].y as f32),
            max: egui::pos2(bb[1].x as f32, bb[1].y as f32),
        },
        None => return false,
    };

    let last_pos = last_pos.unwrap_or(pos.round());
    let last_pos_rect = egui::Rect::from_center_size(
        last_pos,
        egui::vec2(error_radius as f32 * 2.0, error_radius as f32 * 2.0),
    );
    let pos_rect = egui::Rect::from_center_size(
        pos,
        egui::vec2(error_radius as f32 * 2.0, error_radius as f32 * 2.0),
    );

    let needs_second_pass = bb.intersects(last_pos_rect)
        || bb.contains_rect(last_pos_rect)
        || bb.intersects(pos_rect)
        || bb.contains_rect(pos_rect);

    if !needs_second_pass {
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
pub fn rect_to_bb(rect: egui::Rect) -> [DVec2; 2] {
    [
        DVec2 { x: rect.left().into(), y: rect.top().into() },
        DVec2 { x: rect.right().into(), y: rect.bottom().into() },
    ]
}

pub fn get_event_touch_id(event: &egui::Event) -> Option<egui::TouchId> {
    if let egui::Event::Touch { device_id: _, id, phase: _, pos: _, force: _ } = event {
        Some(*id)
    } else {
        None
    }
}

pub fn get_is_pointer_released(ui: &mut egui::Ui) -> (bool, Option<egui::Pos2>) {
    let released_pointer_id = ui.input(|r| {
        r.events.iter().find_map(move |event| {
            if let egui::Event::Touch { device_id: _, id, phase, pos, force: _ } = event {
                if phase == &egui::TouchPhase::End {
                    Some(*pos)
                } else {
                    None
                }
            } else {
                None
            }
        })
    });
    (ui.input(|r| r.pointer.any_released()), released_pointer_id)
}

pub fn is_multi_touch(ui: &mut egui::Ui) -> bool {
    let mut custom_multi_touch = false;
    ui.input(|r| {
        if r.multi_touch().is_some() {
            custom_multi_touch = true;
            return;
        }
        let mut touch_ids = HashSet::new();
        for e in r.events.iter() {
            if let egui::Event::Touch { device_id: _, id, phase: _, pos: _, force: _ } = e {
                touch_ids.insert(id.0);
                if touch_ids.len() > 1 {
                    custom_multi_touch = true;
                    break;
                }
            }
        }
    });
    custom_multi_touch
}
