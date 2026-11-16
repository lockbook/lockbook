use resvg::usvg::Transform;

use super::ViewportSettings;
use super::element::{BoundedElement, stroked_path_bounds};
use super::util::transform_rect;
use lb_rs::model::svg::buffer::u_transform_to_bezier;
use lb_rs::model::svg::element::Element;

use super::Buffer;
pub const MIN_ZOOM_LEVEL: f32 = 0.1;

pub fn transform_canvas(
    buffer: &mut Buffer, viewport_settings: &mut ViewportSettings, t: Transform,
) {
    let new_transform = viewport_settings.master_transform.post_concat(t);

    // max allowed zoom level is 10%
    if viewport_settings.master_transform.sx < MIN_ZOOM_LEVEL
        && new_transform.sx < viewport_settings.master_transform.sx
    {
        return;
    }
    if new_transform.sx == 0.0 || new_transform.sy == 0.0 {
        return;
    }
    viewport_settings.master_transform = new_transform;
    buffer.master_transform_changed = true;

    for el in buffer.elements.values_mut() {
        match el {
            Element::Path(path) => {
                path.diff_state.transformed = Some(t);
                path.data.apply_transform(u_transform_to_bezier(&t));
            }
            Element::Image(image) => {
                if let Some(new_vbox) = image.view_box.transform(t) {
                    image.view_box = new_vbox;
                }
                image.diff_state.transformed = Some(t);
            }
            Element::Text(_) => todo!(),
        }
    }
    viewport_settings.bounded_rect = viewport_settings
        .bounded_rect
        .map(|rect| transform_rect(rect, t));
}

/// returns the fit transform in the non master transform plane
pub fn get_zoom_fit_transform(viewport_settings: &ViewportSettings) -> Option<Transform> {
    let elements_bound = viewport_settings.bounded_rect?;

    get_rect_identity_transform(
        viewport_settings.container_rect,
        elements_bound,
        0.7,
        viewport_settings.container_rect.center(),
    )
}

/// given two rects how to transform them such that they're both equal
pub fn get_rect_identity_transform(
    origin: egui::Rect, source: egui::Rect, padding_coeff: f32, anchor: egui::Pos2,
) -> Option<Transform> {
    let is_width_smaller = source.width() < source.height();
    let zoom_delta = if is_width_smaller {
        origin.height() * padding_coeff / source.height()
    } else {
        origin.width() * padding_coeff / source.width()
    };
    let center_x = anchor.x - zoom_delta * (source.left() + source.width() / 2.0);
    let center_y = anchor.y - zoom_delta * (source.top() + source.height() / 2.0);
    Some(
        Transform::identity()
            .post_scale(zoom_delta, zoom_delta)
            .post_translate(center_x, center_y),
    )
}

/// Content bounds in the buffer's current plane, including rendered stroke
/// width and pressure. Use the canvas's zoom, or 1.0 for an untransformed preview.
pub fn calc_elements_bounds(buffer: &Buffer, viewport_scale: f32) -> Option<egui::Rect> {
    let mut bounds = egui::Rect::NOTHING;
    for (id, el) in &buffer.elements {
        if el.deleted() {
            continue;
        }

        let rect = match el {
            Element::Path(path) => {
                let pressures = buffer
                    .weak_path_pressures
                    .get(id)
                    .map_or(&[][..], Vec::as_slice);
                stroked_path_bounds(path, pressures, viewport_scale)
            }
            Element::Image(image) => image.bounding_box(),
            Element::Text(_) => continue,
        };
        if rect.is_finite() {
            bounds = bounds.union(rect);
        }
    }
    bounds.is_finite().then_some(bounds)
}

pub fn zoom_percentage_to_transform(
    zoom_percentage: f32, viewport_settings: &ViewportSettings, ui: &mut egui::Ui,
) -> Transform {
    let zoom_delta = (zoom_percentage) / (viewport_settings.master_transform.sx * 100.0);
    Transform::identity()
        .post_scale(zoom_delta, zoom_delta)
        .post_translate(
            (1.0 - zoom_delta) * ui.ctx().content_rect().center().x,
            (1.0 - zoom_delta) * ui.ctx().content_rect().center().y,
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canvas_bounds_include_pressure_and_zoom_and_exclude_deleted_paths() {
        let mut buffer = Buffer::new(
            r##"<svg xmlns="http://www.w3.org/2000/svg"><path d="M0 0 L100 0" stroke="#000000" stroke-width="8" fill="none"/></svg>"##,
        );
        let id = *buffer.elements.first().unwrap().0;
        buffer.weak_path_pressures.insert(id, vec![1.0]);
        assert_eq!(
            calc_elements_bounds(&buffer, 1.0),
            Some(egui::Rect::from_min_max(egui::pos2(-8.0, -8.0), egui::pos2(108.0, 8.0),))
        );

        let mut viewport = ViewportSettings::default();
        transform_canvas(&mut buffer, &mut viewport, Transform::from_scale(2.0, 2.0));
        assert_eq!(
            calc_elements_bounds(&buffer, viewport.master_transform.sx),
            Some(egui::Rect::from_min_max(egui::pos2(-16.0, -16.0), egui::pos2(216.0, 16.0),))
        );

        let Element::Path(path) = buffer.elements.get_mut(&id).unwrap() else { panic!() };
        path.deleted = true;
        assert_eq!(calc_elements_bounds(&buffer, 1.0), None);
        assert_eq!(calc_elements_bounds(&Buffer::default(), 1.0), None);
    }
}
