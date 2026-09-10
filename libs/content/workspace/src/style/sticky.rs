//! Sticky virtualized tree: pin ancestor folders, paint in-flow rows underneath.
//!
//! Used by the Files sidebar and the shared folder picker (move / search).

use egui::{Id, Rect, Response, Sense, Ui, pos2, vec2};
use lb_rs::Uuid;

use crate::file_cache::FilesExt;
use crate::style::chrome::{STROKE_HAIRLINE, file_row_icon, phosphor};
use crate::style::color::Theme;
use crate::style::file_row::FileRow;

#[derive(Clone, Copy, Debug)]
pub struct FlatRow {
    pub id: Uuid,
    pub depth: usize,
    pub is_folder: bool,
    /// Folder with no children (context menu hides Expand/Collapse all).
    pub kids_empty: bool,
}

// ── Shared FileRow chrome (every sticky / virtualized tree) ─────────────────

/// Geometry + interact flags common to Files / Shared / delete / folder pick.
#[derive(Clone, Copy, Debug)]
pub struct TreeRowChrome {
    depth: usize,
    elevated: bool,
    /// Top-only radius when an elevated pin sits at a sheet plate’s top edge.
    elevated_top_radius: u8,
    content_inset: f32,
    selected: bool,
    /// Hover wash + click affordance. Off = display-only (delete-sheet docs).
    interactive: bool,
}

impl TreeRowChrome {
    pub fn new(depth: usize) -> Self {
        Self {
            depth,
            elevated: false,
            elevated_top_radius: 0,
            content_inset: 0.0,
            selected: false,
            interactive: true,
        }
    }

    pub fn elevated(mut self, on: bool) -> Self {
        self.elevated = on;
        self
    }

    pub fn content_inset(mut self, pts: f32) -> Self {
        self.content_inset = pts;
        self
    }

    pub fn selected(mut self, on: bool) -> Self {
        self.selected = on;
        self
    }

    pub fn interactive(mut self, on: bool) -> Self {
        self.interactive = on;
        self
    }

    /// Sheet chooser pin: Control radius on the top edge only when stuck at y≈0.
    pub fn with_sheet_pin(mut self, elevated: bool, pin_vy: Option<f32>, pin_top_r: u8) -> Self {
        self.elevated = elevated;
        self.elevated_top_radius =
            if elevated && pin_top_r > 0 && pin_vy.map(|v| v <= 0.5).unwrap_or(false) {
                pin_top_r
            } else {
                0
            };
        self
    }
}

/// Open/closed folder or doc-type glyph (same mapping everywhere).
pub fn row_type_icon(name: &str, is_folder: bool, open: bool) -> &'static str {
    if is_folder {
        if open { phosphor::FOLDER_OPEN } else { phosphor::FOLDER }
    } else {
        file_row_icon(name, false)
    }
}

/// Build + paint a tree [`FileRow`]. `configure` adds surface-specific marks
/// (pin, sync, subtitle, sense). [`TreeRowChrome::interactive`] is applied
/// **last** so it wins over sense overrides when static.
#[allow(clippy::too_many_arguments)]
pub fn paint_tree_file_row<'a>(
    ui: &mut Ui, t: &'a Theme, name: impl Into<String>, icon: &'static str, chrome: TreeRowChrome,
    id: Id, paint_rect: Rect, hit_rect: Rect, configure: impl FnOnce(FileRow<'a>) -> FileRow<'a>,
) -> Response {
    let row = FileRow::new(t, name)
        .icon(icon)
        .depth(chrome.depth)
        .selected(chrome.selected)
        .elevated(chrome.elevated)
        .elevated_top_radius(chrome.elevated_top_radius)
        .content_inset(chrome.content_inset);
    configure(row)
        .interactive(chrome.interactive)
        .paint_at_hit(ui, paint_rect, hit_rect, id)
}

/// Virtualized list + sticky folder headers for a flat walk with per-row heights.
///
/// `paint` gets `(paint_rect, hit_rect, elevated, sticky_vy)` — content still
/// paints under elevated pins (scroll-under), but **hit** is clipped so only
/// one row can hover at a time.
///
/// `bottom_pad` extends scrollable content past the last row (empty hit target /
/// room to unstick a late sticky to the top of the viewport).
///
/// `pin_top_radius`: when > 0, elevated stickies at the top get **top-only**
/// corner radius (sheet plate Frame NW/NE). Files tree passes `0`.
///
/// `content_h_pad`: horizontal inset for **in-flow** row bands (and elevated
/// content via [`FileRow::content_inset`]). Elevated sticky **plates** still
/// paint full `view_screen` width so headers have no side gutters.
///
/// Returns the full-content interact (empty canvas under rows) for clear-select /
/// background context menus. Rows painted afterward win hit-testing on their rects.
#[allow(clippy::too_many_arguments)]
pub fn paint_sticky_viewport(
    ui: &mut Ui, t: &Theme, flat: &[FlatRow], geom: &RowGeom, viewport: Rect, bottom_pad: f32,
    pin_top_radius: u8, content_h_pad: f32,
    mut paint: impl FnMut(&mut Ui, &Theme, FlatRow, Rect, Rect, bool, Option<f32>),
) -> egui::Response {
    // Content (0,0) is `max_rect().min` (scroll inner top-left minus offset).
    let content_min = ui.max_rect().min;
    let view_screen = Rect::from_min_size(content_min + viewport.min.to_vec2(), viewport.size());
    let view_clip = view_screen.intersect(ui.clip_rect());
    ui.set_clip_rect(view_clip);

    let content_h = (geom.total + bottom_pad.max(0.0)).max(view_screen.height());
    let full_w = view_screen.width().max(0.0);
    let pad = content_h_pad.max(0.0);
    let inner_w = (full_w - 2.0 * pad).max(0.0);
    let inner_left = view_screen.left() + pad;
    let (_, bg_resp) = ui.allocate_exact_size(vec2(full_w, content_h), Sense::click());

    let offset = viewport.min.y;
    let sticky = sticky_layout(flat, geom, offset);
    // Screen y below the deepest elevated pin — in-flow hits only start here.
    let pin_hit_floor = sticky
        .iter()
        .filter(|s| s.elevated())
        .map(|s| view_screen.top() + s.vy + s.h)
        .fold(None, |acc: Option<f32>, y| Some(acc.map_or(y, |a| a.max(y))));

    for i in painted_inflow(flat, geom, &sticky, offset, viewport.height()) {
        let top = content_min.y + geom.top(i);
        let h = geom.height(i);
        // In-flow: inset band (wash + content). Sticky plates are full-bleed.
        let paint_rect = Rect::from_min_size(pos2(inner_left, top), vec2(inner_w, h));
        // Full paint (scroll-under stickies); hit excludes elevated pin band.
        let mut hit_rect = paint_rect;
        if let Some(floor) = pin_hit_floor {
            if hit_rect.min.y < floor {
                hit_rect.min.y = floor;
            }
        }
        paint(ui, t, flat[i], paint_rect, hit_rect, false, None);
    }

    let hairline_id = sticky.iter().rev().find(|s| s.elevated()).map(|s| s.row.id);
    for s in &sticky {
        let top = view_screen.top() + s.vy;
        // Full-bleed sticky plate (no side gutters).
        let full = Rect::from_min_size(pos2(view_screen.left(), top), vec2(full_w, s.h));
        let clip = Rect::from_min_max(
            pos2(view_screen.left(), view_screen.top() + s.clip_top),
            pos2(view_screen.right(), top + s.h),
        )
        .intersect(view_clip);
        if clip.height() < 0.5 {
            continue;
        }
        let elevated = s.elevated();
        // Only elevated pins need a plate fill (secondary). Boundary-pushed
        // stickies used to paint square `neutral_bg` full-bleed — same color as
        // the sheet/tree canvas, but sharp corners over a rounded Outside plate
        // border (delete / folder pick). Skip that paint entirely.
        if elevated {
            // Top of the sticky stack flush with the plate top → match plate NW/NE.
            // (Not `vy <= 0.5` alone: mid-stack pins have clip_top > 0.)
            let at_plate_top = pin_top_radius > 0 && s.clip_top <= 0.5;
            let corners = if at_plate_top {
                egui::CornerRadius { nw: pin_top_radius, ne: pin_top_radius, sw: 0, se: 0 }
            } else {
                egui::CornerRadius::ZERO
            };
            ui.painter()
                .with_clip_rect(clip)
                .rect_filled(full, corners, t.neutral_bg_secondary());
        }
        // Elevated pin owns the full plate for hits; wash is 1 px inset in FileRow.
        let hit = full;
        let prev_clip = ui.clip_rect();
        ui.set_clip_rect(clip.intersect(prev_clip));
        paint(ui, t, s.row, full, hit, elevated, Some(s.vy));
        ui.set_clip_rect(prev_clip);
        // Hairline after row paint so elevated hover wash (full plate height)
        // cannot cover the pin / in-flow divider.
        if elevated && hairline_id == Some(s.row.id) {
            ui.painter().with_clip_rect(clip).hline(
                full.x_range(),
                full.bottom() - 0.5,
                egui::Stroke::new(STROKE_HAIRLINE, t.neutral()),
            );
        }
    }

    bg_resp
}

pub fn flatten(
    files: &impl FilesExt, expanded: &std::collections::HashSet<Uuid>, id: Uuid, depth: usize,
    out: &mut Vec<FlatRow>, skip_self: bool,
) {
    if !skip_self {
        let is_folder = files.get_by_id(id).map(|f| f.is_folder()).unwrap_or(false);
        if !is_folder {
            out.push(FlatRow { id, depth, is_folder: false, kids_empty: false });
            return;
        }
        if !expanded.contains(&id) {
            out.push(FlatRow {
                id,
                depth,
                is_folder: true,
                kids_empty: files.children(id).is_empty(),
            });
            return;
        }
    }
    let kids: Vec<_> = files
        .children(id)
        .into_iter()
        .filter(|kid| kid.id != id)
        .collect();
    if !skip_self {
        out.push(FlatRow { id, depth, is_folder: true, kids_empty: kids.is_empty() });
    }
    let child_depth = if skip_self { 0 } else { depth + 1 };
    for kid in kids {
        flatten(files, expanded, kid.id, child_depth, out, false);
    }
}

/// ε for view-y range overlap (subpixel scroll).
const BAND_EPS: f32 = 0.5;

/// Content-space Y tops for a flat list. Supports **variable** row heights.
#[derive(Clone, Debug)]
pub struct RowGeom {
    /// Content y of the top of each row (`len == n`).
    tops: Vec<f32>,
    /// Per-row height (`len == n`).
    heights: Vec<f32>,
    /// Sum of heights (scroll content extent).
    pub total: f32,
}

impl RowGeom {
    pub fn uniform(n: usize, h: f32) -> Self {
        let tops: Vec<f32> = (0..n).map(|i| i as f32 * h).collect();
        Self { tops, heights: vec![h; n], total: n as f32 * h }
    }

    pub fn from_heights(heights: &[f32]) -> Self {
        let mut tops = Vec::with_capacity(heights.len());
        let mut y = 0.0_f32;
        for &h in heights {
            tops.push(y);
            y += h;
        }
        Self { tops, heights: heights.to_vec(), total: y }
    }

    pub fn top(&self, i: usize) -> f32 {
        self.tops[i]
    }

    pub fn height(&self, i: usize) -> f32 {
        self.heights[i]
    }

    /// Row index containing content-y `cy` (half-open `[top, bottom)`).
    pub fn index_at_y(&self, cy: f32) -> Option<usize> {
        if self.tops.is_empty() || cy < 0.0 || cy >= self.total {
            return None;
        }
        // Binary search last top ≤ cy.
        let mut lo = 0usize;
        let mut hi = self.tops.len();
        while lo + 1 < hi {
            let mid = (lo + hi) / 2;
            if self.tops[mid] <= cy {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        Some(lo)
    }
}

/// One sticky header this frame. `vy` / `clip_top` are viewport-relative.
#[derive(Clone, Copy, Debug)]
pub struct Stuck {
    pub row: FlatRow,
    /// Row height in content space (pin band thickness).
    pub h: f32,
    pub vy: f32,
    pub clip_top: f32,
}

impl Stuck {
    pub fn height(self) -> f32 {
        (self.vy + self.h - self.clip_top).max(0.0)
    }

    /// Held at pin slot (elevated chrome). False while boundary-pushed.
    pub fn elevated(self) -> bool {
        self.vy >= self.clip_top - 0.01
    }

    /// Visible band after parent clip; `None` if fully under the stack above.
    pub fn visible_band(self) -> Option<(f32, f32)> {
        let top = self.vy.max(self.clip_top);
        let bot = self.vy + self.h;
        (bot - top >= BAND_EPS).then_some((top, bot))
    }

    pub fn overlaps_row(self, row_vy: f32, row_h: f32) -> bool {
        let Some((top, bot)) = self.visible_band() else {
            return false;
        };
        row_vy < bot - BAND_EPS && row_vy + row_h > top + BAND_EPS
    }
}

/// Sticky ancestor stack at scroll `offset`.
///
/// Folders pin when `natural` reaches the stack bottom, hold through their
/// descendants, then boundary-push under parents as the next shallower row
/// arrives. Collapsed folders never pin. Heights come from [`RowGeom`].
pub fn sticky_layout(flat: &[FlatRow], geom: &RowGeom, offset: f32) -> Vec<Stuck> {
    if flat.is_empty() {
        return Vec::new();
    }
    debug_assert_eq!(flat.len(), geom.tops.len());
    let mut out = Vec::new();
    let mut prev_bottom = 0.0_f32;
    for slot in 0usize.. {
        let probe_cy = offset + prev_bottom;
        let Some(probe) = geom.index_at_y(probe_cy) else {
            break;
        };
        if flat[probe].depth < slot {
            break;
        }
        let Some(fi) = ancestor_at_depth(flat, probe, slot) else {
            break;
        };
        if !flat[fi].is_folder || !folder_has_flat_child(flat, fi) {
            break;
        }
        let h = geom.height(fi);
        let natural = geom.top(fi) - offset;
        if natural > prev_bottom - 0.01 {
            break; // still in-flow; continuous handoff at natural == slot
        }
        let boundary = flat[fi + 1..]
            .iter()
            .position(|r| r.depth <= slot)
            .map(|k| geom.top(fi + 1 + k) - offset)
            .unwrap_or(f32::INFINITY);
        let vy = natural.max(prev_bottom).min(boundary - h);
        let stuck = Stuck { row: flat[fi], h, vy, clip_top: prev_bottom };
        if stuck.height() <= 0.0 {
            break;
        }
        out.push(stuck);
        prev_bottom = prev_bottom.max(vy + h);
    }
    out
}

pub fn folder_has_flat_child(flat: &[FlatRow], fi: usize) -> bool {
    let d = flat[fi].depth;
    flat.get(fi + 1).is_some_and(|r| r.depth > d)
}

/// Flat indices to paint in-flow. Stickied folders are drawn by the sticky pass.
///
/// Content **paints** under elevated pins (scroll-under). Only skip bands that
/// share space with a **boundary-pushed** sticky (unstick stand-in). Exclusive
/// hover is handled by clipping in-flow **hit** rects below elevated pins in
/// [`paint_sticky_viewport`] — not by dropping paint.
pub fn painted_inflow(
    flat: &[FlatRow], geom: &RowGeom, sticky: &[Stuck], offset: f32, view_h: f32,
) -> Vec<usize> {
    let view_bot = offset + view_h;
    let mut out = Vec::new();
    for (i, row) in flat.iter().enumerate() {
        if sticky.iter().any(|s| s.row.id == row.id) {
            continue;
        }
        let y = geom.top(i);
        let h = geom.height(i);
        if y + h < offset || y > view_bot {
            continue;
        }
        let row_vy = y - offset;
        if sticky
            .iter()
            .any(|s| !s.elevated() && s.overlaps_row(row_vy, h))
        {
            continue;
        }
        out.push(i);
    }
    out
}

/// Height of sticky ancestor headers when row `i` is in view (folder ancestors
/// only — matches [`sticky_layout`] for an expanded folder walk).
pub fn sticky_band_above(flat: &[FlatRow], geom: &RowGeom, i: usize) -> f32 {
    let depth = flat[i].depth;
    if depth == 0 {
        return 0.0;
    }
    let mut h = 0.0;
    for d in 0..depth {
        if let Some(ai) = ancestor_at_depth(flat, i, d) {
            if flat[ai].is_folder && folder_has_flat_child(flat, ai) {
                h += geom.height(ai);
            }
        }
    }
    h
}

/// Pre-order flatten ancestor at depth `d` enclosing `flat[i]`.
pub fn ancestor_at_depth(flat: &[FlatRow], i: usize, d: usize) -> Option<usize> {
    if flat[i].depth < d {
        return None;
    }
    let mut j = i;
    loop {
        if flat[j].depth == d {
            return Some(j);
        }
        j = j.checked_sub(1)?;
    }
}

#[cfg(test)]
mod sticky_tests {
    use super::*;
    use crate::style::tree_metrics::ROW_H;
    use lb_rs::Uuid;

    fn folder(id: u128, depth: usize) -> FlatRow {
        FlatRow { id: Uuid::from_u128(id), depth, is_folder: true, kids_empty: false }
    }
    fn doc(id: u128, depth: usize) -> FlatRow {
        FlatRow { id: Uuid::from_u128(id), depth, is_folder: false, kids_empty: false }
    }
    fn uni(flat: &[FlatRow]) -> RowGeom {
        RowGeom::uniform(flat.len(), ROW_H)
    }
    fn sticky(flat: &[FlatRow], off: f32) -> Vec<Stuck> {
        sticky_layout(flat, &uni(flat), off)
    }
    fn ids(s: &[Stuck]) -> Vec<u128> {
        s.iter().map(|r| r.row.id.as_u128()).collect()
    }
    fn bands_overlap(a: (f32, f32), b: (f32, f32)) -> bool {
        a.0 < b.1 - BAND_EPS && a.1 > b.0 + BAND_EPS
    }

    #[test]
    fn collapsed_never_sticky() {
        let flat = vec![doc(1, 0), folder(2, 0), doc(3, 0), folder(4, 0), doc(5, 0), folder(6, 0)];
        let mut off = 0.0;
        while off <= flat.len() as f32 * ROW_H + ROW_H {
            assert!(sticky(&flat, off).is_empty(), "collapsed off={off}");
            off += 0.5;
        }
    }

    #[test]
    fn pins_at_natural_handoff() {
        let flat = vec![folder(1, 0), doc(2, 1), doc(3, 1)];
        assert!(sticky(&flat, 0.0).is_empty());
        let s = sticky(&flat, 2.0);
        assert_eq!(ids(&s), vec![1]);
        assert!(s[0].elevated());
        assert!((s[0].vy).abs() < 0.01);
        let mut first = None;
        let mut off = 0.0;
        while off <= ROW_H * 2.0 {
            if !sticky(&flat, off).is_empty() {
                first = Some(off);
                break;
            }
            off += 0.25;
        }
        assert!(first.expect("should pin") < ROW_H * 0.25);
    }

    #[test]
    fn deep_chain_and_shallow_drop() {
        let deep = vec![folder(1, 0), folder(2, 1), folder(3, 2), doc(4, 3)];
        let s = sticky(&deep, ROW_H + 1.0);
        assert_eq!(ids(&s), vec![1, 2, 3]);
        for (i, stuck) in s.iter().enumerate() {
            assert!(stuck.elevated());
            assert!((stuck.vy - i as f32 * ROW_H).abs() < 0.01);
            assert!((stuck.h - ROW_H).abs() < 0.01);
        }

        let branch = vec![folder(1, 0), folder(2, 1), doc(3, 2), doc(4, 0)];
        assert_eq!(ids(&sticky(&branch, ROW_H + 1.0)), vec![1, 2]);
        assert!(sticky(&branch, ROW_H * 3.0 + 1.0).is_empty());
    }

    #[test]
    fn stays_pinned_through_descendants() {
        let mut flat = vec![folder(1, 0)];
        for i in 0..20u128 {
            flat.push(doc(100 + i, 1));
        }
        let mut saw = false;
        let mut off = 0.0;
        while off <= ROW_H * 15.0 {
            let s = sticky(&flat, off);
            if saw {
                assert_eq!(ids(&s), vec![1], "lost sticky at off={off}");
            } else if !s.is_empty() {
                assert_eq!(ids(&s), vec![1]);
                saw = true;
            }
            off += 0.5;
        }
        assert!(saw);
    }

    #[test]
    fn unstick_pushes_under_with_flow_style() {
        let flat = vec![folder(1, 0), folder(2, 1), doc(3, 2), doc(4, 2), doc(5, 2), doc(6, 0)];
        let held = sticky(&flat, ROW_H * 2.0);
        assert_eq!(ids(&held), vec![1, 2]);
        assert!(held.iter().all(|s| s.elevated()));

        let mid = sticky(&flat, ROW_H * 3.5);
        assert_eq!(ids(&mid), vec![1, 2]);
        assert!(mid[0].elevated());
        assert!(!mid[1].elevated());
        let expect_vy = 5.0 * ROW_H - ROW_H * 3.5 - ROW_H;
        assert!((mid[1].vy - expect_vy).abs() < 0.01);

        let mut prev_vy = f32::INFINITY;
        let mut saw_push = false;
        let mut off = ROW_H + 1.0;
        while off <= ROW_H * 5.5 {
            let s = sticky(&flat, off);
            if let Some(deep) = s.iter().find(|x| x.row.id.as_u128() == 2) {
                assert!(deep.vy <= prev_vy + 0.01, "teleport at off={off}");
                if !deep.elevated() {
                    saw_push = true;
                }
                prev_vy = deep.vy;
            } else if saw_push {
                break;
            }
            off += 0.5;
        }
        assert!(saw_push);
    }

    #[test]
    fn painted_geometry_list_order_and_unstick() {
        let flat = vec![folder(1, 0), folder(2, 1), doc(3, 2), doc(4, 2), doc(5, 2), doc(6, 0)];
        let geom = uni(&flat);
        let view_h = ROW_H * 12.0;
        let mut off = 0.0;
        let mut saw_behind = false;
        let mut saw_push = false;
        while off <= geom.total + ROW_H {
            let sticky = sticky_layout(&flat, &geom, off);
            let inflow = painted_inflow(&flat, &geom, &sticky, off, view_h);

            for s in &sticky {
                let Some(s_band) = s.visible_band() else {
                    continue;
                };
                for &i in &inflow {
                    let row_vy = geom.top(i) - off;
                    let overlaps = bands_overlap(s_band, (row_vy, row_vy + geom.height(i)));
                    if s.elevated() {
                        // Content still paints under elevated pins (scroll-under).
                        saw_behind |= overlaps;
                    } else {
                        assert!(
                            !overlaps,
                            "pushed sticky {} overlaps in-flow {} at off={off}",
                            s.row.id.as_u128(),
                            flat[i].id.as_u128(),
                        );
                        saw_push = true;
                    }
                }
                saw_push |= !s.elevated();
            }

            for (i, a) in sticky.iter().enumerate() {
                let Some(a_band) = a.visible_band() else {
                    continue;
                };
                for b in sticky.iter().skip(i + 1) {
                    if let Some(b_band) = b.visible_band() {
                        assert!(!bands_overlap(a_band, b_band), "sticky×sticky at off={off}");
                    }
                }
            }
            off += 0.25;
        }
        assert!(saw_behind, "expected content under a held sticky");
        assert!(saw_push, "expected a boundary-push phase");
    }

    /// Tall sticky over short kids: pin band uses sticky height, not ROW_H.
    #[test]
    fn variable_height_sticky_band() {
        let tall = FileRow::height_for(true);
        let short = ROW_H;
        assert!(tall > short);
        let flat = vec![folder(1, 0), doc(2, 1), doc(3, 1), doc(4, 1)];
        let geom = RowGeom::from_heights(&[tall, short, short, short]);
        // Scroll past natural of tall root.
        let off = tall * 0.5;
        let s = sticky_layout(&flat, &geom, off);
        assert_eq!(ids(&s), vec![1]);
        assert!((s[0].h - tall).abs() < 0.01);
        assert!(s[0].elevated());
        // Boundary-push when last short child reaches pin bottom.
        let push_off = geom.top(3) + short * 0.5;
        let mid = sticky_layout(&flat, &geom, push_off);
        if let Some(st) = mid.first() {
            assert_eq!(st.row.id.as_u128(), 1);
            assert!((st.h - tall).abs() < 0.01);
        }
    }

    #[test]
    fn index_at_y_variable() {
        let geom = RowGeom::from_heights(&[40.0, 20.0, 60.0]);
        assert_eq!(geom.index_at_y(0.0), Some(0));
        assert_eq!(geom.index_at_y(39.9), Some(0));
        assert_eq!(geom.index_at_y(40.0), Some(1));
        assert_eq!(geom.index_at_y(59.9), Some(1));
        assert_eq!(geom.index_at_y(60.0), Some(2));
        assert_eq!(geom.index_at_y(119.9), Some(2));
        assert_eq!(geom.index_at_y(120.0), None);
    }
}
