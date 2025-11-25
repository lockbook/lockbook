use egui::{Response, Sense, TextStyle, TextWrapMode, Ui, Vec2, WidgetText};

use crate::theme::icons::Icon;

/// A button with only an icon. Has a background when hovered. Colored when clicked.
/// Supports an optional tooltip.
pub struct IconButton {
    icon: Icon,
    tooltip: Option<String>,
    colored: bool,
    disabled: bool,
    size: Option<f32>,
}

impl IconButton {
    /// Create an icon button with the given icon.
    pub fn new(icon: Icon) -> Self {
        Self { icon, tooltip: None, colored: false, size: None, disabled: false }
    }

    /// Add a tooltip for the button. Default: `None`.
    pub fn tooltip(self, tooltip: impl Into<String>) -> Self {
        Self { tooltip: Some(tooltip.into()), ..self }
    }

    /// Make the button colored even if it's not clicked. Default: `false`.
    pub fn colored(self, colored: bool) -> Self {
        Self { colored, ..self }
    }

    /// Make the button bigger (not the icon). Default: `None` (scale with icon)
    pub fn size(self, size: f32) -> Self {
        Self { size: Some(size), ..self }
    }

    /// Disable the button, making it visually dim, and physically unclickable
    pub fn disabled(self, disabled: bool) -> Self {
        Self { disabled, ..self }
    }

    pub fn show(self, ui: &mut Ui) -> Response {
        let wrap_width = ui.available_width();

        let icon_text: WidgetText = (&self.icon).into();
        let galley =
            icon_text.into_galley(ui, Some(TextWrapMode::Extend), wrap_width, TextStyle::Body);

        let desired_size = if let Some(size) = self.size {
            let min_size = galley.mesh_bounds.size().max_elem();
            Vec2::splat(size.max(min_size))
        } else {
            Vec2::splat(galley.mesh_bounds.size().max_elem() * 2.)
        };
        let (rect, mut resp) = ui.allocate_exact_size(
            desired_size,
            if self.disabled { Sense::hover() } else { Sense::click() },
        );

        if resp.hovered() {
            ui.painter()
                .rect(rect, 2., ui.visuals().code_bg_color, egui::Stroke::NONE);
            ui.output_mut(|o: &mut egui::PlatformOutput| {
                o.cursor_icon = egui::CursorIcon::PointingHand
            });
        }

        let mut icon_color = if self.colored || resp.is_pointer_button_down_on() {
            ui.visuals().widgets.active.bg_fill
        } else {
            ui.visuals().text_color()
        };

        if self.disabled {
            icon_color = icon_color.gamma_multiply(0.5);
        }

        ui.painter().galley(
            ((rect.min - galley.mesh_bounds.min)
                + ((rect.size() - galley.mesh_bounds.size()) / 2.0))
                .to_pos2(),
            galley,
            icon_color,
        );

        if let Some(tooltip) = &self.tooltip {
            ui.ctx()
                .style_mut(|s| s.visuals.menu_rounding = (2.).into());
            resp = resp.on_hover_ui(|ui| {
                let text: WidgetText = (tooltip).into();
                let text = text.clone().into_galley(
                    ui,
                    Some(TextWrapMode::Extend),
                    ui.available_width(),
                    egui::TextStyle::Small,
                );
                ui.add(egui::Label::new(text));
            });
        }

        resp
    }
}
