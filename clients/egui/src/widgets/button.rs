//! The button — the whole state space factored to a tiny struct. A button is a
//! `treatment` (rest look) × a `tone` (color), and the interaction states are
//! automatic: hover and press are color/fill shifts (no lift — touch-safe and
//! shareable), focus is a ring. Hue is rationed: the primary fill is neutral
//! ink, and the accent shows up only in the focus ring. Destructive is a
//! Wireframe with `tone = danger` — red text, outline that firms to red on
//! hover; icon-only will be just a content variant.

use egui::{Color32, CornerRadius, FontId, Response, Sense, Stroke, StrokeKind, Ui, vec2};

use crate::theme::tokens::Tokens;

#[derive(Clone, Copy)]
enum Treatment {
    /// Neutral ink fill, inverted text — the one primary action. No hue; a
    /// saturated fill this size reads as loud, so brand color stays out of it.
    Filled,
    /// Surface + line stroke — the workhorse; stroke firms to the tone color
    /// (`fg`, or red for danger) on hover.
    Wireframe,
    /// No fill; muted text firms to `fg` on hover. Icon buttons, toasts.
    Frameless,
}

#[derive(Clone, Copy)]
enum Tone {
    Brand,
    Danger,
}

pub struct Button<'a> {
    tokens: &'a Tokens,
    label: String,
    treatment: Treatment,
    tone: Tone,
}

impl<'a> Button<'a> {
    pub fn primary(t: &'a Tokens, label: impl Into<String>) -> Self {
        Self::new(t, label, Treatment::Filled)
    }
    pub fn secondary(t: &'a Tokens, label: impl Into<String>) -> Self {
        Self::new(t, label, Treatment::Wireframe)
    }
    pub fn quiet(t: &'a Tokens, label: impl Into<String>) -> Self {
        Self::new(t, label, Treatment::Frameless)
    }

    fn new(tokens: &'a Tokens, label: impl Into<String>, treatment: Treatment) -> Self {
        Self { tokens, label: label.into(), treatment, tone: Tone::Brand }
    }

    pub fn danger(mut self) -> Self {
        self.tone = Tone::Danger;
        self
    }

    pub fn show(self, ui: &mut Ui) -> Response {
        let t = self.tokens;
        // Regular 14 label; roomier padding and a 32px floor keep the box from
        // squatting.
        let font = FontId::proportional(14.0);
        let padding = vec2(14.0, 8.0);

        let galley = ui
            .painter()
            .layout_no_wrap(self.label.clone(), font, Color32::PLACEHOLDER);
        let mut desired = galley.size() + padding * 2.0;
        desired.y = desired.y.max(32.0);
        let (rect, response) = ui.allocate_exact_size(desired, Sense::click());

        // Ease hover over a few frames so state changes feel tactile, not abrupt.
        let hover = ui.ctx().animate_bool(response.id, response.hovered());
        let (fill, stroke, text) = self.colors(hover, response.is_pointer_button_down_on());

        let radius = CornerRadius::same(6);
        let painter = ui.painter();
        painter.rect_filled(rect, radius, fill);
        if stroke.a() > 0 {
            painter.rect_stroke(rect, radius, Stroke::new(1.0, stroke), StrokeKind::Inside);
        }
        painter.galley(rect.center() - galley.size() / 2.0, galley, text);

        if response.has_focus() {
            // A soft neutral halo hugging the edge — reads as focus without the
            // loud, spaced accent ring.
            painter.rect_stroke(
                rect,
                radius,
                Stroke::new(3.0, t.fg().gamma_multiply(0.25)),
                StrokeKind::Outside,
            );
        }
        response
    }

    /// (fill, stroke, text) for the current treatment/tone/state. `hover` is a
    /// 0→1 ease factor, `pressed` is instantaneous. Every state adds a state-
    /// layer that deepens with engagement: the ink fill eases toward the canvas,
    /// outlines firm from the neutral line to their tone color.
    fn colors(&self, hover: f32, pressed: bool) -> (Color32, Color32, Color32) {
        let t = self.tokens;
        // The tone color an outline firms *to*: fg for a normal button, red for
        // destructive. The primary fill is deliberately toneless.
        let accent = match self.tone {
            Tone::Brand => t.fg(),
            Tone::Danger => t.danger(),
        };
        match self.treatment {
            Treatment::Filled => {
                let fill = if pressed {
                    t.fg().lerp_to_gamma(t.canvas(), 0.18)
                } else {
                    t.fg().lerp_to_gamma(t.canvas(), 0.10 * hover)
                };
                (fill, Color32::TRANSPARENT, t.canvas())
            }
            Treatment::Wireframe => {
                let stroke = t.line().lerp_to_gamma(accent, hover);
                let fill = if pressed {
                    t.surface().lerp_to_gamma(accent, 0.08)
                } else {
                    t.surface().lerp_to_gamma(accent, 0.04 * hover)
                };
                (fill, stroke, accent)
            }
            Treatment::Frameless => {
                let ink = match self.tone {
                    Tone::Brand => t.text_muted().lerp_to_gamma(t.fg(), hover),
                    Tone::Danger => t.danger(),
                };
                let fill = if pressed {
                    t.fg().gamma_multiply(0.10)
                } else {
                    t.fg().gamma_multiply(0.05 * hover)
                };
                (fill, Color32::TRANSPARENT, ink)
            }
        }
    }
}
