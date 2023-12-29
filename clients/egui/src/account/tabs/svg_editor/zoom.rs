use std::collections::HashSet;

use eframe::egui;
use minidom::Element;

use super::{
    util::{deserialize_transform, serialize_transform},
    Buffer,
};

pub const G_CONTAINER_ID: &str = "lb:zoom_container";

pub fn handle_zoom_input(ui: &mut egui::Ui, working_rect: egui::Rect, buffer: &mut Buffer) {
    let pos = match ui.ctx().pointer_hover_pos() {
        Some(cp) => {
            if ui.is_enabled() && working_rect.contains(cp) {
                cp
            } else {
                return;
            }
        }
        None => egui::Pos2::ZERO,
    };

    let dx = ui.input(|r| r.scroll_delta.x) as f64;
    let dy = ui.input(|r| r.scroll_delta.y) as f64;
    let zoom_delta = ui.input(|r| r.zoom_delta());

    let is_panning = dx.abs() > 0.0 || dy.abs() > 0.0;
    let is_zooming = zoom_delta != 0.0;

    if is_panning || is_zooming {
        let mut original_matrix =
            deserialize_transform(buffer.current.attr("transform").unwrap_or_default());

        let new_transform = if is_panning {
            original_matrix[4] += dx;
            original_matrix[5] += dy;
            serialize_transform(&original_matrix)
        } else {
            let mut original_matrix: Vec<f64> = original_matrix
                .iter()
                .map(|x| zoom_delta as f64 * x)
                .collect();
            original_matrix[4] += ((1.0 - zoom_delta) * pos.x) as f64;
            original_matrix[5] += ((1.0 - zoom_delta) * pos.y) as f64;
            serialize_transform(original_matrix.as_slice())
        };

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
