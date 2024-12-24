use lb_rs::{
    blocking::Lb,
    svg::{
        diff::DiffState,
        element::{Element, Image, Path, WeakImage},
    },
};
use resvg::usvg::{NonZeroRect, Transform};

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
    fn from_weak(value: WeakImage, lb: &Lb) -> Image;
}
impl PromoteWeakImage for Image {
    fn from_weak(value: WeakImage, lb: &Lb) -> Self {
        let data = lb.read_document(value.href).expect("could not read image");

        Image {
            data: resvg::usvg::ImageKind::PNG(data.into()),
            visibility: resvg::usvg::Visibility::Visible,
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
