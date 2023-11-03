use std::borrow::BorrowMut;
use std::fmt::Write;
use std::sync::mpsc;

use eframe::egui;
use minidom::Element;
use resvg::tiny_skia::{Path, PathBuilder, PathSegment, Pixmap};
use resvg::usvg;

pub struct SVGEditor {
    svg: String,
    root: Element,
    draw_rx: mpsc::Receiver<(egui::Pos2, usize)>,
    draw_tx: mpsc::Sender<(egui::Pos2, usize)>,
    path_builder: PathBuilder,
    id_counter: usize,
}

impl SVGEditor {
    pub fn boxed(bytes: &[u8], _ctx: &egui::Context) -> Box<Self> {
        let (draw_tx, draw_rx) = mpsc::channel();

        // todo: handle invalid utf8
        let svg = std::str::from_utf8(bytes).unwrap().to_string();

        let root: Element = svg.parse().unwrap();

        Box::new(Self {
            svg,
            draw_rx,
            draw_tx,
            id_counter: 0,
            root,
            path_builder: PathBuilder::new(),
        })
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        self.setup_draw_events(ui);
        self.draw_event_handler();

        if self.root.attr("data-dark-mode").is_none() {
            self.root
                .set_attr("data-dark-mode", format!("{}", ui.visuals().dark_mode));
            self.build_color_defs(ui);
        }

        if let Some(svg_flag) = self.root.attr("data-dark-mode") {
            let svg_flag: bool = svg_flag.parse().unwrap_or(false);

            if svg_flag != ui.visuals().dark_mode {
                self.build_color_defs(ui);
                self.root
                    .set_attr("data-dark-mode", format!("{}", ui.visuals().dark_mode));
            }
        }

        self.render_svg(ui);
    }

    fn build_color_defs(&mut self, ui: &mut egui::Ui) {
        let text_color = ui.visuals().text_color();
        let color = format!("rgb({} {} {})", text_color.r(), text_color.g(), text_color.b());

        let gradient = Element::builder("linearGradient", "")
            .attr("id", "fg")
            .append(
                Element::builder("stop", "")
                    .attr("stop-color", color)
                    .build(),
            )
            .build();

        self.root.borrow_mut().append_child(gradient);
    }

    fn render_svg(&self, ui: &mut egui::Ui) {
        let mut utree: usvg::Tree =
            usvg::TreeParsing::from_data(&self.svg.as_bytes(), &usvg::Options::default()).unwrap();
        let available_rect = ui.available_rect_before_wrap();
        utree.size = utree.size.scale_to(
            usvg::Size::from_wh(available_rect.width(), available_rect.height()).unwrap(),
        );

        utree.view_box.rect = usvg::NonZeroRect::from_ltrb(
            available_rect.left(),
            available_rect.top(),
            available_rect.right(),
            available_rect.bottom(),
        )
        .unwrap();

        // println!("{}", utree.to_string(&XmlOptions::default()));

        let tree = resvg::Tree::from_usvg(&utree);

        let pixmap_size = tree.size.to_int_size();
        let mut pixmap = Pixmap::new(pixmap_size.width(), pixmap_size.height()).unwrap();

        tree.render(usvg::Transform::default(), &mut pixmap.as_mut());
        let image = egui::ColorImage::from_rgba_unmultiplied(
            [pixmap.width() as usize, pixmap.height() as usize],
            &pixmap.data(),
        );

        let texture = ui
            .ctx()
            .load_texture("svg_image", image, egui::TextureOptions::LINEAR);
        ui.add(
            egui::Image::new(
                &texture,
                egui::vec2(texture.size()[0] as f32, texture.size()[1] as f32),
            )
            .sense(egui::Sense::click()),
        );
    }

    fn draw_event_handler(&mut self) {
        while let Ok((pos, id)) = self.draw_rx.try_recv() {
            let mut current_path = self.root.children_mut().find(|e| {
                if let Some(id) = e.attr("id") {
                    id == self.id_counter.to_string()
                } else {
                    false
                }
            });

            // let a = current_path.unwrap().to;
            if let Some(node) = current_path.as_mut() {
                self.path_builder.line_to(pos.x, pos.y);
                let path = self.path_builder.clone().finish().unwrap();
                let data = get_path_data(path);

                node.set_attr("d", data);
                node.set_attr("stroke", "url(#fg)");
            } else {
                self.path_builder.clear();

                self.path_builder.move_to(pos.x, pos.y);
                self.path_builder.line_to(pos.x, pos.y);
                let path = self.path_builder.clone().finish().unwrap();
                let data = get_path_data(path);
                let child = Element::builder("path", "")
                    .attr("stroke-width", 3)
                    .attr("fill", "none")
                    .attr("id", id)
                    .attr("d", data)
                    .build();

                self.root.append_child(child);
            }

            let mut buffer = Vec::new();
            self.root.write_to(&mut buffer).unwrap();
            self.svg = std::str::from_utf8(&buffer).unwrap().to_string();
            self.svg = self.svg.replace("xmlns=''", "");

            // println!("{}", self.svg);
        }
    }

    fn setup_draw_events(&mut self, ui: &mut egui::Ui) {
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

fn get_path_data(path: Path) -> String {
    let mut s = String::new();
    for segment in path.segments() {
        match segment {
            PathSegment::MoveTo(p) => s.write_str(format!("M {} {} ", p.x, p.y).as_str()),
            PathSegment::LineTo(p) => s.write_str(format!("L {} {} ", p.x, p.y).as_str()),
            PathSegment::QuadTo(p0, p1) => {
                s.write_str(format!("Q {} {} {} {} ", p0.x, p0.y, p1.x, p1.y).as_str())
            }
            PathSegment::CubicTo(p0, p1, p2) => s.write_str(
                format!("C {} {} {} {} {} {} ", p0.x, p0.y, p1.x, p1.y, p2.x, p2.y).as_str(),
            ),
            PathSegment::Close => s.write_str(format!("Z ").as_str()),
        }
        .unwrap();
    }

    s.pop(); // ' '
    return s;
}
