use resvg::usvg::Transform;

use crate::{
    tab::svg_editor::{BackgroundOverlay, SVGEditor},
    theme::palette::ThemePalette,
};

impl SVGEditor {
    pub fn paint_background_colors(&mut self, ui: &mut egui::Ui) {
        ui.painter().rect_filled(
            self.viewport_settings.container_rect,
            0.0,
            ui.visuals().code_bg_color,
        );
        ui.painter().rect_filled(
            self.viewport_settings.working_rect,
            0.0,
            ThemePalette::resolve_dynamic_color(
                self.settings.background_color,
                ui.visuals().dark_mode,
            ),
        );
    }

    pub fn show_background_overlay(&self) {
        match self.settings.background_type {
            BackgroundOverlay::Dots => {
                show_dot_grid(
                    self.viewport_settings.working_rect,
                    self.viewport_settings.master_transform,
                    &self.painter,
                    None,
                );
            }
            BackgroundOverlay::Lines => {
                show_lines_background(
                    false,
                    self.viewport_settings.working_rect,
                    self.viewport_settings.master_transform,
                    &self.painter,
                    None,
                );
            }
            BackgroundOverlay::Grid => {
                show_lines_background(
                    true,
                    self.viewport_settings.working_rect,
                    self.viewport_settings.master_transform,
                    &self.painter,
                    None,
                );
            }
            BackgroundOverlay::Blank => {}
        }
    }
}

pub fn show_lines_background(
    is_grid: bool, container_rect: egui::Rect, transform: Transform, painter: &egui::Painter,
    maybe_stroke_width: Option<f32>,
) {
    let mut dot_radius = maybe_stroke_width.unwrap_or((1. * transform.sx).max(0.6) / 2.0);
    let (vertical_line_padding, offset, end) =
        calc_grid_info(&mut dot_radius, container_rect, transform);

    let stroke = egui::Stroke { width: dot_radius, color: egui::Color32::GRAY.gamma_multiply(0.4) };

    for i in 0..=(end.y.ceil() as i32) {
        painter.line_segment(
            [
                egui::pos2(container_rect.left(), i as f32 * vertical_line_padding + offset.y),
                egui::pos2(container_rect.right(), i as f32 * vertical_line_padding + offset.y),
            ],
            stroke,
        );
    }

    for j in 0..=(end.x.ceil() as i32) {
        if !is_grid {
            break;
        }
        painter.line_segment(
            [
                egui::pos2(j as f32 * vertical_line_padding + offset.x, container_rect.top()),
                egui::pos2(j as f32 * vertical_line_padding + offset.x, container_rect.bottom()),
            ],
            stroke,
        );
    }
}

pub fn show_dot_grid(
    container_rect: egui::Rect, transform: Transform, painter: &egui::Painter,
    maybe_dot_radius: Option<f32>,
) {
    let mut dot_radius = maybe_dot_radius.unwrap_or((1. * transform.sx).max(0.6));
    let (distance_between_dots, offset, end) =
        calc_grid_info(&mut dot_radius, container_rect, transform);

    let mut dot = egui::Pos2::ZERO;
    for i in 0..=(end.y.ceil() as i32) {
        dot.x = 0.0;
        for j in 0..=(end.x.ceil() as i32) {
            let dot = egui::pos2(
                j as f32 * distance_between_dots + offset.x,
                i as f32 * distance_between_dots + offset.y,
            );

            painter.circle(
                dot,
                dot_radius,
                egui::Color32::GRAY.gamma_multiply(0.4),
                egui::Stroke::NONE,
            );
        }
    }
}

fn calc_grid_info(
    dot_radius: &mut f32, container_rect: egui::Rect, transform: Transform,
) -> (f32, egui::Vec2, egui::Vec2) {
    let mut distance_between_dots = 30.0 * transform.sx;
    if distance_between_dots < 7.0 {
        distance_between_dots *= 5.0;
        *dot_radius *= 1.5;
    } else if distance_between_dots < 12.0 {
        distance_between_dots *= 2.0;
        *dot_radius *= 1.5;
    }

    let offset = egui::vec2(
        transform.tx.rem_euclid(distance_between_dots),
        transform.ty.rem_euclid(distance_between_dots),
    );

    let end = egui::vec2(
        (container_rect.right() + distance_between_dots) / distance_between_dots,
        (container_rect.bottom() + distance_between_dots) / distance_between_dots,
    );
    (distance_between_dots, offset, end)
}
