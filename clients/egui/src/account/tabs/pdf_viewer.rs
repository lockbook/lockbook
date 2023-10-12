use eframe::egui;
use lb_pdf::PdfiumWrapper;
use pdfium_render::prelude::*;

use crate::util::data_dir;

pub struct PdfViewer {
    content: Vec<egui::TextureHandle>,
    zoom_factor: f32,
    fit_page_zoom: f32,
    sa_offset: Option<egui::Vec2>,
    scroll_update: Option<f32>,
}

enum ZoomFactor {
    Increase,
    Decrease,
}
const ZOOM_STOP: f32 = 0.1;
const MAX_ZOOM_IN_STOPS: f32 = 15.0;

impl PdfViewer {
    pub fn boxed(bytes: &[u8], ctx: &egui::Context) -> Box<Self> {
        let render_config = PdfRenderConfig::new()
            .set_target_width(2000)
            .set_maximum_height(2000)
            .rotate_if_landscape(PdfPageRenderRotation::Degrees90, true);

        let pdfium_binary_path = format!("{}/egui", data_dir().unwrap());

        PdfiumWrapper::init(&pdfium_binary_path);
        let bindings = Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path(
            &pdfium_binary_path,
        ))
        .unwrap();

        let content: Vec<egui::TextureHandle> = Pdfium::new(bindings)
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

        Box::new(Self {
            content,
            zoom_factor: fit_page_zoom,
            fit_page_zoom,
            sa_offset: None,
            scroll_update: None,
        })
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        self.show_toolbar(ui);

        let mut sao = egui::ScrollArea::both();
        if let Some(delta) = self.scroll_update {
            sao = sao.vertical_scroll_offset(delta);
            self.scroll_update = None;
        }

        self.sa_offset = Some(
            sao.show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    self.content.clone().iter().for_each(|p| {
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

                        if res.clicked() {
                            self.update_zoom_factor(ZoomFactor::Increase);
                        }

                        if res.clicked_by(egui::PointerButton::Secondary) {
                            self.update_zoom_factor(ZoomFactor::Decrease);
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
            })
            .state
            .offset,
        );
    }

    fn show_toolbar(&mut self, ui: &mut egui::Ui) {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
            ui.horizontal(|ui| {
                if ui.button("+").clicked() {
                    self.update_zoom_factor(ZoomFactor::Increase);
                }

                let normalized_zoom_factor =
                    ((self.zoom_factor - self.fit_page_zoom) / ZOOM_STOP).round() * 10.0 + 100.0;
                ui.label(format!("{}%", normalized_zoom_factor));

                if ui.button("-").clicked() {
                    self.update_zoom_factor(ZoomFactor::Decrease);
                }
            });
        });
    }

    fn update_zoom_factor(&mut self, mode: ZoomFactor) {
        let y_offset = self.sa_offset.unwrap_or(egui::vec2(0.0, 0.0)).y;

        let total_height = self.get_sao_height();
        let aspect = total_height / y_offset;

        self.zoom_factor = match mode {
            ZoomFactor::Increase => (self.zoom_factor + ZOOM_STOP)
                .min(ZOOM_STOP * MAX_ZOOM_IN_STOPS + self.fit_page_zoom),
            ZoomFactor::Decrease => (self.zoom_factor - ZOOM_STOP).max(ZOOM_STOP),
        };

        let new_offset: f32 = self.get_sao_height() / aspect;

        self.scroll_update = Some(new_offset);
    }

    fn get_sao_height(&self) -> f32 {
        self.content[0].size()[1] as f32 * self.zoom_factor * self.content.len() as f32
            + 10.0 * self.content.len() as f32
    }
}
