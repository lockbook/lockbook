use std::borrow::BorrowMut;
use std::fmt::Write;
use std::sync::mpsc;

use eframe::egui;
use minidom::Element;
use resvg::tiny_skia::{Path, PathBuilder, PathSegment, Pixmap};
use resvg::usvg::{self, Size, TreeWriting};

use crate::theme::ThemePalette;

use super::toolbar::{ColorSwatch, Toolbar, Component};

const INITIAL_SVG_CONTENT: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\" ></svg>";
const ZOOM_STOP: f32 = 0.1;

pub struct SVGEditor {
    pub content: String,
    root: Element,
    draw_rx: mpsc::Receiver<(egui::Pos2, usize)>,
    draw_tx: mpsc::Sender<(egui::Pos2, usize)>,
    path_builder: PathBuilder,
    zoom_factor: f32,
    id_counter: usize,
    pub toolbar: Toolbar,
    inner_rect: egui::Rect,
    sao_offset: egui::Vec2,
}

impl SVGEditor {
    pub fn boxed(bytes: &[u8], _ctx: &egui::Context) -> Box<Self> {
        let (draw_tx, draw_rx) = mpsc::channel();

        // todo: handle invalid utf8
        let mut content = std::str::from_utf8(bytes).unwrap().to_string();
        if content.is_empty() {
            content = INITIAL_SVG_CONTENT.to_string();
        }
        let root: Element = content.parse().unwrap();

        let max_id = root
            .children()
            .map(|el| {
                let id: i32 = el.attr("id").unwrap_or("0").parse().unwrap_or_default();
                id
            })
            .max_by(|x, y| x.cmp(y))
            .unwrap_or_default()
            + 1;

        Box::new(Self {
            content,
            draw_rx,
            draw_tx,
            id_counter: max_id as usize,
            root,
            toolbar: Toolbar::new(),
            sao_offset: egui::vec2(0.0, 0.0),
            path_builder: PathBuilder::new(),
            inner_rect: egui::Rect::NOTHING,
            zoom_factor: 1.0,
        })
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        self.setup_draw_events(ui);
        self.draw_event_handler();

        self.define_dynamic_colors(ui);

        ui.vertical(|ui| {
            egui::Frame::default()
                .fill(if ui.visuals().dark_mode {
                    egui::Color32::GRAY.gamma_multiply(0.03)
                } else {
                    ui.visuals().faint_bg_color
                })
                .show(ui, |ui| {
                    self.toolbar.show(ui);
                    ui.set_width(ui.available_width());
                });

            self.inner_rect = ui.available_rect_before_wrap();
            self.render_svg(ui);
        });
    }

    pub fn get_minimal_content(&self) -> String {
        let utree: usvg::Tree =
            usvg::TreeParsing::from_data(self.content.as_bytes(), &usvg::Options::default())
                .unwrap();
        utree.to_string(&usvg::XmlOptions::default())
    }
    fn render_svg(&mut self, ui: &mut egui::Ui) {
        let mut utree: usvg::Tree =
            usvg::TreeParsing::from_data(self.content.as_bytes(), &usvg::Options::default())
                .unwrap();
        let available_rect = ui.available_rect_before_wrap();
        utree.size = Size::from_wh(available_rect.width(), available_rect.height()).unwrap();

        utree.view_box.rect = usvg::NonZeroRect::from_ltrb(
            available_rect.left(),
            available_rect.top(),
            available_rect.right(),
            available_rect.bottom(),
        )
        .unwrap();

        let tree = resvg::Tree::from_usvg(&utree);

        let pixmap_size = tree.size.to_int_size();
        let mut pixmap = Pixmap::new(pixmap_size.width(), pixmap_size.height()).unwrap();

        tree.render(usvg::Transform::default(), &mut pixmap.as_mut());
        let image = egui::ColorImage::from_rgba_unmultiplied(
            [pixmap.width() as usize, pixmap.height() as usize],
            pixmap.data(),
        );

        let texture = ui
            .ctx()
            .load_texture("svg_image", image, egui::TextureOptions::LINEAR);

        self.sao_offset = egui::ScrollArea::both()
            .show(ui, |ui| {
                ui.add(
                    egui::Image::new(
                        &texture,
                        egui::vec2(
                            texture.size()[0] as f32 * self.zoom_factor,
                            texture.size()[1] as f32 * self.zoom_factor,
                        ),
                    )
                    .sense(egui::Sense::click()),
                );
            })
            .state
            .offset;
    }

    fn define_dynamic_colors(&mut self, ui: &mut egui::Ui) {
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
    }

    fn build_color_defs(&mut self, ui: &mut egui::Ui) {
        let theme_colors = ThemePalette::as_array(ui.visuals().dark_mode);
        if self.toolbar.active_color.is_none() {
            self.toolbar.active_color = Some(ColorSwatch {
                id: "fg".to_string(),
                color: theme_colors.iter().find(|p| p.0.eq("fg")).unwrap().1,
            });
        }

        let btns = theme_colors.iter().map(|theme_color| {
            Component::ColorSwatch(ColorSwatch { id: theme_color.0.clone(), color: theme_color.1 })
        });
        self.toolbar.components = self
            .toolbar
            .components
            .clone()
            .into_iter()
            .filter(|c| !matches!(c, Component::ColorSwatch(_)))
            .chain(btns)
            .collect();

        theme_colors.iter().for_each(|theme_color| {
            let rgb_color =
                format!("rgb({} {} {})", theme_color.1.r(), theme_color.1.g(), theme_color.1.b());
            let gradient = Element::builder("linearGradient", "")
                .attr("id", theme_color.0.as_str())
                .append(
                    Element::builder("stop", "")
                        .attr("stop-color", rgb_color)
                        .build(),
                )
                .build();
            self.root.borrow_mut().append_child(gradient);

            let mut buffer = Vec::new();
            self.root.write_to(&mut buffer).unwrap();
            self.content = std::str::from_utf8(&buffer).unwrap().to_string();
            self.content = self.content.replace("xmlns='' ", "");
        });
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

            if let Some(node) = current_path.as_mut() {
                self.path_builder.line_to(pos.x, pos.y);
                let path = self.path_builder.clone().finish().unwrap();
                let data = get_path_data(path);

                node.set_attr("d", data);

                if let Some(color) = &self.toolbar.active_color {
                    node.set_attr("stroke", format!("url(#{})", color.id));
                } else {
                    node.set_attr("stroke", "url(#fg)");
                }
            } else {
                self.path_builder.clear();

                self.path_builder.move_to(pos.x, pos.y);
                self.path_builder.line_to(pos.x, pos.y);
                let path = self.path_builder.clone().finish().unwrap();
                let data = get_path_data(path);
                let child = Element::builder("path", "")
                    .attr("stroke-width", self.toolbar.active_stroke_width.to_string())
                    .attr("fill", "none")
                    .attr("stroke-linejoin", "round")
                    .attr("stroke-linecap", "round")
                    .attr("id", id)
                    .attr("d", data)
                    .build();

                self.root.append_child(child);
            }

            let mut buffer = Vec::new();

            self.root.write_to(&mut buffer).unwrap();
            self.content = std::str::from_utf8(&buffer).unwrap().to_string();
            self.content = self.content.replace("xmlns='' ", "");
        }
    }

    fn setup_draw_events(&mut self, ui: &mut egui::Ui) {
        if let Some(mut cursor_pos) = ui.ctx().pointer_hover_pos() {
            if !self.inner_rect.contains(cursor_pos) || !ui.is_enabled() {
                return;
            }

            if ui.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::PlusEquals)) {
                self.zoom_factor += ZOOM_STOP;
            }

            if ui.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::Minus)) {
                self.zoom_factor -= ZOOM_STOP;
            }

            cursor_pos.x = (cursor_pos.x + self.sao_offset.x) / self.zoom_factor;
            cursor_pos.y = (cursor_pos.y + self.sao_offset.y) / self.zoom_factor;

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
            PathSegment::Close => s.write_str("Z "),
        }
        .unwrap();
    }

    s.pop(); // ' '
    s
}
