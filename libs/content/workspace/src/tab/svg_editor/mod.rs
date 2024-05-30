mod clip;
mod eraser;
mod history;
mod parser;
mod pen;
mod selection;
mod toolbar;
mod util;
mod zoom;

use crate::tab::svg_editor::toolbar::Toolbar;
use egui::load::SizedTexture;
pub use eraser::Eraser;
pub use history::DeleteElement;
pub use history::Event;
pub use history::InsertElement;
use lb_rs::Uuid;
pub use parser::Buffer;
pub use pen::CubicBezBuilder;
pub use pen::Pen;
use resvg::usvg::{self, ImageKind, Rect};
pub use toolbar::Tool;
use usvg_parser::Options;
pub use util::node_by_id;

use self::history::History;
use self::zoom::handle_zoom_input;

/// A shorthand for [ImageHrefResolver]'s string function.
pub type ImageHrefStringResolverFn = Box<dyn Fn(&str, &Options) -> Option<ImageKind> + Send + Sync>;

pub struct SVGEditor {
    buffer: parser::Buffer,
    history: History,
    pub toolbar: Toolbar,
    inner_rect: egui::Rect,
    content_area: Option<Rect>,
    core: lb_rs::Core,
    open_file: Uuid,
    skip_frame: bool,
}

impl SVGEditor {
    pub fn new(bytes: &[u8], core: lb_rs::Core, open_file: Uuid) -> Self {
        let content = std::str::from_utf8(bytes).unwrap();

        let buffer = parser::Buffer::new(content);
        let max_id = buffer
            .elements
            .keys()
            .filter_map(|key_str| key_str.parse::<usize>().ok())
            .max()
            .unwrap_or_default()
            + 1;

        let toolbar = Toolbar::new(max_id);

        Self {
            buffer,
            history: History::default(),
            toolbar,
            inner_rect: egui::Rect::NOTHING,
            content_area: None,
            core,
            open_file,
            skip_frame: false,
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            egui::Frame::default()
                .fill(if ui.visuals().dark_mode {
                    egui::Color32::GRAY.gamma_multiply(0.03)
                } else {
                    ui.visuals().faint_bg_color
                })
                .show(ui, |ui| {
                    self.toolbar.show(
                        ui,
                        &mut self.buffer,
                        &mut self.history,
                        &mut self.skip_frame,
                    );
                });

            self.inner_rect = ui.available_rect_before_wrap();
            self.render_svg(ui);
        });

        handle_zoom_input(ui, self.inner_rect, &mut self.buffer);

        if ui.input(|r| r.multi_touch().is_some()) || self.skip_frame {
            self.skip_frame = false;
            return;
        }

        match self.toolbar.active_tool {
            Tool::Pen => {
                if let Some(res) = self.toolbar.pen.handle_input(
                    ui,
                    self.inner_rect,
                    &mut self.buffer,
                    &mut self.history,
                ) {
                    // let pen::PenResponse::ToggleSelection(id) = res;
                    // self.toolbar.set_tool(Tool::Selection);
                    // self.toolbar.selection.select_el_by_id(
                    //     id.to_string().as_str(),
                    //     ui.ctx().pointer_hover_pos().unwrap_or_default(),
                    //     &mut self.buffer,
                    // );
                }
            }
            Tool::Eraser => {
                self.toolbar.eraser.setup_events(ui, self.inner_rect);
                while let Ok(event) = self.toolbar.eraser.rx.try_recv() {
                    self.toolbar
                        .eraser
                        .handle_events(event, &mut self.buffer, &mut self.history);
                }
            }
            Tool::Selection => {
                self.toolbar.selection.handle_input(
                    ui,
                    self.inner_rect,
                    &mut self.buffer,
                    &mut self.history,
                );
            }
        }

        self.handle_clip_input(ui);
    }

    pub fn get_minimal_content(&self) -> String {
        self.buffer.to_string()
    }

    fn render_svg(&mut self, ui: &mut egui::Ui) {
        let painter = ui
            .allocate_painter(self.inner_rect.size(), egui::Sense::click_and_drag())
            .1;
        for (id, el) in self.buffer.elements.iter_mut() {
            match el {
                parser::Element::Path(path) => {
                    if path.data.len() < 1 || path.visibility.eq(&usvg::Visibility::Hidden) {
                        continue;
                    }

                    path.data.iter().for_each(|bezier| {
                        let bezier = bezier.to_cubic();

                        let points: Vec<egui::Pos2> = bezier
                            .get_points()
                            .map(|dvec| egui::pos2(dvec.x as f32, dvec.y as f32))
                            .collect();
                        let stroke = path.stroke.unwrap_or_default();
                        let epath = epaint::CubicBezierShape::from_points_stroke(
                            points.try_into().unwrap(),
                            false,
                            egui::Color32::TRANSPARENT,
                            egui::Stroke {
                                width: stroke.width, // todo determine stroke thickness based on scale
                                color: stroke.color.gamma_multiply(path.opacity),
                            },
                        );
                        painter.add(epath);
                    });
                }
                parser::Element::Image(img) => match &img.data {
                    ImageKind::JPEG(bytes) | ImageKind::PNG(bytes) => {
                        let image = egui::ColorImage::from_rgba_unmultiplied(
                            [
                                img.view_box.rect.width() as usize,
                                img.view_box.rect.height() as usize,
                            ],
                            &bytes,
                        );

                        if img.texture.is_none() {
                            img.texture = Some(ui.ctx().load_texture(
                                id,
                                image,
                                egui::TextureOptions::LINEAR,
                            ));
                        }

                        if let Some(texture) = &img.texture {
                            let img =
                                egui::Image::new(egui::ImageSource::Texture(SizedTexture::new(
                                    texture,
                                    egui::vec2(texture.size()[0] as f32, texture.size()[1] as f32),
                                )));
                            ui.add(img);
                        }
                    }
                    ImageKind::GIF(_) => todo!(),
                    ImageKind::SVG(_) => todo!(),
                },
                parser::Element::Text(text) => todo!(),
            }
        }
    }
}
