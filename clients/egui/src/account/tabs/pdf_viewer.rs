use eframe::egui;
use pdfium_render::prelude::*;

use crate::{
    theme::Icon,
    widgets::{Button, ToolBar},
};

pub struct PdfViewer {
    content: Vec<egui::TextureHandle>,
    current_page_num: usize,
    zoom_factor: f32,
    sao: Option<egui::scroll_area::ScrollAreaOutput<()>>,
}

impl PdfViewer {
    pub fn boxed(bytes: &[u8], ctx: &egui::Context) -> Box<Self> {
        let bindings = Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path("./"))
            .or_else(|_| Pdfium::bind_to_system_library())
            .unwrap();

        let render_config = PdfRenderConfig::new()
            .set_target_width(2000)
            .set_maximum_height(2000)
            .rotate_if_landscape(PdfPageRenderRotation::Degrees90, true);

        let content = Pdfium::new(bindings)
            .load_pdf_from_byte_slice(bytes, None)
            .unwrap()
            .pages()
            .iter()
            .map(|f| {
                let image = f.render_with_config(&render_config).unwrap().as_image();
                let size = [image.width() as _, image.height() as _];
                let image_buffer = image.to_rgba8();
                let pixels = image_buffer.as_flat_samples();
                let image = egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice());
                ctx.load_texture("foo", image, egui::TextureOptions::LINEAR)
            })
            .collect();

        Box::new(Self { content, current_page_num: 0, zoom_factor: 0.5, sao: None })
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        self.sao = Some(
            egui::ScrollArea::both()
                .id_source("sao_pdf")
                .show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        self.content.iter().for_each(|p| {
                            let res = ui.add(
                                egui::Image::new(
                                    p,
                                    egui::vec2(
                                        p.size()[0] as f32 * self.zoom_factor,
                                        p.size()[1] as f32 * self.zoom_factor,
                                    ),
                                )
                                .sense(egui::Sense::click()),
                            );
                            ui.add_space(10.0);

                            let offset: f32 = if self.sao.as_ref().is_some() {
                                self.sao.as_ref().unwrap().state.offset.y
                            } else {
                                0.0
                            };

                            let total_height = self.content[0].size()[1] as f32
                                * self.zoom_factor as f32
                                * self.content.len() as f32
                                + 10.0 * self.content.len() as f32;
                            let aspect = total_height / offset;

                            if res.clicked() {
                                if ui.input(|r| r.modifiers.alt) {
                                    self.zoom_factor -= 0.1;
                                    let new_offset: f32 = (self.content[0].size()[1] as f32
                                        * (self.zoom_factor as f32)
                                        * self.content.len() as f32
                                        + 10.0 * self.content.len() as f32)
                                        / aspect;

                                    println!("scrolling by {new_offset}");
                                    ui.scroll_with_delta(egui::vec2(0.0, -(new_offset - offset)));
                                } else {
                                    self.zoom_factor += 0.1;
                                    let new_offset: f32 = (self.content[0].size()[1] as f32
                                        * (self.zoom_factor as f32)
                                        * self.content.len() as f32
                                        + 10.0 * self.content.len() as f32)
                                        / aspect;

                                    println!("scrolling by {new_offset}");
                                    ui.scroll_with_delta(egui::vec2(0.0, -(new_offset - offset)));
                                }
                            }

                            if res
                                .rect
                                .contains(ui.ctx().pointer_hover_pos().unwrap_or_default())
                            {
                                if ui.input(|r| r.modifiers.alt) {
                                    ui.output_mut(|w| w.cursor_icon = egui::CursorIcon::ZoomOut);
                                } else {
                                    ui.output_mut(|w| w.cursor_icon = egui::CursorIcon::ZoomIn);
                                }
                            }
                        })
                    });
                }),
        );
    }
}
