use lb_rs::Uuid;
use lb_rs::blocking::Lb;
use lb_rs::model::errors::LbResult;
use lb_rs::model::svg::buffer::Buffer;
use lb_rs::model::svg::diff::DiffState;
use lb_rs::model::svg::element::{EditableElement, Image, Path, WeakImage};
use resvg::usvg::{NonZeroRect, Transform};

pub trait BoundedElement {
    fn bounding_box(&self) -> egui::Rect;
}

impl BoundedElement for EditableElement {
    fn bounding_box(&self) -> egui::Rect {
        match self {
            EditableElement::Path(p) => p.bounding_box(),
            EditableElement::Image(image) => image.bounding_box(),

            EditableElement::Text(_) => todo!(),
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

/// Where a drawing's images live: read by id, and pasted images stored
/// beside the drawing.
pub trait CanvasImages: Send + Sync {
    fn read(&self, id: Uuid) -> LbResult<Vec<u8>>;
    fn import(&self, drawing: Uuid, name: Option<&str>, data: &[u8]) -> Result<Uuid, String>;
}

impl CanvasImages for Lb {
    fn read(&self, id: Uuid) -> LbResult<Vec<u8>> {
        self.read_document(id, false)
    }

    fn import(&self, drawing: Uuid, name: Option<&str>, data: &[u8]) -> Result<Uuid, String> {
        crate::tab::import_image(self, drawing, name, data).map(|file| file.id)
    }
}

pub trait PromoteWeakImage {
    fn from_weak(value: WeakImage, images: &dyn CanvasImages) -> Image;
}
impl PromoteWeakImage for Image {
    fn from_weak(value: WeakImage, images: &dyn CanvasImages) -> Self {
        let data = images.read(value.href).expect("could not read image");

        Image {
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
            view_box: NonZeroRect::from_xywh(value.x, value.y, value.width, value.height).unwrap(),
            href: value.href,
            opacity: value.opacity,
            diff_state: DiffState::new(),
            deleted: false,
        }
    }
}

pub trait PromoteBufferWeakImages {
    fn promote_weak_images(&mut self, images: &dyn CanvasImages);
}

impl PromoteBufferWeakImages for Buffer {
    fn promote_weak_images(&mut self, images: &dyn CanvasImages) {
        let mut weak_images: Vec<_> = self.weak_images.drain().collect();
        weak_images.sort_by_key(|(_, image)| image.z_index);
        for (id, weak_image) in weak_images {
            let mut image = Image::from_weak(weak_image, images);

            image.diff_state.transformed = None;

            if weak_image.z_index >= self.elements.len() {
                self.elements
                    .insert(id, EditableElement::Image(Box::new(image)));
            } else {
                self.elements.shift_insert(
                    weak_image.z_index,
                    id,
                    EditableElement::Image(Box::new(image)),
                );
            };
        }
    }
}
