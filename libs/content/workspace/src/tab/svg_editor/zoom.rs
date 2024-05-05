use minidom::Element;
use std::collections::HashSet;

use super::{
    parser,
    util::{deserialize_transform, serialize_transform},
};

pub const G_CONTAINER_ID: &str = "lb:zoom_container";

pub fn handle_zoom_input(ui: &mut egui::Ui, working_rect: egui::Rect, buffer: &mut parser::Buffer) {
    let zoom_delta = ui.input(|r| r.zoom_delta());
    let is_zooming = zoom_delta != 0.0;

    let pan = ui.input(|r| {
        if r.raw_scroll_delta.x.abs() > 0.0 || r.raw_scroll_delta.y.abs() > 0.0 {
            Some(r.raw_scroll_delta)
        } else if let Some(touch_gesture) = r.multi_touch() {
            if touch_gesture.translation_delta.x.abs() > 0.0
                || touch_gesture.translation_delta.y.abs() > 0.0
            {
                Some(touch_gesture.translation_delta)
            } else {
                None
            }
        } else {
            None
        }
    });

    let pos = match ui.ctx().pointer_hover_pos() {
        Some(cp) => {
            if ui.is_enabled() && working_rect.contains(cp) {
                cp
            } else {
                return; // todo: check this doesn't break zoom on touch devices
            }
        }
        None => egui::Pos2::ZERO,
    };

    if pan.is_some() || is_zooming {
        let pan = pan.unwrap_or_default();
        // apply pan
        buffer.master_transform.post_translate(pan.x, pan.y);

        // apply zoom
        buffer.master_transform.post_scale(zoom_delta, zoom_delta);

        // correct the zoom via translate
        buffer.master_transform.post_translate(pos.x, pos.y);

        buffer.needs_path_map_update = true;
    }
}

pub fn zoom_to_percentage(buffer: &mut Buffer, percentage: i32, working_rect: egui::Rect) {
    let original_matrix =
        deserialize_transform(buffer.current.attr("transform").unwrap_or_default());

    let [a, b, _, _, _, _] = original_matrix;

    let scale_x = (a * a + b * b).sqrt();

    let zoom_delta = percentage as f64 / (scale_x * 100.0);

    let mut scaled_matrix: Vec<f64> = original_matrix.iter().map(|x| zoom_delta * x).collect();

    scaled_matrix[4] += (1.0 - zoom_delta) * working_rect.center().x as f64;
    scaled_matrix[5] += (1.0 - zoom_delta) * working_rect.center().y as f64;
    let new_transform = serialize_transform(scaled_matrix.as_slice());

    buffer.current.set_attr("transform", new_transform);
    buffer.needs_path_map_update = true;
}
