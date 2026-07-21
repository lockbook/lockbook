//! The button — the whole state space factored to a tiny struct. A button is a
//! `treatment` (rest look) × a `tone` (color), and the interaction states are
//! automatic: hover and press are color/fill shifts (no lift — touch-safe and
//! shareable), focus is a ring. Hue is rationed: the primary fill is neutral
//! ink, and the accent shows up only in the focus ring. Destructive is a
//! Wireframe with `tone = danger` — red text, outline that firms to red on
//! hover; icon-only will be just a content variant.
//!
//! Elevated **chips** (sidebar New/Import, pin strip) use free functions
//! [`paint_chip`] / [`chip_colors`] — not the labeled `Button` type.
//!
//! Disabled is a state modifier (not a treatment): same geometry, muted fill/
//! ink, no hover/press, no click.

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
    enabled: bool,
    /// When set, force this height (share sheet row, etc.).
    height: Option<f32>,
    /// Cap width; label is ellipsized when content would exceed it.
    max_width: Option<f32>,
    /// Optional keyboard badge drawn after the label (e.g. `esc`, `⌘↩`).
    shortcut: Option<String>,
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
        Self {
            tokens,
            label: label.into(),
            treatment,
            tone: Tone::Brand,
            enabled: true,
            height: None,
            max_width: None,
            shortcut: None,
        }
    }

    pub fn danger(mut self) -> Self {
        self.tone = Tone::Danger;
        self
    }

    /// When false: muted chrome, no hover/press, clicks ignored.
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Pin the control to an exact height (e.g. align with `search_field::HEIGHT`).
    pub fn height(mut self, h: f32) -> Self {
        self.height = Some(h);
        self
    }

    /// Cap outer width; the label ellipsizes when content would exceed it.
    pub fn max_width(mut self, w: f32) -> Self {
        self.max_width = Some(w);
        self
    }

    /// Show a keyboard shortcut badge after the label.
    pub fn shortcut(mut self, s: impl Into<String>) -> Self {
        self.shortcut = Some(s.into());
        self
    }

    pub fn show(self, ui: &mut Ui) -> Response {
        let t = self.tokens;
        // Regular 14 label; roomier padding and a 32px floor keep the box from
        // squatting.
        let font = FontId::proportional(14.0);
        let sc_font = FontId::proportional(12.0);
        let padding = vec2(14.0, 8.0);
        let sc_gap = 8.0_f32;

        let sc_galley = self.shortcut.as_ref().map(|s| {
            ui.painter()
                .layout_no_wrap(s.clone(), sc_font, Color32::PLACEHOLDER)
        });
        let sc_w = sc_galley
            .as_ref()
            .map(|g| g.size().x + sc_gap)
            .unwrap_or(0.0);

        // When max_width is set, reserve padding + shortcut first, then wrap label.
        let label_max = self.max_width.map(|mw| {
            (mw - padding.x * 2.0 - sc_w).max(24.0)
        });
        let galley = if let Some(max_w) = label_max {
            ui.painter().layout(
                self.label.clone(),
                font,
                Color32::PLACEHOLDER,
                max_w,
            )
        } else {
            ui.painter()
                .layout_no_wrap(self.label.clone(), font, Color32::PLACEHOLDER)
        };
        let mut desired = galley.size() + padding * 2.0 + vec2(sc_w, 0.0);
        desired.y = self.height.unwrap_or(desired.y.max(32.0));
        if let Some(mw) = self.max_width {
            desired.x = desired.x.min(mw).max(0.0);
        }
        let sense = if self.enabled {
            Sense::click()
        } else {
            Sense::hover()
        };
        let (rect, response) = ui.allocate_exact_size(desired, sense);

        let hover = if self.enabled {
            ui.ctx().animate_bool(response.id, response.hovered())
        } else {
            0.0
        };
        let pressed = self.enabled && response.is_pointer_button_down_on();
        let (fill, stroke, text) = self.colors(hover, pressed);
        // Shortcut matches label ink (palette) so badges stay readable on
        // primary filled buttons where muted was too low-contrast.
        let sc_ink = text;

        let radius = CornerRadius::same(6);
        let painter = ui.painter();
        painter.rect_filled(rect, radius, fill);
        if stroke.a() > 0 {
            painter.rect_stroke(rect, radius, Stroke::new(1.0, stroke), StrokeKind::Inside);
        }

        // Center the label (+ optional shortcut) as a unit.
        let label_w = galley.size().x;
        let label_h = galley.size().y;
        let content_w = label_w + sc_w;
        let left = rect.center().x - content_w / 2.0;
        let cy = rect.center().y;
        painter.galley(egui::pos2(left, cy - label_h / 2.0), galley, text);
        if let Some(sg) = sc_galley {
            let sc_pos = egui::pos2(left + label_w + sc_gap, cy - sg.size().y / 2.0);
            painter.galley(sc_pos, sg, sc_ink);
        }

        if self.enabled && response.has_focus() {
            // Soft focus halo — hairline palette color, no faded ink.
            painter.rect_stroke(
                rect,
                radius,
                Stroke::new(3.0, t.line()),
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

        if !self.enabled {
            return match self.treatment {
                // Filled primary: raised slab, muted label (not inverted).
                Treatment::Filled => (t.surface_raised(), Color32::TRANSPARENT, t.text_muted()),
                Treatment::Wireframe => (t.surface(), t.line(), t.text_muted()),
                Treatment::Frameless => {
                    (Color32::TRANSPARENT, Color32::TRANSPARENT, t.text_muted())
                }
            };
        }

        // The tone color an outline firms *to*: fg for a normal button, red for
        // destructive. The primary fill is deliberately toneless.
        let accent = match self.tone {
            Tone::Brand => t.fg(),
            Tone::Danger => t.danger(),
        };
        match self.treatment {
            Treatment::Filled => {
                // Ink fill eases toward canvas on engage — both palette ends.
                let fill = if pressed {
                    t.fg().lerp_to_gamma(t.canvas(), 0.18)
                } else {
                    t.fg().lerp_to_gamma(t.canvas(), 0.10 * hover)
                };
                (fill, Color32::TRANSPARENT, t.canvas())
            }
            Treatment::Wireframe => {
                let stroke = if hover > 0.5 || pressed {
                    accent
                } else {
                    t.line()
                };
                let fill = if pressed {
                    t.surface_raised()
                } else if hover > 0.0 {
                    t.surface().lerp_to_gamma(t.surface_raised(), hover)
                } else {
                    t.surface()
                };
                (fill, stroke, accent)
            }
            Treatment::Frameless => {
                let ink = match self.tone {
                    Tone::Brand => {
                        if hover > 0.5 {
                            t.fg()
                        } else {
                            t.text_muted()
                        }
                    }
                    Tone::Danger => t.danger(),
                };
                let fill = if pressed {
                    t.surface_raised()
                } else if hover > 0.0 {
                    t.canvas().lerp_to_gamma(t.surface_raised(), hover)
                } else {
                    Color32::TRANSPARENT
                };
                (fill, Color32::TRANSPARENT, ink)
            }
        }
    }
}

/// Chip chrome colors — palette only.
///
/// Rest: solid `base` (usually canvas), **no** outline.  
/// Hover: soft `line` outline faded with `hover` (0→1 from `animate_bool`) so
/// it eases off properly — not binary on-for-the-whole-fade.  
/// Press: full `line` stroke; fill eases slightly toward ink.
pub fn chip_colors(
    t: &Tokens, base: Color32, hover: f32, pressed: bool,
) -> (Color32, Color32) {
    let fill = if pressed {
        base.lerp_to_gamma(t.fg(), 0.08)
    } else {
        base
    };
    // Quiet hairline; alpha tracks hover so leave eases out (not a sticky solid).
    let stroke = if pressed {
        t.line()
    } else if hover > 0.0 {
        t.line().linear_multiply(hover)
    } else {
        Color32::TRANSPARENT
    };
    (fill, stroke)
}

/// Paint elevated chip chrome (New / Import / Search, pin chips).
///
/// See [`chip_colors`]. `base` is the resting fill — canvas on surface chrome.
/// Pass `animate_bool` hover (0→1) so the outline fades with the ease.
pub fn paint_chip(
    ui: &Ui, t: &Tokens, rect: egui::Rect, radius: impl Into<CornerRadius>, hover: f32,
    pressed: bool, base: Color32,
) {
    let radius = radius.into();
    let (fill, stroke) = chip_colors(t, base, hover, pressed);
    let painter = ui.painter();
    painter.rect_filled(rect, radius, fill);
    if stroke.a() > 0 {
        painter.rect_stroke(rect, radius, Stroke::new(1.0, stroke), StrokeKind::Inside);
    }
}
