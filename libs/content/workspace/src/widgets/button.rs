use egui::{CursorIcon, Rounding, TextStyle, TextWrapMode, WidgetText};

use crate::theme::icons::Icon;

#[derive(Default)]
pub struct Button<'a> {
    icon: Option<&'a Icon>,
    text: Option<WidgetText>,
    text_style: Option<egui::TextStyle>,
    icon_color: Option<egui::Color32>,
    icon_alignment: Option<egui::Align>,
    padding: Option<egui::Vec2>,
    is_loading: bool,
    rounding: egui::Rounding,
    stroke: egui::Stroke,
    frame: bool,
    hexpand: bool,
    margin: egui::Vec2,
    indent: f32,
    default_fill: Option<egui::Color32>,
}
const SPINNER_RADIUS: u32 = 6;

impl<'a> Button<'a> {
    pub fn icon(self, icon: &'a Icon) -> Self {
        Self { icon: Some(icon), ..self }
    }

    pub fn icon_alignment(self, align: egui::Align) -> Self {
        let alignment = match align {
            egui::Align::Center | egui::Align::Min => egui::Align::LEFT,
            egui::Align::Max => egui::Align::RIGHT,
        };
        Self { icon_alignment: Some(alignment), ..self }
    }

    pub fn text(self, text: impl Into<WidgetText>) -> Self {
        Self { text: Some(text.into()), ..self }
    }

    pub fn text_style(self, text_style: TextStyle) -> Self {
        Self { text_style: Some(text_style), ..self }
    }

    pub fn icon_color(self, icon_color: egui::Color32) -> Self {
        Self { icon_color: Some(icon_color), ..self }
    }

    pub fn padding(self, padding: impl Into<egui::Vec2>) -> Self {
        Self { padding: Some(padding.into()), ..self }
    }

    pub fn rounding(self, rounding: impl Into<Rounding>) -> Self {
        Self { rounding: rounding.into(), ..self }
    }

    pub fn is_loading(self, is_loading: bool) -> Self {
        Self { is_loading, ..self }
    }

    pub fn frame(self, frame: bool) -> Self {
        Self { frame, ..self }
    }

    pub fn hexpand(self, hexpand: bool) -> Self {
        Self { hexpand, ..self }
    }

    pub fn margin(self, margin: egui::Vec2) -> Self {
        Self { margin, ..self }
    }

    pub fn indent(self, indent: f32) -> Self {
        Self { indent, ..self }
    }

    pub fn default_fill(self, default_fill: egui::Color32) -> Self {
        Self { default_fill: Some(default_fill), ..self }
    }

    pub fn show(self, ui: &mut egui::Ui) -> egui::Response {
        egui::Frame::none()
            .outer_margin(self.margin)
            .show(ui, |ui| {
                let text_style = self.text_style.unwrap_or(egui::TextStyle::Body);
                let padding = self.padding.unwrap_or_else(|| ui.spacing().button_padding);
                let text_height = ui.text_style_height(&text_style);
                let wrap_width = ui.available_width();

                let mut width = padding.x * 2.0;

                let icon_text_style = text_style.clone();
                let maybe_icon_galley = self.icon.map(|icon| {
                    let icon: egui::WidgetText = icon.into();
                    let galley = icon.into_galley(
                        ui,
                        Some(TextWrapMode::Extend),
                        wrap_width,
                        icon_text_style,
                    );
                    width += galley.size().x;
                    if self.text.is_some() {
                        width += padding.x / 2.;
                    }
                    galley
                });

                let maybe_text_galley = self.text.map(|text| {
                    let galley =
                        text.into_galley(ui, Some(TextWrapMode::Extend), wrap_width, text_style);
                    width += galley.size().x;
                    galley
                });

                if self.hexpand {
                    width = ui.available_size_before_wrap().x;
                }

                let desired_size = egui::vec2(width, text_height + padding.y * 2.0);

                let (rect, resp) =
                    ui.allocate_at_least(desired_size, egui::Sense::click_and_drag());

                if ui.is_rect_visible(rect) {
                    let text_visuals = ui.style().interact(&resp).to_owned();
                    let icon_color = self.icon_color.unwrap_or(text_visuals.text_color());

                    // In stark contrast to its documentation, resp.hovered() is often true even if something is being dragged
                    // and even when it does not contain the pointer! Some suffering occurred here to get desirable behavior.
                    let bg_fill = if resp.hovered()
                        && ui.input(|i| {
                            i.pointer
                                .interact_pos()
                                .map(|p| resp.rect.contains(p))
                                .unwrap_or_default()
                        })
                        && !resp.dragged()
                    {
                        ui.visuals().widgets.hovered.bg_fill
                    } else {
                        self.default_fill.unwrap_or(text_visuals.bg_fill)
                    };

                    ui.painter().add(epaint::RectShape {
                        rect,
                        rounding: self.rounding,
                        fill: if self.frame { bg_fill } else { egui::Color32::TRANSPARENT },
                        stroke: self.stroke,
                        fill_texture_id: egui::TextureId::default(),
                        uv: egui::Rect::ZERO,
                        blur_width: 0.,
                    });

                    let mut text_pos = egui::pos2(
                        rect.min.x + padding.x + self.indent,
                        rect.center().y - 0.5 * text_height,
                    );

                    if self.is_loading {
                        let spinner_pos = egui::pos2(
                            rect.max.x - padding.x - (SPINNER_RADIUS * 2) as f32,
                            rect.center().y,
                        );

                        Self::show_spinner(ui, spinner_pos);
                    } else if let Some(icon) = maybe_icon_galley {
                        let alignment = self.icon_alignment.unwrap_or(egui::Align::LEFT);
                        let icon_width = icon.size().x;

                        let icon_x_pos = match alignment {
                            egui::Align::LEFT => {
                                text_pos.x += icon_width + padding.x / 2.0;
                                rect.min.x + padding.x + self.indent
                            }
                            egui::Align::Center | egui::Align::RIGHT => {
                                rect.max.x - padding.x - icon_width
                            }
                        };

                        let icon_pos = egui::pos2(icon_x_pos, rect.center().y - icon.size().y / 3.);
                        ui.painter().galley(icon_pos, icon, icon_color);

                        if self.icon.unwrap().has_badge {
                            ui.painter().circle(
                                egui::pos2(
                                    rect.left_top().x + icon_width / 2.7,
                                    rect.left_top().y + icon_width / 2.5,
                                ),
                                icon_width / 3.2,
                                ui.visuals().hyperlink_color,
                                egui::Stroke::NONE,
                            );
                        }
                    }

                    if let Some(text) = maybe_text_galley {
                        ui.painter()
                            .galley(text_pos, text, text_visuals.text_color())
                    }
                }

                resp
            })
            .inner
    }

    /// copied from the egui spinner impl.
    fn show_spinner(ui: &mut egui::Ui, spinner_pos: egui::Pos2) {
        let color = ui.visuals().strong_text_color();

        ui.ctx().request_repaint();

        let n_points = 20;
        let time = ui.input(|i| i.time);
        let start_angle = time * std::f64::consts::TAU;
        let end_angle = start_angle + 240f64.to_radians() * time.sin();
        let points: Vec<egui::Pos2> = (0..n_points)
            .map(|i| {
                let angle = egui::lerp(start_angle..=end_angle, i as f64 / n_points as f64);
                let (sin, cos) = angle.sin_cos();
                spinner_pos + SPINNER_RADIUS as f32 * egui::vec2(cos as f32, sin as f32)
            })
            .collect();
        ui.painter()
            .add(egui::Shape::line(points, egui::Stroke::new(3.0, color)));
    }
}
