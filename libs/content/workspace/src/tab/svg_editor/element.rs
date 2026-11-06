use lb_rs::model::svg::WeakTransform;
use lb_rs::model::svg::buffer::{Buffer, WeakViewportSettings};
use lb_rs::model::svg::diff::DiffState;
use lb_rs::model::svg::element::{Element, Image, Path, WeakImage};
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

pub trait PromoteWeakImage {
    fn from_weak(value: WeakImage) -> Option<Image>;
}
impl PromoteWeakImage for Image {
    fn from_weak(value: WeakImage) -> Option<Self> {
        let transform = Transform::from(value.transform);
        if ![
            transform.sx,
            transform.sy,
            transform.kx,
            transform.ky,
            transform.tx,
            transform.ty,
            value.opacity,
        ]
        .iter()
        .all(|v| v.is_finite())
        {
            return None;
        }
        Some(Image {
            visible: true,
            transform,
            view_box: NonZeroRect::from_xywh(value.x, value.y, value.width, value.height)?,
            href: value.href,
            opacity: value.opacity.clamp(0.0, 1.0),
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
    fn promote_weak_images(&mut self, master_transform: Transform);
}

impl PromoteBufferWeakImages for Buffer {
    fn promote_weak_images(&mut self, master_transform: Transform) {
        // Restore stacking deterministically; failed geometry stays in metadata on save.
        let mut images: Vec<_> = self
            .weak_images
            .iter()
            .map(|(id, image)| (*id, *image))
            .collect();
        images.sort_by_key(|(id, image)| (image.z_index, *id));
        for (id, mut weak_image) in images {
            weak_image.transform(master_transform);
            let Some(mut image) = Image::from_weak(weak_image) else { continue };
            image.diff_state.transformed = None;
            let index = weak_image.z_index.min(self.elements.len());
            self.elements
                .shift_insert(index, id, Element::Image(Box::new(image)));
            self.weak_images.remove(&id);
        }
    }
}
