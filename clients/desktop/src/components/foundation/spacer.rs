//! Layout child that occupies one [`Space`] token along the main axis.
//!
//! [`Rule`] is a zero-main-axis hairline — sandwich between spacers for dividers.

#[cfg(test)]
use egui::{Context, Id};
use egui::{Pos2, Rect, Response, Sense, Stroke, Ui, Vec2, Widget};

use super::chrome::STROKE_HAIRLINE;
use super::color::ThemeExt;
use super::overlay;
use super::space::Space;

/// One allocated [`Spacer`] (for headless layout diagnostics).
#[cfg(test)]
#[derive(Clone, Debug)]
pub struct SpacerHit {
    pub token: Space,
    pub rect: Rect,
    /// Explicit cross-axis size when [`Spacer::fill_cross`] was used.
    pub fill_cross: Option<f32>,
    /// Parent layout was horizontal when allocated.
    pub horizontal: bool,
}

#[cfg(test)]
fn record_id() -> Id {
    Id::new("lb.design.spacer_record")
}

/// Start collecting every [`Spacer`] allocate this frame (tests / diagnostics).
#[cfg(test)]
pub fn begin_record(ctx: &Context) {
    ctx.data_mut(|d| d.insert_temp(record_id(), Vec::<SpacerHit>::new()));
}

/// Drain recorded spacer hits (empty if not recording).
#[cfg(test)]
pub fn take_record(ctx: &Context) -> Vec<SpacerHit> {
    ctx.data_mut(|d| d.remove_temp::<Vec<SpacerHit>>(record_id()))
        .unwrap_or_default()
}

#[cfg(test)]
fn recording(ctx: &Context) -> bool {
    ctx.data(|d| d.get_temp::<Vec<SpacerHit>>(record_id()).is_some())
}

#[cfg(test)]
fn push_hit(ctx: &Context, hit: SpacerHit) {
    ctx.data_mut(|d| {
        if d.get_temp::<Vec<SpacerHit>>(record_id()).is_none() {
            return;
        }
        d.get_temp_mut_or_default::<Vec<SpacerHit>>(record_id())
            .push(hit);
    });
}

/// Occupies one [`Space`] step on the main axis.
///
/// Vertical: full width of the parent `max_rect` × token height.  
/// Horizontal: token width × 1 px, or token width × **parent-supplied** cross
/// height via [`Spacer::fill_cross`].
///
/// Never reads `available_height()` — the parent owns cross-axis metrics
/// (available_* residual / parent-owned metrics).
pub struct Spacer {
    token: Space,
    /// When set in a horizontal layout: height of this pad. Vertical layout
    /// ignores this (main axis is already the token).
    cross: Option<f32>,
}

impl Spacer {
    pub fn new(token: Space) -> Self {
        Self { token, cross: None }
    }

    /// Stretch on the cross axis to an explicit size (parent-owned).
    ///
    /// In a **horizontal** row this is the pad height. Call only after the
    /// parent has decided the band (e.g. `control_height()`, `with_h_pad_in`
    /// with `Some(h)`).
    pub fn fill_cross(mut self, cross: f32) -> Self {
        self.cross = Some(cross.max(1.0));
        self
    }

    /// Paint a pad band at an **explicit rect** (measure+place).
    ///
    /// Does **not** advance the placer — the parent already claimed the composed
    /// outer rect. Used by pad helpers so side/top/bottom bands match measured
    /// content height without horizontal `Align::Center` air.
    pub fn paint_at(ui: &Ui, token: Space, rect: Rect) {
        if rect.width() <= 0.0 || rect.height() <= 0.0 {
            return;
        }
        if overlay::is_enabled(ui.ctx()) {
            let t = ui.ctx().get_lb_theme();
            ui.painter().rect_filled(rect, 0.0, token.overlay_fill(&t));
        }
        #[cfg(test)]
        if recording(ui.ctx()) {
            let horizontal = rect.width() < rect.height();
            push_hit(
                ui.ctx(),
                SpacerHit {
                    token,
                    rect,
                    fill_cross: Some(if horizontal { rect.height() } else { rect.width() }),
                    horizontal,
                },
            );
        }
    }
}

impl Widget for Spacer {
    fn ui(self, ui: &mut Ui) -> Response {
        let pts = self.token.pts();
        let horizontal = ui.layout().main_dir().is_horizontal();

        let desired = if horizontal {
            let h = self.cross.unwrap_or(1.0);
            Vec2::new(pts, h)
        } else {
            // Full width of this ui’s max rect — not residual `available_width`
            // after siblings (side pads are horizontal).
            Vec2::new(ui.max_rect().width().max(0.0), pts)
        };

        let (rect, response) = ui.allocate_exact_size(desired, Sense::hover());
        // F2 + record via the shared path (same as absolute place).
        Spacer::paint_at(ui, self.token, rect);
        response
    }
}

/// Zero-main-axis hairline (`line` token). Place between spacers for part dividers.
///
/// Vertical layout → horizontal rule (full `max_rect` width).
/// Horizontal layout → vertical rule (1 pt cross).
#[derive(Default)]
pub struct Rule {}

impl Rule {
    pub fn new() -> Self {
        Self {}
    }
}

impl Widget for Rule {
    fn ui(self, ui: &mut Ui) -> Response {
        let horizontal = ui.layout().main_dir().is_horizontal();
        let desired = if horizontal {
            Vec2::new(0.0, 1.0)
        } else {
            Vec2::new(ui.max_rect().width().max(0.0), 0.0)
        };

        let (rect, response) = ui.allocate_exact_size(desired, Sense::hover());
        let t = ui.ctx().get_lb_theme();
        let stroke = Stroke::new(STROKE_HAIRLINE, t.neutral());

        if horizontal {
            let x = rect.center().x;
            ui.painter()
                .line_segment([Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())], stroke);
        } else {
            let y = rect.center().y;
            ui.painter()
                .line_segment([Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)], stroke);
        }

        response
    }
}
