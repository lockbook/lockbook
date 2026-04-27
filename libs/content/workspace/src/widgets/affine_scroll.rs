//! A vertical scroll area whose content has two notions of height per
//! row: a cheap **approximate** height and an expensive **precise**
//! height. The scrollbar operates in approx units (so the bar's range is
//! a constant function of the doc), but visible content is laid out
//! precisely. Scroll *events* are interpreted in precise units (= screen
//! pixels of intended movement) and translated to approx via a piecewise
//! affine map before they touch the scrollbar.
//!
//! # Why
//!
//! When rows are only cheap to estimate (rich text, shaped lines,
//! etc.), measuring every row precisely to size the scrollbar is too
//! slow. Approximating sizes is fast but breaks the "user scrolls N
//! pixels, content moves N pixels" invariant.
//!
//! This widget bridges the two: scrollbar is approx (so always known
//! and consistent), but the user's wheel/drag input is interpreted as a
//! request to move content by N **precise** pixels. The widget walks
//! the affine map at the current scroll position, converts to the
//! corresponding approx delta, and updates the scrollbar.
//!
//! # Contract with `Rows`
//!
//! After [`Rows::reset`] the cursor sits before the first row; the
//! first `next()` yields it. Once `next()` returns `None` the cursor
//! is past the last row, and `prev()` from there yields the last.
//! Symmetric for [`Rows::reset_back`] + `prev()`.

use egui::{Pos2, Rect, Response, Sense, Stroke, Ui, Vec2};

/// Sequence of variable-height rows the scroll area walks.
///
/// `approx` / `precise` / `render` operate on the row at the cursor's
/// current position; calling them when the cursor is off a row (before
/// start, past end, or after a `next`/`prev` returned `false`) panics.
pub trait Rows {
    fn reset(&mut self);
    fn reset_back(&mut self);
    fn next(&mut self) -> bool;
    fn prev(&mut self) -> bool;

    /// Called frequently while sizing the scrollbar and finding the anchor.
    fn approx(&self) -> f32;

    /// Called for rows the widget renders or walks in the precise-
    /// from-end tail.
    fn precise(&mut self) -> f32;

    fn render(&mut self, ui: &mut Ui, top_left: Pos2);

    /// Hint that the row is about to enter the viewport. Content can
    /// kick off background work (e.g. start downloading images)
    /// without painting. Called by the scroll area on rows within a
    /// viewport-sized buffer above and below the visible window.
    /// Default is a no-op.
    fn warm(&mut self) {}
}

/// Per-frame state of the scroll area. Persisted in egui memory between
/// frames keyed by [`AffineScrollArea::id_salt`].
#[derive(Clone, Copy, Default)]
struct ScrollState {
    /// Position in approx units. `[0, max_offset]`.
    offset_approx: f32,
    /// Touch-scroll velocity in precise pixels per second. Positive
    /// means content is moving up on screen (scroll offset growing).
    velocity_precise: f32,
    /// Sliding window of recent drag samples — `(delta_precise, dt)` —
    /// averaged into `velocity_precise` to smooth out single-frame
    /// noise.
    drag_window: [(f32, f32); DRAG_WINDOW_LEN],
    drag_window_idx: u8,
}

const DRAG_WINDOW_LEN: usize = 6;

pub struct AffineScrollArea {
    id_salt: egui::Id,
    /// When true, drag on the body (not just the scrollbar) scrolls
    /// the content with momentum. Intended for touch input.
    touch_scroll: bool,
}

impl AffineScrollArea {
    pub fn new(id_salt: impl std::hash::Hash) -> Self {
        Self { id_salt: egui::Id::new(id_salt), touch_scroll: false }
    }

    pub fn touch_scroll(mut self, enabled: bool) -> Self {
        self.touch_scroll = enabled;
        self
    }

    /// Current touch-scroll velocity (precise px/sec). y is vertical;
    /// x is always 0. Non-zero while momentum scroll is in flight.
    /// Used to block other touch handling (e.g. cursor placement)
    /// while content is coasting.
    pub fn velocity(&self, ctx: &egui::Context) -> egui::Vec2 {
        let state: ScrollState = ctx.data(|d| d.get_temp(self.id_salt)).unwrap_or_default();
        egui::Vec2::new(0.0, state.velocity_precise)
    }

    /// Current scroll offset in approx units. For persistence.
    pub fn offset(&self, ctx: &egui::Context) -> f32 {
        ctx.data(|d| d.get_temp::<ScrollState>(self.id_salt))
            .unwrap_or_default()
            .offset_approx
    }

    /// Set scroll offset directly. Skips clamping — the scroll area
    /// will clamp on next show against the current `max_offset`.
    pub fn set_offset(&self, ctx: &egui::Context, offset_approx: f32) {
        let mut state: ScrollState = ctx.data(|d| d.get_temp(self.id_salt)).unwrap_or_default();
        state.offset_approx = offset_approx;
        ctx.data_mut(|d| d.insert_temp(self.id_salt, state));
        ctx.request_repaint();
    }

    pub fn show<R: Rows>(&mut self, ui: &mut Ui, content: &mut R) -> Response {
        // Take the parent's full max_rect — callers control the scroll
        // area's size by setting the surrounding ui's max_rect (e.g.
        // via `ui.scope_builder().max_rect(...)`).
        let rect = ui.max_rect();

        let body_sense = if self.touch_scroll { Sense::click_and_drag() } else { Sense::hover() };
        let response = ui.allocate_rect(rect, body_sense);

        // Scrollbar hit area registered AFTER the body so it shadows
        // the body's hover in z-order. Same dimensions used by
        // `draw_scrollbar` below.
        const BAR_WIDTH: f32 = 10.0;
        const BAR_INSET: f32 = 3.0;
        let bar_x = rect.max.x - BAR_WIDTH - BAR_INSET;
        let bar_track =
            Rect::from_min_size(Pos2::new(bar_x, rect.min.y), Vec2::new(BAR_WIDTH, rect.height()));
        let bar_id = self.id_salt.with("scrollbar");
        let bar_response = ui.interact(bar_track, bar_id, Sense::click_and_drag());

        // Persisted scroll state.
        let mut state: ScrollState = ui
            .ctx()
            .data(|d| d.get_temp(self.id_salt))
            .unwrap_or_default();

        let viewport_height = rect.height();

        // approx_total: walk forward summing approx. Cheap.
        let approx_total = sum_approx(content);

        // max_offset: walk backward from the doc end summing precise
        // until we cover one viewport. The crossing row becomes the
        // anchor at max scroll; convert its intra-position back to
        // approx. Bounded by the viewport's worth of rows.
        let max_offset = compute_max_offset(content, viewport_height, approx_total);

        // Scrollbar dimensions reason in approx space (cheap sizing),
        // independent of `max_offset`'s precise-aware clamp.
        let scrollbar_total = approx_total.max(viewport_height);

        // Process scroll events: wheel, drag on scrollbar, programmatic.
        // Wheel events are in screen pixels = precise. Translate to
        // approx via the affine map at current offset.
        //
        // Use `raw_scroll_delta` (immediate, unsmoothed) rather than
        // `smooth_scroll_delta` (kinetic). Tests need the full delta in
        // one frame; production wheel input lands in raw too.
        let raw_scroll_delta = ui.input(|i| i.raw_scroll_delta.y);
        if raw_scroll_delta != 0.0 {
            // egui's scroll convention: positive y means scroll up
            // (content moves down). We want our offset to *increase*
            // when user scrolls down (content moves up), so negate.
            let precise_delta = -raw_scroll_delta;
            let approx_delta = precise_to_approx_delta(content, state.offset_approx, precise_delta);
            state.offset_approx = (state.offset_approx + approx_delta).clamp(0.0, max_offset);
        }

        // Touch body drag → scroll + velocity tracking.
        let dt = ui.input(|i| i.stable_dt).max(0.0001);
        if max_offset > 0.0 && self.touch_scroll && response.drag_started() {
            // Tap-during-momentum or drag-start: clear stale velocity
            // and the sliding-window history so the new gesture
            // doesn't inherit anything.
            state.velocity_precise = 0.0;
            state.drag_window = [(0.0, 0.0); DRAG_WINDOW_LEN];
            state.drag_window_idx = 0;
        }
        if max_offset > 0.0 && self.touch_scroll && response.dragged() {
            let drag_y = ui.input(|i| i.pointer.delta().y);
            // Drag UP (negative y) → scroll DOWN (offset grows). Same
            // sign convention as wheel.
            let precise_delta = -drag_y;
            let approx_delta = precise_to_approx_delta(content, state.offset_approx, precise_delta);
            state.offset_approx = (state.offset_approx + approx_delta).clamp(0.0, max_offset);
            // Push this frame's sample into the sliding window.
            // Velocity = sum(deltas) / sum(dts) over the window. Held
            // frames (delta=0) drag the average toward 0, so a pause
            // before release won't produce phantom momentum.
            state.drag_window[state.drag_window_idx as usize] = (precise_delta, dt);
            state.drag_window_idx = (state.drag_window_idx + 1) % (DRAG_WINDOW_LEN as u8);
            let (sum_d, sum_dt) = state
                .drag_window
                .iter()
                .fold((0.0, 0.0), |(sd, st), (d, t)| (sd + d, st + t));
            state.velocity_precise = if sum_dt > 0.001 { sum_d / sum_dt } else { 0.0 };
        } else if state.velocity_precise.abs() > 1.0 && !response.dragged() {
            // Coast: apply velocity * dt, decay velocity.
            const DECAY_PER_SEC: f32 = 4.0;
            let precise_step = state.velocity_precise * dt;
            let approx_step = precise_to_approx_delta(content, state.offset_approx, precise_step);
            let new_offset = (state.offset_approx + approx_step).clamp(0.0, max_offset);
            // If we hit a scroll boundary, kill momentum.
            if (new_offset - state.offset_approx).abs() < 0.001 {
                state.velocity_precise = 0.0;
            } else {
                state.offset_approx = new_offset;
                state.velocity_precise *= (-DECAY_PER_SEC * dt).exp();
            }
            ui.ctx().request_repaint();
        } else {
            state.velocity_precise = 0.0;
        }
        // Tap (click without drag) cancels momentum.
        if self.touch_scroll && response.clicked() {
            state.velocity_precise = 0.0;
        }

        // Scrollbar drag/click. The thumb spans `visible_fraction *
        // viewport_height`; the user can drag the thumb across the
        // remaining `(1 - visible_fraction) * viewport_height` of
        // track. That drag range maps onto `[0, max_offset]` in approx.
        if max_offset > 0.0 && (bar_response.dragged() || bar_response.clicked()) {
            let visible_fraction = (viewport_height / scrollbar_total).clamp(0.0, 1.0);
            let track_drag_range = (rect.height() * (1.0 - visible_fraction)).max(1.0);
            if bar_response.dragged() {
                let drag_y = ui.input(|i| i.pointer.delta().y);
                state.offset_approx = (state.offset_approx
                    + drag_y * (max_offset / track_drag_range))
                    .clamp(0.0, max_offset);
            } else if let Some(pos) = bar_response.interact_pointer_pos() {
                // Click jumps so the thumb's center lands at the cursor.
                let thumb_h = visible_fraction * rect.height();
                let target_thumb_top =
                    (pos.y - rect.min.y - thumb_h / 2.0).clamp(0.0, track_drag_range);
                state.offset_approx = target_thumb_top * (max_offset / track_drag_range);
            }
        }

        // Find anchor and render visible rows.
        ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
            ui.set_clip_rect(rect);
            render_visible(ui, content, rect, state.offset_approx);
        });

        // Draw scrollbar (simple vertical bar on the right).
        if max_offset > 0.0 {
            draw_scrollbar(ui, rect, state.offset_approx, scrollbar_total, viewport_height);
        }

        // Persist state.
        ui.ctx().data_mut(|d| d.insert_temp(self.id_salt, state));

        response
    }
}

/// Where to position a target row within the viewport.
#[derive(Clone, Copy, Debug)]
pub enum Align {
    /// Target's top at viewport top.
    Top,
    /// Target's center at viewport center.
    Center,
    /// Target's bottom at viewport bottom.
    Bottom,
}

/// Compute the offset that places the first row matching `find` in the
/// viewport per `align`. `find` is run on each row in forward order
/// until it returns true. Returns `None` if no row matches.
///
/// Cost is bounded by the position of the target plus (for non-Top
/// alignments) the precise content needed to fill the gap above the
/// target — at most one viewport's worth.
pub fn align_offset<R, F>(
    content: &mut R, viewport_height: f32, align: Align, mut find: F,
) -> Option<f32>
where
    R: Rows,
    F: FnMut(&mut R) -> bool,
{
    content.reset();
    let mut approx_top = 0.0f32;
    let mut target_approx = 0.0f32;
    let mut target_precise = 0.0f32;
    let mut found = false;
    while content.next() {
        let h = content.approx();
        if find(content) {
            target_approx = h;
            target_precise = content.precise();
            found = true;
            break;
        }
        approx_top += h;
    }
    if !found {
        return None;
    }

    let target_gap = match align {
        Align::Top => 0.0,
        Align::Center => (viewport_height - target_precise) / 2.0,
        Align::Bottom => viewport_height - target_precise,
    };

    if target_gap < 0.0 {
        // Target is taller than the gap allows — anchor at target with
        // its top above the viewport.
        let intra_precise = -target_gap;
        let slope = if target_approx > 0.0 { target_precise / target_approx } else { 1.0 };
        let intra_approx = if slope > 0.0 { intra_precise / slope } else { 0.0 };
        return Some((approx_top + intra_approx).max(0.0));
    }

    // Walk back from target, summing precise until we cover the gap.
    // The crossing row becomes the anchor.
    let mut precise_sum = 0.0f32;
    let mut approx_sum = 0.0f32;
    while content.prev() {
        let p = content.precise();
        let a = content.approx();
        if precise_sum + p >= target_gap && a > 0.0 {
            let intra_precise = p - (target_gap - precise_sum);
            let slope = p / a;
            let intra_approx = if slope > 0.0 { intra_precise / slope } else { 0.0 };
            let anchor_approx_top = approx_top - approx_sum - a;
            return Some((anchor_approx_top + intra_approx).max(0.0));
        }
        precise_sum += p;
        approx_sum += a;
    }

    // Walked back to the doc start without filling the gap.
    Some(0.0)
}

/// "Make visible" semantics: only return a new offset if `current_offset`
/// has the target outside the viewport. When the target is already
/// visible, returns `None` so the caller leaves scroll alone — avoids
/// the constant-recenter feel of `Align::Center`. When out of view,
/// returns the offset that brings the nearest edge of the target just
/// into the viewport (Top alignment if scrolling up, Bottom if down).
pub fn make_visible_offset<R, F>(
    content: &mut R, viewport_height: f32, current_offset: f32, mut find: F,
) -> Option<f32>
where
    R: Rows,
    F: FnMut(&mut R) -> bool,
{
    let top = align_offset(content, viewport_height, Align::Top, &mut find)?;
    let bottom = align_offset(content, viewport_height, Align::Bottom, &mut find)?;
    let lo = top.min(bottom);
    let hi = top.max(bottom);
    if current_offset >= lo && current_offset <= hi {
        None
    } else if current_offset < lo {
        Some(lo)
    } else {
        Some(hi)
    }
}

/// Forward walk summing approx heights.
fn sum_approx<R: Rows>(content: &mut R) -> f32 {
    content.reset();
    let mut total = 0.0f32;
    while content.next() {
        total += content.approx();
    }
    total
}

/// Convert a scroll offset (approx units) into the row the offset
/// lands in plus an `intra_offset` (approx units within that row).
/// Used for width-independent scroll persistence — the returned `idx`
/// is the row's flat-order position from the start of the doc.
pub fn offset_to_anchor<R: Rows>(content: &mut R, offset: f32) -> (usize, f32) {
    content.reset();
    let mut acc = 0.0f32;
    let mut idx = 0usize;
    let mut last_acc = 0.0f32;
    let mut last_idx = 0usize;
    while content.next() {
        let h = content.approx();
        if acc + h > offset {
            return (idx, (offset - acc).max(0.0));
        }
        last_acc = acc;
        last_idx = idx;
        acc += h;
        idx += 1;
    }
    if idx == 0 { (0, 0.0) } else { (last_idx, offset - last_acc) }
}

/// Inverse of [`offset_to_anchor`]. Out-of-range `anchor_idx` clamps to
/// the last row; `intra` clamps to that row's approx height.
pub fn anchor_to_offset<R: Rows>(content: &mut R, anchor_idx: usize, intra: f32) -> f32 {
    content.reset();
    let mut acc = 0.0f32;
    let mut idx = 0usize;
    let mut last_h = 0.0f32;
    while content.next() {
        let h = content.approx();
        if idx == anchor_idx {
            return acc + intra.clamp(0.0, h);
        }
        acc += h;
        last_h = h;
        idx += 1;
    }
    if idx == 0 { 0.0 } else { acc - last_h + intra.clamp(0.0, last_h) }
}

/// Walk precise heights from the end of the doc backwards until the
/// cumulative reaches `viewport_height`. The row where it crosses is
/// the anchor at max scroll; convert its intra-position back to approx
/// units (via that row's slope) to get an `offset_approx` cap that,
/// when rendered, places the doc's tail at the viewport bottom.
///
/// Cost is bounded by the tail rows visible at max scroll — not the
/// whole doc — so this stays cheap even on long documents.
///
/// Edge cases:
/// - viewport_height <= 0: returns 0.
/// - Total precise content < viewport_height: returns 0 (doc fits).
/// - Anchor would land in a 0-approx row: skip and continue
///   backwards. The 0-approx row can't host an anchor because the
///   slope is undefined.
fn compute_max_offset<R: Rows>(content: &mut R, viewport_height: f32, approx_total: f32) -> f32 {
    if viewport_height <= 0.0 {
        return 0.0;
    }
    content.reset_back();
    let mut cumulative_precise = 0.0f32;
    let mut approx_tail_sum = 0.0f32;
    while content.prev() {
        let p = content.precise();
        let a = content.approx();
        cumulative_precise += p;
        approx_tail_sum += a;
        if cumulative_precise >= viewport_height && a > 0.0 {
            let intra_precise = cumulative_precise - viewport_height;
            let slope = p / a;
            let intra_approx = if slope > 0.0 { intra_precise / slope } else { 0.0 };
            return (approx_total - approx_tail_sum + intra_approx).max(0.0);
        }
    }
    0.0
}

/// Walks rows until cumulative approx >= `offset_approx` to find the
/// anchor row. Then paints anchor and downstream rows at consecutive
/// precise positions. Anchor's intra-position is placed at the viewport
/// top in screen space.
fn render_visible<R: Rows>(ui: &mut Ui, content: &mut R, viewport: Rect, offset_approx: f32) {
    content.reset();

    let mut acc = 0.0f32;
    let mut anchor_p = 0.0f32;
    let mut intra_precise = 0.0f32;
    let mut found = false;
    while content.next() {
        let h = content.approx();
        if acc + h > offset_approx {
            anchor_p = content.precise();
            let anchor_a = h;
            let intra_approx = offset_approx - acc;
            let slope = if anchor_a > 0.0 { anchor_p / anchor_a } else { 1.0 };
            intra_precise = intra_approx * slope;
            content.render(ui, Pos2::new(viewport.min.x, viewport.min.y - intra_precise));
            found = true;
            break;
        }
        acc += h;
    }
    if !found {
        return;
    }

    let mut y = -intra_precise + anchor_p;
    while y < viewport.height() && content.next() {
        let p = content.precise();
        content.render(ui, Pos2::new(viewport.min.x, viewport.min.y + y));
        y += p;
    }

    // Warm a viewport's worth past the bottom edge so images/etc.
    // start loading before the user scrolls them in.
    let warm_until = y + viewport.height();
    while y < warm_until && content.next() {
        y += content.precise();
        content.warm();
    }

    // Warm a viewport's worth above the rendered region. Re-walk
    // forward to the anchor (cheap; approx access is O(1) per row),
    // then prev() warming until we've covered one viewport.
    content.reset();
    let mut acc = 0.0f32;
    while content.next() {
        let h = content.approx();
        if acc + h > offset_approx {
            break;
        }
        acc += h;
    }
    let mut warm_back = 0.0f32;
    while warm_back < viewport.height() && content.prev() {
        warm_back += content.precise();
        content.warm();
    }
}

/// Translate a precise delta (screen pixels of intended movement) into
/// an approx delta to apply to the scrollbar offset. Walks rows
/// starting at `offset_approx` consuming precise units; each row
/// contributes at its approx/precise ratio.
fn precise_to_approx_delta<R: Rows>(
    content: &mut R, offset_approx: f32, precise_delta: f32,
) -> f32 {
    if precise_delta == 0.0 {
        return 0.0;
    }
    content.reset();

    let mut acc = 0.0f32;
    let mut precise_remaining = precise_delta.abs();
    let mut approx_consumed = 0.0f32;
    let mut found = false;
    while content.next() {
        let approx_h = content.approx();
        if acc + approx_h > offset_approx {
            let intra_approx = offset_approx - acc;
            let precise_h = content.precise();
            let slope = if approx_h > 0.0 { precise_h / approx_h } else { 1.0 };
            let approx_remaining_in_row = if precise_delta > 0.0 {
                (approx_h - intra_approx).max(0.0)
            } else {
                intra_approx.max(0.0)
            };
            let precise_remaining_in_row = approx_remaining_in_row * slope;
            if precise_remaining <= precise_remaining_in_row && slope > 0.0 {
                let signed = precise_remaining / slope;
                return if precise_delta > 0.0 { signed } else { -signed };
            }
            approx_consumed = approx_remaining_in_row;
            precise_remaining -= precise_remaining_in_row;
            found = true;
            break;
        }
        acc += approx_h;
    }
    if !found {
        return 0.0;
    }

    // Past the anchor every subsequent row is consumed across its
    // full approx span.
    loop {
        let stepped = if precise_delta > 0.0 { content.next() } else { content.prev() };
        if !stepped {
            return if precise_delta > 0.0 { approx_consumed } else { -approx_consumed };
        }
        let approx_h = content.approx();
        let precise_h = content.precise();
        let slope = if approx_h > 0.0 { precise_h / approx_h } else { 1.0 };
        let precise_in_row = approx_h * slope;
        if precise_remaining <= precise_in_row && slope > 0.0 {
            approx_consumed += precise_remaining / slope;
            return if precise_delta > 0.0 { approx_consumed } else { -approx_consumed };
        }
        approx_consumed += approx_h;
        precise_remaining -= precise_in_row;
        if precise_remaining <= 0.0 {
            return if precise_delta > 0.0 { approx_consumed } else { -approx_consumed };
        }
    }
}

fn draw_scrollbar(
    ui: &Ui, viewport: Rect, offset_approx: f32, approx_total: f32, viewport_height: f32,
) {
    use crate::theme::palette_v2::ThemeExt as _;
    // Must match the dimensions in `show()` — the scrollbar's hit area
    // is allocated there, this only paints into it.
    const BAR_WIDTH: f32 = 10.0;
    const BAR_INSET: f32 = 3.0;

    let theme = ui.ctx().get_lb_theme();
    // Track: lerp neutral_bg toward neutral (a darker tone) — barely
    // visible, just enough to anchor the thumb. Thumb: pure neutral
    // (grey in either mode), prominent against the bg.
    let track_color = theme.neutral_bg().lerp_to_gamma(theme.neutral(), 0.3);
    let thumb_color = theme.neutral();

    let bar_x = viewport.max.x - BAR_WIDTH - BAR_INSET;
    let bar_track = Rect::from_min_size(
        Pos2::new(bar_x, viewport.min.y),
        Vec2::new(BAR_WIDTH, viewport.height()),
    );
    let visible_fraction = (viewport_height / approx_total).clamp(0.0, 1.0);
    let offset_fraction = (offset_approx / approx_total).clamp(0.0, 1.0 - visible_fraction);
    let thumb = Rect::from_min_size(
        Pos2::new(bar_x, viewport.min.y + offset_fraction * viewport.height()),
        Vec2::new(BAR_WIDTH, visible_fraction * viewport.height()),
    );
    ui.painter().rect_filled(bar_track, 3.0, track_color);
    ui.painter()
        .rect(thumb, 3.0, thumb_color, Stroke::NONE, egui::epaint::StrokeKind::Inside);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Synthetic content for testing the affine scroll math. Records
    /// rendered rows so tests can assert positioning.
    ///
    /// `cursor` semantics: `None` = before first; `Some(i)` for `i <
    /// len` = at row i; `Some(len)` = after last.
    struct MockContent {
        approx: Vec<f32>,
        precise: Vec<f32>,
        rendered: Vec<(usize, Pos2)>,
        cursor: Option<usize>,
    }

    impl MockContent {
        fn new(rows: Vec<(f32, f32)>) -> Self {
            let approx = rows.iter().map(|(a, _)| *a).collect();
            let precise = rows.iter().map(|(_, p)| *p).collect();
            Self { approx, precise, rendered: Vec::new(), cursor: None }
        }
        fn at(&self) -> Option<usize> {
            match self.cursor {
                Some(i) if i < self.approx.len() => Some(i),
                _ => None,
            }
        }
    }

    impl Rows for MockContent {
        fn reset(&mut self) {
            self.cursor = None;
        }
        fn reset_back(&mut self) {
            self.cursor = Some(self.approx.len());
        }
        fn next(&mut self) -> bool {
            let n = self.approx.len();
            self.cursor = match self.cursor {
                None => Some(0),
                Some(i) if i < n => Some(i + 1),
                Some(i) => Some(i),
            };
            self.at().is_some()
        }
        fn prev(&mut self) -> bool {
            let n = self.approx.len();
            self.cursor = match self.cursor {
                Some(0) => None,
                Some(i) if i <= n => Some(i - 1),
                Some(_) => Some(n.saturating_sub(1)),
                None => None,
            };
            self.at().is_some()
        }
        fn approx(&self) -> f32 {
            self.approx[self.at().expect("approx() not at row")]
        }
        fn precise(&mut self) -> f32 {
            self.precise[self.at().expect("precise() not at row")]
        }
        fn render(&mut self, _: &mut Ui, top_left: Pos2) {
            let i = self.at().expect("render() not at row");
            self.rendered.push((i, top_left));
        }
    }

    #[test]
    fn precise_to_approx_within_single_row() {
        let mut c = MockContent::new(vec![(50.0, 100.0)]);
        let approx_delta = precise_to_approx_delta(&mut c, 0.0, 30.0);
        assert!((approx_delta - 15.0).abs() < 0.01, "got {}", approx_delta);
    }

    #[test]
    fn precise_to_approx_crossing_row_boundary() {
        let mut c = MockContent::new(vec![(50.0, 100.0), (50.0, 50.0)]);
        let approx_delta = precise_to_approx_delta(&mut c, 0.0, 150.0);
        assert!((approx_delta - 100.0).abs() < 0.01, "got {}", approx_delta);
    }

    #[test]
    fn precise_to_approx_negative() {
        let mut c = MockContent::new(vec![(50.0, 100.0)]);
        let approx_delta = precise_to_approx_delta(&mut c, 30.0, -30.0);
        assert!((approx_delta + 15.0).abs() < 0.01, "got {}", approx_delta);
    }

    /// 10 rows of 50px each, viewport=100px (2 rows visible). Target
    /// is row 5 → Top alignment offset=250, Bottom alignment offset=200.
    #[test]
    fn make_visible_no_scroll_when_target_already_in_view() {
        let mut c = MockContent::new(vec![(50.0, 50.0); 10]);
        let result = make_visible_offset(&mut c, 100.0, 225.0, |c| c.at() == Some(5));
        assert_eq!(result, None, "should not scroll when target is visible");
    }

    #[test]
    fn make_visible_scrolls_down_when_target_below() {
        let mut c = MockContent::new(vec![(50.0, 50.0); 10]);
        let result = make_visible_offset(&mut c, 100.0, 0.0, |c| c.at() == Some(5));
        let o = result.expect("should scroll");
        assert!((o - 200.0).abs() < 0.5, "expected ~200, got {o}");
    }

    #[test]
    fn make_visible_scrolls_up_when_target_above() {
        let mut c = MockContent::new(vec![(50.0, 50.0); 10]);
        let result = make_visible_offset(&mut c, 100.0, 400.0, |c| c.at() == Some(5));
        let o = result.expect("should scroll");
        assert!((o - 250.0).abs() < 0.5, "expected ~250, got {o}");
    }

    use rand::{Rng, SeedableRng, rngs::StdRng};

    fn random_rows(rng: &mut StdRng, n: usize) -> Vec<(f32, f32)> {
        (0..n)
            .map(|_| {
                let approx = rng.gen_range(10.0..200.0);
                let ratio = rng.gen_range(0.3..3.0);
                (approx, approx * ratio)
            })
            .collect()
    }

    /// Reference: visible (idx, screen_y) pairs at offset. Used by
    /// property tests.
    fn visible_row_positions<R: Rows>(
        content: &mut R, viewport_height: f32, offset_approx: f32,
    ) -> Vec<(usize, f32)> {
        content.reset();
        let mut acc = 0.0f32;
        let mut idx = 0usize;
        let mut anchor_p = 0.0f32;
        let mut intra_precise = 0.0f32;
        let mut out = Vec::new();
        let mut found = false;
        while content.next() {
            let h = content.approx();
            if acc + h > offset_approx {
                anchor_p = content.precise();
                let intra_approx = offset_approx - acc;
                let slope = if h > 0.0 { anchor_p / h } else { 1.0 };
                intra_precise = intra_approx * slope;
                out.push((idx, -intra_precise));
                found = true;
                break;
            }
            acc += h;
            idx += 1;
        }
        if !found {
            return out;
        }
        let mut y = -intra_precise + anchor_p;
        while y < viewport_height && content.next() {
            idx += 1;
            let p = content.precise();
            out.push((idx, y));
            y += p;
        }
        out
    }

    /// Property: after submitting a scroll event of `precise_delta`,
    /// rows visible in both the before and after renders shift by
    /// exactly `precise_delta` in screen space.
    ///
    /// This is the core invariant the affine scroll area exists to
    /// satisfy. If broken, scrolling produces visible "jumps" at row
    /// boundaries.
    #[test]
    fn property_scroll_delta_preserved_in_screen_space() {
        const EPS: f32 = 0.01;
        let viewport_height = 200.0;
        let mut rng = StdRng::seed_from_u64(0);
        for seed in 0..2048u64 {
            let mut rng_inner = StdRng::seed_from_u64(seed);
            let n_rows = rng_inner.gen_range(1..20);
            let rows = random_rows(&mut rng_inner, n_rows);
            let mut c = MockContent::new(rows.clone());

            let approx_total: f32 = rows.iter().map(|(a, _)| *a).sum();
            if approx_total <= viewport_height {
                continue;
            }
            let max_offset = approx_total - viewport_height;
            let offset_a: f32 = rng.gen_range(0.0..=max_offset);
            let precise_delta: f32 = rng.gen_range(-150.0..=150.0);

            let approx_delta = precise_to_approx_delta(&mut c, offset_a, precise_delta);
            let offset_b = (offset_a + approx_delta).clamp(0.0, max_offset);

            let effective_approx_delta = offset_b - offset_a;
            let effective_precise_delta =
                approx_to_precise_delta(&mut c, offset_a, effective_approx_delta);

            let before = visible_row_positions(&mut c, viewport_height, offset_a);
            let after = visible_row_positions(&mut c, viewport_height, offset_b);

            for (idx_a, y_a) in &before {
                if let Some(&(_, y_b)) = after.iter().find(|(i, _)| i == idx_a) {
                    let diff = y_b - y_a;
                    let expected = -effective_precise_delta;
                    assert!(
                        (diff - expected).abs() < EPS,
                        "seed {seed}: row {idx_a} shifted by {diff}, expected {expected} \
                         (offset {offset_a} → {offset_b}, precise {precise_delta}, \
                         effective precise {effective_precise_delta})",
                    );
                }
            }
        }
    }

    /// Inverse of `precise_to_approx_delta`: given an approx delta,
    /// returns the precise distance covered. Used in tests.
    fn approx_to_precise_delta<R: Rows>(
        content: &mut R, offset_approx: f32, approx_delta: f32,
    ) -> f32 {
        if approx_delta == 0.0 {
            return 0.0;
        }
        content.reset();
        let mut acc = 0.0f32;
        let mut precise_consumed = 0.0f32;
        let mut approx_remaining = approx_delta.abs();
        let mut found = false;
        while content.next() {
            let approx_h = content.approx();
            if acc + approx_h > offset_approx {
                let intra_approx = offset_approx - acc;
                let precise_h = content.precise();
                let slope = if approx_h > 0.0 { precise_h / approx_h } else { 1.0 };
                let approx_remaining_in_row = if approx_delta > 0.0 {
                    (approx_h - intra_approx).max(0.0)
                } else {
                    intra_approx.max(0.0)
                };
                if approx_remaining <= approx_remaining_in_row {
                    let signed = approx_remaining * slope;
                    return if approx_delta > 0.0 { signed } else { -signed };
                }
                precise_consumed = approx_remaining_in_row * slope;
                approx_remaining -= approx_remaining_in_row;
                found = true;
                break;
            }
            acc += approx_h;
        }
        if !found {
            return 0.0;
        }

        loop {
            let stepped = if approx_delta > 0.0 { content.next() } else { content.prev() };
            if !stepped {
                return if approx_delta > 0.0 { precise_consumed } else { -precise_consumed };
            }
            let approx_h = content.approx();
            let precise_h = content.precise();
            let slope = if approx_h > 0.0 { precise_h / approx_h } else { 1.0 };
            if approx_remaining <= approx_h {
                precise_consumed += approx_remaining * slope;
                return if approx_delta > 0.0 { precise_consumed } else { -precise_consumed };
            }
            precise_consumed += approx_h * slope;
            approx_remaining -= approx_h;
            if approx_remaining <= 0.0 {
                return if approx_delta > 0.0 { precise_consumed } else { -precise_consumed };
            }
        }
    }

    /// Property: precise→approx and approx→precise are inverses (for
    /// deltas that don't run off the end of the doc).
    #[test]
    fn property_affine_map_invertible() {
        const EPS: f32 = 0.01;
        let mut rng = StdRng::seed_from_u64(42);
        for seed in 0..2048u64 {
            let mut rng_inner = StdRng::seed_from_u64(seed);
            let n_rows = rng_inner.gen_range(1..20);
            let rows = random_rows(&mut rng_inner, n_rows);
            let mut c = MockContent::new(rows.clone());
            let approx_total: f32 = rows.iter().map(|(a, _)| *a).sum();
            let offset: f32 = rng.gen_range(0.0..=approx_total.max(1.0));
            let approx_delta: f32 = rng.gen_range(-100.0..=100.0);

            let new_offset = offset + approx_delta;
            const EDGE: f32 = 5.0;
            if new_offset < EDGE || new_offset > approx_total - EDGE {
                continue;
            }

            let precise = approx_to_precise_delta(&mut c, offset, approx_delta);
            let back = precise_to_approx_delta(&mut c, offset, precise);
            assert!(
                (back - approx_delta).abs() < EPS,
                "seed {seed}: approx→precise→approx not identity ({approx_delta} → {precise} → {back})",
            );
        }
    }
}
