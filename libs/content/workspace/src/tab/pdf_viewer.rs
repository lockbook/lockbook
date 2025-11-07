use std::sync::Arc;

use crate::theme::icons::Icon;
use crate::widgets::Button;
use egui::load::SizedTexture;
use hayro::{InterpreterSettings, Pdf, RenderSettings};
use lb_rs::Uuid;

pub struct PdfViewer {
    pub id: Uuid,
    pub pdf: Pdf,

    renders: Vec<Content>,
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
const MAX_ZOOM_IN_STOPS: f32 = 15.0;
const SIDEBAR_WIDTH: f32 = 230.0;
const SPACE_BETWEEN_PAGES: f32 = 10.0;

impl PdfViewer {
    pub fn new(
        id: Uuid, bytes: Vec<u8>, ctx: &egui::Context, data_dir: &str, is_mobile_viewport: bool,
    ) -> Self {
        let pdf = Pdf::new(Arc::new(bytes)).unwrap();

        let renders = pdf
            .pages()
            .iter()
            .map(|page| {
                let image = hayro::render(
                    page,
                    &InterpreterSettings::default(),
                    &RenderSettings {
                        width: None,
                        height: None,
                        x_scale: ctx.pixels_per_point(),
                        y_scale: ctx.pixels_per_point(),
                    },
                );
                let size = [image.width() as _, image.height() as _];
                println!("SIZE: {:?}", size);
                let image =
                    egui::ColorImage::from_rgba_premultiplied(size, image.data_as_u8_slice());
                Content {
                    offset: None,
                    texture: ctx.load_texture("pdf_image", image, egui::TextureOptions::LINEAR),
                }
            })
            .collect();

        let sidebar = if is_mobile_viewport {
            None
        } else {
            let mut tn_render_settings = RenderSettings::default();
            tn_render_settings.x_scale = 0.5;
            tn_render_settings.y_scale = 0.5;

            let thumbnails = pdf
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
                        texture: ctx.load_texture(
                            "pdf_thumbnail",
                            image,
                            egui::TextureOptions::LINEAR,
                        ),
                    }
                })
                .collect();
            Some(SideBar {
                thumbnails,
                is_visible: true,
                active_thumbnail: 0,
                sa_offset: None,
                scroll_update: None,
            })
        };

        Self {
            id,
            renders,
            zoom_factor: None,
            fit_page_zoom: None,
            scroll_offset: None,
            scroll_update: None,
            sidebar,
            pdf,
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        ui.painter().rect_filled(
            ui.available_rect_before_wrap(),
            0.,
            ui.visuals().extreme_bg_color,
        );

        ui.vertical(|ui| {
            self.show_toolbar(ui);
        });

        self.show_sidebar(ui);

        if let Some(page) = self.renders.first() {
            if self.fit_page_zoom.is_none() {
                self.fit_page_zoom = Some(ui.available_height() / page.texture.size()[1] as f32);
                self.zoom_factor = self.fit_page_zoom;
            }
        }

        let mut sao = egui::ScrollArea::both();
        if let Some(delta) = self.scroll_update {
            sao = sao.vertical_scroll_offset(delta);
            self.scroll_update = None;
        }

        let mut offset_sum = 0.0;
        let res = egui::CentralPanel::default().show_inside(ui, |ui| {
            Some(
                // todo: read more about viewport to optimize large pdf rendering
                sao.show_viewport(ui, |ui, _| {
                    let renders_res = ui
                        .vertical_centered(|ui| {
                            for (i, p) in self.renders.iter_mut().enumerate() {
                                let img = egui::Image::new(egui::ImageSource::Texture(
                                    SizedTexture::new(
                                        &p.texture,
                                        egui::vec2(
                                            p.texture.size()[0] as f32
                                                * self.zoom_factor.unwrap_or(1.0),
                                            p.texture.size()[1] as f32
                                                * self.zoom_factor.unwrap_or(1.0),
                                        ),
                                    ),
                                ))
                                .sense(egui::Sense::click());

                                let res = if ui.available_size_before_wrap().x
                                    < img.size().unwrap_or_default()[0]
                                {
                                    ui.with_layout(
                                        egui::Layout::left_to_right(egui::Align::Center)
                                            .with_cross_justify(true),
                                        |ui| ui.add(img),
                                    )
                                    .inner
                                } else {
                                    ui.add(img)
                                };

                                if p.offset.is_none() {
                                    p.offset = Some(offset_sum);
                                    offset_sum += res.rect.height() + SPACE_BETWEEN_PAGES;
                                }

                                if let Some(sidebar) = &mut self.sidebar {
                                    if ui.clip_rect().contains(res.rect.center())
                                        && sidebar.active_thumbnail != i
                                    {
                                        sidebar.active_thumbnail = i;
                                        Self::scroll_thumbnail_to_page(sidebar);
                                    }
                                }

                                ui.add_space(SPACE_BETWEEN_PAGES);
                            }
                        })
                        .response;

                    if renders_res.clicked()
                        || ui.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::Equals))
                    {
                        self.update_zoom_factor(ZoomFactor::Increase);
                    }

                    if renders_res.clicked_by(egui::PointerButton::Secondary)
                        || ui.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::Minus))
                    {
                        self.update_zoom_factor(ZoomFactor::Decrease);
                    }
                })
                .state
                .offset,
            )
        });

        self.scroll_offset = res.inner;
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
                        self.update_zoom_factor(ZoomFactor::Decrease);
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
                        self.update_zoom_factor(ZoomFactor::Increase);
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

        egui::SidePanel::right("pdf_sidebar")
            .resizable(false)
            .show_separator_line(false)
            .show_animated_inside(ui, sidebar.is_visible, |ui| {
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
                                        egui::Image::new(egui::ImageSource::Texture(
                                            SizedTexture::new(
                                                &p.texture,
                                                egui::vec2(
                                                    SIDEBAR_WIDTH - sidebar_margin,
                                                    p.texture.size()[1] as f32
                                                        * (SIDEBAR_WIDTH - sidebar_margin)
                                                        / p.texture.size()[0] as f32,
                                                ),
                                            ),
                                        ))
                                        .tint(tint_color)
                                        .sense(egui::Sense::click()),
                                    );

                                    if sidebar.thumbnails[i].offset.is_none() {
                                        sidebar.thumbnails[i].offset = Some(offset_sum);
                                        offset_sum += res.rect.height() + sidebar_margin;
                                    }

                                    if res.hovered() {
                                        ui.output_mut(|w| {
                                            w.cursor_icon = egui::CursorIcon::PointingHand
                                        })
                                    }
                                    if res.clicked() {
                                        sidebar.active_thumbnail = i;

                                        // scroll to the page
                                        if let Some(content) =
                                            self.renders.get(sidebar.active_thumbnail)
                                        {
                                            if let Some(offset) = content.offset {
                                                self.scroll_update = Some(offset);
                                            }
                                        } else {
                                            return;
                                        }

                                        Self::scroll_thumbnail_to_page(sidebar);
                                    }

                                    ui.add_space(sidebar_margin);
                                }
                            });
                    })
                    .state
                    .offset,
                );
            });
    }

    fn update_zoom_factor(&mut self, mode: ZoomFactor) {
        if self.fit_page_zoom.is_none() || self.zoom_factor.is_none() {
            return;
        }
        self.renders.iter_mut().for_each(|r| {
            r.offset = None;
        });

        let y_offset = self.scroll_offset.unwrap_or(egui::vec2(0.0, 0.0)).y;

        let total_height = self.get_sao_height();
        let aspect = total_height / y_offset;

        self.zoom_factor = Some(match mode {
            ZoomFactor::Increase => (self.zoom_factor.unwrap() + ZOOM_STOP)
                .min(ZOOM_STOP * MAX_ZOOM_IN_STOPS + self.fit_page_zoom.unwrap()),
            ZoomFactor::Decrease => (self.zoom_factor.unwrap() - ZOOM_STOP).max(ZOOM_STOP),
        });

        let new_offset: f32 = self.get_sao_height() / aspect;

        self.scroll_update = Some(new_offset);
    }

    fn scroll_thumbnail_to_page(sidebar: &mut SideBar) {
        if let Some(content) = sidebar.thumbnails.get(sidebar.active_thumbnail) {
            if let Some(offset) = content.offset {
                sidebar.scroll_update = Some(offset);
            }
        }
    }
    // todo: refactor for dynamic sizing
    fn get_sao_height(&self) -> f32 {
        self.renders[0].texture.size()[1] as f32
            * self.zoom_factor.unwrap_or(1.0)
            * self.renders.len() as f32
            + 10.0 * self.renders.len() as f32
    }
}
