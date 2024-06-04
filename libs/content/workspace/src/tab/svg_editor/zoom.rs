use resvg::usvg::Transform;

use super::{parser, selection::u_transform_to_bezier};

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
                parser::Element::Image(img) => {
                    img.apply_transform(t);
                }
                parser::Element::Text(_) => todo!(),
            }
        }
    }
}
