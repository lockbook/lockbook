//! Files tree: canvas body, sticky folder headers, DnD.
//!
//! Flat expanded walk painted in list order. Row pitch may **vary** (prefix
//! sums in [`RowGeom`]); sticky math uses each row's height. Sticky ancestors
//! overlay scrolled content. Unstick is the exception: a boundary-pushed
//! folder stands in for its last descendant. Metrics in
//! [`crate::components::foundation::tree_metrics`]; scrollbars use overlay style.
//!
//! ## Surfaces & row interactivity
//!
//! All sticky / virtualized lists paint through [`paint_tree_file_row`] so
//! chrome (depth, elevated pin, content inset, wash) stays one plan.
//!
//! | Surface | Folders | Non-folders |
//! |---------|---------|-------------|
//! | **Files** nav | expand + select + drag | open + drag |
//! | **Recents** | — | open |
//! | **Shared** | expand | open (+ Save on roots) |
//! | **Folder pick** (create/move/import/accept) | select + expand | *folders only* |
//! | **Delete preview** | expand only | **omitted when roots are files-only**
//! | | | (summary names them); under a folder: static, no wash |
//!
//! Hover wash means “this row does something.” Delete shows a tree only when
//! at least one root is a folder (cascade / expand). Files-only confirms are
//! summary copy alone. Docs under an expanded folder stay static.

use std::time::Duration;

use egui::{
    Color32, CursorIcon, DragAndDrop, Id, LayerId, Layout, Order, Rect, ScrollArea, Sense, Stroke,
    StrokeKind, Ui, pos2, vec2,
};
use lb::Uuid;
use workspace_rs::file_cache::{FileCache, FilesExt};

use crate::components::domain::pins;
use crate::components::{
    LIST_PAD, ROW_H, Radius, STROKE_HAIRLINE, Space, Spacer, Theme, TypeRole, context_menu,
    phosphor, with_overlay_scroll,
};
use crate::shell::ShellApp;
use crate::shell::action::Action;
use crate::shell::ops::{ids_are_saved_shares, is_pinned};

/// Dwell before expanding a collapsed folder under a drag.
const DROP_EXPAND_SECS: f64 = 0.6;
/// Viewport edge band that drives auto-scroll while dragging.
const DROP_EDGE_BAND: f32 = 28.0;
/// Max edge-scroll speed (points / second) at full band penetration.
const DROP_EDGE_SPEED: f32 = 900.0;

// ── Files tree scroll animator (reveal / tab open → into view, not center) ─────
/// Snap if closer than this (no tween noise).
const SCROLL_SNAP_PX: f32 = 4.0;
/// Distance → duration scale; clamped by min/max below.
const SCROLL_PPS: f32 = 1200.0;
const SCROLL_DUR_MIN: f32 = 0.08;
const SCROLL_DUR_MAX: f32 = 0.22;

#[derive(Clone, Copy, Debug)]
struct TreeScrollAnim {
    id: Uuid,
    from: f32,
    to: f32,
    t0: f64,
    duration: f32,
}

// egui `remove_temp` requires Default (placeholder only — never read as live anim).
impl Default for TreeScrollAnim {
    fn default() -> Self {
        Self { id: Uuid::nil(), from: 0.0, to: 0.0, t0: 0.0, duration: 0.0 }
    }
}

fn tree_scroll_anim_id() -> Id {
    Id::new("shell_tree_scroll_anim")
}

fn tree_scroll_last_y_id() -> Id {
    Id::new("shell_tree_scroll_last_y")
}

fn clear_tree_scroll_anim(ui: &Ui) {
    ui.ctx().data_mut(|d| {
        d.remove_temp::<TreeScrollAnim>(tree_scroll_anim_id());
    });
}

fn ease_in_out_cubic(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 { 4.0 * t * t * t } else { 1.0 - (-2.0 * t + 2.0).powi(3) / 2.0 }
}

fn scroll_duration(dist: f32) -> f32 {
    if dist < SCROLL_SNAP_PX {
        0.0
    } else {
        (dist / SCROLL_PPS).clamp(SCROLL_DUR_MIN, SCROLL_DUR_MAX)
    }
}

/// Scroll offset that shows row `i` in the free viewport under sticky pins.
/// Already fully visible → keep `cur_off` (no re-center on click).
fn offset_reveal_row(
    flat: &[FlatRow], geom: &RowGeom, i: usize, band_h: f32, max_off: f32, cur_off: f32,
) -> f32 {
    let sticky_h = sticky_band_above(flat, geom, i);
    let row_h = geom.height(i);
    let row_top = geom.top(i);
    let row_bot = row_top + row_h;
    let pad = Space::Xs.pts();
    let vis_top = cur_off + sticky_h + pad;
    let vis_bot = cur_off + band_h - pad;
    let free_h = (band_h - sticky_h - 2.0 * pad).max(0.0);
    if row_h >= free_h {
        return (row_top - sticky_h - pad).clamp(0.0, max_off);
    }
    if row_top >= vis_top && row_bot <= vis_bot {
        return cur_off.clamp(0.0, max_off);
    }
    if row_top < vis_top {
        return (row_top - sticky_h - pad).clamp(0.0, max_off);
    }
    (row_bot - band_h + pad).clamp(0.0, max_off)
}

#[derive(Clone, Debug)]
struct DragIds(pub Vec<Uuid>);

/// Hover hit for drag (sticky pin floor via hit_rect). Outline geometry comes
/// from the flat list span, not from these rects.
#[derive(Clone, Copy, Debug)]
struct DropHitRow {
    id: Uuid,
    is_folder: bool,
    /// File's parent, or folder's own id when the row is a folder.
    parent_for_alongside: Uuid,
    hit_rect: Rect,
}

fn drop_hit_list_id() -> Id {
    Id::new("shell_tree_drop_hits")
}

fn drop_dwell_id() -> Id {
    Id::new("shell_tree_drop_dwell")
}

fn drop_edge_scroll_id() -> Id {
    Id::new("shell_tree_edge_scroll_y")
}

fn drop_hit_begin(ui: &Ui) {
    ui.ctx()
        .data_mut(|d| d.insert_temp(drop_hit_list_id(), Vec::<DropHitRow>::new()));
}

fn drop_hit_push(ui: &Ui, row: DropHitRow) {
    ui.ctx().data_mut(|d| {
        let list: &mut Vec<DropHitRow> = d.get_temp_mut_or_default(drop_hit_list_id());
        list.push(row);
    });
}

fn drop_hit_take(ui: &Ui) -> Vec<DropHitRow> {
    ui.ctx()
        .data_mut(|d| d.remove_temp::<Vec<DropHitRow>>(drop_hit_list_id()))
        .unwrap_or_default()
}

fn clear_drop_dwell(ui: &Ui) {
    ui.ctx().data_mut(|d| {
        d.remove_temp::<(Uuid, f64)>(drop_dwell_id());
    });
}

/// Vertical edge auto-scroll while a tree drag is active. Writes next-frame offset.
fn tree_edge_scroll(ui: &Ui, viewport: Rect, content_total: f32) {
    let Some(pointer) = ui.input(|i| i.pointer.interact_pos()) else {
        return;
    };
    let clip = ui.clip_rect();
    if clip.height() < DROP_EDGE_BAND * 2.0 {
        return;
    }
    let dt = ui.input(|i| i.unstable_dt).clamp(1.0 / 240.0, 0.05);
    let max_off = (content_total - viewport.height()).max(0.0);
    if max_off <= 0.5 {
        return;
    }
    let mut off = viewport.min.y;
    let mut moved = false;
    if pointer.y < clip.top() + DROP_EDGE_BAND {
        let depth = ((clip.top() + DROP_EDGE_BAND - pointer.y) / DROP_EDGE_BAND).clamp(0.0, 1.0);
        off = (off - DROP_EDGE_SPEED * depth * dt).max(0.0);
        moved = true;
    } else if pointer.y > clip.bottom() - DROP_EDGE_BAND {
        let depth =
            ((pointer.y - (clip.bottom() - DROP_EDGE_BAND)) / DROP_EDGE_BAND).clamp(0.0, 1.0);
        off = (off + DROP_EDGE_SPEED * depth * dt).min(max_off);
        moved = true;
    }
    if moved && (off - viewport.min.y).abs() > 0.25 {
        ui.ctx()
            .data_mut(|d| d.insert_temp(drop_edge_scroll_id(), off));
        ui.ctx().request_repaint();
    }
}

/// Minimal drag ghost: name (+N for multi) near the pointer. Confirms *what* is held;
/// destination chrome confirms *where*.
fn paint_drag_ghost(ui: &Ui, t: &Theme, files: &FileCache, ids: &[Uuid], pointer: egui::Pos2) {
    if ids.is_empty() {
        return;
    }
    let primary = files
        .get_by_id(ids[0])
        .map(|f| f.name.clone())
        .unwrap_or_else(|| "Item".into());
    let label = if ids.len() > 1 { format!("{primary}  +{}", ids.len() - 1) } else { primary };
    let font = TypeRole::Body.font_id();
    let color = t.neutral_fg();
    let galley = ui.fonts(|f| f.layout(label, font, color, 180.0));
    let pad_x = Space::Sm.pts();
    let pad_y = Space::Xs.pts();
    let size = galley.size() + vec2(pad_x * 2.0, pad_y * 2.0);
    let origin = pointer + vec2(12.0, 10.0);
    let rect = Rect::from_min_size(origin, size);
    let painter = ui
        .ctx()
        .layer_painter(LayerId::new(Order::Tooltip, Id::new("shell_dnd_ghost")));
    let fill = t.neutral_bg_secondary();
    let fill = Color32::from_rgba_unmultiplied(fill.r(), fill.g(), fill.b(), 230);
    let r = Radius::Control.pts() as f32;
    painter.rect_filled(rect, r, fill);
    painter.rect_stroke(rect, r, Stroke::new(STROKE_HAIRLINE, t.neutral()), StrokeKind::Outside);
    painter.galley(rect.min + vec2(pad_x, pad_y), galley, color);
}

/// Dwell timer for spring-loading a collapsed folder. Returns `Some(id)` when
/// the timer fires (caller inserts into `expanded`).
///
/// **Reset only on target folder change** — not on pointer micro-moves, and not
/// when the hit briefly misses or lands on a non-folder (those used to call
/// [`clear_drop_dwell`] and restart the 600 ms clock).
///
/// While dwelling, schedule a wake for the remaining time so a still pointer
/// still fires (~idle egui stops painting without `request_repaint*`).
fn try_drop_expand(ui: &Ui, candidate: Option<Uuid>, already_expanded: bool) -> Option<Uuid> {
    let Some(folder_id) = candidate else {
        // Keep last (id, t0). Fire only while still over that folder.
        return None;
    };
    if already_expanded {
        clear_drop_dwell(ui);
        return None;
    }
    let now = ui.input(|i| i.time);
    let (fire, started) = ui.ctx().data_mut(|d| {
        let st = d.get_temp_mut_or_insert_with(drop_dwell_id(), || (folder_id, now));
        if st.0 != folder_id {
            *st = (folder_id, now);
            (false, now)
        } else {
            (now - st.1 >= DROP_EXPAND_SECS, st.1)
        }
    });
    if fire {
        clear_drop_dwell(ui);
        ui.ctx().request_repaint();
        Some(folder_id)
    } else {
        let remaining = (DROP_EXPAND_SECS - (now - started)).max(0.0);
        // Wake at deadline; floor so a zero remaining still runs a frame.
        ui.ctx()
            .request_repaint_after(Duration::from_secs_f64(remaining.max(1.0 / 120.0)));
        None
    }
}

/// True if `id` is `ancestor` or nested under it.
fn is_under_or_eq(files: &FileCache, id: Uuid, ancestor: Uuid) -> bool {
    if id == ancestor {
        return true;
    }
    let mut cur = id;
    for _ in 0..256 {
        let Some(f) = files.get_by_id(cur) else {
            return false;
        };
        if f.parent == ancestor {
            return true;
        }
        if f.parent == cur {
            return false;
        }
        cur = f.parent;
    }
    false
}

/// Cycle / self: cannot move `id` into `parent` if parent is under id.
fn move_into_ok(files: &FileCache, ids: &[Uuid], parent: Uuid) -> bool {
    for &id in ids {
        if id == parent {
            return false;
        }
        if is_under_or_eq(files, parent, id) {
            return false;
        }
    }
    true
}

/// Hover row → destination folder (folder itself, or file's parent).
fn drop_dest_parent(row: DropHitRow) -> Uuid {
    if row.is_folder { row.id } else { row.parent_for_alongside }
}

/// Contiguous flat span for a folder: `[start, end]` inclusive (end may equal
/// start when collapsed / no expanded kids in the walk).
fn folder_flat_span(flat: &[FlatRow], folder_id: Uuid) -> Option<(usize, usize)> {
    let start = flat.iter().position(|r| r.id == folder_id)?;
    let d = flat[start].depth;
    let mut end = start;
    for (j, row) in flat.iter().enumerate().skip(start + 1) {
        if row.depth <= d {
            break;
        }
        end = j;
    }
    Some((start, end))
}

/// Absolute top of row `i` as drawn (sticky slot if sticky, else content y).
fn row_abs_top(
    i: usize, flat: &[FlatRow], geom: &RowGeom, sticky: &[Stuck], content_min: egui::Pos2,
    view_top: f32,
) -> f32 {
    let id = flat[i].id;
    if let Some(s) = sticky.iter().find(|s| s.row.id == id) {
        view_top + s.vy
    } else {
        content_min.y + geom.top(i)
    }
}

fn row_abs_bottom(
    i: usize, flat: &[FlatRow], geom: &RowGeom, sticky: &[Stuck], content_min: egui::Pos2,
    view_top: f32,
) -> f32 {
    let id = flat[i].id;
    if let Some(s) = sticky.iter().find(|s| s.row.id == id) {
        view_top + s.vy + s.h
    } else {
        content_min.y + geom.top(i) + geom.height(i)
    }
}

/// Drop outline: top = dest folder as shown; bottom = last flat descendant.
/// Lives in scroll content; clip does the rest — no “visible-only” union.
fn drop_group_outline(
    flat: &[FlatRow], geom: &RowGeom, sticky: &[Stuck], parent: Uuid, content_min: egui::Pos2,
    view_screen: Rect, content_pad: f32,
) -> Option<Rect> {
    let (start, end) = folder_flat_span(flat, parent)?;
    let view_top = view_screen.top();
    let top = row_abs_top(start, flat, geom, sticky, content_min, view_top);
    let bot = row_abs_bottom(end, flat, geom, sticky, content_min, view_top);
    let left = view_screen.left() + content_pad;
    let right = view_screen.right() - content_pad;
    if right <= left + 1.0 {
        return None;
    }
    Some(Rect::from_min_max(pos2(left, top), pos2(right, bot.max(top + 1.0))))
}

/// After viewport paint: ghost, edge-scroll, dwell-expand, dest outline, commit.
///
/// Cursor stays **Grabbing** for the whole drag (egui default while payload is
/// set). Valid dest = group outline only — no NotAllowed scold on no-op/invalid.
///
/// Returns a folder id to expand when dwell fires (caller mutates `expanded`).
fn finish_tree_drop(
    ui: &Ui, t: &Theme, queue: &mut Vec<Action>, files: &FileCache, flat: &[FlatRow],
    geom: &RowGeom, viewport: Rect, content_pad: f32, content_total: f32,
    expanded: &std::collections::HashSet<Uuid>, root: Uuid,
) -> Option<Uuid> {
    let hits = drop_hit_take(ui);
    let Some(payload) = DragAndDrop::payload::<DragIds>(ui.ctx()) else {
        clear_drop_dwell(ui);
        return None;
    };
    // Dragging affordance for the full hold — never flip to an error cursor.
    ui.ctx().set_cursor_icon(CursorIcon::Grabbing);

    let pointer = ui.input(|i| i.pointer.interact_pos())?;

    paint_drag_ghost(ui, t, files, &payload.0, pointer);
    tree_edge_scroll(ui, viewport, content_total);

    let content_min = ui.max_rect().min;
    let view_screen = Rect::from_min_size(content_min + viewport.min.to_vec2(), viewport.size());
    let clip = view_screen.intersect(ui.clip_rect());
    let sticky = sticky_layout(flat, geom, viewport.min.y);

    // Sticky last in the hit list → prefer topmost under the pointer.
    let hover = hits.iter().rev().find(|r| r.hit_rect.contains(pointer));

    // Spring-load: only a collapsed *folder row* is a candidate. Misses / file
    // rows do not clear the dwell clock (that was resetting on any wiggle).
    let expand_candidate = hover.and_then(|h| {
        if !h.is_folder {
            return None;
        }
        let parent = drop_dest_parent(*h);
        if !move_into_ok(files, &payload.0, parent) {
            return None;
        }
        Some(h.id)
    });
    let already = expand_candidate.is_some_and(|id| expanded.contains(&id));
    let expand = try_drop_expand(ui, expand_candidate, already);

    // Empty canvas below the last row is the account root (same as the
    // background context menu). Gutters beside a row are not — those stay a miss
    // so they don't steal a folder dest.
    let parent = if let Some(hover) = hover {
        drop_dest_parent(*hover)
    } else if pointer_below_tree(pointer, clip, flat, geom, &sticky, content_min, view_screen) {
        root
    } else {
        return expand;
    };
    if !move_into_ok(files, &payload.0, parent) {
        return expand;
    }

    let all_already = payload
        .0
        .iter()
        .all(|&id| files.get_by_id(id).is_some_and(|f| f.parent == parent));
    if all_already {
        return expand;
    }

    let group = if parent == root {
        root_drop_outline(view_screen, content_pad)
    } else {
        drop_group_outline(flat, geom, &sticky, parent, content_min, view_screen, content_pad)
    };
    if let Some(group) = group {
        // Clip to scroll viewport — outline is content, not a separate layer.
        ui.painter().with_clip_rect(clip).rect_stroke(
            group,
            Radius::Control.pts() as f32,
            Stroke::new(STROKE_HAIRLINE, t.accent()),
            StrokeKind::Inside,
        );
    }
    if ui.input(|i| i.pointer.any_released()) {
        queue.push(Action::MoveInto { ids: payload.0.clone(), parent });
        DragAndDrop::clear_payload(ui.ctx());
        clear_drop_dwell(ui);
    }
    expand
}

/// Pointer is in the tree viewport, below the last painted row (or the tree is empty).
fn pointer_below_tree(
    pointer: egui::Pos2, clip: Rect, flat: &[FlatRow], geom: &RowGeom, sticky: &[Stuck],
    content_min: egui::Pos2, view_screen: Rect,
) -> bool {
    if !clip.contains(pointer) {
        return false;
    }
    if flat.is_empty() {
        return true;
    }
    let last = flat.len() - 1;
    let last_bot = row_abs_bottom(last, flat, geom, sticky, content_min, view_screen.top());
    pointer.y >= last_bot
}

/// Accent plate around the whole Files list (account root drop).
fn root_drop_outline(view_screen: Rect, content_pad: f32) -> Option<Rect> {
    let left = view_screen.left() + content_pad;
    let right = view_screen.right() - content_pad;
    if right <= left + 1.0 || view_screen.height() < 4.0 {
        return None;
    }
    Some(Rect::from_min_max(pos2(left, view_screen.top()), pos2(right, view_screen.bottom())))
}

pub(crate) use workspace_rs::style::{
    FlatRow, RowGeom, Stuck, TreeRowChrome, flatten, paint_sticky_viewport, paint_tree_file_row,
    row_type_icon, sticky_band_above, sticky_layout,
};

#[derive(Clone, Debug)]
pub(crate) enum FileCmd {
    Open,
    OpenNewTab,
    Share,
    Rename,
    Pin,
    Move,
    Delete,
    /// Opens create sheet (location from row; type always Note).
    Create,
    Duplicate,
    Export,
    CopyLink,
    ExpandAll,
    CollapseAll,
    /// Pending share → destination picker.
    OrganizeShare,
    /// Pending share → decline confirm.
    DeclineShare,
}

pub fn show_tree(app: &mut ShellApp, ui: &mut Ui, t: &Theme, queue: &mut Vec<Action>) {
    if let Some(ready) = app.session.ready() {
        let pins = ready.pinned.clone();
        let files = ready.workspace.files.read().unwrap();
        pins::show(ui, t, &*files, &pins, &ready.workspace.account.username, queue);
    }

    let (root, flat) = {
        let Some(ready) = app.session.ready_mut() else {
            return;
        };
        let files = ready.workspace.files.read().unwrap();
        let root = files.root().id;
        ready.expanded.insert(root);
        let mut flat = Vec::new();
        flatten(&*files, &ready.expanded, root, 0, &mut flat, true);
        ready.nav_order = flat.iter().map(|r| r.id).collect();
        (root, flat)
    };

    if flat.is_empty() {
        empty_state(ui, t, "No files");
        return;
    }

    if let Some(ready) = app.session.ready() {
        let files = ready.workspace.files.read().unwrap();
        app.sync_dots
            .refresh(ui.ctx(), &ready.status, &ready.expanded, &*files);
    }

    // Files tree: uniform single-line pitch (variable path ready for Shared).
    let geom = RowGeom::uniform(flat.len(), ROW_H);
    let scroll_id = Id::new("shell_tree_scroll");

    // Residual band after pins: full parent width × height from cursor to max_rect.
    let band_w = crate::components::ui_width(ui);
    let band_h = crate::components::remaining_height(ui).max(ROW_H);
    let band = egui::vec2(band_w, band_h);
    if band.x < 1.0 || band.y < 1.0 {
        return;
    }

    let bottom_pad = 3.0 * ROW_H;
    let content_total = geom.total + bottom_pad;
    let max_off = (content_total - band_h).max(0.0);
    let now = ui.input(|i| i.time);
    let cur_y = ui
        .ctx()
        .data(|d| d.get_temp::<f32>(tree_scroll_last_y_id()))
        .unwrap_or(0.0);
    // User wheel/trackpad owns the viewport — cancel programmatic anim.
    let user_scroll = ui.input(|i| i.smooth_scroll_delta.y != 0.0 || i.raw_scroll_delta.y != 0.0);

    let mut scroll = ScrollArea::vertical()
        .id_salt("shell_tree_scroll")
        .auto_shrink([false, false])
        // We drive offsets ourselves (sticky math); don't also run stock scroll_to_*.
        .animated(false);

    // 1) DnD edge scroll wins (velocity), kills reveal/keyboard anim.
    if let Some(y) = ui
        .ctx()
        .data_mut(|d| d.remove_temp::<f32>(drop_edge_scroll_id()))
    {
        clear_tree_scroll_anim(ui);
        if let Some(r) = app.session.ready_mut() {
            r.tree_scroll = None;
        }
        scroll = scroll.vertical_scroll_offset(y);
    } else if user_scroll {
        clear_tree_scroll_anim(ui);
        if let Some(r) = app.session.ready_mut() {
            r.tree_scroll = None;
        }
    } else {
        // 2) Pending reveal (held while ancestors expand until id is in flat).
        let intent = app.session.ready().and_then(|r| r.tree_scroll);
        if let Some(id) = intent {
            if let Some(i) = flat.iter().position(|r| r.id == id) {
                let from = cur_y;
                let to = offset_reveal_row(&flat, &geom, i, band_h, max_off, from);
                let dist = (to - from).abs();
                if dist >= SCROLL_SNAP_PX {
                    let duration = scroll_duration(dist);
                    let prev = ui
                        .ctx()
                        .data(|d| d.get_temp::<TreeScrollAnim>(tree_scroll_anim_id()));
                    let retarget = prev.is_none_or(|a| a.id != id || (a.to - to).abs() > 1.0);
                    if retarget {
                        ui.ctx().data_mut(|d| {
                            d.insert_temp(
                                tree_scroll_anim_id(),
                                TreeScrollAnim { id, from, to, t0: now, duration },
                            );
                        });
                    }
                }
                // Consumed once the row exists — anim carries the rest.
                if let Some(r) = app.session.ready_mut() {
                    r.tree_scroll = None;
                }
            }
            // else: still expanding — keep intent for next frame
        }

        // 3) Tick animator (recompute target each frame for sticky/expand).
        let anim = ui
            .ctx()
            .data(|d| d.get_temp::<TreeScrollAnim>(tree_scroll_anim_id()));
        if let Some(mut anim) = anim {
            if let Some(i) = flat.iter().position(|r| r.id == anim.id) {
                anim.to = offset_reveal_row(&flat, &geom, i, band_h, max_off, anim.from);
                let y = if anim.duration <= 0.0 {
                    anim.to
                } else {
                    let t = ((now - anim.t0) / f64::from(anim.duration)) as f32;
                    if t >= 1.0 {
                        anim.to
                    } else {
                        let e = ease_in_out_cubic(t);
                        anim.from + (anim.to - anim.from) * e
                    }
                };
                let done = anim.duration <= 0.0
                    || (now - anim.t0) >= f64::from(anim.duration)
                    || (y - anim.to).abs() < 0.5;
                if done {
                    clear_tree_scroll_anim(ui);
                    scroll = scroll.vertical_scroll_offset(anim.to);
                } else {
                    ui.ctx()
                        .data_mut(|d| d.insert_temp(tree_scroll_anim_id(), anim));
                    ui.ctx().request_repaint();
                    scroll = scroll.vertical_scroll_offset(y);
                }
            } else {
                // Row left the flat walk mid-anim (collapsed, etc.).
                clear_tree_scroll_anim(ui);
            }
        }
    }
    // Full-width scroll: sticky plates paint edge-to-edge. Content inset
    // (LIST_PAD) lives inside paint_sticky_viewport / FileRow — not outer Spacers
    // that would leave gutters beside elevated headers.
    ui.allocate_ui_with_layout(band, Layout::top_down(egui::Align::Min), |ui| {
        ui.set_min_size(band);
        ui.set_max_size(band);
        let tree_h = band.y.max(ROW_H);
        let content_pad = LIST_PAD.pts();
        with_overlay_scroll(ui, scroll_id, |ui| {
            let out = scroll
                .max_height(tree_h)
                .min_scrolled_height(tree_h)
                .show_viewport(ui, |ui, viewport| {
                    drop_hit_begin(ui);
                    let bg = paint_sticky_viewport(
                        ui,
                        t,
                        &flat,
                        &geom,
                        viewport,
                        bottom_pad,
                        0, // square pins
                        content_pad,
                        |ui, t, row, paint_r, hit_r, elev, _pin_vy| {
                            // Elevated: full-bleed paint_rect; content_inset = pad.
                            // Inflow: paint_rect already inset; content_inset = 0.
                            let inset = if elev { content_pad } else { 0.0 };
                            paint_row(app, ui, t, queue, row, paint_r, hit_r, elev, inset);
                        },
                    );
                    // Ghost / edge-scroll / dwell-expand / outline / commit.
                    let expand_id = if let Some(ready) = app.session.ready() {
                        let expanded = ready.expanded.clone();
                        let files = ready.workspace.files.read().unwrap();
                        finish_tree_drop(
                            ui,
                            t,
                            queue,
                            &files,
                            &flat,
                            &geom,
                            viewport,
                            content_pad,
                            content_total,
                            &expanded,
                            root,
                        )
                    } else {
                        let _ = drop_hit_take(ui);
                        clear_drop_dwell(ui);
                        None
                    };
                    if let Some(id) = expand_id {
                        if let Some(r) = app.session.ready_mut() {
                            r.expanded.insert(id);
                        }
                    }
                    if bg.clicked() {
                        queue.push(Action::SetSelection(vec![]));
                    }
                    if let Some(cmd) = context_menu::show(&bg, t, |e| {
                        e.item(phosphor::NOTE_PENCIL, "Create…", FileCmd::Create);
                        if flat.iter().any(|r| r.is_folder) {
                            e.separator();
                            e.item(phosphor::CARET_DOWN, "Expand all", FileCmd::ExpandAll);
                            e.item(phosphor::CARET_LEFT, "Collapse all", FileCmd::CollapseAll);
                        }
                    }) {
                        match cmd {
                            FileCmd::Create => {
                                queue.push(Action::OpenCreate {
                                    folder: Some(root),
                                    alongside: None,
                                    is_folder: false,
                                });
                            }
                            FileCmd::ExpandAll => queue.push(Action::ExpandSubtree(root)),
                            FileCmd::CollapseAll => queue.push(Action::CollapseSubtree(root)),
                            _ => {}
                        }
                    }
                });
            // Feed next frame's animator / ensure-visible baseline.
            ui.ctx()
                .data_mut(|d| d.insert_temp(tree_scroll_last_y_id(), out.state.offset.y));
            ((), out.state.offset.y, out.id)
        });
    });
}

/// Max full rows the delete list keeps visible (then scroll).
pub(crate) mod folder;
pub(crate) mod recents;
pub(crate) mod shared;

pub use folder::{
    expand_ancestors_of, folder_tree_default_height, folder_tree_scroll_key,
    is_forbidden_move_dest, show_delete_tree, show_folder_tree,
};
pub use recents::show_recents;
pub use shared::show_shared;

fn paint_row(
    app: &mut ShellApp, ui: &mut Ui, t: &Theme, queue: &mut Vec<Action>, row: FlatRow,
    paint_rect: Rect, hit_rect: Rect, elevated: bool, content_inset: f32,
) {
    let Some(ready) = app.session.ready() else {
        return;
    };
    let kids_empty = row.kids_empty;
    let selected = ready.selected.contains(&row.id) || ready.cursor == Some(row.id);
    let expanded = ready.expanded.contains(&row.id);
    let pinned = is_pinned(ready, row.id);
    let multi = ready.selection_vec();
    let (name, create_parent, is_shared, targets_are_saved_shares) = {
        let files = ready.workspace.files.read().unwrap();
        let Some(f) = files.get_by_id(row.id) else {
            return;
        };
        // Drop dest: into a folder, or alongside a document (same parent).
        let parent = if row.is_folder { row.id } else { f.parent };
        // Classic tree: share chrome when the file itself carries share metadata.
        let shared = !f.shares.is_empty();
        let targets =
            if multi.len() > 1 && multi.contains(&row.id) { &multi[..] } else { &[row.id][..] };
        let links = ids_are_saved_shares(&*files, targets, &ready.workspace.account.username);
        (f.name.clone(), parent, shared, links)
    };
    let sync_dot = app.sync_dots.color_for(row.id, t);

    // Open/closed folder icon (no chevron). Whole-row click toggles.
    // Shared folders still use open/closed; a trailing people mark carries share.
    let icon = row_type_icon(&name, row.is_folder, expanded);

    // Namespace elevated stickies so a pinned header and its off-screen twin
    // don't share hover/anim state; boundary-pushed stickies share in-flow id.
    let id = ui.id().with("tree_row").with(row.id).with(elevated);
    // Files nav: every row is interactive (folders expand, docs open, both drag).
    let chrome = TreeRowChrome::new(row.depth)
        .elevated(elevated)
        .content_inset(content_inset)
        .selected(selected);
    let resp = paint_tree_file_row(ui, t, name, icon, chrome, id, paint_rect, hit_rect, |r| {
        r.pinned(pinned)
            .shared(is_shared)
            .sync_dot(sync_dot)
            .sense(Sense::click_and_drag())
    });

    // Drag/hover extras: geometry on the hit band only (not under elevated pins).
    let over = ui
        .ctx()
        .rect_contains_pointer(ui.layer_id(), hit_rect.intersect(ui.clip_rect()));

    // Drag start + collect paint rects for group outline (finished after viewport).
    if resp.drag_started() {
        let ids =
            if multi.contains(&row.id) && multi.len() > 1 { multi.clone() } else { vec![row.id] };
        DragAndDrop::set_payload(ui.ctx(), DragIds(ids));
    }
    if DragAndDrop::has_payload_of_type::<DragIds>(ui.ctx()) {
        let parent_for_alongside = if row.is_folder {
            row.id
        } else {
            create_parent // file's parent (alongside)
        };
        drop_hit_push(
            ui,
            DropHitRow {
                id: row.id,
                is_folder: row.is_folder,
                parent_for_alongside,
                // Match click hit (under elevated pins, inflow is clipped).
                hit_rect: hit_rect.intersect(ui.clip_rect()),
            },
        );
    }
    let _ = over;

    let modifiers = ui.input(|i| i.modifiers);
    if resp.clicked() && !resp.double_clicked() {
        if modifiers.shift {
            queue.push(Action::SelectRange(row.id));
        } else if modifiers.command {
            queue.push(Action::ToggleSelect(row.id));
        } else if row.is_folder {
            queue.push(Action::ToggleExpand(row.id));
            queue.push(Action::SelectFile(row.id));
        } else {
            queue.push(Action::OpenFile(row.id));
        }
    }
    if resp.middle_clicked() && !row.is_folder {
        queue.push(Action::OpenFileNewTab(row.id));
    }
    if resp.secondary_clicked() && !selected {
        queue.push(Action::SelectFile(row.id));
    }

    // Multi only when the right-clicked row is inside the current selection.
    let targets =
        if multi.len() > 1 && multi.contains(&row.id) { multi.clone() } else { vec![row.id] };
    let multi_sel = targets.len() > 1;

    if let Some(cmd) = context_menu::show(&resp, t, |e| {
        // Open | Create / expand | Arrange | Share / export | Delete
        if !row.is_folder {
            e.item(phosphor::ARROW_SQUARE_OUT, "Open", FileCmd::Open);
            e.item(phosphor::APP_WINDOW, "Open in new tab", FileCmd::OpenNewTab);
            e.separator();
        }
        e.item(phosphor::NOTE_PENCIL, "Create…", FileCmd::Create);
        if row.is_folder && !kids_empty {
            e.separator();
            e.item(phosphor::CARET_DOWN, "Expand all", FileCmd::ExpandAll);
            e.item(phosphor::CARET_LEFT, "Collapse all", FileCmd::CollapseAll);
        }
        e.separator();
        // Arrange — structure + pin (sidebar surface).
        if !multi_sel {
            e.item(phosphor::PENCIL, "Rename…", FileCmd::Rename);
        }
        if !row.is_folder {
            e.item(phosphor::PUSH_PIN, if pinned { "Unpin" } else { "Pin" }, FileCmd::Pin);
        }
        e.item(phosphor::FOLDERS, "Move…", FileCmd::Move);
        if !row.is_folder {
            e.item(phosphor::COPY, "Duplicate", FileCmd::Duplicate);
        }
        e.separator();
        // Outbound / portable.
        if !multi_sel {
            e.item(phosphor::USERS, "Share…", FileCmd::Share);
            if !row.is_folder {
                e.item(phosphor::LINK, "Copy link", FileCmd::CopyLink);
            }
        }
        if !row.is_folder {
            e.item(phosphor::DOWNLOAD_SIMPLE, "Export…", FileCmd::Export);
        }
        e.separator();
        if targets_are_saved_shares {
            // Opposite of Shared with me → "Save to your files…".
            e.item(phosphor::FOLDER_MINUS, "Remove from files…", FileCmd::Delete);
        } else {
            e.item_danger(phosphor::TRASH, "Delete…", FileCmd::Delete);
        }
    }) {
        match cmd {
            FileCmd::Open => {
                queue.push(Action::OpenDocuments { ids: targets.clone(), new_tab: false })
            }
            FileCmd::OpenNewTab => {
                queue.push(Action::OpenDocuments { ids: targets.clone(), new_tab: true })
            }
            FileCmd::Share => queue.push(Action::OpenShare(row.id)),
            FileCmd::Rename => queue.push(Action::OpenRename(row.id)),
            FileCmd::Pin => {
                if multi_sel {
                    queue.push(Action::TogglePinMany(targets.clone()));
                } else {
                    queue.push(Action::TogglePin(row.id));
                }
            }
            FileCmd::Move => queue.push(Action::OpenMove(targets.clone())),
            FileCmd::Delete => queue.push(Action::OpenDelete(targets.clone())),
            FileCmd::Create => {
                // Folder-context selects Choose; file-context selects Alongside.
                // Type stays Note (not “Folder” just because the row is a folder).
                if row.is_folder {
                    queue.push(Action::OpenCreate {
                        folder: Some(row.id),
                        alongside: None,
                        is_folder: false,
                    });
                } else {
                    queue.push(Action::OpenCreate {
                        folder: None,
                        alongside: Some(row.id),
                        is_folder: false,
                    });
                }
            }
            FileCmd::Duplicate => queue.push(Action::Duplicate(targets.clone())),
            FileCmd::Export => queue.push(Action::Export(targets.clone())),
            FileCmd::CopyLink => queue.push(Action::CopyLink(row.id)),
            FileCmd::ExpandAll => queue.push(Action::ExpandSubtree(row.id)),
            FileCmd::CollapseAll => queue.push(Action::CollapseSubtree(row.id)),
            FileCmd::OrganizeShare | FileCmd::DeclineShare => {}
        }
    }
}

pub(crate) fn empty_state(ui: &mut Ui, t: &Theme, msg: &str) {
    ui.add(Spacer::new(Space::Lg));
    ui.vertical_centered(|ui| {
        ui.label(TypeRole::Body.rich(msg).color(t.neutral_fg_secondary()));
    });
}
