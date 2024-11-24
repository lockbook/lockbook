use lb_rs::svg::element::{Element, Image, Path};

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
