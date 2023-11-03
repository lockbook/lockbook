use std::rc::Rc;
use std::sync::mpsc;

use eframe::egui;
use resvg::tiny_skia::{PathBuilder, Pixmap};
use resvg::usvg::{
    self, Align, Color, Fill, NodeExt, NodeKind, NonZeroPositiveF32, NonZeroRect, NormalizedF32,
    Paint, Path, Size, Stroke, Transform, TreeWriting, ViewBox, XmlOptions,
};

pub struct SVGEditor {
    raw: Vec<u8>,
    draw_rx: mpsc::Receiver<(egui::Pos2, usize)>,
    draw_tx: mpsc::Sender<(egui::Pos2, usize)>,
    path_builder: PathBuilder,
    id_counter: usize,
}

impl SVGEditor {
    pub fn boxed(bytes: &[u8], _ctx: &egui::Context) -> Box<Self> {
        let (draw_tx, draw_rx) = mpsc::channel();

        Box::new(Self {
            raw: bytes.to_vec(),
            draw_rx,
            draw_tx,
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

        self.show_svg_render(&utree, ui);
        /*
         * how to detect if there's a theme switch?
         * 1. mismatch between egui dark_mode flag and  svg custom attribute data-theme says
         * I think this is impossible with resvg because it erases any custom attributes
         * 2. store global memory in egui cache
         * 3. fire an event when setting up the light/dark in the settings modal
         *
         * ---
         *
         * how to change color dynamically?
         * add css styles. difficulties will arise from the fact that resvg erases css after applying
         * use stroke=url() to a def variable and ensure 2 way binding between variable value and theme state
         *
         * ==> resvg is pretty poor for manipulating and composing a tree. I'm going with Bodoni/svg
         */
        // approach 1
        //, then go through each node and update color
        // have data-color='label' store a hex that informs color

        // approach 2
        // if the egui says it's dark, but svg says it's light, then update defs

        println!("{}", ui.visuals().dark_mode);
    }

    fn show_svg_render(&self, utree: &usvg::Tree, ui: &mut egui::Ui) {
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
        while let Ok((pos, id)) = self.draw_rx.try_recv() {
            let text_color = ui.visuals().text_color();
            if let Some(node) = utree.node_by_id(&id.to_string()) {
                if let NodeKind::Path(ref mut p) = *node.borrow_mut() {
                    self.path_builder.line_to(pos.x, pos.y);
                    if let Some(stroke) = &mut p.stroke {
                        // stroke.paint = Paint::Color(Color::new_rgb(
                        //     text_color.r(),
                        //     text_color.g(),
                        //     text_color.b(),
                        // ));
                        stroke.paint = Paint::Color(Color::black());
                        stroke.opacity = NormalizedF32::new(0.3).unwrap();
                    }
                    p.data = Rc::new(self.path_builder.clone().finish().unwrap());
                }
            } else {
                self.path_builder.clear();

                self.path_builder.move_to(pos.x, pos.y);
                self.path_builder.line_to(pos.x, pos.y);
                let path = self.path_builder.clone().finish().unwrap();

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
                self.draw_tx.send((cursor_pos, self.id_counter)).unwrap();
            }
            if ui.input(|i| i.pointer.primary_released()) {
                self.id_counter += 1;
            }
        }
    }
}
