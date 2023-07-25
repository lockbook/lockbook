use eframe::{egui, epaint};

use crate::theme::Icon;

#[derive(Default)]
pub struct Button<'a> {
    icon: Option<&'a Icon>,
    text: Option<&'a str>,
    text_style: Option<egui::TextStyle>,
    icon_style: Option<egui::Style>,
    padding: Option<egui::Vec2>,
    rounding: egui::Rounding,
    stroke: egui::Stroke,
    frame: bool,
    hexpand: bool,
    default_fill: Option<egui::Color32>,
}

impl<'a> Button<'a> {
    pub fn icon(self, icon: &'a Icon) -> Self {
        Self { icon: Some(icon), ..self }
    }

    pub fn text(self, text: &'a str) -> Self {
        Self { text: Some(text), ..self }
    }

    pub fn icon_style(self, icon_style: egui::Style) -> Self {
        Self { icon_style: Some(icon_style), ..self }
    }

    pub fn padding(self, padding: impl Into<egui::Vec2>) -> Self {
        Self { padding: Some(padding.into()), ..self }
    }

    pub fn frame(self, frame: bool) -> Self {
        Self { frame, ..self }
    }

    pub fn show(self, ui: &mut egui::Ui) -> egui::Response {
        let text_style = self.text_style.unwrap_or(egui::TextStyle::Body);
        let padding = self.padding.unwrap_or_else(|| ui.spacing().button_padding);
        let text_height = ui.text_style_height(&text_style);
        let wrap_width = ui.available_width();

        let mut width = padding.x * 2.0;

        let icon_text_style = text_style.clone();
        let maybe_icon_galley = self.icon.map(|icon| {
            let icon: egui::WidgetText = icon.into();
            let galley = icon.into_galley(ui, Some(false), wrap_width, icon_text_style);
            width += galley.size().x;
            galley
        });

        let maybe_text_galley = self.text.map(|text| {
            let text: egui::WidgetText = text.into();
            let galley = text.into_galley(ui, Some(false), wrap_width, text_style);
            width += galley.size().x + padding.x;
            galley
        });

        if self.hexpand {
            width = ui.available_size_before_wrap().x;
        }

        let desired_size = egui::vec2(width, text_height + padding.y * 2.0);

        let (rect, resp) = ui.allocate_at_least(desired_size, egui::Sense::click());

        if ui.is_rect_visible(rect) {
            let text_visuals = ui.style().interact(&resp);
            let icon_visuals = self.icon_style.as_ref().unwrap_or(ui.style().as_ref());
            let icon_visuals = icon_visuals.interact(&resp);

            let bg_fill = if resp.hovered() {
                ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
                text_visuals.bg_fill
            } else {
                self.default_fill.unwrap_or(text_visuals.bg_fill)
            };

            ui.painter().add(epaint::RectShape {
                rect,
                rounding: self.rounding,
                fill: if self.frame { bg_fill } else { egui::Color32::TRANSPARENT },
                stroke: self.stroke,
            });

            let mut text_pos =
                egui::pos2(rect.min.x + padding.x, rect.center().y - 0.5 * text_height);

            if let Some(icon) = maybe_icon_galley {
                let icon_pos =
                    egui::pos2(rect.min.x + padding.x, rect.center().y - icon.size().y / 4.1 - 1.0);
                text_pos.x += icon.size().x + padding.x;

                let icon_width = icon.size().x;

                icon.paint_with_visuals(ui.painter(), icon_pos, icon_visuals);

                if self.icon.unwrap().has_badge {
                    ui.painter().circle(
                        egui::pos2(
                            rect.left_top().x + icon_width / 2.7,
                            rect.left_top().y + icon_width / 2.5,
                        ),
                        icon_width / 3.2,
                        ui.visuals().hyperlink_color,
                        egui::Stroke::NONE,
                    )
                }
            }

            if let Some(text) = maybe_text_galley {
                text.paint_with_visuals(ui.painter(), text_pos, text_visuals);
            }
        }

        resp
    }
}
