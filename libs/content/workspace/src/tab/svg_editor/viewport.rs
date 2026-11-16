use resvg::usvg::Transform;

use super::ViewportSettings;
use super::element::BoundedElement;
use super::util::promote_weak_rect;
use lb_rs::model::svg::buffer::WeakViewportSettings;

use super::Buffer;
use super::coordinates::{
    CanvasCoordinates, DocumentPoint, DocumentRect, ScreenPoint, ScreenRect, ScreenTransform,
    ScreenVector,
};
pub const MIN_ZOOM_LEVEL: f32 = 0.1;

impl ViewportSettings {
    pub fn coordinates(&self) -> CanvasCoordinates {
        CanvasCoordinates::new(self.master_transform)
    }

    pub fn document_to_screen(&self, point: egui::Pos2) -> egui::Pos2 {
        self.coordinates()
            .screen_point(DocumentPoint(point))
            .into_inner()
    }

    pub fn screen_to_document(&self, point: egui::Pos2) -> egui::Pos2 {
        self.coordinates()
            .document_point(ScreenPoint(point))
            .into_inner()
    }

    pub fn screen_vector_to_document(&self, vector: egui::Vec2) -> egui::Vec2 {
        self.coordinates()
            .document_vector(ScreenVector(vector))
            .into_inner()
    }

    pub fn document_rect_to_screen(&self, rect: egui::Rect) -> egui::Rect {
        self.coordinates()
            .screen_rect(DocumentRect(rect))
            .into_inner()
    }

    pub fn screen_rect_to_document(&self, rect: egui::Rect) -> egui::Rect {
        self.coordinates()
            .document_rect(ScreenRect(rect))
            .into_inner()
    }

    pub fn screen_transform_to_document(&self, transform: Transform) -> Transform {
        self.coordinates()
            .document_transform(ScreenTransform(transform))
            .into_inner()
    }

    /// The saved transform maps into viewport-local coordinates. The UI's
    /// current screen position is runtime layout, not part of the saved view.
    pub fn viewport_transform(&self) -> Transform {
        self.master_transform
            .post_translate(-self.container_rect.min.x, -self.container_rect.min.y)
    }

    fn restore_transform(&mut self, transform: Transform) {
        self.master_transform =
            transform.post_translate(self.container_rect.min.x, self.container_rect.min.y);
    }

    pub fn update_container(&mut self, container: egui::Rect) {
        if self.container_rect.min != container.min {
            let transform = self.viewport_transform();
            self.container_rect = container;
            self.restore_transform(transform);
        } else {
            self.container_rect = container;
        }
    }

    pub fn initialize(&mut self, container: egui::Rect, buffer: &Buffer, saved: Option<Transform>) {
        self.container_rect = container;
        let bounds = calc_elements_bounds(buffer);
        if let Some(transform) = saved.filter(is_valid_transform) {
            self.restore_transform(transform);
            self.bounded_rect = bounds;
        } else if buffer.has_legacy_viewport {
            self.restore_legacy(buffer.weak_viewport_settings);
            self.bounded_rect = self.bounded_rect.or(bounds);
            if self.master_transform.sx < MIN_ZOOM_LEVEL {
                let center = self.screen_to_document(container.center());
                self.master_transform = Transform::from_scale(MIN_ZOOM_LEVEL, MIN_ZOOM_LEVEL)
                    .post_translate(
                        container.center().x - center.x * MIN_ZOOM_LEVEL,
                        container.center().y - center.y * MIN_ZOOM_LEVEL,
                    );
            }
        } else {
            self.bounded_rect = bounds;
            if let Some(transform) = get_zoom_fit_transform(self) {
                transform_canvas(self, transform);
            }
        }
        if self.bounded_rect.is_none() {
            self.bounded_rect = Some(self.screen_rect_to_document(container));
        }
    }

    pub fn restore_legacy(&mut self, state: WeakViewportSettings) {
        let transform = Transform::from(state.master_transform);
        if transform.is_finite()
            && transform.sx > 0.0
            && transform.sx == transform.sy
            && transform.kx == 0.0
            && transform.ky == 0.0
            && transform.invert().is_some()
        {
            self.master_transform = transform;
        }
        self.bounded_rect = state
            .bounded_rect
            .map(promote_weak_rect)
            .filter(|r| r.is_finite() && r.width() >= 0.0 && r.height() >= 0.0)
            .map(|r| self.screen_rect_to_document(r));
        self.left_locked = state.left_locked;
        self.right_locked = state.right_locked;
        self.top_locked = state.top_locked;
        self.bottom_locked = state.bottom_locked;
    }
}

pub fn transform_canvas(viewport_settings: &mut ViewportSettings, t: Transform) {
    let new_transform = viewport_settings.master_transform.post_concat(t);

    if !is_valid_transform(&new_transform) {
        return;
    }
    viewport_settings.master_transform = new_transform;
}

pub fn is_valid_transform(transform: &Transform) -> bool {
    transform.is_finite()
        && transform.sx >= MIN_ZOOM_LEVEL
        && transform.sx == transform.sy
        && transform.kx == 0.0
        && transform.ky == 0.0
        && transform.invert().is_some()
}

/// returns the fit transform in the non master transform plane
pub fn get_zoom_fit_transform(viewport_settings: &ViewportSettings) -> Option<Transform> {
    let elements_bound = viewport_settings.document_rect_to_screen(viewport_settings.bounded_rect?);

    let fit = get_rect_identity_transform(
        viewport_settings.container_rect,
        elements_bound,
        0.7,
        viewport_settings.container_rect.center(),
    )?;
    let scale = fit
        .sx
        .max(MIN_ZOOM_LEVEL / viewport_settings.master_transform.sx);
    let anchor = viewport_settings.container_rect.center();
    let center = elements_bound.center();
    Some(
        Transform::from_scale(scale, scale)
            .post_translate(anchor.x - scale * center.x, anchor.y - scale * center.y),
    )
}

/// given two rects how to transform them such that they're both equal
pub fn get_rect_identity_transform(
    origin: egui::Rect, source: egui::Rect, padding_coeff: f32, anchor: egui::Pos2,
) -> Option<Transform> {
    if !origin.is_finite()
        || !source.is_finite()
        || source.width() < 0.0
        || source.height() < 0.0
        || !anchor.is_finite()
    {
        return None;
    }
    let zoom_delta = (origin.width() / source.width().max(1.0))
        .min(origin.height() / source.height().max(1.0))
        * padding_coeff;
    if !zoom_delta.is_finite() || zoom_delta <= 0.0 {
        return None;
    }
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
        if !el_rect.is_finite() || el_rect.width() < 0.0 || el_rect.height() < 0.0 {
            continue;
        }
        dirty_bound = true;

        elements_bound.min.x = elements_bound.min.x.min(el_rect.min.x);
        elements_bound.min.y = elements_bound.min.y.min(el_rect.min.y);

        elements_bound.max.x = elements_bound.max.x.max(el_rect.max.x);
        elements_bound.max.y = elements_bound.max.y.max(el_rect.max.y);
    }
    if !dirty_bound { None } else { Some(elements_bound) }
}

pub fn zoom_percentage_to_transform(
    zoom_percentage: f32, viewport_settings: &ViewportSettings, _ui: &mut egui::Ui,
) -> Transform {
    let zoom_delta = (zoom_percentage) / (viewport_settings.master_transform.sx * 100.0);
    Transform::identity()
        .post_scale(zoom_delta, zoom_delta)
        .post_translate(
            (1.0 - zoom_delta) * viewport_settings.container_rect.center().x,
            (1.0 - zoom_delta) * viewport_settings.container_rect.center().y,
        )
}
