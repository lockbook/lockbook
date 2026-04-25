//! A vertical scroll area whose content has two notions of height per
//! block: a cheap **approximate** height and an expensive **precise**
//! height. The scrollbar operates in approx units (so the bar's range is
//! a constant function of the doc), but visible content is laid out
//! precisely. Scroll *events* are interpreted in precise units (= screen
//! pixels of intended movement) and translated to approx via a piecewise
//! affine map before they touch the scrollbar.
//!
//! # Why
//!
//! In a long doc with content that's only cheap to estimate (markdown,
//! code with syntax highlighting, etc.), measuring every block precisely
//! to size the scrollbar is too slow. Approximating sizes is fast but
//! breaks the "user scrolls N pixels, content moves N pixels" invariant.
//!
//! This widget bridges the two: scrollbar is approx (so always known
//! and consistent), but the user's wheel/drag input is interpreted as a
//! request to move content by N **precise** pixels. The widget walks
//! the affine map at the current scroll position, converts to the
//! corresponding approx delta, and updates the scrollbar.
//!
//! # Contract with `ScrollContent`
//!
//! - `block_count()` and `approx_height(i)` are called frequently and
//!   should be O(1) (cached).
//! - `precise_height(i)` is called only for blocks the widget renders
//!   (anchor + downstream until viewport full). May be expensive on
//!   first call; content should cache.
//! - `render_block(ui, i, top_left)` is called with `top_left` in
//!   **screen-space** (relative to the egui ui's origin). Output that
//!   the content records (e.g. galleys) should also be screen-space.

use egui::{Pos2, Rect, Response, Sense, Stroke, Ui, Vec2};

/// Content the scroll area renders. See module docs.
pub trait ScrollContent {
    /// Number of blocks. Stable across a single `show()` call.
    fn block_count(&self) -> usize;

    /// Cheap, cached. Used to find the anchor and size the scrollbar.
    fn approx_height(&self, block_idx: usize) -> f32;

    /// Used to lay out visible blocks precisely. May be expensive on
    /// first call; content should cache.
    fn precise_height(&mut self, block_idx: usize) -> f32;

    /// Paint block `block_idx` with its top-left at the given screen
    /// position. Output recorded by the content (galleys, hit-test
    /// rects) should be in screen space.
    fn render_block(&mut self, ui: &mut Ui, block_idx: usize, top_left: Pos2);
}

/// Per-frame state of the scroll area. Persisted in egui memory between
/// frames keyed by [`AffineScrollArea::id_salt`].
#[derive(Clone, Copy, Default)]
struct ScrollState {
    /// Position in approx units. `[0, approx_total - viewport_height]`.
    offset_approx: f32,
}

pub struct AffineScrollArea {
    id_salt: egui::Id,
}

impl AffineScrollArea {
    pub fn new(id_salt: impl std::hash::Hash) -> Self {
        Self { id_salt: egui::Id::new(id_salt) }
    }

    pub fn show(&mut self, ui: &mut Ui, content: &mut dyn ScrollContent) -> Response {
        // Take the parent's full max_rect — callers control the scroll
        // area's size by setting the surrounding ui's max_rect (e.g.
        // via `ui.scope_builder().max_rect(...)`).
        let rect = ui.max_rect();

        let response = ui.allocate_rect(rect, Sense::hover());

        // Scrollbar hit area registered AFTER the body so it shadows
        // the body's hover in z-order. Same dimensions used by
        // `draw_scrollbar` below.
        const BAR_WIDTH: f32 = 10.0;
        const BAR_INSET: f32 = 3.0;
        let bar_x = rect.max.x - BAR_WIDTH - BAR_INSET;
        let bar_track = Rect::from_min_size(
            Pos2::new(bar_x, rect.min.y),
            Vec2::new(BAR_WIDTH, rect.height()),
        );
        let bar_id = self.id_salt.with("scrollbar");
        let bar_response = ui.interact(bar_track, bar_id, Sense::click_and_drag());

        // Persisted scroll state.
        let mut state: ScrollState = ui
            .ctx()
            .data(|d| d.get_temp(self.id_salt))
            .unwrap_or_default();

        // Max scroll = max of:
        // - approx-y top of the last block (user can put it at top of
        //   viewport — useful when last block is short).
        // - approx_total − viewport_height (user can scroll within a
        //   tall single block).
        // Whichever's larger wins. Both are constant functions of the doc.
        let n = content.block_count();
        let viewport_height = rect.height();
        let approx_total: f32 = (0..n).map(|i| content.approx_height(i)).sum();
        let max_offset = if n == 0 {
            0.0
        } else {
            let approx_y_top_last: f32 =
                (0..n - 1).map(|i| content.approx_height(i)).sum();
            approx_y_top_last.max(approx_total - viewport_height).max(0.0)
        };

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
            let approx_delta = precise_to_approx_delta(
                content,
                state.offset_approx,
                precise_delta,
            );
            state.offset_approx =
                (state.offset_approx + approx_delta).clamp(0.0, max_offset);
        }

        // Scrollbar drag/click. The thumb spans `visible_fraction *
        // viewport_height`; the user can drag the thumb across the
        // remaining `(1 - visible_fraction) * viewport_height` of
        // track. That drag range maps onto `[0, max_offset]` in approx.
        if max_offset > 0.0 && (bar_response.dragged() || bar_response.clicked()) {
            let approx_total = max_offset + viewport_height;
            let visible_fraction = (viewport_height / approx_total).clamp(0.0, 1.0);
            let track_drag_range = (rect.height() * (1.0 - visible_fraction)).max(1.0);
            if bar_response.dragged() {
                let drag_y = ui.input(|i| i.pointer.delta().y);
                state.offset_approx = (state.offset_approx
                    + drag_y * (max_offset / track_drag_range))
                    .clamp(0.0, max_offset);
            } else if let Some(pos) = bar_response.interact_pointer_pos() {
                // Click jumps so the thumb's center lands at the cursor.
                let thumb_h = visible_fraction * rect.height();
                let target_thumb_top = (pos.y - rect.min.y - thumb_h / 2.0)
                    .clamp(0.0, track_drag_range);
                state.offset_approx = target_thumb_top * (max_offset / track_drag_range);
            }
        }

        // Find anchor and render visible blocks.
        ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
            ui.set_clip_rect(rect);
            render_visible(ui, content, rect, state.offset_approx);
        });

        // Draw scrollbar (simple vertical bar on the right).
        if max_offset > 0.0 {
            draw_scrollbar(ui, rect, state.offset_approx, approx_total, viewport_height);
        }

        // Persist state.
        ui.ctx()
            .data_mut(|d| d.insert_temp(self.id_salt, state));

        response
    }
}

/// Pure layout: returns `(block_idx, screen_y)` pairs for blocks the
/// scroll area would paint at this offset. `screen_y` is in viewport-
/// relative coordinates (i.e. `viewport.min.y == 0`). Doesn't touch
/// `ui` so it's testable on its own.
fn visible_block_positions(
    content: &mut dyn ScrollContent, viewport_height: f32, offset_approx: f32,
) -> Vec<(usize, f32)> {
    let n = content.block_count();
    if n == 0 {
        return Vec::new();
    }

    // Find anchor: first block whose approx range crosses `offset_approx`.
    let mut acc_approx = 0.0f32;
    let mut anchor_idx = n - 1;
    let mut anchor_approx_top = 0.0f32;
    for i in 0..n {
        let h = content.approx_height(i);
        if acc_approx + h > offset_approx {
            anchor_idx = i;
            anchor_approx_top = acc_approx;
            break;
        }
        acc_approx += h;
        anchor_idx = i;
        anchor_approx_top = acc_approx - h;
    }

    // Anchor's intra-offset within itself, in approx units.
    let intra_approx = (offset_approx - anchor_approx_top).max(0.0);
    let anchor_approx_h = content.approx_height(anchor_idx);
    let anchor_precise_h = content.precise_height(anchor_idx);
    let slope = if anchor_approx_h > 0.0 { anchor_precise_h / anchor_approx_h } else { 1.0 };
    let intra_precise = intra_approx * slope;

    // Paint anchor's intra-position at screen y=0; downstream blocks at
    // consecutive precise positions.
    let mut out = Vec::new();
    let mut y = -intra_precise;
    let mut idx = anchor_idx;
    while idx < n && y < viewport_height {
        out.push((idx, y));
        y += content.precise_height(idx);
        idx += 1;
    }
    out
}

/// Walks blocks until cumulative approx >= `offset_approx` to find the
/// anchor block. Then paints anchor and downstream blocks at consecutive
/// precise positions. Anchor's intra-position is placed at the viewport
/// top in screen space.
fn render_visible(
    ui: &mut Ui, content: &mut dyn ScrollContent, viewport: Rect, offset_approx: f32,
) {
    let positions = visible_block_positions(content, viewport.height(), offset_approx);
    for (idx, screen_y) in positions {
        content.render_block(ui, idx, Pos2::new(viewport.min.x, viewport.min.y + screen_y));
    }
}

/// Translate a precise delta (screen pixels of intended movement) into
/// an approx delta to apply to the scrollbar offset. Walks blocks
/// starting at `offset_approx` consuming precise units; each block
/// contributes at its approx/precise ratio.
fn precise_to_approx_delta(
    content: &mut dyn ScrollContent, offset_approx: f32, precise_delta: f32,
) -> f32 {
    if precise_delta == 0.0 {
        return 0.0;
    }
    let n = content.block_count();
    if n == 0 {
        return 0.0;
    }

    // Find anchor block at offset.
    let mut acc = 0.0f32;
    let mut anchor_idx = 0usize;
    let mut anchor_top = 0.0f32;
    for i in 0..n {
        let h = content.approx_height(i);
        if acc + h > offset_approx {
            anchor_idx = i;
            anchor_top = acc;
            break;
        }
        acc += h;
        anchor_idx = i;
        anchor_top = acc - h;
    }

    let intra_approx = (offset_approx - anchor_top).max(0.0);

    if precise_delta > 0.0 {
        // Scrolling forward.
        let mut precise_remaining = precise_delta;
        let mut approx_consumed = 0.0;
        let mut idx = anchor_idx;
        let mut start_intra = intra_approx;

        while idx < n && precise_remaining > 0.0 {
            let approx_h = content.approx_height(idx);
            let precise_h = content.precise_height(idx);
            let slope = if approx_h > 0.0 { precise_h / approx_h } else { 1.0 };

            let approx_remaining_in_block = (approx_h - start_intra).max(0.0);
            let precise_remaining_in_block = approx_remaining_in_block * slope;

            if precise_remaining <= precise_remaining_in_block && slope > 0.0 {
                approx_consumed += precise_remaining / slope;
                return approx_consumed;
            }

            approx_consumed += approx_remaining_in_block;
            precise_remaining -= precise_remaining_in_block;
            idx += 1;
            start_intra = 0.0;
        }
        approx_consumed
    } else {
        // Scrolling backward; symmetric.
        let mut precise_remaining = -precise_delta;
        let mut approx_consumed = 0.0;
        let mut idx = anchor_idx as isize;
        let mut start_intra = intra_approx;

        while idx >= 0 && precise_remaining > 0.0 {
            let approx_h = content.approx_height(idx as usize);
            let precise_h = content.precise_height(idx as usize);
            let slope = if approx_h > 0.0 { precise_h / approx_h } else { 1.0 };

            // From start_intra back to 0.
            let approx_remaining_in_block = start_intra.max(0.0);
            let precise_remaining_in_block = approx_remaining_in_block * slope;

            if precise_remaining <= precise_remaining_in_block && slope > 0.0 {
                approx_consumed -= precise_remaining / slope;
                return approx_consumed;
            }

            approx_consumed -= approx_remaining_in_block;
            precise_remaining -= precise_remaining_in_block;
            idx -= 1;
            if idx >= 0 {
                start_intra = content.approx_height(idx as usize);
            }
        }
        approx_consumed
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
    ui.painter().rect(
        thumb,
        3.0,
        thumb_color,
        Stroke::NONE,
        egui::epaint::StrokeKind::Inside,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Synthetic content for testing the affine scroll math. Records
    /// rendered blocks so tests can assert positioning.
    struct MockContent {
        approx: Vec<f32>,
        precise: Vec<f32>,
        rendered: Vec<(usize, Pos2)>,
    }

    impl MockContent {
        fn new(blocks: Vec<(f32, f32)>) -> Self {
            let approx = blocks.iter().map(|(a, _)| *a).collect();
            let precise = blocks.iter().map(|(_, p)| *p).collect();
            Self { approx, precise, rendered: Vec::new() }
        }
    }

    impl ScrollContent for MockContent {
        fn block_count(&self) -> usize {
            self.approx.len()
        }
        fn approx_height(&self, i: usize) -> f32 {
            self.approx[i]
        }
        fn precise_height(&mut self, i: usize) -> f32 {
            self.precise[i]
        }
        fn render_block(&mut self, _: &mut Ui, i: usize, top_left: Pos2) {
            self.rendered.push((i, top_left));
        }
    }

    #[test]
    fn precise_to_approx_within_single_block() {
        // B1: approx 50, precise 100 → slope 2.
        let mut c = MockContent::new(vec![(50.0, 100.0)]);
        let approx_delta = precise_to_approx_delta(&mut c, 0.0, 30.0);
        // 30 precise / slope 2 = 15 approx.
        assert!((approx_delta - 15.0).abs() < 0.01, "got {}", approx_delta);
    }

    #[test]
    fn precise_to_approx_crossing_block_boundary() {
        // B1: approx 50, precise 100 (slope 2). B2: approx 50, precise 50 (slope 1).
        // From offset 0, scroll by 150 precise:
        //   B1 contributes 100 precise = 50 approx.
        //   B2 needs 50 more precise = 50 approx.
        //   Total: 100 approx.
        let mut c = MockContent::new(vec![(50.0, 100.0), (50.0, 50.0)]);
        let approx_delta = precise_to_approx_delta(&mut c, 0.0, 150.0);
        assert!((approx_delta - 100.0).abs() < 0.01, "got {}", approx_delta);
    }

    #[test]
    fn precise_to_approx_negative() {
        // From offset 30 (in B1, intra approx 30 = intra precise 60),
        // scroll by -30 precise → 15 approx backward.
        let mut c = MockContent::new(vec![(50.0, 100.0)]);
        let approx_delta = precise_to_approx_delta(&mut c, 30.0, -30.0);
        assert!((approx_delta + 15.0).abs() < 0.01, "got {}", approx_delta);
    }

    use rand::{Rng, SeedableRng, rngs::StdRng};

    fn random_blocks(rng: &mut StdRng, n: usize) -> Vec<(f32, f32)> {
        (0..n)
            .map(|_| {
                let approx = rng.gen_range(10.0..200.0);
                // Precise can be smaller, equal, or larger than approx —
                // ratios sampled in a realistic range.
                let ratio = rng.gen_range(0.3..3.0);
                (approx, approx * ratio)
            })
            .collect()
    }

    /// Property: after submitting a scroll event of `precise_delta`,
    /// blocks visible in both the before and after renders shift by
    /// exactly `precise_delta` in screen space.
    ///
    /// This is the core invariant the affine scroll area exists to
    /// satisfy. If broken, scrolling produces visible "jumps" at block
    /// boundaries.
    #[test]
    fn property_scroll_delta_preserved_in_screen_space() {
        const EPS: f32 = 0.01;
        let viewport_height = 200.0;
        let mut rng = StdRng::seed_from_u64(0);
        for seed in 0..2048u64 {
            let mut rng_inner = StdRng::seed_from_u64(seed);
            let n_blocks = rng_inner.gen_range(1..20);
            let blocks = random_blocks(&mut rng_inner, n_blocks);
            let mut c = MockContent::new(blocks.clone());

            let approx_total: f32 = blocks.iter().map(|(a, _)| *a).sum();
            if approx_total <= viewport_height {
                continue; // nothing to scroll
            }
            let max_offset = approx_total - viewport_height;
            let offset_a: f32 = rng.gen_range(0.0..=max_offset);

            // Pick a random precise delta the user might wheel.
            let precise_delta: f32 = rng.gen_range(-150.0..=150.0);

            // Translate to approx delta and apply.
            let approx_delta = precise_to_approx_delta(&mut c, offset_a, precise_delta);
            let offset_b = (offset_a + approx_delta).clamp(0.0, max_offset);

            // If the offset got clamped, the scroll event was partially
            // consumed; effective precise delta is smaller. Recompute.
            let effective_approx_delta = offset_b - offset_a;
            let effective_precise_delta = approx_to_precise_delta(
                &mut c, offset_a, effective_approx_delta,
            );

            let before = visible_block_positions(&mut c, viewport_height, offset_a);
            let after = visible_block_positions(&mut c, viewport_height, offset_b);

            // For blocks in both: screen_y diff = -effective_precise_delta
            // (positive scroll moves content up = smaller screen_y).
            for (idx_a, y_a) in &before {
                if let Some(&(_, y_b)) = after.iter().find(|(i, _)| i == idx_a) {
                    let diff = y_b - y_a;
                    let expected = -effective_precise_delta;
                    assert!(
                        (diff - expected).abs() < EPS,
                        "seed {seed}: block {idx_a} shifted by {diff}, expected {expected} \
                         (offset {offset_a} → {offset_b}, precise {precise_delta}, \
                         effective precise {effective_precise_delta})",
                    );
                }
            }
        }
    }

    /// Inverse of `precise_to_approx_delta`: given an approx delta,
    /// returns the precise distance covered. Used in tests to compute
    /// the effective precise delta after offset clamping.
    fn approx_to_precise_delta(
        content: &mut dyn ScrollContent, offset_approx: f32, approx_delta: f32,
    ) -> f32 {
        if approx_delta == 0.0 {
            return 0.0;
        }
        let n = content.block_count();
        if n == 0 {
            return 0.0;
        }

        // Find anchor block at offset.
        let mut acc = 0.0f32;
        let mut anchor_idx = n - 1;
        let mut anchor_top = 0.0f32;
        for i in 0..n {
            let h = content.approx_height(i);
            if acc + h > offset_approx {
                anchor_idx = i;
                anchor_top = acc;
                break;
            }
            acc += h;
            anchor_idx = i;
            anchor_top = acc - h;
        }
        let intra_approx = (offset_approx - anchor_top).max(0.0);

        if approx_delta > 0.0 {
            let mut approx_remaining = approx_delta;
            let mut precise_consumed = 0.0;
            let mut idx = anchor_idx;
            let mut start_intra = intra_approx;
            while idx < n && approx_remaining > 0.0 {
                let approx_h = content.approx_height(idx);
                let precise_h = content.precise_height(idx);
                let slope = if approx_h > 0.0 { precise_h / approx_h } else { 1.0 };
                let approx_remaining_in_block = (approx_h - start_intra).max(0.0);
                if approx_remaining <= approx_remaining_in_block {
                    precise_consumed += approx_remaining * slope;
                    return precise_consumed;
                }
                precise_consumed += approx_remaining_in_block * slope;
                approx_remaining -= approx_remaining_in_block;
                idx += 1;
                start_intra = 0.0;
            }
            precise_consumed
        } else {
            let mut approx_remaining = -approx_delta;
            let mut precise_consumed = 0.0;
            let mut idx = anchor_idx as isize;
            let mut start_intra = intra_approx;
            while idx >= 0 && approx_remaining > 0.0 {
                let approx_h = content.approx_height(idx as usize);
                let precise_h = content.precise_height(idx as usize);
                let slope = if approx_h > 0.0 { precise_h / approx_h } else { 1.0 };
                let approx_remaining_in_block = start_intra.max(0.0);
                if approx_remaining <= approx_remaining_in_block {
                    precise_consumed -= approx_remaining * slope;
                    return precise_consumed;
                }
                precise_consumed -= approx_remaining_in_block * slope;
                approx_remaining -= approx_remaining_in_block;
                idx -= 1;
                if idx >= 0 {
                    start_intra = content.approx_height(idx as usize);
                }
            }
            precise_consumed
        }
    }

    /// Property: precise→approx and approx→precise are inverses (for
    /// deltas that don't run off the end of the doc).
    #[test]
    fn property_affine_map_invertible() {
        const EPS: f32 = 0.01;
        let mut rng = StdRng::seed_from_u64(0);
        for seed in 0..2048u64 {
            let mut rng_inner = StdRng::seed_from_u64(seed);
            let n_blocks = rng_inner.gen_range(1..20);
            let blocks = random_blocks(&mut rng_inner, n_blocks);
            let mut c = MockContent::new(blocks.clone());

            let approx_total: f32 = blocks.iter().map(|(a, _)| *a).sum();
            let offset: f32 = rng.gen_range(0.0..=approx_total.max(1.0));
            let precise_delta: f32 = rng.gen_range(-100.0..=100.0);

            let approx_delta = precise_to_approx_delta(&mut c, offset, precise_delta);
            let new_offset = offset + approx_delta;
            // Skip if the scroll ran off the doc — `precise_to_approx`
            // caps at the end and the recovered precise will be smaller
            // than the original.
            const EDGE: f32 = 0.001;
            if new_offset < EDGE || new_offset > approx_total - EDGE {
                continue;
            }

            let recovered_precise = approx_to_precise_delta(&mut c, offset, approx_delta);
            assert!(
                (recovered_precise - precise_delta).abs() < EPS,
                "seed {seed}: roundtrip {precise_delta} → {approx_delta} → {recovered_precise} \
                 (offset {offset}, blocks {blocks:?})",
            );
        }
    }
}
