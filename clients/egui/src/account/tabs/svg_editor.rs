use std::rc::Rc;
use std::sync::mpsc;

use eframe::egui;
use resvg::tiny_skia::{PathBuilder, Pixmap};
use resvg::usvg::{
    self, Align, Color, Fill, NodeExt, NodeKind, NonZeroPositiveF32, NonZeroRect, Paint, Path,
    Size, Stroke, Transform, TreeWriting, ViewBox, XmlOptions,
};

pub struct SVGEditor {
    raw: Vec<u8>,
    stroke_rx: mpsc::Receiver<(egui::Pos2, usize)>,
    stroke_tx: mpsc::Sender<(egui::Pos2, usize)>,
    path_builder: PathBuilder,
    id_counter: usize,
}

impl SVGEditor {
    pub fn boxed(bytes: &[u8], _ctx: &egui::Context) -> Box<Self> {
        let (stroke_tx, stroke_rx) = mpsc::channel();

        Box::new(Self {
            raw: bytes.to_vec(),
            stroke_rx,
            stroke_tx,
            id_counter: 0,
            path_builder: PathBuilder::new(),
        })
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        let mut utree: usvg::Tree =
            usvg::TreeParsing::from_data(&self.raw, &usvg::Options::default()).unwrap();

        let available_rect = ui.available_rect_before_wrap();
        utree.size = utree
            .size
            .scale_to(Size::from_wh(available_rect.width(), available_rect.height()).unwrap());

        utree.view_box.rect = NonZeroRect::from_ltrb(
            available_rect.left(),
            available_rect.top(),
            available_rect.right(),
            available_rect.bottom(),
        )
        .unwrap();
        self.raw = utree.to_string(&XmlOptions::default()).as_bytes().to_vec();

        self.setup_path_events(ui);
        self.path_handler(&utree, ui);

        let tree = resvg::Tree::from_usvg(&utree);

        let pixmap_size = tree.size.to_int_size();
        let mut pixmap = Pixmap::new(pixmap_size.width(), pixmap_size.height()).unwrap();

        tree.render(Transform::default(), &mut pixmap.as_mut());
        let image = egui::ColorImage::from_rgba_unmultiplied(
            [pixmap.width() as usize, pixmap.height() as usize],
            &pixmap.data(),
        );

        let texture = ui
            .ctx()
            .load_texture("pdf_image", image, egui::TextureOptions::LINEAR);
        ui.add(
            egui::Image::new(
                &texture,
                egui::vec2(texture.size()[0] as f32, texture.size()[1] as f32),
            )
            .sense(egui::Sense::click()),
        );
    }

    fn path_handler(&mut self, utree: &usvg::Tree, ui: &mut egui::Ui) {
        while let Ok((pos, id)) = self.stroke_rx.try_recv() {
            if let Some(node) = utree.node_by_id(&id.to_string()) {
                if let NodeKind::Path(ref mut p) = *node.borrow_mut() {
                    self.path_builder.line_to(pos.x, pos.y);
                    p.data = Rc::new(self.path_builder.clone().finish().unwrap());
                }
            } else {
                self.path_builder.clear();

                self.path_builder.move_to(pos.x, pos.y);
                self.path_builder.line_to(pos.x, pos.y);
                let path = self.path_builder.clone().finish().unwrap();

                let text_color = ui.visuals().text_color();
                let mut stroke = Stroke::default();
                stroke.width = NonZeroPositiveF32::new(4.0).unwrap();
                stroke.paint =
                    Paint::Color(Color::new_rgb(text_color.r(), text_color.g(), text_color.b()));

                let node = NodeKind::Path(Path {
                    id: id.to_string(),
                    transform: Transform::default(),
                    visibility: usvg::Visibility::Visible,
                    fill: None,
                    stroke: Some(stroke),
                    paint_order: usvg::PaintOrder::StrokeAndFill,
                    rendering_mode: usvg::ShapeRendering::GeometricPrecision,
                    text_bbox: None,
                    data: Rc::new(path),
                });
                let new_child = usvg::Node::new(node);
                utree.root.append(new_child);
            }
            println!("{}", utree.to_string(&XmlOptions::default()));
            self.raw = utree.to_string(&XmlOptions::default()).as_bytes().to_vec();
        }
    }

    fn setup_path_events(&mut self, ui: &mut egui::Ui) {
        if let Some(cursor_pos) = ui.ctx().pointer_hover_pos() {
            if ui.input(|i| i.pointer.primary_down()) {
                self.stroke_tx.send((cursor_pos, self.id_counter)).unwrap();
            }
            if ui.input(|i| i.pointer.primary_released()) {
                self.id_counter += 1;
            }
        }
    }
}
