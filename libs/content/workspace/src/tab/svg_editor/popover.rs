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

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum SelectionAction {
    Copy,
    Delete,
    More,
    Back,
}

pub(super) struct SelectionControls {
    pub action: Option<SelectionAction>,
    pub row_rect: egui::Rect,
    pub properties_rect: Option<egui::Rect>,
}

/// One expansion state machine and layout for both selection entry points:
/// Copy/Delete/More collapses to a back caret beside the advanced controls.
pub(super) fn selection_controls(
    ui: &mut egui::Ui, expanded: &mut bool, read_only: bool, properties: impl FnOnce(&mut egui::Ui),
) -> SelectionControls {
    let row = ui
        .scope(|ui| action_row(ui, |ui| selection_buttons(ui, *expanded, read_only)))
        .inner;
    let action = row.inner.inner;
    match action {
        Some(SelectionAction::More) => *expanded = true,
        Some(SelectionAction::Back) => *expanded = false,
        _ => {}
    }
    // Let the opening click finish before introducing property widgets under
    // the pointer, matching the regular selection tooltip's behavior.
    let properties_rect = if *expanded && action != Some(SelectionAction::More) {
        let rect = egui::Rect::from_min_size(
            row.response.rect.right_top() + egui::vec2(5., 0.),
            egui::Vec2::ZERO,
        );
        Some(
            ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
                egui::Frame::window(ui.style()).show(ui, |ui| ui.vertical(properties));
            })
            .response
            .rect,
        )
    } else {
        None
    };
    SelectionControls { action, row_rect: row.response.rect, properties_rect }
}

/// The regular selection tooltip and a context-menu selection hit draw the
/// exact same controls. Only their placement and action dispatch differ.
pub(super) fn selection_buttons(
    ui: &mut egui::Ui, expanded: bool, read_only: bool,
) -> Option<SelectionAction> {
    use SelectionAction::*;
    if expanded {
        action_buttons(ui, [(Back, ActionLabel::ChevronLeft, true)])
    } else {
        action_buttons(
            ui,
            [
                (Copy, ActionLabel::Text("Copy"), true),
                (Delete, ActionLabel::Text("Delete"), !read_only),
                (More, ActionLabel::ChevronRight, true),
            ],
        )
    }
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
