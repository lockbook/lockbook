use minidom::Element;
use resvg::usvg::Transform;
use std::collections::HashSet;

use super::{
    parser,
    selection::u_transform_to_bezier,
    util::{deserialize_transform, serialize_transform},
};

pub const G_CONTAINER_ID: &str = "lb:zoom_container";

pub fn handle_zoom_input(ui: &mut egui::Ui, working_rect: egui::Rect, buffer: &mut parser::Buffer) {
    let zoom_delta = ui.input(|r| r.zoom_delta());
    let is_zooming = zoom_delta != 1.0;

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

    let mut t = Transform::identity();

    if let Some(p) = pan {
        t = t.post_translate(p.x, p.y);
    }

    if is_zooming {
        // apply zoom
        t = t.post_scale(zoom_delta, zoom_delta);

        // correct the zoom to center
        t = t.post_translate((1.0 - zoom_delta) * pos.x, (1.0 - zoom_delta) * pos.y);
    }

    if pan.is_some() || is_zooming {
        let transform = u_transform_to_bezier(&t);
        for el in buffer.elements.values_mut() {
            match el {
                parser::Element::Path(path) => {
                    path.data.apply_transform(transform);
                }
                parser::Element::Image(img) => todo!(),
                parser::Element::Text(text) => todo!(),
            }
        }
    }
}

pub fn zoom_to_percentage(buffer: &mut parser::Buffer, percentage: i32, working_rect: egui::Rect) {
    // let original_matrix =
    //     deserialize_transform(buffer.current.attr("transform").unwrap_or_default());

    // let [a, b, _, _, _, _] = original_matrix;

    // let scale_x = (a * a + b * b).sqrt();

    // let zoom_delta = percentage as f64 / (scale_x * 100.0);

    // let mut scaled_matrix: Vec<f64> = original_matrix.iter().map(|x| zoom_delta * x).collect();

    // scaled_matrix[4] += (1.0 - zoom_delta) * working_rect.center().x as f64;
    // scaled_matrix[5] += (1.0 - zoom_delta) * working_rect.center().y as f64;
    // let new_transform = serialize_transform(scaled_matrix.as_slice());

    // buffer.current.set_attr("transform", new_transform);
    // buffer.needs_path_map_update = true;
}
