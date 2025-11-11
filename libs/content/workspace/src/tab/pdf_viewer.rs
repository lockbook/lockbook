use std::{collections::HashMap, sync::Arc};

use crate::theme::icons::Icon;
use crate::widgets::Button;
use egui::{
    Align, CentralPanel, ColorImage, Context, Event, Image, ImageSource, Key, Modifiers,
    Pos2, Rect, ScrollArea, SidePanel, TextureHandle, Ui, Vec2,
    load::SizedTexture,
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
    fit_width: bool,
    fit_height: bool,
    current_page: usize,
    scroll_to: Option<usize>,

    render_area: Rect,
    current_viewport: Rect,
    viewport_adjustment: Option<Vec2>,

    sidebar: Option<SideBar>,
}

struct SideBar {
    thumbnails: Vec<Content>,
    is_visible: bool,
    scroll_target: usize,
}

#[derive(Clone)]
struct Content {
    texture: egui::TextureHandle,
}

const ZOOM_STOP: f32 = 0.1;
const SIDEBAR_WIDTH: f32 = 230.0;
const SPACE_BETWEEN_PAGES: f32 = 10.0;

impl PdfViewer {
    pub fn new(id: Uuid, bytes: Vec<u8>, ctx: &egui::Context, is_mobile_viewport: bool) -> Self {
        let pdf = Pdf::new(Arc::new(bytes)).unwrap();

        let mut s = Self {
            id,
            sidebar: Default::default(),
            pdf,
            page_cache: Default::default(),
            page_bounds: Default::default(),
            ctx: ctx.clone(),
            scale: 1.,
            fit_width: true,
            scroll_to: None,
            current_page: 0,
            fit_height: false,
            current_viewport: Rect::ZERO,
            viewport_adjustment: Default::default(),
            render_area: Rect::ZERO,
        };

        if !is_mobile_viewport {
            s.setup_sidebar();
        };

        s.compute_bounds();

        s
    }

    fn setup_sidebar(&mut self) {
        let tn_render_settings =
            RenderSettings { x_scale: 0.15, y_scale: 0.15, ..Default::default() };

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
                    texture: self.ctx.load_texture(
                        "pdf_thumbnail",
                        image,
                        egui::TextureOptions::LINEAR,
                    ),
                }
            })
            .collect();

        self.sidebar = Some(SideBar { thumbnails, is_visible: true, scroll_target: 0 });
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
        self.show_pages(ui);
    }

    fn handle_keys(&mut self, ui: &mut egui::Ui) {
        if ui.input_mut(|w| w.consume_key(Modifiers::NONE, Key::ArrowDown)) {
            ui.scroll_with_delta(Vec2 { x: 0., y: -20. });
        }

        if ui.input_mut(|w| w.consume_key(Modifiers::NONE, Key::ArrowUp)) {
            ui.scroll_with_delta(Vec2 { x: 0., y: 20. });
        }

        if (ui.input_mut(|w| w.consume_key(Modifiers::NONE, Key::PageDown))
            || ui.input_mut(|w| w.consume_key(Modifiers::NONE, Key::ArrowRight)))
            && self.current_page != self.page_bounds.len() - 1
        {
            self.scroll_to = Some(self.current_page + 1);
        }

        if (ui.input_mut(|w| w.consume_key(Modifiers::NONE, Key::PageUp))
            || ui.input_mut(|w| w.consume_key(Modifiers::NONE, Key::ArrowLeft)))
            && self.current_page != 0
        {
            self.scroll_to = Some(self.current_page - 1);
        }

        if ui.input_mut(|w| w.consume_key(Modifiers::NONE, Key::End)) {
            self.scroll_to = Some(self.page_bounds.len() - 1);
        }

        if ui.input_mut(|w| w.consume_key(Modifiers::NONE, Key::Home)) {
            self.scroll_to = Some(0);
        }

        let event = ui.input(|r| {
            for e in &r.events {
                if let Event::Zoom(f) = e {
                    return Some(Event::Zoom(*f));
                }
            }
            None
        });

        let pos = ui.input(|r| r.pointer.latest_pos());
        if let Some(Event::Zoom(f)) = event {
            self.fit_height = false;
            self.fit_width = false;
            self.scale_updated(self.scale * f, pos);
        }
    }

    fn show_toolbar(&mut self, ui: &mut egui::Ui) {
        let sidebar_is_visible = match &mut self.sidebar {
            Some(s) => s.is_visible,
            None => false,
        };

        let zoom_controls_width = 250.0;
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
            ui.columns(5, |cols| {
                cols[0].vertical_centered(|ui| {
                    if Button::default().icon(&Icon::ZOOM_OUT).show(ui).clicked() {
                        self.scale_updated(self.scale - ZOOM_STOP, None);
                        self.fit_height = false;
                        self.fit_width = false;
                    }
                });

                let zoom_percentage = (self.scale * 100.).round();

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
                        self.scale_updated(ZOOM_STOP + self.scale, None);
                        self.fit_height = false;
                        self.fit_width = false;
                    };
                });

                cols[3].vertical_centered(|ui| {
                    if Button::default()
                        .icon(&Icon::FIT_WIDTH)
                        .icon_color(if self.fit_width {
                            ui.visuals().text_color()
                        } else {
                            ui.visuals().text_color().linear_multiply(0.25)
                        })
                        .show(ui)
                        .clicked()
                    {
                        self.fit_width = !self.fit_width;
                        if self.fit_width {
                            self.fit_height = false;
                        }
                    };
                });

                cols[4].vertical_centered(|ui| {
                    if Button::default()
                        .icon(&Icon::FIT_HEIGHT)
                        .icon_color(if self.fit_height {
                            ui.visuals().text_color()
                        } else {
                            ui.visuals().text_color().linear_multiply(0.25)
                        })
                        .show(ui)
                        .clicked()
                    {
                        self.fit_height = !self.fit_height;
                        if self.fit_height {
                            self.fit_width = false;
                        }
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

        SidePanel::right("pdf_sidebar")
            .resizable(false)
            .show_separator_line(false)
            .show_animated_inside(ui, sidebar.is_visible, |ui| {
                ScrollArea::vertical().show(ui, |ui| {
                    egui::Frame::default()
                        .inner_margin(sidebar_margin)
                        .show(ui, |ui| {
                            for (i, p) in sidebar.thumbnails.clone().iter_mut().enumerate() {
                                let tint_color = if i == self.current_page {
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

                                if res.hovered() {
                                    ui.output_mut(|w| {
                                        w.cursor_icon = egui::CursorIcon::PointingHand
                                    })
                                }
                                if res.clicked() {
                                    self.scroll_to = Some(i);
                                }

                                if i == self.current_page && sidebar.scroll_target != i {
                                    ui.scroll_to_rect(res.rect, None);
                                    sidebar.scroll_target = i;
                                }

                                ui.add_space(sidebar_margin);
                            }
                        });
                })
            });
    }

    fn show_pages(&mut self, ui: &mut Ui) {
        let available_width = ui.available_width() * 0.95;
        let available_height = ui.available_height() * 0.95;
        self.render_area = ui.available_rect_before_wrap();

        CentralPanel::default().show_inside(ui, |ui| {
            ScrollArea::both()
                .animated(false)
                .show_viewport(ui, |ui, viewport| {
                    self.current_viewport = viewport;

                    let target_scale = if self.fit_width {
                        match self.pdf.pages().first().map(|p| p.render_dimensions().0) {
                            Some(width) => available_width / width,
                            None => 1.,
                        }
                    } else if self.fit_height {
                        match self.pdf.pages().first().map(|p| p.render_dimensions().1) {
                            Some(height) => available_height / height,
                            None => 1.,
                        }
                    } else {
                        self.scale
                    };

                    if target_scale != self.scale {
                        self.scale_updated(target_scale, None);
                    }

                    let max_height = self.page_bounds[self.page_bounds.len() - 1].max.y;
                    let max_width = self
                        .page_bounds
                        .iter()
                        .map(|r| r.width().ceil() as u32)
                        .max()
                        .unwrap_or_default() as f32;

                    let (_, rect) = ui.allocate_space(egui::Vec2 {
                        x: max_width.max(available_width),
                        y: max_height,
                    });

                    let mut intersect_areas = vec![];
                    let draw_adjustment = rect.min.to_vec2();

                    for idx in 0..self.page_bounds.len() {
                        let page_rect = self.page_bounds[idx];
                        let center_adjustment = Vec2 {
                            x: if page_rect.width() < available_width {
                                (available_width - page_rect.width()) / 2.
                            } else {
                                0.
                            },
                            y: 0.,
                        };
                        let page_rect = page_rect.translate(center_adjustment);

                        if page_rect.intersects(viewport) {
                            let paint_location = page_rect.translate(draw_adjustment);
                            let img = self.get_page(idx);
                            img.paint_at(ui, paint_location);

                            intersect_areas
                                .push((idx, page_rect.intersect(viewport).area() as u32));
                        }
                    }
                    self.handle_keys(ui);
                    if let Some(scroll_adj) = self.viewport_adjustment {
                        ui.scroll_with_delta(scroll_adj);
                        self.viewport_adjustment = None;

                        ui.ctx().request_repaint();
                    }
                    if let Some(scroll_idx) = self.scroll_to {
                        // this doesn't take into account `center_adjustment` from above
                        // but it doesn't matter, as if center_adjustment != 0, there is
                        // no horizontal scroll bar
                        ui.scroll_to_rect(
                            self.page_bounds[scroll_idx].translate(draw_adjustment),
                            Some(Align::TOP),
                        );
                        self.scroll_to = None;
                    }

                    let max_area = intersect_areas
                        .iter()
                        .map(|t| t.1)
                        .max()
                        .unwrap_or_default();
                    self.current_page = intersect_areas
                        .iter()
                        .filter(|t| t.1 == max_area)
                        .min_by_key(|t| t.0)
                        .map(|t| t.0)
                        .unwrap_or_default();
                });
        });
    }

    fn get_page(&mut self, idx: usize) -> Image<'_> {
        let texture = if idx != self.pdf.pages().len() {
            self.page_cache.get(&idx).cloned().unwrap_or_else(|| {
                let page = &self.pdf.pages()[idx];
                let pixmap = hayro::render(
                    page,
                    &InterpreterSettings::default(),
                    &RenderSettings {
                        x_scale: self.scale * self.ctx.pixels_per_point(),
                        y_scale: self.scale * self.ctx.pixels_per_point(),
                        ..Default::default()
                    },
                );
                let image = ColorImage::from_rgba_premultiplied(
                    [pixmap.width() as _, pixmap.height() as _],
                    pixmap.data_as_u8_slice(),
                );

                self.ctx
                    .load_texture("pdf_page", image, egui::TextureOptions::LINEAR)
            })
        } else {
            let image = ColorImage::from_rgba_premultiplied([1, 1], &[0, 0, 0, 0]);
            self.ctx
                .load_texture("pdf_page", image, egui::TextureOptions::LINEAR)
        };

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

        let mut safe_area = Vec2::new(500., 500.);
        safe_area *= self.scale;
        pages.push(Rect { min: offset, max: offset + safe_area });

        self.page_bounds = pages;
    }

    /// zoom from indicates the position of the cursor. If None it will zoom from the center
    fn scale_updated(&mut self, new_scale: f32, zoom_from: Option<Pos2>) {
        if self.viewport_adjustment.is_some() {
            return;
        }
        // location in the old viewport
        println!("zoom from {zoom_from:?}");
        let old_viewport_location = match zoom_from {
            Some(mouse) => {
                ((mouse - self.render_area.min) + self.current_viewport.min.to_vec2()).to_pos2()
            }
            None => self.current_viewport.center(),
        };
        println!("old_viewport {old_viewport_location:?}");

        // normalized point location in page space
        let normalized_page_space = old_viewport_location.to_vec2()
            / self.page_bounds.last().map(|r| r.max.to_vec2()).unwrap();
        println!("normalized_page space {normalized_page_space:?}");

        self.scale = new_scale;
        self.page_bounds.clear();
        self.page_cache.clear();
        self.compute_bounds();

        // calculate the new viewport
        let new_location =
            normalized_page_space * self.page_bounds.last().map(|r| r.max.to_vec2()).unwrap();
        println!("new location {new_location:?}");
        if !self.fit_width && !self.fit_height {
            self.viewport_adjustment = Some(-1. * (new_location - old_viewport_location.to_vec2()));
        }
        println!("adjustment {:?}", self.viewport_adjustment);
        self.ctx.request_repaint();
    }
}
