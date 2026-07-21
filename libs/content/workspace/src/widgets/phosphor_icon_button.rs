//! Icon-only button that paints a Phosphor glyph (family `"phosphor"`).
//!
//! Layout is **slot-first**: the hit target is a fixed square; the glyph is
//! drawn centered inside it. Glyph mesh bounds never drive size (Phosphor
//! advance widths vary enough to make rows look uneven).

use std::sync::Arc;

use egui::{FontFamily, FontId, Response, Sense, Ui, Vec2, pos2};

use crate::theme::palette_v2::ThemeExt;
use crate::theme::phosphor;

/// Default outer hit target (matches chrome strip tools).
pub const DEFAULT_SLOT: f32 = 28.0;
/// Default glyph size inside the slot.
pub const DEFAULT_GLYPH: f32 = 15.0;

/// A square click target with a centered Phosphor glyph.
pub struct PhosphorIconButton {
    glyph: &'static str,
    /// Drawn glyph size (independent of slot).
    glyph_size: f32,
    /// Outer square hit target.
    slot: f32,
    tooltip: Option<String>,
    colored: bool,
    disabled: bool,
    subdued: bool,
    hover_bg: bool,
}

impl PhosphorIconButton {
    pub fn new(glyph: &'static str) -> Self {
        Self {
            glyph,
            glyph_size: DEFAULT_GLYPH,
            slot: DEFAULT_SLOT,
            tooltip: None,
            colored: false,
            disabled: false,
            subdued: false,
            hover_bg: true,
        }
    }

    /// Glyph paint size (does not change the hit target).
    pub fn icon_size(self, glyph_size: f32) -> Self {
        Self { glyph_size, ..self }
    }

    /// Outer square slot / hit target.
    pub fn size(self, slot: f32) -> Self {
        Self { slot, ..self }
    }

    pub fn tooltip(self, tooltip: impl Into<String>) -> Self {
        Self { tooltip: Some(tooltip.into()), ..self }
    }

    pub fn colored(self, colored: bool) -> Self {
        Self { colored, ..self }
    }

    pub fn disabled(self, disabled: bool) -> Self {
        Self { disabled, ..self }
    }

    pub fn subdued(self, subdued: bool) -> Self {
        Self { subdued, ..self }
    }

    pub fn hover_bg(self, hover_bg: bool) -> Self {
        Self { hover_bg, ..self }
    }

    pub fn measure(&self) -> Vec2 {
        Vec2::splat(self.slot)
    }

    pub fn show(self, ui: &mut Ui) -> Response {
        let theme = ui.ctx().get_lb_theme();
        let desired = self.measure();
        let (rect, resp) = ui.allocate_exact_size(
            desired,
            if self.disabled { Sense::hover() } else { Sense::click() },
        );

        if resp.hovered() && !self.disabled {
            if self.hover_bg {
                ui.painter().rect(
                    rect,
                    4.,
                    theme.neutral_bg_secondary(),
                    egui::Stroke::NONE,
                    egui::epaint::StrokeKind::Inside,
                );
            }
            ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
        }

        let icon_color = if self.colored || resp.is_pointer_button_down_on() {
            theme.fg().get_color(theme.prefs().primary)
        } else if self.disabled {
            theme.neutral()
        } else if self.subdued {
            theme.neutral_fg_secondary()
        } else {
            theme.neutral_fg()
        };

        // Cap glyph so it never visually overflows the slot.
        let glyph_size = self.glyph_size.min(self.slot * 0.62);
        let font = FontId::new(glyph_size, FontFamily::Name(Arc::from(phosphor::FAMILY)));
        let galley = ui
            .painter()
            .layout_no_wrap(self.glyph.into(), font, icon_color);
        // Center by mesh bounds so optical weight sits in the slot.
        ui.painter().galley(
            pos2(
                rect.center().x - galley.size().x * 0.5,
                rect.center().y - galley.size().y * 0.5,
            ),
            galley,
            icon_color,
        );

        if let Some(tooltip) = &self.tooltip {
            crate::widgets::tip_text(ui.ctx(), &resp, tooltip.as_str());
        }

        resp
    }
}
