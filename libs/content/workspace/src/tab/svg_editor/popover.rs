//! Shared presentation for the selection popover and desktop canvas menu.
use crate::theme::icons::Icon;
use crate::widgets::Button;

pub(super) fn apply_style(ui: &mut egui::Ui) {
    ui.visuals_mut().window_corner_radius = egui::CornerRadius::same(7);
    for style in [egui::TextStyle::Body, egui::TextStyle::Button] {
        ui.style_mut()
            .text_styles
            .insert(style, egui::FontId::new(15.0, egui::FontFamily::Proportional));
    }
    ui.visuals_mut().window_shadow = egui::Shadow::NONE;
    if ui.visuals().dark_mode {
        ui.visuals_mut().window_stroke = egui::Stroke::NONE;
        ui.visuals_mut().window_fill = egui::Color32::from_rgb(42, 42, 42);
    }
}

pub(super) fn action_row<R>(
    ui: &mut egui::Ui, contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<egui::InnerResponse<R>> {
    apply_style(ui);
    ui.style_mut().spacing.window_margin = egui::Margin::symmetric(5, 0);
    egui::Frame::window(ui.style()).show(ui, |ui| ui.horizontal(contents))
}

#[derive(Clone, Copy)]
pub(super) enum ActionLabel<'a> {
    Text(&'a str),
    ChevronRight,
    ChevronLeft,
}

/// Both menus use the same buttons, separators, margins and icon glyphs.
pub(super) fn action_buttons<T: Copy>(
    ui: &mut egui::Ui, actions: impl IntoIterator<Item = (T, ActionLabel<'static>, bool)>,
) -> Option<T> {
    if ui.visuals().dark_mode {
        ui.visuals_mut().window_stroke =
            egui::Stroke::new(1.5, egui::Color32::from_rgb(240, 240, 240));
    } else {
        ui.visuals_mut().widgets.noninteractive.bg_stroke = ui.visuals().window_stroke;
        ui.visuals_mut().widgets.noninteractive.bg_stroke.width = 1.0;
    }
    ui.set_max_width(f32::INFINITY);
    let mut clicked = None;
    for (index, (action, label, enabled)) in actions.into_iter().enumerate() {
        if index != 0 {
            ui.separator();
        }
        if ui
            .add_enabled_ui(enabled, |ui| match label {
                ActionLabel::Text(text) => Button::default()
                    .text(text)
                    .margin(egui::vec2(5.0, 2.0))
                    .show(ui),
                ActionLabel::ChevronRight => Button::default()
                    .icon(&Icon::CHEVRON_RIGHT.size(13.0))
                    .show(ui),
                ActionLabel::ChevronLeft => Button::default()
                    .icon(&Icon::CHEVRON_LEFT.size(13.0))
                    .margin(egui::vec2(0.0, 2.0))
                    .show(ui),
            })
            .inner
            .clicked()
        {
            clicked = Some(action);
        }
    }
    clicked
}
