use eframe::egui;
use egui_extras::{Size, StripBuilder};
use resvg::tiny_skia::{Pixmap, PixmapMut};
use resvg::usvg::{self, Transform};

use crate::theme::{DrawingPalette, Icon};
use crate::widgets::ButtonGroup;

pub struct SVGEditor {
    texture: egui::TextureHandle,
}

impl SVGEditor {
    pub fn boxed(bytes: &[u8], ctx: &egui::Context) -> Box<Self> {
        let tree = usvg::TreeParsing::from_data(bytes, &usvg::Options::default()).unwrap();
        let tree = resvg::Tree::from_usvg(&tree);

        let pixmap_size = tree.size.to_int_size();

        let mut pixmap = Pixmap::new(pixmap_size.width(), pixmap_size.height()).unwrap();

        tree.render(Transform::default(), &mut pixmap.as_mut());

        let result = pixmap.encode_png().unwrap();

        let image = egui::ColorImage::from_rgba_unmultiplied(
            [pixmap.width() as usize, pixmap.height() as usize],
            &pixmap.data(),
        );
        let texture = ctx.load_texture("pdf_image", image, egui::TextureOptions::LINEAR);

        Box::new(Self { texture })
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        ui.add(
            egui::Image::new(
                &self.texture,
                egui::vec2(self.texture.size()[0] as f32, self.texture.size()[1] as f32),
            )
            .sense(egui::Sense::click()),
        );
    }
}
