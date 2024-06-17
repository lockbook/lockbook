mod clip;
mod eraser;
mod history;
mod parser;
mod pen;
mod selection;
mod toolbar;
mod util;
mod zoom;

use std::time::Instant;

use crate::tab::svg_editor::toolbar::Toolbar;
pub use eraser::Eraser;
pub use history::DeleteElement;
pub use history::Event;
pub use history::InsertElement;
use lb_rs::Uuid;
pub use parser::Buffer;
pub use pen::CubicBezBuilder;
pub use pen::Pen;
use resvg::usvg::{self, ImageKind};
pub use toolbar::Tool;
use usvg_parser::Options;

use self::history::History;
use self::zoom::handle_zoom_input;

/// A shorthand for [ImageHrefResolver]'s string function.
pub type ImageHrefStringResolverFn = Box<dyn Fn(&str, &Options) -> Option<ImageKind> + Send + Sync>;

pub struct SVGEditor {
    buffer: parser::Buffer,
    history: History,
    pub toolbar: Toolbar,
    inner_rect: egui::Rect,
    core: lb_rs::Core,
    open_file: Uuid,
    skip_frame: bool,
    last_render: Instant,
}

impl SVGEditor {
    pub fn new(bytes: &[u8], core: lb_rs::Core, open_file: Uuid) -> Self {
        let content = std::str::from_utf8(bytes).unwrap();

        let buffer = parser::Buffer::new(content, &core);
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
            core,
            open_file,
            skip_frame: false,
            last_render: Instant::now(),
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        let frame_cost = Instant::now() - self.last_render;
        self.last_render = Instant::now();
        let mut anchor_count = 0;
        self.buffer.elements.iter().for_each(|(_, el)| {
            if let parser::Element::Path(p) = el {
                anchor_count += p.data.len()
            }
        });

        let mut top = self.inner_rect.right_top();
        top.x -= 150.0;
        ui.painter().debug_text(
            top,
            egui::Align2::LEFT_TOP,
            egui::Color32::RED,
            format!("{} anchor | {}fps", anchor_count, 1000 / frame_cost.as_millis()),
        );
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
                        self.inner_rect,
                    );

                    self.inner_rect = ui.available_rect_before_wrap();
                    self.render_svg(ui);
                });
        });

        handle_zoom_input(ui, self.inner_rect, &mut self.buffer);

        if ui.input(|r| r.multi_touch().is_some()) || self.skip_frame {
            self.skip_frame = false;
            return;
        }

        match self.toolbar.active_tool {
            Tool::Pen => {
                if let Some(_) = self.toolbar.pen.handle_input(
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

        self.buffer.elements.iter_mut().for_each(|(id, el)| {
            if let parser::Element::Image(img) = el {
                render_image(img, ui, id, &painter);
            }
        });

        for (_, el) in self.buffer.elements.iter_mut() {
            if let parser::Element::Path(path) = el {
                if path.data.len() < 1 || path.visibility.eq(&usvg::Visibility::Hidden) {
                    continue;
                }

                let stroke = path.stroke.unwrap_or_default();
                let alpha_stroke_color = stroke.color.gamma_multiply(path.opacity);

                if path.data.is_point() {
                    let origin = &path.data.manipulator_groups()[0];
                    let origin = egui::pos2(origin.anchor.x as f32, origin.anchor.y as f32);
                    let circle =
                        epaint::CircleShape::filled(origin, stroke.width / 2.0, alpha_stroke_color);
                    painter.add(circle);
                } else {
                    path.data.iter().for_each(|bezier| {
                        let bezier = bezier.to_cubic();

                        let points: Vec<egui::Pos2> = bezier
                            .get_points()
                            .map(|dvec| egui::pos2(dvec.x as f32, dvec.y as f32))
                            .collect();
                        let epath = epaint::CubicBezierShape::from_points_stroke(
                            points.try_into().unwrap(),
                            false,
                            egui::Color32::TRANSPARENT,
                            egui::Stroke {
                                width: stroke.width * self.buffer.master_transform.sx,
                                color: alpha_stroke_color,
                            },
                        );
                        painter.add(epath);
                    });
                };
            }
        }
    }
}

fn render_image(img: &mut parser::Image, ui: &mut egui::Ui, id: &String, painter: &egui::Painter) {
    match &img.data {
        ImageKind::JPEG(bytes) | ImageKind::PNG(bytes) => {
            let image = image::load_from_memory(&bytes).unwrap();

            let egui_image = egui::ColorImage::from_rgba_unmultiplied(
                [image.width() as usize, image.height() as usize],
                &image.to_rgba8(),
            );
            if img.texture.is_none() {
                img.texture = Some(ui.ctx().load_texture(
                    format!("canvas_img_{}", id),
                    egui_image,
                    egui::TextureOptions::LINEAR,
                ));
            }

            if let Some(texture) = &img.texture {
                let rect = egui::Rect {
                    min: egui::pos2(img.view_box.rect.left(), img.view_box.rect.top()),
                    max: egui::pos2(img.view_box.rect.right(), img.view_box.rect.bottom()),
                };
                let uv = egui::Rect {
                    min: egui::Pos2 { x: 0.0, y: 0.0 },
                    max: egui::Pos2 { x: 1.0, y: 1.0 },
                };

                let mut mesh = egui::Mesh::with_texture(texture.id());
                mesh.add_rect_with_uv(rect, uv, egui::Color32::WHITE.gamma_multiply(img.opacity));
                painter.add(egui::Shape::mesh(mesh));
            }
        }
        ImageKind::GIF(_) => todo!(),
        ImageKind::SVG(_) => todo!(),
    }
}
