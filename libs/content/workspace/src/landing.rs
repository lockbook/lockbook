use egui::text::{LayoutJob, TextFormat};
use egui::{
    Button, Color32, ColorImage, FontId, Image, Label, Layout, Rect, RichText, Sense, Stroke,
    TextureHandle, Vec2,
};
use image::ImageReader;
use lb_rs::model::file::File;
use serde::{Deserialize, Serialize};
use std::{io::Cursor, time::Duration};

use crate::GlyphonRendererCallback;
use crate::file_cache::{FileCache, FilesExt};
use crate::show::{ElapsedHumanString as _, NEW_NOTE_SHORTCUT, SEARCH_SHORTCUT};
use crate::tab::markdown_editor::widget::link_completions::abbreviate_segments;
use crate::theme::palette_v2::ThemeExt as _;
use crate::widgets::{GlyphonLabel, TextOverflow};
use crate::workspace::Workspace;

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct LandingPage {
    #[serde(skip)]
    grayscale_logo_texture: Option<(bool, TextureHandle)>,
    #[serde(skip)]
    button_row_width: f32,
    #[serde(skip)]
    recent_files: Vec<RecentFile>,
}

#[derive(Clone)]
struct RecentFile {
    file: File,
    edited: String,
    path_segments: Vec<(String, bool)>,
}

struct RecentRowLayout {
    rect: Rect,
    idx: usize,
    visible: usize,
    separator: Stroke,
    section_opacity: f32,
}

impl PartialEq for LandingPage {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

const LANDING_LOGO_SIZE: f32 = 112.0;
const LANDING_PRIMARY_FONT_SIZE: f32 = 16.0;
const RECENTS_ROW_HEIGHT: f32 = 60.0;
const RECENTS_ROW_HORIZONTAL_PADDING: f32 = 18.0;

impl LandingPage {
    pub fn update_recent_files(&mut self, files: &FileCache) {
        let mut recent: Vec<_> = files
            .iter_files()
            .filter(|file| file.is_document())
            .map(|file| {
                let mut parent_segments = files.path_segments(file.id);
                let is_user_rooted = parent_segments.first().is_some_and(|(text, _)| text == "/");
                parent_segments.pop();
                if parent_segments.last().is_some_and(|(text, _)| text == "/") {
                    parent_segments.pop();
                }
                if parent_segments.is_empty() && is_user_rooted {
                    parent_segments.push(("/".to_string(), false));
                }

                RecentFile {
                    file: file.clone(),
                    edited: format!(
                        "{} • {}",
                        files.last_modified_by_recursive(file.id),
                        files
                            .last_modified_recursive(file.id)
                            .elapsed_human_string()
                    ),
                    path_segments: parent_segments,
                }
            })
            .collect();

        recent.sort_by_key(|recent| u64::MAX - files.last_modified_recursive(recent.file.id));
        self.recent_files = recent;
    }
}

impl Workspace {
    pub fn show_landing_page(&mut self, ui: &mut egui::Ui) {
        let theme = ui.ctx().get_lb_theme();
        let light_background_blend = 0.05;
        let viewport_height = ui.max_rect().height();
        let top_padding = viewport_height * 0.21;
        let logo_button_gap = viewport_height * 0.03;
        let button_recents_gap = viewport_height * 0.08;
        let new_note_shortcut = ui.ctx().format_shortcut(&NEW_NOTE_SHORTCUT);
        let search_shortcut = ui.ctx().format_shortcut(&SEARCH_SHORTCUT);

        if !theme.dark() {
            ui.painter().rect_filled(
                ui.max_rect(),
                0.0,
                theme
                    .neutral_bg_secondary()
                    .lerp_to_gamma(theme.neutral(), light_background_blend),
            );
        }

        ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
            ui.add_space(top_padding);
            self.show_landing_logo(ui);
            ui.add_space(logo_button_gap);
            self.show_landing_buttons(ui, &new_note_shortcut, &search_shortcut);

            ui.add_space(button_recents_gap);
            self.show_recent_files(ui);
        });
    }

    fn show_landing_buttons(
        &mut self, ui: &mut egui::Ui, new_note_shortcut: &str, search_shortcut: &str,
    ) {
        let stack_width_breakpoint = 300.0;
        let available_width = ui.available_width();
        let stack_buttons = available_width < stack_width_breakpoint;
        let button_gap = if stack_buttons { 9.0 } else { 12.0 };
        let button_group_width = if stack_buttons {
            available_width
        } else {
            self.landing_page.button_row_width.max(1.0)
        };
        let center = ui.available_rect_before_wrap().center().x;
        let rect = Rect::from_min_size(
            egui::pos2(center - button_group_width / 2.0, ui.cursor().top()),
            Vec2::new(button_group_width, 0.0),
        );

        let inner = ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
            if stack_buttons {
                ui.vertical_centered(|ui| {
                    if self
                        .show_landing_mobile_button(ui, "New Note", new_note_shortcut)
                        .clicked()
                    {
                        self.create_doc(false);
                    }
                    ui.add_space(button_gap);
                    if self
                        .show_landing_mobile_button(ui, "Search", search_shortcut)
                        .clicked()
                    {
                        self.upsert_search(None);
                    }
                })
            } else {
                ui.horizontal(|ui| {
                    if self
                        .show_landing_desktop_button(ui, "New Note", new_note_shortcut)
                        .clicked()
                    {
                        self.create_doc(false);
                    }
                    ui.add_space(button_gap);
                    if self
                        .show_landing_desktop_button(ui, "Search", search_shortcut)
                        .clicked()
                    {
                        self.upsert_search(None);
                    }
                })
            }
        });
        if !stack_buttons {
            self.landing_page.button_row_width = inner.response.rect.width();
        }
        ui.advance_cursor_after_rect(inner.response.rect);
    }

    fn show_landing_mobile_button(
        &self, ui: &mut egui::Ui, title: &str, _shortcut: &str,
    ) -> egui::Response {
        let theme = ui.ctx().get_lb_theme();
        let font_id = FontId::proportional(LANDING_PRIMARY_FONT_SIZE);
        let primary = theme.fg().get_color(theme.prefs().primary);
        let mut text = LayoutJob::default();
        text.append(title, 0.0, TextFormat { font_id, color: primary, ..Default::default() });

        ui.add(Button::new(text).frame(false))
    }

    fn show_landing_desktop_button(
        &self, ui: &mut egui::Ui, title: &str, shortcut: &str,
    ) -> egui::Response {
        let theme = ui.ctx().get_lb_theme();
        let font_id = FontId::proportional(LANDING_PRIMARY_FONT_SIZE);
        let vertical_padding = 8.0;
        let horizontal_padding = 14.0;
        let corner_radius = 8.0;
        let fill = theme.neutral_bg_tertiary();
        let title_color = theme.neutral_fg();
        let shortcut_color = title_color.lerp_to_gamma(fill, 0.45);

        let mut text = LayoutJob::default();
        text.append(
            &format!("{} ", title),
            0.0,
            TextFormat { font_id: font_id.clone(), color: title_color, ..Default::default() },
        );
        text.append(
            shortcut,
            0.0,
            TextFormat { font_id, color: shortcut_color, ..Default::default() },
        );
        let galley = ui.painter().layout_job(text);
        let desired_size = Vec2::new(
            galley.size().x + horizontal_padding * 2.0,
            galley.size().y + vertical_padding,
        );
        let (rect, response) = ui.allocate_exact_size(desired_size, Sense::click());

        if ui.is_rect_visible(rect) {
            ui.painter().rect_filled(rect, corner_radius, fill);
            ui.painter().galley(
                egui::pos2(
                    rect.center().x - galley.size().x / 2.0,
                    rect.center().y - galley.size().y / 2.0,
                ),
                galley,
                title_color,
            );
        }

        response
    }

    fn show_landing_logo(&mut self, ui: &mut egui::Ui) {
        if let Some(texture) = self.grayscale_logo_texture(ui) {
            ui.add(
                Image::new(egui::ImageSource::Texture(egui::load::SizedTexture::new(
                    &texture,
                    Vec2::splat(LANDING_LOGO_SIZE),
                )))
                .fit_to_exact_size(Vec2::splat(LANDING_LOGO_SIZE)),
            );
        } else {
            ui.allocate_space(Vec2::splat(LANDING_LOGO_SIZE));
        }
    }

    fn grayscale_logo_texture(&mut self, ui: &mut egui::Ui) -> Option<TextureHandle> {
        let dark = ui.ctx().get_lb_theme().dark();
        if let Some((texture_dark, texture)) = &self.landing_page.grayscale_logo_texture {
            if *texture_dark == dark {
                return Some(texture.clone());
            }
        }

        let image = ImageReader::new(Cursor::new(include_bytes!("../logo.png")))
            .with_guessed_format()
            .ok()?
            .decode()
            .ok()?
            .grayscale();
        let size = [image.width() as usize, image.height() as usize];
        let mut pixels = image.to_rgba8();
        let opacity = if dark { 0.3 } else { 0.2 };
        for pixel in pixels.pixels_mut() {
            pixel.0[3] = ((pixel.0[3] as f32) * opacity).round() as u8;
        }
        let color_image = ColorImage::from_rgba_unmultiplied(size, &pixels);
        let texture = ui.ctx().load_texture(
            if dark { "landing_grayscale_logo_dark" } else { "landing_grayscale_logo_light" },
            color_image,
            egui::TextureOptions::LINEAR,
        );
        self.landing_page.grayscale_logo_texture = Some((dark, texture.clone()));
        Some(texture)
    }

    fn show_recent_files(&mut self, ui: &mut egui::Ui) {
        let visibility = ui.ctx().animate_value_with_time(
            egui::Id::new("landing_recent_files_visible"),
            if self.sidebar_open { 0.0 } else { 1.0 },
            0.2,
        );
        if visibility <= 0.0 {
            return;
        }

        let theme = ui.ctx().get_lb_theme();
        let max_width = 500.0;
        let mobile_gutter = 32.0;
        let visible_limit = 5;
        let header_height = 21.0;
        let header_line_width = 1.0;
        let header_line_gap = 4.0;
        let header_card_gap = 14.0;
        let empty_height = 24.0;
        let header_font_size = 16.0;
        let header_text_gamma = 0.75;
        let header_line_gamma = 0.28;
        let card_corner_radius = 12.0;
        let light_card_blend = 0.05;
        let light_separator_blend = 0.05;
        let dark_card_gamma = 0.3;
        let dark_separator_gamma = 0.82;
        let separator_width = 3.0;
        let recent = self.landing_page.recent_files.clone();
        let width = (ui.available_width() - mobile_gutter)
            .min(max_width)
            .max(0.0);
        let visible = recent.len().min(visible_limit);
        let card_h = RECENTS_ROW_HEIGHT * visible as f32;
        let height =
            header_height + header_card_gap + if recent.is_empty() { empty_height } else { card_h };
        let section_rect = Rect::from_min_size(
            egui::pos2(ui.available_rect_before_wrap().center().x - width / 2.0, ui.cursor().top()),
            Vec2::new(width, height),
        );

        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(section_rect)
                .layout(Layout::top_down(egui::Align::Min)),
            |ui| {
                if self.sidebar_open {
                    ui.disable();
                }
                ui.set_opacity(visibility);
                let header = ui.add(Label::new(
                    RichText::new("Pick up where you left")
                        .font(FontId::proportional(header_font_size))
                        .color(
                            theme
                                .neutral_fg_secondary()
                                .gamma_multiply(header_text_gamma),
                        ),
                ));
                let line_y = header.rect.bottom() + header_line_gap;
                ui.painter().line_segment(
                    [
                        egui::pos2(ui.min_rect().left(), line_y),
                        egui::pos2(ui.min_rect().left() + width, line_y),
                    ],
                    Stroke::new(
                        header_line_width,
                        theme.neutral().gamma_multiply(header_line_gamma),
                    ),
                );

                ui.add_space(header_card_gap);

                if recent.is_empty() {
                    ui.label(
                        RichText::new("No recent notes yet")
                            .font(FontId::proportional(header_font_size))
                            .color(theme.neutral_fg_secondary()),
                    );
                    return;
                }

                let card_fill = if theme.dark() {
                    theme.neutral_bg_secondary().gamma_multiply(dark_card_gamma)
                } else {
                    theme
                        .neutral_bg()
                        .lerp_to_gamma(theme.neutral(), light_card_blend)
                };
                let separator_color = if theme.dark() {
                    theme.neutral_bg().gamma_multiply(dark_separator_gamma)
                } else {
                    theme
                        .neutral_bg_secondary()
                        .lerp_to_gamma(theme.neutral(), light_separator_blend)
                };
                let separator = Stroke::new(separator_width, separator_color);
                let (card_rect, _) =
                    ui.allocate_exact_size(Vec2::new(width, card_h), Sense::hover());
                ui.painter()
                    .rect_filled(card_rect, card_corner_radius, card_fill);

                for (idx, file) in recent.iter().take(visible).enumerate() {
                    let row_rect = Rect::from_min_size(
                        egui::pos2(
                            card_rect.left(),
                            card_rect.top() + idx as f32 * RECENTS_ROW_HEIGHT,
                        ),
                        Vec2::new(width, RECENTS_ROW_HEIGHT),
                    );
                    self.show_recent_file_row(
                        ui,
                        file,
                        RecentRowLayout {
                            rect: row_rect,
                            idx,
                            visible,
                            separator,
                            section_opacity: visibility,
                        },
                    );
                }
            },
        );
        ui.advance_cursor_after_rect(section_rect);
    }

    fn show_recent_file_row(
        &mut self, ui: &mut egui::Ui, recent: &RecentFile, layout: RecentRowLayout,
    ) {
        let theme = ui.ctx().get_lb_theme();
        let file = &recent.file;
        let row_rect = layout.rect;
        let idx = layout.idx;
        let visible = layout.visible;
        let separator = layout.separator;
        let section_opacity = layout.section_opacity;
        let row_edge_radius = 10.0;
        let row_hover_vertical_inset = 1.0;
        let detail_gap = 8.0;
        let detail_font_size = 14.5;
        let detail_text_y_offset = 13.0;
        let detail_hover_delay_seconds = 0.03;
        let detail_animation_seconds = 0.2;
        let dark_hover_gamma = 0.18;
        let detail_text_gamma = 0.92;
        let row_resp =
            ui.interact(row_rect, egui::Id::new("landing_recent").with(file.id), Sense::click());

        if row_resp.hovered() && theme.dark() {
            ui.painter().rect_filled(
                row_rect.shrink2(Vec2::new(0.0, row_hover_vertical_inset)),
                if idx == 0 || idx + 1 == visible { row_edge_radius } else { 0.0 },
                theme.neutral().gamma_multiply(dark_hover_gamma),
            );
        }
        if row_resp.clicked() {
            self.open_file(file.id, true, false);
        }

        if idx > 0 {
            ui.painter().line_segment(
                [
                    egui::pos2(row_rect.left(), row_rect.top()),
                    egui::pos2(row_rect.right(), row_rect.top()),
                ],
                separator,
            );
        }

        let now = ui.input(|i| i.time);
        let hover_started_at_id = egui::Id::new("landing_recent_hover_started_at").with(file.id);
        let hover_started_at = if row_resp.hovered() {
            ui.data_mut(|data| {
                let hover_started_at = data.get_temp::<f64>(hover_started_at_id).unwrap_or(now);
                data.insert_temp(hover_started_at_id, hover_started_at);
                hover_started_at
            })
        } else {
            ui.data_mut(|data| data.remove_temp::<f64>(hover_started_at_id));
            f64::INFINITY
        };
        let hover_duration = now - hover_started_at;
        let detail_visible = row_resp.hovered() && hover_duration >= detail_hover_delay_seconds;
        if row_resp.hovered() && !detail_visible {
            ui.ctx().request_repaint_after(Duration::from_secs_f64(
                (detail_hover_delay_seconds - hover_duration).max(0.0),
            ));
        }

        let detail_opacity = ui.ctx().animate_value_with_time(
            egui::Id::new("landing_recent_detail").with(file.id),
            if detail_visible { 1.0 } else { 0.0 },
            detail_animation_seconds,
        );
        let max_detail_width =
            (row_rect.width() - RECENTS_ROW_HORIZONTAL_PADDING * 2.0 - detail_gap).max(0.0);
        let detail_width_id = egui::Id::new("landing_recent_detail_width").with(file.id);
        let mut detail = None;
        let detail_width = if detail_opacity > 0.0 {
            let edited = recent.edited.clone();
            let mut path_segments = recent.path_segments.clone();
            let detail_color = with_alpha(
                theme
                    .neutral_fg_secondary()
                    .gamma_multiply(detail_text_gamma),
                detail_opacity,
            );
            let detail_font = FontId::proportional(detail_font_size);
            let measure_detail = |text: &str| -> f32 {
                ui.fonts(|f| {
                    f.layout_no_wrap(text.to_owned(), detail_font.clone(), detail_color)
                        .size()
                        .x
                })
            };
            abbreviate_segments(&mut path_segments, max_detail_width, &measure_detail);
            let path: String = path_segments
                .iter()
                .map(|(text, _)| text.as_str())
                .collect();
            let detail_width = measure_detail(&edited).max(measure_detail(&path));
            ui.data_mut(|data| data.insert_temp(detail_width_id, detail_width));
            detail = Some((edited, path, detail_color, detail_font));
            detail_width
        } else {
            ui.data_mut(|data| data.get_temp::<f32>(detail_width_id))
                .unwrap_or(0.0)
        };
        let detail_width = detail_width.min(max_detail_width);
        let detail_rect = Rect::from_min_max(
            egui::pos2(
                row_rect.right() - RECENTS_ROW_HORIZONTAL_PADDING - detail_width,
                row_rect.top(),
            ),
            egui::pos2(row_rect.right() - RECENTS_ROW_HORIZONTAL_PADDING, row_rect.bottom()),
        );
        let title_clip_right = if detail_opacity > 0.0 {
            detail_rect.left() - detail_gap
        } else {
            row_rect.right() - RECENTS_ROW_HORIZONTAL_PADDING
        };
        let title_rect_left = row_rect.left() + RECENTS_ROW_HORIZONTAL_PADDING;
        let title_rect = Rect::from_min_max(
            egui::pos2(title_rect_left, row_rect.top()),
            egui::pos2(title_clip_right.max(title_rect_left), row_rect.bottom()),
        );
        let title_clip = title_rect.intersect(ui.clip_rect());
        if title_clip.width() > 0.0 && title_clip.height() > 0.0 {
            let shaped =
                GlyphonLabel::new(&file.name, with_alpha(theme.neutral_fg(), section_opacity))
                    .font_size(LANDING_PRIMARY_FONT_SIZE)
                    .line_height(RECENTS_ROW_HEIGHT)
                    .max_width(title_rect.width())
                    .text_overflow(TextOverflow::EndEllipsis)
                    .build(ui.ctx());
            let text_area = shaped.text_area(title_rect, ui.ctx(), title_clip);
            ui.painter()
                .add(egui_wgpu_renderer::egui_wgpu::Callback::new_paint_callback(
                    title_clip,
                    GlyphonRendererCallback::new(vec![text_area]),
                ));
        }

        if let Some((edited, path, detail_color, detail_font)) = detail {
            ui.painter().text(
                egui::pos2(detail_rect.right(), row_rect.center().y - detail_text_y_offset),
                egui::Align2::RIGHT_CENTER,
                edited,
                detail_font.clone(),
                detail_color,
            );
            ui.painter().text(
                egui::pos2(detail_rect.right(), row_rect.center().y + detail_text_y_offset),
                egui::Align2::RIGHT_CENTER,
                path,
                detail_font,
                detail_color,
            );
        }
    }
}

fn with_alpha(color: Color32, opacity: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(
        color.r(),
        color.g(),
        color.b(),
        ((color.a() as f32) * opacity.clamp(0.0, 1.0)).round() as u8,
    )
}
