use eframe::egui;
use pdfium_render::prelude::*;

pub struct TemplateApp {
    pdf: Vec<egui::TextureHandle>,
    current_page_num: usize,
    zoom_factor: f32,
}
pub struct PdfViewer {
    content: Vec<egui::TextureHandle>,
    current_page_num: usize,
    zoom_factor: f32,
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

        Box::new(Self { content, current_page_num: 0, zoom_factor: 0.5 })
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::both().show(ui, |ui| {
            ui.vertical_centered(|ui| {
                self.content.iter().for_each(|p| {
                    let animated_zoom_factor = ui.ctx().animate_value_with_time(
                        egui::Id::from("width"),
                        self.zoom_factor,
                        0.2,
                    );
                    ui.image(
                        p,
                        egui::vec2(
                            p.size()[0] as f32 * animated_zoom_factor,
                            p.size()[1] as f32 * animated_zoom_factor,
                        ),
                    );
                });
            });
        });

        ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
            // ui.horizontal(|ui| {
            //     if ui.button("<- PREV").clicked() {
            //         self.current_page_num -= 1;
            //     }
            //     if ui.button("NEXT ->").clicked() {
            //         self.current_page_num += 1;
            //     }
            //     ui.add_space(5.0);
            // });

            ui.add_space(30.0);

            ui.horizontal(|ui| {
                if ui.button("+").clicked() {
                    self.zoom_factor += 0.1;
                }
                ui.add_space(5.0);
                if ui.button("-").clicked() {
                    self.zoom_factor -= 0.1;
                }
            })
        });
        // ui.add_space(50.0);
    }
}
