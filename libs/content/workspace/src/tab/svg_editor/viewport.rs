use resvg::usvg::Transform;

use super::ViewportSettings;
use super::element::BoundedElement;
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

/// result is in absolute plane
pub fn calc_elements_bounds(buffer: &Buffer) -> Option<egui::Rect> {
    let mut elements_bound =
        egui::Rect { min: egui::pos2(f32::MAX, f32::MAX), max: egui::pos2(f32::MIN, f32::MIN) };
    let mut dirty_bound = false;
    for (_, el) in buffer.elements.iter() {
        if el.deleted() {
            continue;
        }

        let el_rect = el.bounding_box();
        dirty_bound = true;

        elements_bound.min.x = elements_bound.min.x.min(el_rect.min.x);
        elements_bound.min.y = elements_bound.min.y.min(el_rect.min.y);

        elements_bound.max.x = elements_bound.max.x.max(el_rect.max.x);
        elements_bound.max.y = elements_bound.max.y.max(el_rect.max.y);
    }
    if !dirty_bound { None } else { Some(elements_bound) }
}

pub fn zoom_percentage_to_transform(
    zoom_percentage: f32, viewport_settings: &ViewportSettings, ui: &mut egui::Ui,
) -> Transform {
    let zoom_delta = (zoom_percentage) / (viewport_settings.master_transform.sx * 100.0);
    Transform::identity()
        .post_scale(zoom_delta, zoom_delta)
        .post_translate(
            (1.0 - zoom_delta) * ui.ctx().screen_rect().center().x,
            (1.0 - zoom_delta) * ui.ctx().screen_rect().center().y,
        )
}
