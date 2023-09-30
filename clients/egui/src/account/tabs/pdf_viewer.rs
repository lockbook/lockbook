use eframe::egui;
use pdfium_render::prelude::*;
use pdfium_wrapper::PdfiumWrapper;

pub struct PdfViewer {
    content: Vec<egui::TextureHandle>,
    zoom_factor: f32,
    fit_page_zoom: f32,
    sao: Option<egui::scroll_area::ScrollAreaOutput<()>>,
}

const ZOOM_STOP: f32 = 0.1;
const MAX_ZOOM_IN_STOPS: f32 = 15.0;

impl PdfViewer {
    pub fn boxed(bytes: &[u8], ctx: &egui::Context) -> Box<Self> {
        let render_config = PdfRenderConfig::new()
            .set_target_width(2000)
            .set_maximum_height(2000)
            .rotate_if_landscape(PdfPageRenderRotation::Degrees90, true);

        let content: Vec<egui::TextureHandle> = PdfiumWrapper::new()
            .pdfium
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

        let mut fit_page_zoom = 0.0;
        if let Some(page) = content.get(0) {
            fit_page_zoom = ctx.used_rect().height() / page.size()[1] as f32;
        }
        Box::new(Self { content, zoom_factor: 2.0, fit_page_zoom, sao: None })
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
                                    self.zoom_factor =
                                        (self.zoom_factor - ZOOM_STOP).max(ZOOM_STOP);
                                } else {
                                    self.zoom_factor = (self.zoom_factor + ZOOM_STOP)
                                        .min(ZOOM_STOP * MAX_ZOOM_IN_STOPS + self.fit_page_zoom);
                                }

                                let new_offset: f32 = (self.content[0].size()[1] as f32
                                    * (self.zoom_factor as f32)
                                    * self.content.len() as f32
                                    + 10.0 * self.content.len() as f32)
                                    / aspect;
                                ui.scroll_with_delta(egui::vec2(100.0, -(new_offset - offset)));
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
