use minidom::Element;
use std::collections::HashSet;

use super::{
    util::{deserialize_transform, serialize_transform},
    Buffer,
};

pub const G_CONTAINER_ID: &str = "lb:zoom_container";

pub fn handle_zoom_input(ui: &mut egui::Ui, working_rect: egui::Rect, buffer: &mut Buffer) {
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
        let mut original_matrix =
            deserialize_transform(buffer.current.attr("transform").unwrap_or_default());

        // apply pan
        original_matrix[4] += pan.unwrap_or_default().x as f64;
        original_matrix[5] += pan.unwrap_or_default().y as f64;

        // apply zoom/scale
        let mut scaled_matrix: Vec<f64> = original_matrix
            .iter()
            .map(|x| zoom_delta as f64 * x)
            .collect();
        scaled_matrix[4] += ((1.0 - zoom_delta) * pos.x) as f64;
        scaled_matrix[5] += ((1.0 - zoom_delta) * pos.y) as f64;
        let new_transform = serialize_transform(scaled_matrix.as_slice());

        buffer.current.set_attr("transform", new_transform);
        buffer.needs_path_map_update = true;
    }
}

pub fn verify_zoom_g(buffer: &mut Buffer) {
    if buffer.current.attr("id").unwrap_or_default() != G_CONTAINER_ID {
        let mut g = Element::builder("g", "").attr("id", G_CONTAINER_ID).build();
        let mut moved_ids = HashSet::new();
        buffer.current.children().for_each(|child| {
            g.append_child(child.clone());
            moved_ids.insert(child.attr("id").unwrap_or_default().to_string());
        });

        moved_ids.iter().for_each(|id| {
            buffer.current.remove_child(id);
        });

        buffer.current = g;
    }
}

pub fn zoom_to_percentage(buffer: &mut Buffer, percentage: i32, working_rect: egui::Rect) {
    let original_matrix =
        deserialize_transform(buffer.current.attr("transform").unwrap_or_default());

    let [a, b, _, _, _, _] = original_matrix;

    let scale_x = (a * a + b * b).sqrt();

    let zoom_delta = percentage as f64 / (scale_x * 100.0);

    let mut scaled_matrix: Vec<f64> = original_matrix
        .iter()
        .map(|x| zoom_delta as f64 * x)
        .collect();

    scaled_matrix[4] += ((1.0 - zoom_delta) * working_rect.center().x as f64) as f64;
    scaled_matrix[5] += ((1.0 - zoom_delta) * working_rect.center().y as f64) as f64;
    let new_transform = serialize_transform(scaled_matrix.as_slice());

    buffer.current.set_attr("transform", new_transform);
    buffer.needs_path_map_update = true;
}
