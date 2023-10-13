use eframe::egui;
use lb_pdf::PdfiumWrapper;
use pdfium_render::prelude::*;

use crate::{theme::Icon, util::data_dir, widgets::Button};

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
                ctx.load_texture("pdf_image", image, egui::TextureOptions::LINEAR)
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
        ui.vertical(|ui| {
            self.show_toolbar(ui);
            ui.separator();
        });

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
                    })
                });
            })
            .state
            .offset,
        );
    }

    fn show_toolbar(&mut self, ui: &mut egui::Ui) {
        let zoom_controls_width = 150.0;
        let zoom_controls_height = 30.0;

        let centered_rect = egui::Rect {
            min: egui::pos2(
                ui.available_rect_before_wrap().left()
                    + ((ui.available_rect_before_wrap().width() - zoom_controls_width) / 2.0),
                ui.available_rect_before_wrap().top(),
            ),
            max: egui::pos2(
                ui.available_rect_before_wrap().left()
                    + ((ui.available_rect_before_wrap().width() - zoom_controls_width) / 2.0)
                    + 150.0,
                ui.available_rect_before_wrap().top() + zoom_controls_height,
            ),
        };

        ui.allocate_ui_at_rect(centered_rect, |ui| {
            // ui.spacing_mut().button_padding = egui::vec2(5.0, 5.0);
            ui.columns(3, |cols| {
                cols[0].vertical_centered(|ui| {
                    if Button::default().icon(&Icon::ZOOM_OUT).show(ui).clicked() {
                        self.update_zoom_factor(ZoomFactor::Decrease);
                    }
                });

                let normalized_zoom_factor =
                    ((self.zoom_factor - self.fit_page_zoom) / ZOOM_STOP).round() * 10.0 + 100.0;
                cols[1].horizontal_centered(|ui| {
                    ui.add_space(7.0);
                    ui.vertical(|ui| {
                        ui.add_space(7.0);
                        ui.colored_label(
                            ui.visuals().text_color().gamma_multiply(0.7),
                            format!("{}%", normalized_zoom_factor),
                        );
                    });
                });

                cols[2].vertical_centered(|ui| {
                    if Button::default().icon(&Icon::ZOOM_IN).show(ui).clicked() {
                        self.update_zoom_factor(ZoomFactor::Increase);
                    };
                });
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
