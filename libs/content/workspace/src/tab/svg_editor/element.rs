use lb_rs::blocking::Lb;
use lb_rs::model::svg::WeakTransform;
use lb_rs::model::svg::buffer::{Buffer, WeakViewportSettings};
use lb_rs::model::svg::diff::DiffState;
use lb_rs::model::svg::element::{Element, Image, Path, Stroke, WeakImage};
use resvg::usvg::{NonZeroRect, Transform};

use super::ViewportSettings;
use super::util::{demote_to_weak_rect, promote_weak_rect};

pub trait BoundedElement {
    fn bounding_box(&self) -> egui::Rect;
}

impl BoundedElement for Element {
    fn bounding_box(&self) -> egui::Rect {
        match self {
            Element::Path(p) => p.bounding_box(),
            Element::Image(image) => image.bounding_box(),

            Element::Text(_) => todo!(),
        }
    }
}

impl BoundedElement for Image {
    fn bounding_box(&self) -> egui::Rect {
        egui::Rect {
            min: egui::pos2(self.view_box.left(), self.view_box.top()),
            max: egui::pos2(self.view_box.right(), self.view_box.bottom()),
        }
    }
}

impl BoundedElement for Path {
    fn bounding_box(&self) -> egui::Rect {
        let default_rect = egui::Rect::NOTHING;
        if self.data.len() < 2 {
            return default_rect;
        }
        let bb = match self.data.bounding_box() {
            Some(val) => val,
            None => return default_rect,
        };

        egui::Rect {
            min: egui::pos2(bb[0].x as f32, bb[0].y as f32),
            max: egui::pos2(bb[1].x as f32, bb[1].y as f32),
        }
    }
}

/// Stroke width in drawing coordinates, including stylus pressure.
pub fn stroke_width(stroke: Stroke, transform: Transform, pressure: f32) -> f32 {
    stroke.width * (transform.sx + transform.sy) / 2.0 * (1.0 + pressure)
}

/// Include the widest stroke when measuring a path for rendering or export.
pub fn stroked_path_bounds(path: &Path, pressures: &[f32], viewport_scale: f32) -> egui::Rect {
    let bounds = path.bounding_box();
    let Some(stroke) = path.stroke else { return bounds };
    let pressure = pressures.iter().copied().fold(0.0_f32, f32::max);
    bounds.expand((stroke_width(stroke, path.transform, pressure) * viewport_scale).abs() / 2.0)
}

pub trait PromoteWeakImage {
    fn from_weak(value: WeakImage, lb: &Lb) -> Result<Image, String>;
}
impl PromoteWeakImage for Image {
    fn from_weak(value: WeakImage, lb: &Lb) -> Result<Self, String> {
        let data = lb
            .read_document(value.href, false)
            .map_err(|e| e.to_string())?;
        // Validate on the loading thread rather than panic during painting.
        image::load_from_memory(&data).map_err(|e| e.to_string())?;

        Ok(Image {
            data: resvg::usvg::ImageKind::PNG(data.into()),
            visible: true,
            transform: Transform::from_row(
                value.transform.sx,
                value.transform.ky,
                value.transform.kx,
                value.transform.sy,
                value.transform.tx,
                value.transform.ty,
            ),
            view_box: NonZeroRect::from_xywh(value.x, value.y, value.width, value.height)
                .ok_or("invalid drawing image bounds")?,
            href: value.href,
            opacity: value.opacity,
            diff_state: DiffState::new(),
            deleted: false,
        })
    }
}

impl From<ViewportSettings> for WeakViewportSettings {
    fn from(viewport: ViewportSettings) -> Self {
        WeakViewportSettings {
            bounded_rect: viewport.bounded_rect.map(demote_to_weak_rect),
            master_transform: WeakTransform::from(viewport.master_transform),
            left_locked: viewport.left_locked,
            right_locked: viewport.right_locked,
            bottom_locked: viewport.bottom_locked,
            top_locked: viewport.top_locked,
            viewport_transform: viewport.viewport_transform.map(WeakTransform::from),
        }
    }
}

impl From<WeakViewportSettings> for ViewportSettings {
    fn from(weak: WeakViewportSettings) -> Self {
        ViewportSettings {
            bounded_rect: weak.bounded_rect.map(promote_weak_rect),
            working_rect: egui::Rect::NOTHING,
            viewport_transform: weak.viewport_transform.map(Transform::from),
            master_transform: Transform::from(weak.master_transform),
            container_rect: egui::Rect::NOTHING,
            left_locked: weak.left_locked,
            right_locked: weak.right_locked,
            bottom_locked: weak.bottom_locked,
            top_locked: weak.top_locked,
        }
    }
}

pub trait PromoteBufferWeakImages {
    fn promote_weak_images(&mut self, master_transform: Transform, lb: &Lb) -> Result<(), String>;
}

impl PromoteBufferWeakImages for Buffer {
    fn promote_weak_images(&mut self, master_transform: Transform, lb: &Lb) -> Result<(), String> {
        // Resolve every reference before changing the buffer, so a failed read
        // leaves the original drawing intact and can be retried later.
        let mut images = Vec::with_capacity(self.weak_images.len());
        for (id, weak_image) in self.weak_images.iter() {
            let mut weak_image = *weak_image;
            weak_image.transform(master_transform);
            let mut image = Image::from_weak(weak_image, lb)?;
            image.diff_state.transformed = None;
            images.push((*id, weak_image.z_index, image));
        }
        images.sort_by_key(|(_, z_index, _)| *z_index);
        for (id, z_index, image) in images {
            self.elements.shift_insert(
                z_index.min(self.elements.len()),
                id,
                Element::Image(Box::new(image)),
            );
        }
        self.weak_images.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lb_rs::Uuid;

    #[test]
    fn failed_image_promotion_preserves_the_drawing() {
        let core = crate::tab::markdown_editor::tests::harness::build_lb();
        let mut buffer = Buffer::new(
            r##"<svg xmlns="http://www.w3.org/2000/svg"><path d="M0 0 L100 100" stroke="#000000"/></svg>"##,
        );
        buffer.weak_images.insert(
            Uuid::new_v4(),
            WeakImage {
                href: Uuid::new_v4(),
                transform: Default::default(),
                opacity: 1.0,
                width: 100.0,
                height: 100.0,
                x: 20.0,
                y: 30.0,
                z_index: 0,
            },
        );
        let before = buffer.serialize();
        assert!(
            buffer
                .promote_weak_images(Transform::from_scale(2.0, 2.0), &core)
                .is_err()
        );
        assert_eq!(buffer.serialize(), before);
    }
}
