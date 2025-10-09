use std::collections::HashSet;

use super::element::BoundedElement;
use super::{CanvasSettings, ViewportSettings};

use bezier_rs::{Bezier, Subpath};
use egui::TouchPhase;
use glam::DVec2;
use lb_rs::model::svg::WeakRect;
use lb_rs::model::svg::element::{Element, ManipulatorGroupId};
use lyon::math::Point;
use resvg::usvg::Transform;

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

pub fn transform_point(point: egui::Pos2, t: Transform) -> egui::Pos2 {
    egui::Pos2 { x: t.sx * point.x + t.tx, y: t.sy * point.y + t.ty }
}

pub fn transform_rect(rect: egui::Rect, t: Transform) -> egui::Rect {
    egui::Rect { min: transform_point(rect.min, t), max: transform_point(rect.max, t) }
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
            if let egui::Event::Touch { device_id: _, id, phase, pos: _, force: _ } = *e {
                if phase != TouchPhase::Cancel {
                    touch_ids.insert(id.0);
                    if touch_ids.len() > 1 {
                        custom_multi_touch = true;
                        break;
                    }
                }
            }
        }
    });
    custom_multi_touch
}

pub fn is_scroll(ui: &mut egui::Ui) -> bool {
    let mut is_scroll = false;

    ui.input(|r| {
        for e in r.events.iter() {
            if let egui::Event::MouseWheel { unit: _, delta: _, modifiers: _ } = *e {
                is_scroll = true;
            }
        }
    });
    is_scroll
}

pub fn devc_to_point(dvec: DVec2) -> Point {
    Point::new(dvec.x as f32, dvec.y as f32)
}

pub fn bb_to_rect(bb: [DVec2; 2]) -> egui::Rect {
    egui::Rect {
        min: egui::pos2(bb[0].x as f32, bb[0].y as f32),
        max: egui::pos2(bb[1].x as f32, bb[1].y as f32),
    }
}

pub fn promote_weak_rect(wk: WeakRect) -> egui::Rect {
    egui::Rect::from_min_max(egui::pos2(wk.min.0, wk.min.1), egui::pos2(wk.max.0, wk.max.1))
}
pub fn demote_to_weak_rect(src: egui::Rect) -> WeakRect {
    WeakRect { min: (src.min.x, src.min.y), max: (src.max.x, src.max.y) }
}

impl CanvasSettings {
    pub fn update_viewport_settings(&mut self, vs: &ViewportSettings) {
        self.bottom_locked = vs.bottom_locked;
        self.left_locked = vs.left_locked;
        self.right_locked = vs.right_locked;
        self.top_locked = vs.top_locked;
    }
}

pub fn draw_dashed_line(
    painter: &egui::Painter, edges: &[egui::Pos2], dash_length: f32, gap_length: f32,
    stroke: egui::Stroke,
) {
    let start = edges[0];

    let end = edges[1];

    let vec = end - start;

    let length = vec.length();

    let dir = vec / length;

    let dash_gap = dash_length + gap_length;

    let dash_count = (length / dash_gap).floor() as usize;

    for i in 0..dash_count {
        let dash_start = start + dir * (i as f32 * dash_gap);

        let dash_end = dash_start + dir * dash_length.min(length - i as f32 * dash_gap);

        painter.line_segment([dash_start, dash_end], stroke);
    }

    // Draw the remaining part if there's space

    let remaining = length - dash_count as f32 * dash_gap;

    if remaining > 0.0 {
        let last_dash_start = start + dir * (dash_count as f32 * dash_gap);

        let last_dash_end = last_dash_start + dir * remaining.min(dash_length);

        if (last_dash_end - last_dash_start).length() > 0.0 {
            painter.line_segment([last_dash_start, last_dash_end], stroke);
        }
    }
}
