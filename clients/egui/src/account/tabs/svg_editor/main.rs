use eframe::egui;
use minidom::Element;
use resvg::tiny_skia::Pixmap;
use resvg::usvg::{self, Size, TreeWriting};

use crate::theme::ThemePalette;

use super::history::Buffer;
use super::toolbar::{ColorSwatch, Component, Tool, Toolbar};

const INITIAL_SVG_CONTENT: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\" ></svg>";

pub struct SVGEditor {
    buffer: Buffer,
    pub toolbar: Toolbar,
    inner_rect: egui::Rect,
    sao_offset: egui::Vec2,
}

impl SVGEditor {
    pub fn boxed(bytes: &[u8]) -> Box<Self> {
        // todo: handle invalid utf8
        let mut content = std::str::from_utf8(bytes).unwrap().to_string();
        if content.is_empty() {
            content = INITIAL_SVG_CONTENT.to_string();
        }
        let root: Element = content.parse().unwrap();

        let max_id = root
            .children()
            .map(|el| {
                let id: usize = el.attr("id").unwrap_or("0").parse().unwrap_or_default();
                id
            })
            .max_by(|x, y| x.cmp(y))
            .unwrap_or_default()
            + 1;

        Box::new(Self {
            buffer: Buffer::new(root),
            toolbar: Toolbar::new(max_id),
            sao_offset: egui::vec2(0.0, 0.0),
            inner_rect: egui::Rect::NOTHING,
        })
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        match self.toolbar.active_tool {
            Tool::Pen => {
                self.toolbar.pen.setup_events(ui, self.inner_rect);
                while let Ok(event) = self.toolbar.pen.rx.try_recv() {
                    self.toolbar.pen.handle_events(event, &mut self.buffer);
                }
            }
            Tool::Eraser => {
                self.toolbar.eraser.setup_events(ui, self.inner_rect);
                while let Ok(event) = self.toolbar.eraser.rx.try_recv() {
                    self.toolbar.eraser.handle_events(event, &mut self.buffer);
                }
            }
        }

        self.define_dynamic_colors(ui);

        ui.vertical(|ui| {
            egui::Frame::default()
                .fill(if ui.visuals().dark_mode {
                    egui::Color32::GRAY.gamma_multiply(0.03)
                } else {
                    ui.visuals().faint_bg_color
                })
                .show(ui, |ui| {
                    self.toolbar.show(ui, &mut self.buffer);

                    ui.set_width(ui.available_width());
                });

            self.inner_rect = ui.available_rect_before_wrap();
            self.render_svg(ui);
        });
    }

    pub fn get_minimal_content(&self) -> String {
        let utree: usvg::Tree =
            usvg::TreeParsing::from_str(&self.buffer.to_string(), &usvg::Options::default())
                .unwrap();
        utree.to_string(&usvg::XmlOptions::default())
    }

    fn render_svg(&mut self, ui: &mut egui::Ui) {
        let mut utree: usvg::Tree =
            usvg::TreeParsing::from_str(&self.buffer.to_string(), &usvg::Options::default())
                .unwrap();

        // todo: only index when history changes
        self.toolbar.eraser.index_rects(&utree.root);

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
                        egui::vec2(texture.size()[0] as f32, texture.size()[1] as f32),
                    )
                    .sense(egui::Sense::click()),
                );
            })
            .state
            .offset;
    }

    fn define_dynamic_colors(&mut self, ui: &mut egui::Ui) {
        if self.buffer.current.attr("data-dark-mode").is_none() {
            self.buffer
                .current
                .set_attr("data-dark-mode", format!("{}", ui.visuals().dark_mode));
            self.build_color_defs(ui);
        }

        if let Some(svg_flag) = self.buffer.current.attr("data-dark-mode") {
            let svg_flag: bool = svg_flag.parse().unwrap_or(false);

            if svg_flag != ui.visuals().dark_mode {
                self.build_color_defs(ui);
                self.buffer
                    .current
                    .set_attr("data-dark-mode", format!("{}", ui.visuals().dark_mode));
            }
        }
    }

    fn build_color_defs(&mut self, ui: &mut egui::Ui) {
        let theme_colors = ThemePalette::as_array(ui.visuals().dark_mode);
        if self.toolbar.pen.active_color.is_none() {
            self.toolbar.pen.active_color = Some(ColorSwatch {
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

            self.buffer.current.append_child(gradient);
        });
    }
}
