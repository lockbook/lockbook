//! Vertical scroll area driven by a piecewise-affine map between cheap
//! per-row `approx` heights (used by the scrollbar) and expensive
//! per-row `precise` heights (used by layout).
//!
//! Pixel-precise scroll input is exact: scrolling by N pixels moves
//! content by exactly N pixels. The scrollbar coordinate is in approx
//! units; the slope `precise/approx` only enters scrollbar math, so
//! scrollbar accuracy degrades gracefully as the approximation worsens
//! while the scroll-input feel stays correct.
//!
//! # Layout
//!
//! - [`Rows`] + [`Offset`] are the public content surface. Id-keyed and
//!   `&self`: the trait is a content provider, not a state machine.
//! - [`ScrollArea<Id>`] holds offset + viewport + touch-momentum state and
//!   exposes the affine math via id-typed methods that take a [`Rows`]
//!   impl by reference. Pure data — no egui dependency.
//! - [`AffineScrollArea<Id>`] is the egui adapter: owns a
//!   [`ScrollArea<Id>`] as a field, layers wheel / touch-drag /
//!   scrollbar-drag input + momentum on top. Embed in the type that
//!   owns the scrollable view.
//! - [`Reveal`] + [`Align`] is the make-visible API.
//!
//! # Behaviors
//!
//! ## Scroll input
//!
//! - Wheel / trackpad scroll: 1 px of input moves content by 1 px,
//!   regardless of `approx`/`precise` drift.
//! - Touch body drag (when `touch_scroll = true`): content tracks
//!   finger motion 1:1.
//! - Touch fling momentum: after release, content coasts with
//!   exponential decay; cancelled by tap, drag-start on body or
//!   scrollbar, programmatic scroll, or hitting a scroll boundary.
//! - A tap that cancels momentum is consumed by the scroll area; it
//!   doesn't also place the cursor or toggle interactive elements
//!   (fold buttons, task-list checkboxes). Embedders gate other touch
//!   handlers on [`AffineScrollArea::velocity`].
//! - Scrollbar thumb drag: 1 px of mouse-pointer motion ≈ 1 px of
//!   thumb motion. The drag rate is in approx units, so it drifts
//!   gracefully when per-row `approx`/`precise` is off.
//! - Scrollbar track click: thumb snaps so its center lands at the
//!   click; offset moves accordingly.
//!
//! ## Programmatic positioning
//!
//! - [`AffineScrollArea::set_offset`] / [`Action::ScrollToTop`] /
//!   [`Action::ScrollToBottom`]: jump and clamp.
//! - [`AffineScrollArea::reveal`] with [`Align::Top`] / [`Align::Bottom`]:
//!   align the rect's edge with the corresponding viewport edge.
//! - [`Align::Center`]: rect's vertical midpoint at viewport center.
//! - [`Align::Nearest`]: no-op if the rect is fully visible; otherwise
//!   minimum motion to bring it in.
//! - For rects taller than the viewport (no in-viewport position
//!   shows the whole rect), all alignments prefer the rect's top.
//! - Typical use: cursor scroll uses `Nearest`; find-match scroll
//!   uses `Center`.
//!
//! ## Rendering
//!
//! - Pull-style: [`AffineScrollArea::show`] returns rows in viewport
//!   order with per-row top + height in viewport-local coordinates.
//!   Embedders paint themselves; the widget never invokes a paint
//!   callback.
//! - Warm window: [`Rows::warm`] is called for rows in a
//!   viewport-sized window above and below the visible region — gives
//!   image caches and similar resources a head start.
//!
//! ## Scrollbar
//!
//! - Thumb size proportional to the visible fraction; floored at
//!   [`MIN_THUMB_PX`] for grabability; fills the track when the doc
//!   fits the viewport.
//! - Thumb position proportional to scroll progress in approx
//!   coordinate.
//! - Track + thumb expressed in window-local pixels; embedder chooses
//!   track placement and width.
//!
//! ## Persistence
//!
//! - [`AffineScrollArea::stored_offset`] returns the raw persisted
//!   anchor without row projection — for change detection and
//!   serialization.
//! - On read ([`AffineScrollArea::offset`]), the stored anchor is
//!   projected onto current rows: live anchor with valid intra-row
//!   position → as-is; intra-row position larger than the row's
//!   current `precise` (row shrunk under us) → walk forward by the
//!   excess, advancing the anchor; anchor's row deleted entirely →
//!   consult [`Rows::recover_anchor`].
//!
//! ## Doc-edit resilience
//!
//! - The scroll area holds no layout cache. Every read of `offset`,
//!   `visible`, or `scrollbar` walks `Rows::approx` / `Rows::precise`
//!   fresh, so changes to row heights (image load, syntax reveal,
//!   width change, edits) are picked up without an invalidation step.
//! - Rows added or removed mid-doc: the projection above keeps the
//!   user near where they were if the anchor is still valid; if the
//!   anchor's row is gone, [`Rows::recover_anchor`] supplies the new
//!   home.
//! - Empty rows: `offset` returns `None`, `visible` is empty, actions
//!   are safe no-ops.
//! - Resize: persisted offset re-clamps against the new viewport on
//!   the next read.
//!
//! ## Performance budget
//!
//! - Per frame: O(`precise()` × viewport-rows + `approx()` × doc-rows).
//!   Document size enters only through the cheap `approx()` walk
//!   (constant per row, no layout work).
//! - Every function calling `rows.precise(id)` takes a budget that
//!   bounds its walk — a `bound: f32` argument, the `delta` of
//!   `scroll_by`, or the row-shrink excess in `normalize`.

use egui::{Pos2, Rect, Response, Sense, Stroke, Ui, Vec2};
use std::hash::Hash;

// ============================================================================
// Trait + offset
// ============================================================================

/// Source of vertical content for a [`ScrollArea`].
///
/// Cursor-free: every method takes a [`RowId`](Rows::RowId), so the trait
/// is stateless about position. Caches and laid-out content live behind
/// the impl's interior mutability if needed.
///
/// # Contract
///
/// - `approx(id) == 0.0  ⟺  precise(id) == 0.0` — zero-set agreement.
///   A row that contributes nothing in approx must contribute nothing in
///   precise, and vice versa, or the scrollbar/scroll-input mapping
///   becomes ambiguous (div-by-zero, infinite slope).
/// - `approx(id)` is constant per row — must not depend on layout work
///   (text shaping, image loading) that hasn't happened yet. Drift
///   between approx and precise across edits is fine; behaviour
///   degrades smoothly as the approximation worsens.
/// - `next(prev(id)) == Some(id)` and `prev(next(id)) == Some(id)`
///   when both sides are `Some` — the row sequence is a doubly-linked
///   walk.
/// - **`next(id)` and `prev(id)` return `None` if `id` is not currently
///   in the row sequence** — whether because `id` is at the boundary,
///   *or* because `id` was once in the sequence but has since been
///   removed. Callers (and [`ScrollArea`]) lean on this to detect a
///   stale anchor after external mutation.
///
/// Behaviour when contracts are violated is bounded by the slope-band
/// clamp on `approx / precise` per row: the scrollbar may drift, but
/// `ScrollArea` will not panic.
///
/// # Choosing a `RowId`
///
/// The trait works with two natural design styles. They differ only
/// in what kind of stability the id has across edits.
///
/// **Index-based** (`usize` into a `Vec`). Cheap to compute, cheap to
/// compare, and the scroll position drifts gracefully under
/// edits-above-the-anchor: insertions and deletions shift surrounding
/// indices, so the anchor's id changes meaning to refer to whatever
/// row took over its position — visually, the user stays put. Goes
/// stale only when the index runs off the end (e.g., the doc shrunk
/// past the saved anchor). Override [`recover_anchor`](Rows::recover_anchor)
/// to clamp to the new last index rather than the default `first()`.
///
/// **Stable identity** (hash, generational handle, AST node pointer).
/// The id refers to *the same content* regardless of edits to other
/// rows; the row only goes stale when its content is deleted. The
/// scroll position drifts under edits-above-the-anchor (the row's
/// physical y changes as siblings are added/removed), but if the
/// anchor row itself moves, the user follows it. The default
/// [`recover_anchor`](Rows::recover_anchor) (returning `first()`) is
/// usually right.
///
/// Pick based on which property your UI wants to express: stable
/// *position* (index) or stable *content reference* (identity).
pub trait Rows {
    /// Stable identity of a row. `Clone` rather than `Copy` so non-`Copy`
    /// ids — `String` hashes, `Arc<Path>` keys, etc. — can be used
    /// directly without a sidecar mapping.
    type RowId: Clone + Eq + std::fmt::Debug;

    fn first(&self) -> Option<Self::RowId>;
    fn last(&self) -> Option<Self::RowId>;
    fn next(&self, id: &Self::RowId) -> Option<Self::RowId>;
    fn prev(&self, id: &Self::RowId) -> Option<Self::RowId>;

    /// Cheap height. Constant per row — must not depend on layout work.
    fn approx(&self, id: &Self::RowId) -> f32;

    /// Expensive height. Computed only for rows the widget paints.
    fn precise(&self, id: &Self::RowId) -> f32;

    /// Hint that the row is about to enter the viewport. Default no-op.
    fn warm(&self, _id: &Self::RowId) {}

    /// Called by the scroll area when a stored anchor is no longer in
    /// the row sequence — i.e. the anchor's row has been removed and
    /// neither `next` nor `prev` can locate it. The returned `RowId`
    /// is placed at viewport top, then clamped against `max_offset`.
    /// `None` means "drop the offset entirely" (treats rows as empty).
    ///
    /// Default returns `first()`, suitable for stable-id row impls
    /// where a deleted anchor is genuinely gone and the user is best
    /// returned to the top. Index-based impls (whose id "drifts" with
    /// surrounding edits and only goes stale at the tail end) typically
    /// override to return the new `last()`.
    fn recover_anchor(&self, _stale: &Self::RowId) -> Option<Self::RowId> {
        self.first()
    }
}

/// Anchored scroll position. The top of the viewport sits at
/// `intra_precise` precise pixels below the top of the row identified
/// by `anchor`.
///
/// Anchor identity survives content edits that don't remove the anchor
/// row. If the anchor is deleted, `Rows::next(anchor)` and
/// `Rows::prev(anchor)` return `None`; widget callers detect this and
/// fall back to a sentinel offset.
#[derive(Debug, Clone, PartialEq)]
pub struct Offset<Id> {
    pub anchor: Id,
    pub intra_precise: f32,
}

impl<Id> Offset<Id> {
    pub fn new(anchor: Id, intra_precise: f32) -> Self {
        Self { anchor, intra_precise }
    }

    pub fn at_top_of(anchor: Id) -> Self {
        Self { anchor, intra_precise: 0.0 }
    }
}

// ============================================================================
// Affine math (pure functions over Rows)
// ============================================================================
//
// Cost-class invariant: every function in this module that calls
// `rows.precise(id)` takes either a `bound: f32` (precise pixels) or
// the equivalent budget as one of its inputs (`scroll_by`'s `delta`,
// `normalize`'s row-shrink excess). No public function performs an
// unbounded `precise()` walk.

mod affine {
    use super::{Offset, Rows};

    /// Apply a precise-pixel scroll delta to an offset. Pure precise math
    /// — slope never enters here. Clamps at document edges.
    pub fn scroll_by<R: Rows>(rows: &R, mut off: Offset<R::RowId>, delta: f32) -> Offset<R::RowId> {
        if delta > 0.0 {
            let mut remaining = delta;
            loop {
                let row_precise = rows.precise(&off.anchor);
                let in_row_left = (row_precise - off.intra_precise).max(0.0);
                if remaining < in_row_left {
                    off.intra_precise += remaining;
                    return off;
                }
                match rows.next(&off.anchor) {
                    Some(next_id) => {
                        remaining -= in_row_left;
                        off.anchor = next_id;
                        off.intra_precise = 0.0;
                    }
                    None => {
                        off.intra_precise = row_precise;
                        return off;
                    }
                }
            }
        } else if delta < 0.0 {
            let mut remaining = -delta;
            loop {
                if remaining <= off.intra_precise {
                    off.intra_precise -= remaining;
                    return off;
                }
                match rows.prev(&off.anchor) {
                    Some(prev_id) => {
                        remaining -= off.intra_precise;
                        let prev_precise = rows.precise(&prev_id);
                        off.anchor = prev_id;
                        off.intra_precise = prev_precise;
                    }
                    None => {
                        off.intra_precise = 0.0;
                        return off;
                    }
                }
            }
        } else {
            off
        }
    }

    pub enum Direction {
        Forward,
        Backward,
    }

    /// Cheap structural probe — does not call `precise()`. Walks
    /// forward and backward simultaneously from `a_anchor` until one
    /// side hits `b_anchor`. Returns `None` if `b_anchor` isn't in the
    /// row sequence.
    pub fn probe_direction<R: Rows>(
        rows: &R, a_anchor: &R::RowId, b_anchor: &R::RowId,
    ) -> Option<Direction> {
        if a_anchor == b_anchor {
            return None;
        }
        let mut fwd = a_anchor.clone();
        let mut bwd = a_anchor.clone();
        let mut fwd_alive = true;
        let mut bwd_alive = true;
        while fwd_alive || bwd_alive {
            if fwd_alive {
                match rows.next(&fwd) {
                    Some(next) if &next == b_anchor => return Some(Direction::Forward),
                    Some(next) => fwd = next,
                    None => fwd_alive = false,
                }
            }
            if bwd_alive {
                match rows.prev(&bwd) {
                    Some(prev) if &prev == b_anchor => return Some(Direction::Backward),
                    Some(prev) => bwd = prev,
                    None => bwd_alive = false,
                }
            }
        }
        None
    }

    /// Precise-pixel distance forward from `from` to `to`, walking
    /// `precise()` per row up to `bound` precise pixels. Returns
    /// `None` if `to` isn't reached forward within `bound`.
    pub fn precise_distance_forward<R: Rows>(
        rows: &R, from: &Offset<R::RowId>, to: &Offset<R::RowId>, bound: f32,
    ) -> Option<f32> {
        if from.anchor == to.anchor {
            let d = to.intra_precise - from.intra_precise;
            return (d >= 0.0).then_some(d);
        }
        let mut id = from.anchor.clone();
        let mut dist = -from.intra_precise;
        loop {
            dist += rows.precise(&id);
            if dist > bound {
                return None;
            }
            match rows.next(&id) {
                Some(next_id) => {
                    if next_id == to.anchor {
                        return Some(dist + to.intra_precise);
                    }
                    id = next_id;
                }
                None => return None,
            }
        }
    }

    /// Signed precise-pixel distance from `a` to `b`, bounded by
    /// `bound`. Positive if `b` is below `a`. Returns `None` if `b`
    /// isn't within `bound` of `a` in either direction, or if `b`'s
    /// anchor isn't in the row sequence.
    ///
    /// Implementation: a cheap [`probe_direction`] (no `precise()`
    /// calls) decides which side to walk, then exactly one
    /// [`precise_distance_forward`] runs in that direction.
    pub fn precise_distance<R: Rows>(
        rows: &R, a: &Offset<R::RowId>, b: &Offset<R::RowId>, bound: f32,
    ) -> Option<f32> {
        if a.anchor == b.anchor {
            let d = b.intra_precise - a.intra_precise;
            return (d.abs() <= bound).then_some(d);
        }
        match probe_direction(rows, &a.anchor, &b.anchor)? {
            Direction::Forward => precise_distance_forward(rows, a, b, bound),
            Direction::Backward => precise_distance_forward(rows, b, a, bound).map(|d| -d),
        }
    }

    /// Offset whose distance from `top` equals its distance from
    /// `bottom`. `None` if the rect is taller than `bound` precise
    /// pixels — caller should fall back (e.g. align Top) since no
    /// midpoint exists within the budget.
    pub fn midpoint<R: Rows>(
        rows: &R, top: Offset<R::RowId>, bottom: Offset<R::RowId>, bound: f32,
    ) -> Option<Offset<R::RowId>> {
        let dist = precise_distance(rows, &top, &bottom, bound)?;
        Some(scroll_by(rows, top, dist / 2.0))
    }

    /// Allowed range for `approx / precise` per row in scrollbar math.
    /// Out-of-band rows (zero-set agreement violated, or cheap
    /// estimate is way off true layout) substitute the clamped slope
    /// — scrollbar geometry drifts proportionally, scroll-input
    /// fidelity is unaffected.
    const SLOPE_BAND: (f32, f32) = (1e-3, 1e3);

    /// Position of the scroll thumb in approx coordinate. Walks rows
    /// above the anchor for cumulative approx; uses local slope inside
    /// the anchor row.
    pub fn thumb_approx<R: Rows>(rows: &R, off: &Offset<R::RowId>) -> f32 {
        let mut acc = 0.0;
        let mut id = off.anchor.clone();
        while let Some(prev_id) = rows.prev(&id) {
            acc += rows.approx(&prev_id);
            id = prev_id;
        }
        let row_precise = rows.precise(&off.anchor);
        let row_approx = rows.approx(&off.anchor);
        let intra_approx = if row_precise > 0.0 {
            let slope = (row_approx / row_precise).clamp(SLOPE_BAND.0, SLOPE_BAND.1);
            off.intra_precise * slope
        } else {
            0.0
        };
        acc + intra_approx
    }

    /// Inverse of `thumb_approx`.
    pub fn from_thumb<R: Rows>(rows: &R, approx_pos: f32) -> Option<Offset<R::RowId>> {
        let first = rows.first()?;
        if approx_pos <= 0.0 {
            return Some(Offset::at_top_of(first));
        }
        let mut id = first;
        let mut acc = 0.0;
        loop {
            let row_approx = rows.approx(&id);
            if acc + row_approx >= approx_pos {
                let intra_approx = approx_pos - acc;
                let row_precise = rows.precise(&id);
                let intra_precise = if row_approx > 0.0 {
                    let inv_slope =
                        (row_precise / row_approx).clamp(1.0 / SLOPE_BAND.1, 1.0 / SLOPE_BAND.0);
                    intra_approx * inv_slope
                } else {
                    0.0
                };
                return Some(Offset { anchor: id, intra_precise });
            }
            match rows.next(&id) {
                Some(next_id) => {
                    acc += row_approx;
                    id = next_id;
                }
                None => {
                    let last_precise = rows.precise(&id);
                    return Some(Offset { anchor: id, intra_precise: last_precise });
                }
            }
        }
    }

    /// Total approx — walks the full row sequence. Cheap per-row but
    /// O(N). Override-friendly slot for impls that maintain their own
    /// counter; for now the walk is good enough.
    pub fn total_approx<R: Rows>(rows: &R) -> f32 {
        let mut acc = 0.0;
        let mut id = match rows.first() {
            Some(id) => id,
            None => return 0.0,
        };
        loop {
            acc += rows.approx(&id);
            match rows.next(&id) {
                Some(next_id) => id = next_id,
                None => return acc,
            }
        }
    }

    /// Approx-coordinate extent of one viewport's worth of content
    /// starting from `first()`. Used as the thumb-sizing numerator —
    /// gives a stable, scroll-position-independent measure of "how
    /// much approx fits in a viewport". Bounded precise walk.
    pub fn viewport_extent_in_approx<R: Rows>(rows: &R, viewport_height: f32) -> f32 {
        let Some(mut id) = rows.first() else {
            return 0.0;
        };
        let mut precise_acc = 0.0;
        let mut approx_acc = 0.0;
        loop {
            let p = rows.precise(&id);
            let a = rows.approx(&id);
            if precise_acc + p >= viewport_height {
                let remaining = viewport_height - precise_acc;
                let slope = if p > 0.0 { a / p } else { 0.0 };
                return approx_acc + remaining * slope;
            }
            precise_acc += p;
            approx_acc += a;
            match rows.next(&id) {
                Some(next_id) => id = next_id,
                None => return approx_acc,
            }
        }
    }
}

// ============================================================================
// Public types: Action, VisibleRow, HitRow, Reveal, Align, Scrollbar
// ============================================================================

#[derive(Debug, Clone)]
pub enum Action {
    /// Scroll content by `delta` precise pixels. Positive = down.
    ScrollByPixels(f32),
    /// Drag the scrollbar thumb by `delta` approx units. Positive =
    /// thumb moves down.
    ScrollByThumb(f32),
    ScrollToTop,
    ScrollToBottom,
    Resize(f32),
}

/// One row in the visible window emitted by [`ScrollArea::visible`].
///
/// `top` is in viewport-local coordinates (`0.0` is the viewport's top
/// edge). The first row's `top` is `<= 0.0` when offset sits mid-row;
/// the last row's `top + height` may exceed `viewport_height`. Caller
/// clips at the viewport rect.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VisibleRow<Id> {
    pub id: Id,
    pub top: f32,
    pub height: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HitRow<Id> {
    pub id: Id,
    pub intra_y: f32,
}

/// A rect to reveal via [`ScrollArea::reveal`]. `top == bottom` is a
/// valid "point" reveal (cursor-style).
#[derive(Debug, Clone, PartialEq)]
pub struct Reveal<Id> {
    pub top: Offset<Id>,
    pub bottom: Offset<Id>,
}

impl<Id: Clone> Reveal<Id> {
    pub fn point_at(off: Offset<Id>) -> Self {
        Self { top: off.clone(), bottom: off }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Top,
    Bottom,
    Center,
    /// No-op when the rect is fully visible. Otherwise minimum motion;
    /// rect taller than viewport falls through to [`Top`](Align::Top).
    Nearest,
}

#[derive(Debug, Clone, Copy)]
pub struct Scrollbar {
    pub track: Rect,
    pub thumb: Rect,
    pub total_approx: f32,
    pub thumb_approx: f32,
    pub scrollable_approx: f32,
}

impl Scrollbar {
    pub fn hit(&self, y: f32) -> ScrollbarHit {
        if y < self.track.min.y || y >= self.track.max.y {
            return ScrollbarHit::None;
        }
        if y >= self.thumb.min.y && y <= self.thumb.max.y {
            ScrollbarHit::Thumb
        } else if y < self.thumb.min.y {
            ScrollbarHit::TrackAbove
        } else {
            ScrollbarHit::TrackBelow
        }
    }

    /// Convert a pixel delta on the track into an approx delta to feed
    /// [`Action::ScrollByThumb`]. Returns `0.0` when the track is
    /// degenerate or the document doesn't scroll.
    pub fn pixel_to_approx(&self, pixel_delta: f32) -> f32 {
        let movable = self.track.height() - self.thumb.height();
        if movable <= 0.0 || self.scrollable_approx <= 0.0 {
            return 0.0;
        }
        pixel_delta * self.scrollable_approx / movable
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollbarHit {
    Thumb,
    TrackAbove,
    TrackBelow,
    None,
}

const MIN_THUMB_PX: f32 = 12.0;

// ============================================================================
// ScrollArea — the math-only widget core
// ============================================================================

/// Vertical scroll area state. Pure data — no egui dependency. Persists
/// the offset + viewport + touch-momentum state; takes [`Rows`] by
/// reference at each query so the rows can borrow other state without
/// fighting our lifetimes.
///
/// **Empty / mutated rows.** [`ScrollArea`] tolerates `Rows` becoming
/// empty or having its current anchor removed out from under it. The
/// stored offset can become stale; reads project it onto current rows
/// via [`offset`](ScrollArea::offset). Mutation methods start from the
/// projected offset and write back.
#[derive(Debug, Clone)]
pub struct ScrollArea<Id: Clone + Eq + std::fmt::Debug> {
    /// User-intent offset. Read it through [`offset`](ScrollArea::offset),
    /// which projects onto current rows.
    stored_offset: Option<Offset<Id>>,
    pub viewport_height: f32,
    /// Touch-scroll velocity (precise px/sec). Positive = content moving
    /// up on screen.
    velocity_precise: f32,
    /// Sliding window of recent drag samples (delta_precise, dt) for
    /// smoothing single-frame noise into a stable release velocity.
    drag_window: [(f32, f32); DRAG_WINDOW_LEN],
    drag_window_idx: u8,
}

const DRAG_WINDOW_LEN: usize = 6;

impl<Id: Clone + Eq + std::fmt::Debug> Default for ScrollArea<Id> {
    fn default() -> Self {
        Self {
            stored_offset: None,
            viewport_height: 0.0,
            velocity_precise: 0.0,
            drag_window: [(0.0, 0.0); DRAG_WINDOW_LEN],
            drag_window_idx: 0,
        }
    }
}

impl<Id: Clone + Eq + std::fmt::Debug> ScrollArea<Id> {
    pub fn new(viewport_height: f32) -> Self {
        Self { viewport_height: viewport_height.max(0.0), ..Default::default() }
    }

    pub fn velocity(&self) -> Vec2 {
        Vec2::new(0.0, self.velocity_precise)
    }

    /// Raw persisted offset, *not* projected onto any rows. Use for
    /// change-detection across frames where projecting would require
    /// constructing rows just to read state.
    pub fn stored_offset(&self) -> Option<Offset<Id>> {
        self.stored_offset.clone()
    }

    /// The current scroll offset, projected onto the current rows.
    /// Returns `None` iff rows is empty.
    ///
    /// Two cases the projection absorbs:
    /// - **Anchor removed** → top of the first surviving row.
    /// - **Anchor row shrank** (external mutation reduced
    ///   `precise(anchor)` below stored `intra_precise`) → walk forward
    ///   by the excess. Patch-through, not clamp.
    pub fn offset<R: Rows<RowId = Id>>(&self, rows: &R) -> Option<Offset<Id>> {
        match &self.stored_offset {
            Some(off) if Self::anchor_valid(rows, &off.anchor) => {
                Some(Self::normalize(rows, off.clone()))
            }
            Some(off) => rows.recover_anchor(&off.anchor).map(Offset::at_top_of),
            None => rows.first().map(Offset::at_top_of),
        }
    }

    fn normalize<R: Rows<RowId = Id>>(rows: &R, mut off: Offset<Id>) -> Offset<Id> {
        if off.intra_precise < 0.0 {
            off.intra_precise = 0.0;
            return off;
        }
        loop {
            let row_precise = rows.precise(&off.anchor);
            if off.intra_precise <= row_precise {
                return off;
            }
            match rows.next(&off.anchor) {
                Some(next_id) => {
                    off.intra_precise -= row_precise;
                    off.anchor = next_id;
                }
                None => {
                    off.intra_precise = row_precise;
                    return off;
                }
            }
        }
    }

    fn anchor_valid<R: Rows<RowId = Id>>(rows: &R, anchor: &Id) -> bool {
        if rows.first().is_none() {
            return false;
        }
        if rows.next(anchor).is_some() {
            return true;
        }
        if rows.prev(anchor).is_some() {
            return true;
        }
        rows.first().as_ref() == Some(anchor)
    }

    /// The largest offset where the document still fills the viewport.
    /// `None` when rows is empty.
    pub fn max_offset<R: Rows<RowId = Id>>(&self, rows: &R) -> Option<Offset<Id>> {
        let last = rows.last()?;
        let intra_precise = rows.precise(&last);
        let last_bottom = Offset { anchor: last, intra_precise };
        Some(affine::scroll_by(rows, last_bottom, -self.viewport_height))
    }

    fn clamp_to_max<R: Rows<RowId = Id>>(&self, rows: &R, off: Offset<Id>) -> Offset<Id> {
        let Some(max) = self.max_offset(rows) else {
            return off;
        };
        let Some(last) = rows.last() else {
            return off;
        };
        let intra_precise = rows.precise(&last);
        let last_bottom = Offset { anchor: last, intra_precise };
        // Directed walk: `last_bottom` is forward of any valid offset
        // by construction. Bounded by viewport_height — past that we
        // know there's enough content below to fill the viewport, so
        // no clamp is needed.
        match affine::precise_distance_forward(rows, &off, &last_bottom, self.viewport_height) {
            Some(dist) if dist < self.viewport_height => max,
            _ => off,
        }
    }

    pub fn total_approx<R: Rows<RowId = Id>>(&self, rows: &R) -> f32 {
        affine::total_approx(rows)
    }

    pub fn thumb_approx<R: Rows<RowId = Id>>(&self, rows: &R) -> f32 {
        match self.offset(rows) {
            Some(off) => affine::thumb_approx(rows, &off),
            None => 0.0,
        }
    }

    /// Signed precise-pixel distance from `a` to `b`, bounded by
    /// `bound`. Positive if `b` is below `a`. Returns `None` if `b`
    /// isn't within `bound` in either direction.
    pub fn precise_distance<R: Rows<RowId = Id>>(
        &self, rows: &R, a: &Offset<Id>, b: &Offset<Id>, bound: f32,
    ) -> Option<f32> {
        affine::precise_distance(rows, a, b, bound)
    }

    /// The [`Offset`] corresponding to the given viewport-local y. Walks
    /// from the current offset by `viewport_y` precise pixels — useful
    /// for converting screen-space hit positions (e.g. cursor / find-
    /// match galley positions) into doc anchors for [`Reveal`].
    /// Returns `None` if rows is empty.
    pub fn offset_at_viewport_y<R: Rows<RowId = Id>>(
        &self, rows: &R, viewport_y: f32,
    ) -> Option<Offset<Id>> {
        let current = self.offset(rows)?;
        Some(affine::scroll_by(rows, current, viewport_y))
    }

    /// Rows currently within the viewport, top-down. Bounded by viewport
    /// rows, not document size.
    pub fn visible<R: Rows<RowId = Id>>(&self, rows: &R) -> Vec<VisibleRow<Id>> {
        let Some(off) = self.offset(rows) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        let mut id = off.anchor;
        let mut y = -off.intra_precise;
        while y < self.viewport_height {
            let height = rows.precise(&id);
            let next = rows.next(&id);
            out.push(VisibleRow { id: id.clone(), top: y, height });
            y += height;
            match next {
                Some(n) => id = n,
                None => break,
            }
        }
        out
    }

    /// Translate a viewport-local y into the row at that position.
    pub fn hit_row<R: Rows<RowId = Id>>(&self, rows: &R, y: f32) -> Option<HitRow<Id>> {
        let off = self.offset(rows)?;
        if y < 0.0 || y >= self.viewport_height {
            return None;
        }
        let mut id = off.anchor;
        let mut row_top = -off.intra_precise;
        loop {
            let height = rows.precise(&id);
            if y >= row_top && y < row_top + height {
                return Some(HitRow { id, intra_y: y - row_top });
            }
            row_top += height;
            if row_top > self.viewport_height {
                return None;
            }
            match rows.next(&id) {
                Some(n) => id = n,
                None => return None,
            }
        }
    }

    /// Place the offset at a specific anchor + intra-row precise.
    /// Result clamps so the document still fills the viewport.
    ///
    /// If the caller's anchor isn't in the row sequence (e.g., a
    /// persisted index referring to a row that's since been deleted),
    /// runs [`Rows::recover_anchor`] to obtain a replacement anchor
    /// and places at its top. Returns silently if rows is empty or
    /// recovery returns `None`.
    ///
    /// Cancels touch-scroll momentum so a programmatic jump isn't
    /// fought by a coasting flick.
    pub fn set_offset<R: Rows<RowId = Id>>(&mut self, rows: &R, off: Offset<Id>) {
        let off = if Self::anchor_valid(rows, &off.anchor) {
            off
        } else {
            match rows.recover_anchor(&off.anchor) {
                Some(recovered) => Offset::at_top_of(recovered),
                None => return,
            }
        };
        self.stored_offset = Some(self.clamp_to_max(rows, off));
        self.kill_momentum();
    }

    /// Process an [`Action`]. Stale anchors recover via `offset()` first.
    pub fn handle<R: Rows<RowId = Id>>(&mut self, rows: &R, action: Action) {
        match action {
            Action::ScrollByPixels(delta) => {
                if let Some(off) = self.offset(rows) {
                    let new = affine::scroll_by(rows, off, delta);
                    self.stored_offset = Some(self.clamp_to_max(rows, new));
                }
            }
            Action::ScrollByThumb(delta) => {
                if self.offset(rows).is_some() {
                    let target = self.thumb_approx(rows) + delta;
                    if let Some(new_off) = affine::from_thumb(rows, target) {
                        self.stored_offset = Some(self.clamp_to_max(rows, new_off));
                    }
                }
            }
            Action::ScrollToTop => {
                self.stored_offset = rows.first().map(Offset::at_top_of);
            }
            Action::ScrollToBottom => {
                self.stored_offset = self.max_offset(rows);
            }
            Action::Resize(h) => {
                self.viewport_height = h.max(0.0);
                if let Some(off) = self.offset(rows) {
                    self.stored_offset = Some(self.clamp_to_max(rows, off));
                }
            }
        }
    }

    /// Scroll so `rect` is positioned per `align` within the viewport.
    /// `Align::Nearest` does bounded visibility classification —
    /// document size doesn't enter the cost.
    pub fn reveal<R: Rows<RowId = Id>>(&mut self, rows: &R, rect: Reveal<Id>, align: Align) {
        if !Self::anchor_valid(rows, &rect.top.anchor)
            || !Self::anchor_valid(rows, &rect.bottom.anchor)
        {
            return;
        }
        let target = match align {
            Align::Top => rect.top,
            Align::Bottom => affine::scroll_by(rows, rect.bottom, -self.viewport_height),
            Align::Center => {
                // Rect taller than viewport has no meaningful center
                // anchor inside the viewport; degrade to Top.
                match affine::midpoint(rows, rect.top.clone(), rect.bottom, self.viewport_height) {
                    Some(mid) => affine::scroll_by(rows, mid, -self.viewport_height / 2.0),
                    None => rect.top,
                }
            }
            Align::Nearest => {
                let Some(target) = self.nearest_target(rows, rect) else {
                    return;
                };
                target
            }
        };
        self.set_offset(rows, target);
    }

    fn nearest_target<R: Rows<RowId = Id>>(
        &self, rows: &R, rect: Reveal<Id>,
    ) -> Option<Offset<Id>> {
        let current = self.offset(rows)?;
        let bound = self.viewport_height;

        let y_top = affine::precise_distance(rows, &current, &rect.top, bound);
        let y_bot = affine::precise_distance(rows, &current, &rect.bottom, bound);

        let rect_height = affine::precise_distance(rows, &rect.top, &rect.bottom, bound + 1.0);
        let rect_taller_than_viewport = rect_height.map(|h| h > bound).unwrap_or(true);

        if rect_taller_than_viewport {
            return Some(rect.top);
        }

        match (y_top, y_bot) {
            (Some(t), Some(b)) => {
                if t >= 0.0 && b <= bound {
                    None
                } else if t < 0.0 {
                    Some(rect.top)
                } else {
                    Some(affine::scroll_by(rows, rect.bottom, -bound))
                }
            }
            (Some(t), None) => {
                if t < 0.0 {
                    Some(rect.top)
                } else {
                    Some(affine::scroll_by(rows, rect.bottom, -bound))
                }
            }
            (None, Some(b)) => {
                if b > bound {
                    Some(affine::scroll_by(rows, rect.bottom, -bound))
                } else {
                    Some(rect.top)
                }
            }
            (None, None) => {
                // Both endpoints are far away; only direction matters.
                // No `precise()` needed.
                match affine::probe_direction(rows, &current.anchor, &rect.top.anchor) {
                    Some(affine::Direction::Forward) => {
                        Some(affine::scroll_by(rows, rect.bottom, -bound))
                    }
                    _ => Some(rect.top),
                }
            }
        }
    }

    /// Scrollbar geometry within `track`. Thumb's vertical extent is
    /// proportional to the visible fraction in approx coordinates,
    /// floored at `MIN_THUMB_PX` for grabability.
    ///
    /// `track.x` and `track.width()` pass through unchanged; the
    /// embedder controls scrollbar width and horizontal placement.
    pub fn scrollbar<R: Rows<RowId = Id>>(&self, rows: &R, track: Rect) -> Scrollbar {
        let total = self.total_approx(rows);
        let thumb_pos = self.thumb_approx(rows);
        let scrollable_approx = self
            .max_offset(rows)
            .map(|off| affine::thumb_approx(rows, &off))
            .unwrap_or(0.0);
        // Geometric model: thumb top travels [0, scrollable_approx],
        // thumb body extends viewport_approx below it, so thumb bottom
        // travels [viewport_approx, scrollable_approx + viewport_approx].
        // The track represents that full range, so:
        //     visible_fraction = viewport_approx / (scrollable_approx + viewport_approx)
        // Edges: scrollable=0 → fraction=1 (thumb fills track). Doc
        // shorter than viewport but pad makes things scrollable →
        // scrollable>0, fraction<1 (thumb correctly shows headroom).
        let viewport_approx = affine::viewport_extent_in_approx(rows, self.viewport_height);
        let denom = scrollable_approx + viewport_approx;
        let visible_fraction =
            if denom > 0.0 { (viewport_approx / denom).clamp(0.0, 1.0) } else { 1.0 };
        let thumb_h = (track.height() * visible_fraction)
            .max(MIN_THUMB_PX)
            .min(track.height());
        let thumb_top_fraction = if scrollable_approx > 0.0 {
            (thumb_pos / scrollable_approx).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let thumb_y = track.min.y + (track.height() - thumb_h) * thumb_top_fraction;
        Scrollbar {
            track,
            thumb: Rect::from_min_size(
                Pos2::new(track.min.x, thumb_y),
                Vec2::new(track.width(), thumb_h),
            ),
            total_approx: total,
            thumb_approx: thumb_pos,
            scrollable_approx,
        }
    }

    fn kill_momentum(&mut self) {
        self.velocity_precise = 0.0;
        self.drag_window = [(0.0, 0.0); DRAG_WINDOW_LEN];
        self.drag_window_idx = 0;
    }

    fn record_drag_sample(&mut self, precise_delta: f32, dt: f32) {
        self.drag_window[self.drag_window_idx as usize] = (precise_delta, dt);
        self.drag_window_idx = (self.drag_window_idx + 1) % (DRAG_WINDOW_LEN as u8);
        let (sum_d, sum_dt) = self
            .drag_window
            .iter()
            .fold((0.0, 0.0), |(sd, st), (d, t)| (sd + d, st + t));
        self.velocity_precise = if sum_dt > 0.001 { sum_d / sum_dt } else { 0.0 };
    }
}

// ============================================================================
// Egui adapter
// ============================================================================

/// Egui adapter that owns a [`ScrollArea<Id>`] and layers wheel /
/// touch-drag / scrollbar-drag input plus momentum on top. Embed this
/// directly in the type that owns the scrollable view; it persists
/// state across frames as a plain field.
pub struct AffineScrollArea<Id: Clone + Eq + std::fmt::Debug> {
    pub state: ScrollArea<Id>,
    /// When true, drag on the body (not just the scrollbar) scrolls
    /// the content with momentum. Intended for touch input.
    pub touch_scroll: bool,
    /// Salt for the scrollbar's interact rect — must be stable across
    /// frames and unique among visible scroll areas.
    id_salt: egui::Id,
}

impl<Id: Clone + Eq + std::fmt::Debug> AffineScrollArea<Id> {
    pub fn new(id_salt: impl Hash) -> Self {
        Self {
            state: ScrollArea::default(),
            touch_scroll: false,
            id_salt: egui::Id::new(id_salt),
        }
    }

    /// Touch-scroll velocity (precise px/sec). y is vertical; x is
    /// always 0. Non-zero while momentum is in flight.
    pub fn velocity(&self) -> Vec2 {
        self.state.velocity()
    }

    /// Raw persisted offset for change-detection (no row projection).
    pub fn stored_offset(&self) -> Option<Offset<Id>> {
        self.state.stored_offset()
    }

    /// Current offset projected onto the given rows, or `None` if rows
    /// is empty.
    pub fn offset<R: Rows<RowId = Id>>(&self, rows: &R) -> Option<Offset<Id>> {
        self.state.offset(rows)
    }

    /// Set offset directly. Cancels touch-scroll momentum.
    pub fn set_offset<R: Rows<RowId = Id>>(&mut self, rows: &R, off: Offset<Id>) {
        self.state.set_offset(rows, off);
    }

    /// Reveal a rect with the given alignment.
    pub fn reveal<R: Rows<RowId = Id>>(&mut self, rows: &R, rect: Reveal<Id>, align: Align) {
        self.state.reveal(rows, rect, align);
    }

    /// Per-frame: allocate body + scrollbar hit areas, process input,
    /// draw scrollbar, return body Response + visible rows. Caller
    /// paints rows into the body using the returned `top` offsets.
    ///
    /// Walks rows in a viewport-sized window above and below the
    /// visible region, calling [`Rows::warm`] on each so impls can
    /// kick off background work for rows about to enter view.
    pub fn show<R: Rows<RowId = Id>>(&mut self, ui: &mut Ui, rows: &R) -> ShowResponse<Id> {
        let rect = ui.max_rect();
        let body_sense = if self.touch_scroll { Sense::click_and_drag() } else { Sense::hover() };
        let response = ui.allocate_rect(rect, body_sense);

        // Scrollbar hit area registered AFTER the body so it shadows
        // body hover in z-order.
        const BAR_WIDTH: f32 = 10.0;
        const BAR_INSET: f32 = 3.0;
        let bar_x = rect.max.x - BAR_WIDTH - BAR_INSET;
        let bar_track =
            Rect::from_min_size(Pos2::new(bar_x, rect.min.y), Vec2::new(BAR_WIDTH, rect.height()));
        let bar_id = self.id_salt.with("scrollbar");
        let bar_response = ui.interact(bar_track, bar_id, Sense::click_and_drag());

        self.state.handle(rows, Action::Resize(rect.height()));

        // `scrollable_approx` is a function of (rows, viewport_height,
        // slope_band), independent of the current offset, so this
        // snapshot stays valid through input processing. Used both to
        // size scrollbar-drag math and to gate touch-body-drag.
        let bar_geom = self.state.scrollbar(rows, bar_track);
        let scrollable = bar_geom.scrollable_approx > 0.0;

        // Wheel: precise pixels. egui convention: positive y = scroll up
        // (content moves down). We want offset to grow when user scrolls
        // down (content moves up), so negate.
        let raw_scroll_delta =
            if ui.rect_contains_pointer(rect) { ui.input(|i| i.raw_scroll_delta.y) } else { 0.0 };
        if raw_scroll_delta != 0.0 {
            self.state.handle(rows, Action::ScrollByPixels(-raw_scroll_delta));
        }

        // Touch body drag → scroll + velocity tracking.
        let dt = ui.input(|i| i.stable_dt).max(0.0001);
        if scrollable && self.touch_scroll && response.drag_started() {
            self.state.kill_momentum();
        }
        if scrollable && self.touch_scroll && response.dragged() {
            let drag_y = ui.input(|i| i.pointer.delta().y);
            let precise_delta = -drag_y;
            self.state.handle(rows, Action::ScrollByPixels(precise_delta));
            self.state.record_drag_sample(precise_delta, dt);
        } else if self.state.velocity_precise.abs() > 1.0 && !response.dragged() {
            const DECAY_PER_SEC: f32 = 4.0;
            let precise_step = self.state.velocity_precise * dt;
            let before = self.state.offset(rows);
            self.state.handle(rows, Action::ScrollByPixels(precise_step));
            let after = self.state.offset(rows);
            if before == after {
                self.state.velocity_precise = 0.0;
            } else {
                self.state.velocity_precise *= (-DECAY_PER_SEC * dt).exp();
            }
            ui.ctx().request_repaint();
        } else {
            self.state.velocity_precise = 0.0;
        }
        if self.touch_scroll && response.clicked() {
            self.state.velocity_precise = 0.0;
        }

        // Scrollbar drag/click. Drag deltas use the pre-input geometry
        // — the user's drag is a pixel-velocity gesture, not a position
        // command, so reading the live thumb position would double-apply
        // any scroll that already happened this frame.
        if scrollable && (bar_response.dragged() || bar_response.clicked()) {
            if bar_response.dragged() {
                let drag_y = ui.input(|i| i.pointer.delta().y);
                let approx_delta = bar_geom.pixel_to_approx(drag_y);
                self.state.handle(rows, Action::ScrollByThumb(approx_delta));
            } else if let Some(pos) = bar_response.interact_pointer_pos() {
                let movable = bar_geom.track.height() - bar_geom.thumb.height();
                let target_thumb_top =
                    (pos.y - bar_geom.track.min.y - bar_geom.thumb.height() / 2.0)
                        .clamp(0.0, movable.max(1.0));
                let target_approx = if movable > 0.0 {
                    target_thumb_top * (bar_geom.scrollable_approx / movable)
                } else {
                    0.0
                };
                let delta = target_approx - bar_geom.thumb_approx;
                self.state.handle(rows, Action::ScrollByThumb(delta));
            }
        }

        let visible = self.state.visible(rows);
        warm_around_visible(rows, &visible, rect.height());

        let bar_after = self.state.scrollbar(rows, bar_track);
        draw_scrollbar(ui, bar_after);

        ShowResponse { response, visible, scrollbar_track: bar_track }
    }
}

/// Returned by [`AffineScrollArea::show`].
#[derive(Debug, Clone)]
pub struct ShowResponse<Id> {
    /// Body rect Response (hover, or click+drag for touch_scroll). Use
    /// for context-menu attachment etc.
    pub response: Response,
    /// Visible rows top-down. `top` is viewport-local; add `viewport.min.y`
    /// for screen coordinates.
    pub visible: Vec<VisibleRow<Id>>,
    /// Track rect of the rendered scrollbar (window-local pixels).
    /// Embedders register this with their touch-consuming surface so
    /// taps on the scrollbar don't fall through to other touch handlers
    /// (cursor placement, virtual keyboard).
    pub scrollbar_track: Rect,
}

fn warm_around_visible<R: Rows>(rows: &R, visible: &[VisibleRow<R::RowId>], viewport_height: f32) {
    let Some(first) = visible.first() else {
        return;
    };
    let last = visible.last().unwrap();

    // Below the visible window.
    let mut id = last.id.clone();
    let mut y = 0.0f32;
    while y < viewport_height {
        match rows.next(&id) {
            Some(next_id) => {
                rows.warm(&next_id);
                y += rows.precise(&next_id);
                id = next_id;
            }
            None => break,
        }
    }

    // Above the visible window.
    let mut id = first.id.clone();
    let mut y = 0.0f32;
    while y < viewport_height {
        match rows.prev(&id) {
            Some(prev_id) => {
                rows.warm(&prev_id);
                y += rows.precise(&prev_id);
                id = prev_id;
            }
            None => break,
        }
    }
}

fn draw_scrollbar(ui: &Ui, bar: Scrollbar) {
    use crate::theme::palette_v2::ThemeExt as _;
    let theme = ui.ctx().get_lb_theme();
    let track_color = theme.neutral_bg().lerp_to_gamma(theme.neutral(), 0.3);
    let thumb_color = theme.neutral();
    ui.painter().rect_filled(bar.track, 3.0, track_color);
    ui.painter()
        .rect(bar.thumb, 3.0, thumb_color, Stroke::NONE, egui::epaint::StrokeKind::Inside);
}
