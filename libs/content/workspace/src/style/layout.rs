//! Layout helpers: padded plates and measure/place primitives.
//!
//! Prefer [`ui_width`] / [`remaining_height`] over residual `available_*`.
//! Prefer [`place_at`] / [`claim`] when default `Align::Center` would invent air.

use egui::{Align, Layout, Pos2, Rect, Response, Sense, Ui, UiBuilder, pos2, vec2};

use super::space::Space;
use super::spacer::Spacer;

// ── Parent-owned metrics ────────────────────────────────────────────────────

/// Parent-owned full width of this `Ui` (`max_rect`), not residual after siblings.
#[inline]
pub fn ui_width(ui: &Ui) -> f32 {
    ui.max_rect().width().max(0.0)
}

/// Vertical space from the current cursor to `max_rect.bottom`.
#[inline]
pub fn remaining_height(ui: &Ui) -> f32 {
    (ui.max_rect().bottom() - ui.cursor().top()).max(0.0)
}

// ── Place primitives ────────────────────────────────────────────────────────

/// Where the next composed block starts: full `max_rect` width, cursor top.
#[inline]
pub fn origin(ui: &Ui) -> Pos2 {
    pos2(ui.max_rect().left(), ui.cursor().top())
}

/// Place contents in an explicit rect (parent-owned geometry).
///
/// Like the markdown editor’s `show_*(ui, top_left)`: the child paints inside
/// `rect` with the given layout; **this does not advance the parent cursor**.
/// Call [`claim`] with the composed outer rect when done.
///
/// Returns the closure result and the child’s used `min_rect`.
pub fn place_at<R>(
    ui: &mut Ui, rect: Rect, layout: Layout, add_contents: impl FnOnce(&mut Ui) -> R,
) -> (R, Rect) {
    let mut child = ui.new_child(UiBuilder::new().max_rect(rect).layout(layout));
    child.spacing_mut().item_spacing = vec2(0.0, 0.0);
    let out = add_contents(&mut child);
    let used = child.min_rect();
    (out, used)
}

/// Claim a composed rect in the parent (advance cursor past it).
#[inline]
pub fn claim(ui: &mut Ui, rect: Rect) -> Response {
    ui.allocate_rect(rect, Sense::hover())
}

/// Content band inside a control after horizontal / vertical pad.
#[inline]
pub fn inset(rect: Rect, pad_x: f32, pad_y: f32) -> Rect {
    let left = rect.left() + pad_x;
    let top = rect.top() + pad_y;
    let right = (rect.right() - pad_x).max(left);
    let bottom = (rect.bottom() - pad_y).max(top);
    Rect::from_min_max(pos2(left, top), pos2(right, bottom))
}

/// F2-visible pad bands around a control: top/bottom full width, sides mid height.
pub fn paint_control_pads(ui: &Ui, rect: Rect, pad_x: Space, pad_y: Space) {
    let px = pad_x.pts();
    let py = pad_y.pts();
    if px <= 0.0 && py <= 0.0 {
        return;
    }
    let mid_h = (rect.height() - py * 2.0).max(0.0);
    if py > 0.0 {
        Spacer::paint_at(ui, pad_y, Rect::from_min_size(rect.min, vec2(rect.width(), py)));
        Spacer::paint_at(
            ui,
            pad_y,
            Rect::from_min_size(pos2(rect.left(), rect.bottom() - py), vec2(rect.width(), py)),
        );
    }
    if px > 0.0 && mid_h > 0.0 {
        Spacer::paint_at(
            ui,
            pad_x,
            Rect::from_min_size(pos2(rect.left(), rect.top() + py), vec2(px, mid_h)),
        );
        Spacer::paint_at(
            ui,
            pad_x,
            Rect::from_min_size(pos2(rect.right() - px, rect.top() + py), vec2(px, mid_h)),
        );
    }
}

// ── Pad content protocol (this container only) ──────────────────────────────

/// Mid-column of a pad: parent assigns **width**, content returns **height**,
/// parent places the mid rect, content paints into it.
///
/// No egui sizing pass, no seed-height: measure is from the content’s children /
/// metrics; place does not decide outer size.
pub trait PadContent {
    /// Natural height for the given mid width (measure only — do not paint).
    fn measure(&self, ui: &Ui, width: f32) -> f32;

    /// Paint into `rect` (size already chosen from [`measure`] or a fixed band).
    fn place(&mut self, ui: &mut Ui, rect: Rect);
}

/// Pad mid whose height is already known; `place` draws into the mid rect.
///
/// `place` is [`FnOnce`] so sheets can move values (e.g. confirm [`Action`]) in.
pub struct FixedPadContent<F> {
    pub height: f32,
    place: Option<F>,
}

impl<F: FnOnce(&mut Ui)> FixedPadContent<F> {
    pub fn new(height: f32, place: F) -> Self {
        Self { height, place: Some(place) }
    }
}

impl<F: FnOnce(&mut Ui)> PadContent for FixedPadContent<F> {
    fn measure(&self, _ui: &Ui, _width: f32) -> f32 {
        self.height.max(0.0)
    }

    fn place(&mut self, ui: &mut Ui, rect: Rect) {
        let Some(place) = self.place.take() else {
            return;
        };
        let w = rect.width().max(0.0);
        let h = rect.height().max(0.0);
        let _ = place_at(ui, rect, Layout::top_down(Align::Min), |ui| {
            ui.set_width(w);
            ui.set_max_width(w);
            ui.set_height(h);
            ui.set_min_height(h);
            ui.set_max_height(h);
            place(ui);
        });
    }
}

// ── Horizontal pad ──────────────────────────────────────────────────────────

/// Leading + trailing horizontal [`Spacer`]s; mid from [`PadContent`].
///
/// Measure mid height at mid width, place mid, paint side pads, [`claim`] outer.
pub fn with_h_pad(ui: &mut Ui, pad: Space, content: &mut impl PadContent) -> Response {
    with_h_pad_in(ui, pad, None, content)
}

/// Like [`with_h_pad`], with optional parent-owned band height.
///
/// - `Some(h)`: skip content measure; place into fixed mid band `h`.
/// - `None`: `content.measure(ui, mid_w)` then place.
pub fn with_h_pad_in(
    ui: &mut Ui, pad: Space, band_h: Option<f32>, content: &mut impl PadContent,
) -> Response {
    let p = pad.pts();
    let row_w = ui_width(ui);
    let mid_w = (row_w - p * 2.0).max(0.0);
    let top_left = origin(ui);

    let content_h = band_h
        .unwrap_or_else(|| content.measure(ui, mid_w))
        .max(0.0);
    let mid_rect = Rect::from_min_size(top_left + vec2(p, 0.0), vec2(mid_w, content_h.max(1.0)));
    content.place(ui, mid_rect);

    let outer = Rect::from_min_size(top_left, vec2(row_w, content_h.max(1.0)));
    Spacer::paint_at(ui, pad, Rect::from_min_size(top_left, vec2(p, content_h.max(1.0))));
    Spacer::paint_at(
        ui,
        pad,
        Rect::from_min_size(pos2(top_left.x + row_w - p, top_left.y), vec2(p, content_h.max(1.0))),
    );
    claim(ui, outer)
}

// ── Four-side pad ───────────────────────────────────────────────────────────

/// Pad all four sides. Mid from [`PadContent`] (width in → height out → place).
pub fn with_pad(ui: &mut Ui, pad: Space, content: &mut impl PadContent) {
    let p = pad.pts();
    let outer_w = ui_width(ui);
    let mid_w = (outer_w - p * 2.0).max(0.0);
    let top_left = origin(ui);
    let content_tl = top_left + vec2(p, p);

    let content_h = content.measure(ui, mid_w).max(0.0);
    let content_rect = Rect::from_min_size(content_tl, vec2(mid_w, content_h.max(1.0)));
    content.place(ui, content_rect);

    let total_h = content_h + 2.0 * p;
    let outer = Rect::from_min_size(top_left, vec2(outer_w, total_h.max(2.0 * p)));

    Spacer::paint_at(ui, pad, Rect::from_min_size(top_left, vec2(outer_w, p)));
    Spacer::paint_at(
        ui,
        pad,
        Rect::from_min_size(pos2(top_left.x, content_tl.y + content_h), vec2(outer_w, p)),
    );
    Spacer::paint_at(
        ui,
        pad,
        Rect::from_min_size(pos2(top_left.x, content_tl.y), vec2(p, content_h.max(1.0))),
    );
    Spacer::paint_at(
        ui,
        pad,
        Rect::from_min_size(
            pos2(top_left.x + outer_w - p, content_tl.y),
            vec2(p, content_h.max(1.0)),
        ),
    );

    claim(ui, outer);
}

/// One-pass four-side pad: layout `add` in a measure budget, claim **used** height.
///
/// Use this for interactive content (buttons, fields). [`with_pad`] measures
/// then places — a second layout pass would double-fire clicks.
pub fn with_pad_fit(ui: &mut Ui, pad: Space, add: impl FnOnce(&mut Ui)) {
    let p = pad.pts();
    let outer_w = ui_width(ui);
    let mid_w = (outer_w - p * 2.0).max(0.0);
    let top_left = origin(ui);
    let content_tl = top_left + vec2(p, p);
    const MEASURE_BUDGET: f32 = 50_000.0;
    let mid_rect = Rect::from_min_size(content_tl, vec2(mid_w, MEASURE_BUDGET));
    let (_, used) = place_at(ui, mid_rect, Layout::top_down(Align::Min), |ui| {
        ui.set_width(mid_w);
        ui.set_max_width(mid_w);
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        add(ui);
    });
    let content_h = used.height().max(1.0);
    let total_h = content_h + 2.0 * p;
    let outer = Rect::from_min_size(top_left, vec2(outer_w, total_h.max(2.0 * p)));

    Spacer::paint_at(ui, pad, Rect::from_min_size(top_left, vec2(outer_w, p)));
    Spacer::paint_at(
        ui,
        pad,
        Rect::from_min_size(pos2(top_left.x, content_tl.y + content_h), vec2(outer_w, p)),
    );
    Spacer::paint_at(
        ui,
        pad,
        Rect::from_min_size(pos2(top_left.x, content_tl.y), vec2(p, content_h.max(1.0))),
    );
    Spacer::paint_at(
        ui,
        pad,
        Rect::from_min_size(
            pos2(top_left.x + outer_w - p, content_tl.y),
            vec2(p, content_h.max(1.0)),
        ),
    );
    claim(ui, outer);
}
