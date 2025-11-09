use std::{collections::HashMap, ops::Range, sync::Arc};

use crate::theme::icons::Icon;
use crate::widgets::Button;
use egui::{
    CentralPanel, Color32, ColorImage, Context, Image, ImageSource, Pos2, Rect, Rounding,
    ScrollArea, Sense, Stroke, TextureHandle, Ui, Vec2, load::SizedTexture,
};
use hayro::{InterpreterSettings, Pdf, RenderSettings};
use lb_rs::Uuid;

pub struct PdfViewer {
    pub id: Uuid,

    ctx: Context,
    pdf: Pdf,
    page_bounds: Vec<Rect>,
    page_cache: HashMap<usize, TextureHandle>,
    scale: f32,

    zoom_factor: Option<f32>,
    fit_page_zoom: Option<f32>,
    scroll_offset: Option<egui::Vec2>,
    scroll_update: Option<f32>,
    sidebar: Option<SideBar>,
}

struct SideBar {
    thumbnails: Vec<Content>,
    is_visible: bool,
    active_thumbnail: usize,
    sa_offset: Option<egui::Vec2>,
    scroll_update: Option<f32>,
}

#[derive(Clone)]
struct Content {
    offset: Option<f32>,
    texture: egui::TextureHandle,
}

enum ZoomFactor {
    Increase,
    Decrease,
}
const ZOOM_STOP: f32 = 0.1;
const SIDEBAR_WIDTH: f32 = 230.0;
const SPACE_BETWEEN_PAGES: f32 = 10.0;

// get dimensions from the pdf
// these dimensions will be '100%'
// but these aren't great starting dimensions
// we need to probably match height
// we will load the pdf calculate global sizing and provide that to the scroll area
// when we scroll we will re-calculate everything and ask the scroll area to scroll to
// the new top-left corner of the viewport that's zoomed in. basically need to confirm
// that the center stays in the same location.
impl PdfViewer {
    pub fn new(
        id: Uuid, bytes: Vec<u8>, ctx: &egui::Context, data_dir: &str, is_mobile_viewport: bool,
    ) -> Self {
        let pdf = Pdf::new(Arc::new(bytes)).unwrap();

        let mut s = Self {
            id,
            zoom_factor: None,
            fit_page_zoom: None,
            scroll_offset: None,
            scroll_update: None,
            sidebar: Default::default(),
            pdf,
            page_cache: Default::default(),
            page_bounds: Default::default(),
            ctx: ctx.clone(),
            scale: Default::default(),
        };

        if !is_mobile_viewport {
            s.setup_sidebar();
        };

        s
    }

    fn setup_sidebar(&mut self) {
        let tn_render_settings =
            RenderSettings { x_scale: 0.5, y_scale: 0.5, ..Default::default() };

        let thumbnails = self
            .pdf
            .pages()
            .iter()
            .map(|page| {
                let image =
                    hayro::render(page, &InterpreterSettings::default(), &tn_render_settings);
                let size = [image.width() as _, image.height() as _];
                let image =
                    egui::ColorImage::from_rgba_premultiplied(size, image.data_as_u8_slice());
                Content {
                    offset: None,
                    texture: self.ctx.load_texture(
                        "pdf_thumbnail",
                        image,
                        egui::TextureOptions::LINEAR,
                    ),
                }
            })
            .collect();

        self.sidebar = Some(SideBar {
            thumbnails,
            is_visible: true,
            active_thumbnail: 0,
            sa_offset: None,
            scroll_update: None,
        });
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        ui.painter().rect_filled(
            ui.available_rect_before_wrap(),
            0.,
            ui.visuals().extreme_bg_color,
        );

        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(
                Vec2 { x: ui.available_width() - SIDEBAR_WIDTH, y: ui.available_height() },
                Sense::focusable_noninteractive(),
            );

            println!("rect {rect:?}");
            ui.allocate_ui_at_rect(rect, |ui| {
                self.show_pages(ui);
            });
        });

        return; 
        ui.vertical(|ui| {
            // self.show_toolbar(ui);
            ui.horizontal(|ui| {
                let mut page_width = ui.available_width();
                if self.sidebar.is_some() {
                    page_width -= SIDEBAR_WIDTH;
                }

                ui.allocate_ui(Vec2 { x: page_width, y: ui.available_height() }, |ui| {
                    ui.vertical(|ui| {});
                });

                // ui.vertical(|ui| {
                //     self.show_sidebar(ui);
                // });
            });
        });
    }

    fn show_toolbar(&mut self, ui: &mut egui::Ui) {
        let sidebar_is_visible = match &mut self.sidebar {
            Some(s) => s.is_visible,
            None => false,
        };

        let zoom_controls_width = 150.0;
        let zoom_controls_height = 30.0;

        let centered_rect = egui::Rect {
            min: egui::pos2(
                ui.available_rect_before_wrap().left()
                    + ((ui.available_rect_before_wrap().width()
                        - if sidebar_is_visible { SIDEBAR_WIDTH } else { 0.0 }
                        - zoom_controls_width)
                        / 2.0),
                ui.available_rect_before_wrap().top(),
            ),
            max: egui::pos2(
                ui.available_rect_before_wrap().left()
                    + ((ui.available_rect_before_wrap().width()
                        - if sidebar_is_visible { SIDEBAR_WIDTH } else { 0.0 }
                        - zoom_controls_width)
                        / 2.0)
                    + zoom_controls_width,
                ui.available_rect_before_wrap().top() + zoom_controls_height,
            ),
        };

        let end_of_line_rect = egui::Rect {
            min: egui::pos2(
                ui.available_rect_before_wrap().right() - 50.0,
                ui.available_rect_before_wrap().top(),
            ),
            max: egui::pos2(
                ui.available_rect_before_wrap().right(),
                ui.available_rect_before_wrap().top() + zoom_controls_height,
            ),
        };

        if let Some(sidebar) = &mut self.sidebar {
            ui.allocate_ui_at_rect(end_of_line_rect, |ui| {
                let icon = Icon::TOGGLE_SIDEBAR;
                if Button::default().icon(&icon).show(ui).clicked() {
                    sidebar.is_visible = !sidebar.is_visible;
                }
            });
        }

        ui.allocate_ui_at_rect(centered_rect, |ui| {
            ui.columns(3, |cols| {
                cols[0].vertical_centered(|ui| {
                    if Button::default().icon(&Icon::ZOOM_OUT).show(ui).clicked() {
                        // self.update_zoom_factor(ZoomFactor::Decrease);
                    }
                });

                let mut zoom_percentage = 100.0;
                if self.zoom_factor.is_some() && self.fit_page_zoom.is_some() {
                    zoom_percentage = ((self.zoom_factor.unwrap() - self.fit_page_zoom.unwrap())
                        / ZOOM_STOP)
                        .round()
                        * 10.0
                        + 100.0;
                }

                cols[1].horizontal_centered(|ui| {
                    ui.add_space(7.0);
                    ui.vertical(|ui| {
                        ui.add_space(7.0);
                        ui.colored_label(
                            ui.visuals().text_color().linear_multiply(0.7),
                            format!("{zoom_percentage}%"),
                        );
                    });
                });

                cols[2].vertical_centered(|ui| {
                    if Button::default().icon(&Icon::ZOOM_IN).show(ui).clicked() {
                        // self.update_zoom_factor(ZoomFactor::Increase);
                    };
                });
            });
        });
    }

    fn show_sidebar(&mut self, ui: &mut egui::Ui) {
        let sidebar = match &mut self.sidebar {
            Some(s) => s,
            None => return,
        };

        let sidebar_margin = 50.0;
        let mut offset_sum = 0.0;

        let mut sao = egui::ScrollArea::vertical();
        if let Some(delta) = sidebar.scroll_update {
            sao = sao.vertical_scroll_offset(delta);
            sidebar.scroll_update = None;
        }

        sidebar.sa_offset = Some(
            sao.show(ui, |ui| {
                egui::Frame::default()
                    .inner_margin(sidebar_margin)
                    .show(ui, |ui| {
                        for (i, p) in sidebar.thumbnails.clone().iter_mut().enumerate() {
                            let tint_color = if i == sidebar.active_thumbnail {
                                egui::Color32::WHITE
                            } else {
                                egui::Color32::GRAY.linear_multiply(0.5)
                            };

                            let res = ui.add(
                                egui::Image::new(egui::ImageSource::Texture(SizedTexture::new(
                                    &p.texture,
                                    egui::vec2(
                                        SIDEBAR_WIDTH - sidebar_margin,
                                        p.texture.size()[1] as f32
                                            * (SIDEBAR_WIDTH - sidebar_margin)
                                            / p.texture.size()[0] as f32,
                                    ),
                                )))
                                .tint(tint_color)
                                .sense(egui::Sense::click()),
                            );

                            if sidebar.thumbnails[i].offset.is_none() {
                                sidebar.thumbnails[i].offset = Some(offset_sum);
                                offset_sum += res.rect.height() + sidebar_margin;
                            }

                            if res.hovered() {
                                ui.output_mut(|w| w.cursor_icon = egui::CursorIcon::PointingHand)
                            }
                            // if res.clicked() {
                            //     sidebar.active_thumbnail = i;

                            //     // scroll to the page
                            //     if let Some(content) =
                            //         self.renders.get(sidebar.active_thumbnail)
                            //     {
                            //         if let Some(offset) = content.offset {
                            //             self.scroll_update = Some(offset);
                            //         }
                            //     } else {
                            //         return;
                            //     }

                            //     Self::scroll_thumbnail_to_page(sidebar);
                            // }

                            ui.add_space(sidebar_margin);
                        }
                    });
            })
            .state
            .offset,
        );
    }

    fn show_pages(&mut self, ui: &mut Ui) {
        ScrollArea::both().show_viewport(ui, |ui, viewport| {
            if self.scale == 0. {
                self.scale = match self.pdf.pages().first().map(|p| p.render_dimensions().1) {
                    Some(height) => ui.available_height() / height,
                    None => 1.,
                };
            }

            if self.page_bounds.is_empty() {
                self.compute_bounds();
            }
            let max_height = self.page_bounds[self.page_bounds.len() - 1].max.y;
            let max_width = self
                .page_bounds
                .iter()
                .map(|r| r.width().ceil() as u32)
                .max()
                .unwrap_or_default() as f32;

            // ui.vertical_centered(|ui| {
                let (_, rect) = ui.allocate_space(egui::Vec2 { x: max_width, y: max_height });
                println!("{rect:?}");

                for idx in 0..self.page_bounds.len() {
                    let page_rect = self.page_bounds[idx];
                    if page_rect.intersects(viewport) {
                        let paint_location = page_rect.translate(rect.min.to_vec2());
                        let img = self.get_page(idx);
                        ui.painter().rect_stroke(
                            paint_location,
                            Rounding::ZERO,
                            Stroke::new(1., Color32::RED),
                        );
                        img.paint_at(ui, paint_location);
                    }
                }
            //});
        });
    }

    fn get_page(&mut self, idx: usize) -> Image<'_> {
        let page = &self.pdf.pages()[idx];

        let texture = self.page_cache.get(&idx).cloned().unwrap_or_else(|| {
            let pixmap = hayro::render(
                page,
                &InterpreterSettings::default(),
                &RenderSettings { x_scale: self.scale, y_scale: self.scale, ..Default::default() },
            );
            let image = ColorImage::from_rgba_premultiplied(
                [pixmap.width() as _, pixmap.height() as _],
                pixmap.data_as_u8_slice(),
            );
            self.ctx
                .load_texture("pdf_page", image, egui::TextureOptions::LINEAR)
        });
        self.page_cache.insert(idx, texture.clone());

        let img = Image::new(ImageSource::Texture(SizedTexture {
            id: texture.id(),
            size: texture.size_vec2(),
        }));

        img
    }

    fn compute_bounds(&mut self) {
        let mut pages = vec![];

        let mut offset = Pos2::ZERO;

        for page in self.pdf.pages().iter() {
            let mut dims = Vec2::new(page.render_dimensions().0, page.render_dimensions().1);
            dims *= self.scale;

            pages.push(Rect { min: offset, max: offset + dims });

            offset.y += dims.y + SPACE_BETWEEN_PAGES;
        }

        self.page_bounds = pages;
    }

    // fn update_zoom_factor(&mut self, mode: ZoomFactor) {
    //     if self.fit_page_zoom.is_none() || self.zoom_factor.is_none() {
    //         return;
    //     }
    //     self.renders.iter_mut().for_each(|r| {
    //         r.offset = None;
    //     });

    //     let y_offset = self.scroll_offset.unwrap_or(egui::vec2(0.0, 0.0)).y;

    //     let total_height = self.get_sao_height();
    //     let aspect = total_height / y_offset;

    //     self.zoom_factor = Some(match mode {
    //         ZoomFactor::Increase => (self.zoom_factor.unwrap() + ZOOM_STOP)
    //             .min(ZOOM_STOP * MAX_ZOOM_IN_STOPS + self.fit_page_zoom.unwrap()),
    //         ZoomFactor::Decrease => (self.zoom_factor.unwrap() - ZOOM_STOP).max(ZOOM_STOP),
    //     });

    //     let new_offset: f32 = self.get_sao_height() / aspect;

    //     self.scroll_update = Some(new_offset);
    // }

    // fn scroll_thumbnail_to_page(sidebar: &mut SideBar) {
    //     if let Some(content) = sidebar.thumbnails.get(sidebar.active_thumbnail) {
    //         if let Some(offset) = content.offset {
    //             sidebar.scroll_update = Some(offset);
    //         }
    //     }
    // }

    // // todo: refactor for dynamic sizing
    // fn get_sao_height(&self) -> f32 {
    //     self.renders[0].texture.size()[1] as f32
    //         * self.zoom_factor.unwrap_or(1.0)
    //         * self.renders.len() as f32
    //         + 10.0 * self.renders.len() as f32
    // }
}
