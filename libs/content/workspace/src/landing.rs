use egui::text::{LayoutJob, TextFormat};
use egui::{
    Button, Color32, ColorImage, FontId, Image, Label, Layout, Rect, RichText, Sense, Stroke,
    TextureHandle, Vec2,
};
use image::ImageReader;
use lb_rs::model::file::File;
use serde::{Deserialize, Serialize};
use std::io::Cursor;

use crate::file_cache::FilesExt;
use crate::show::{ElapsedHumanString as _, NEW_NOTE_SHORTCUT, SEARCH_SHORTCUT};
use crate::tab::markdown_editor::widget::link_completions::abbreviate_segments;
use crate::theme::palette_v2::ThemeExt as _;
use crate::workspace::Workspace;

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct LandingPage {
    #[serde(skip)]
    grayscale_logo_texture: Option<TextureHandle>,
    #[serde(skip)]
    button_row_width: f32,
}

impl PartialEq for LandingPage {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl Workspace {
    pub fn show_landing_page(&mut self, ui: &mut egui::Ui) {
        let theme = ui.ctx().get_lb_theme();
        let new_note_shortcut = ui.ctx().format_shortcut(&NEW_NOTE_SHORTCUT);
        let search_shortcut = ui.ctx().format_shortcut(&SEARCH_SHORTCUT);

        if !theme.dark() {
            ui.painter().rect_filled(
                ui.max_rect(),
                0.0,
                theme
                    .neutral_bg_secondary()
                    .lerp_to_gamma(theme.neutral(), 0.05),
            );
        }

        ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
            ui.add_space(120.0);
            self.show_landing_logo(ui);
            ui.add_space(30.0);
            self.show_landing_buttons(ui, &new_note_shortcut, &search_shortcut);

            ui.add_space(150.0);
            self.show_recent_files(ui);
        });
    }

    fn show_landing_buttons(
        &mut self, ui: &mut egui::Ui, new_note_shortcut: &str, search_shortcut: &str,
    ) {
        let row_width = self.landing_page.button_row_width.max(1.0);
        let center = ui.available_rect_before_wrap().center().x;
        let rect = Rect::from_min_size(
            egui::pos2(center - row_width / 2.0, ui.cursor().top()),
            Vec2::new(row_width, 0.0),
        );

        let inner = ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
            ui.horizontal(|ui| {
                if self
                    .show_landing_button(ui, "New Note", new_note_shortcut)
                    .clicked()
                {
                    self.create_doc(false);
                }
                ui.add_space(18.0);
                if self
                    .show_landing_button(ui, "Search", search_shortcut)
                    .clicked()
                {
                    self.upsert_search(None);
                }
            })
        });
        self.landing_page.button_row_width = inner.response.rect.width();
        ui.advance_cursor_after_rect(inner.response.rect);
    }

    fn show_landing_logo(&mut self, ui: &mut egui::Ui) {
        if let Some(texture) = self.grayscale_logo_texture(ui) {
            ui.add(
                Image::new(egui::ImageSource::Texture(egui::load::SizedTexture::new(
                    &texture,
                    Vec2::splat(112.0),
                )))
                .fit_to_exact_size(Vec2::splat(112.0)),
            );
        } else {
            ui.allocate_space(Vec2::splat(112.0));
        }
    }

    fn grayscale_logo_texture(&mut self, ui: &mut egui::Ui) -> Option<TextureHandle> {
        const LOGO_BYTES: &[u8] = include_bytes!("../logo.png");
        if let Some(texture) = &self.landing_page.grayscale_logo_texture {
            return Some(texture.clone());
        }

        let image = ImageReader::new(Cursor::new(LOGO_BYTES))
            .with_guessed_format()
            .ok()?
            .decode()
            .ok()?
            .grayscale();
        let size = [image.width() as usize, image.height() as usize];
        let mut pixels = image.to_rgba8();
        for pixel in pixels.pixels_mut() {
            pixel.0[3] = ((pixel.0[3] as f32) * 0.1).round() as u8; // lower the opacity
        }
        let color_image = ColorImage::from_rgba_unmultiplied(size, &pixels);
        let texture = ui.ctx().load_texture(
            "landing_grayscale_logo",
            color_image,
            egui::TextureOptions::LINEAR,
        );
        self.landing_page.grayscale_logo_texture = Some(texture.clone());
        Some(texture)
    }

    fn show_landing_button(
        &self, ui: &mut egui::Ui, title: &str, shortcut: &str,
    ) -> egui::Response {
        let theme = ui.ctx().get_lb_theme();
        let font_id = FontId::proportional(16.0);
        let primary = theme.fg().get_color(theme.prefs().primary);
        let shortcut_color = primary.lerp_to_gamma(theme.neutral_bg(), 0.5);
        let mut text = LayoutJob::default();
        text.append(
            &format!("{} ", title),
            0.0,
            TextFormat { font_id: font_id.clone(), color: primary, ..Default::default() },
        );

        text.append(
            shortcut,
            0.0,
            TextFormat { font_id, color: shortcut_color, ..Default::default() },
        );

        ui.add(Button::new(text).frame(false))
    }

    fn show_recent_files(&mut self, ui: &mut egui::Ui) {
        let theme = ui.ctx().get_lb_theme();
        let recent = self.recent_files();
        let width = ui.available_width().min(500.0);
        let row_h = 60.0;
        let visible = recent.len().min(5);
        let card_h = row_h * visible as f32;
        let height = 21.0 + 14.0 + if recent.is_empty() { 24.0 } else { card_h };

        ui.allocate_ui_with_layout(
            Vec2::new(width, height),
            Layout::top_down(egui::Align::Min),
            |ui| {
                let header = ui.add(Label::new(
                    RichText::new("Pick up where you left")
                        .font(FontId::proportional(16.0))
                        .color(theme.neutral_fg_secondary().gamma_multiply(0.75)),
                ));
                let line_y = header.rect.bottom() + 4.0;
                ui.painter().line_segment(
                    [
                        egui::pos2(ui.min_rect().left(), line_y),
                        egui::pos2(ui.min_rect().left() + width, line_y),
                    ],
                    Stroke::new(1.0, theme.neutral().gamma_multiply(0.28)),
                );

                ui.add_space(14.0);

                if recent.is_empty() {
                    ui.label(
                        RichText::new("No recent notes yet")
                            .font(FontId::proportional(16.0))
                            .color(theme.neutral_fg_secondary()),
                    );
                    return;
                }

                let (card_rect, _) =
                    ui.allocate_exact_size(Vec2::new(width, card_h), Sense::hover());
                let card_fill = if theme.dark() {
                    theme.neutral_bg_secondary().gamma_multiply(0.3)
                } else {
                    theme.neutral_bg().lerp_to_gamma(theme.neutral(), 0.05)
                };
                let sperator_color = if theme.dark() {
                    theme.neutral_bg().gamma_multiply(0.82)
                } else {
                    theme
                        .neutral_bg_secondary()
                        .lerp_to_gamma(theme.neutral(), 0.05)
                };
                let separator = Stroke::new(3.0, sperator_color);
                ui.painter().rect_filled(card_rect, 12.0, card_fill);

                for (idx, file) in recent.iter().take(visible).enumerate() {
                    let row_rect = Rect::from_min_size(
                        egui::pos2(card_rect.left(), card_rect.top() + idx as f32 * row_h),
                        Vec2::new(width, row_h),
                    );
                    let row_resp = ui.interact(
                        row_rect,
                        egui::Id::new("landing_recent").with(file.id),
                        Sense::click(),
                    );
                    if row_resp.hovered() && theme.dark() {
                        ui.painter().rect_filled(
                            row_rect.shrink2(Vec2::new(2.0, 1.0)),
                            if idx == 0 || idx + 1 == visible { 10.0 } else { 0.0 },
                            theme.neutral().gamma_multiply(0.18),
                        );
                    }
                    if row_resp.clicked() {
                        self.open_file(file.id, true, false);
                    }

                    if idx > 0 {
                        ui.painter().line_segment(
                            [
                                egui::pos2(card_rect.left(), row_rect.top()),
                                egui::pos2(card_rect.right(), row_rect.top()),
                            ],
                            separator,
                        );
                    }

                    ui.painter().text(
                        egui::pos2(row_rect.left() + 28.0, row_rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        &file.name,
                        FontId::proportional(16.0),
                        theme.neutral_fg(),
                    );

                    let detail_target = if row_resp.hovered() { 1.0 } else { 0.0 };
                    let detail_opacity = ui.ctx().animate_value_with_time(
                        egui::Id::new("landing_recent_detail").with(file.id),
                        detail_target,
                        0.1,
                    );
                    if detail_opacity > 0.0 {
                        let (edited, mut path_segments) = self.recent_detail(file);
                        let detail_color = with_alpha(
                            theme.neutral_fg_secondary().gamma_multiply(0.92),
                            detail_opacity,
                        );
                        let path_font = FontId::proportional(14.5);
                        let measure_path = |text: &str| -> f32 {
                            ui.fonts(|f| {
                                f.layout_no_wrap(text.to_owned(), path_font.clone(), detail_color)
                                    .size()
                                    .x
                            })
                        };
                        abbreviate_segments(
                            &mut path_segments,
                            row_rect.width() * 0.46,
                            &measure_path,
                        );
                        let path: String = path_segments
                            .iter()
                            .map(|(text, _)| text.as_str())
                            .collect();
                        ui.painter().text(
                            egui::pos2(row_rect.right() - 28.0, row_rect.center().y - 12.0),
                            egui::Align2::RIGHT_CENTER,
                            edited,
                            FontId::proportional(15.5),
                            detail_color,
                        );
                        ui.painter().text(
                            egui::pos2(row_rect.right() - 28.0, row_rect.center().y + 13.0),
                            egui::Align2::RIGHT_CENTER,
                            path,
                            path_font,
                            detail_color,
                        );
                    }
                }
            },
        );
    }

    fn recent_files(&self) -> Vec<File> {
        let files_guard = self.files.read().unwrap();
        let files = &*files_guard;
        let root = files.root().id;
        let mut recent: Vec<File> = files
            .descendents(root)
            .into_iter()
            .chain(files.shared.values())
            .filter(|file| file.is_document())
            .cloned()
            .collect();
        recent.sort_by_key(|file| u64::MAX - files.last_modified_recursive(file.id));
        recent
    }

    fn recent_detail(&self, file: &File) -> (String, Vec<(String, bool)>) {
        let files_guard = self.files.read().unwrap();
        let files = &*files_guard;
        let mut parent_segments = files.path_segments(file.id);
        parent_segments.pop();
        if parent_segments.last().is_some_and(|(text, _)| text == "/") {
            parent_segments.pop();
        }
        let edited_by = files.last_modified_by_recursive(file.id);
        (
            format!(
                "{} • {}",
                edited_by,
                files
                    .last_modified_recursive(file.id)
                    .elapsed_human_string()
            ),
            parent_segments,
        )
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
