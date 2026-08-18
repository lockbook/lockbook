//! Delete-preview and folder-picker trees (sheet-facing).

use egui::{Id, Layout, Rect, ScrollArea, Ui, vec2};
use lb::Uuid;
use workspace_rs::file_cache::FilesExt;

use crate::components::interact::sense_click;
use crate::components::{ROW_H, Theme, with_overlay_scroll};
use crate::shell::ShellApp;

use super::{
    FlatRow, RowGeom, TreeRowChrome, ancestor_at_depth, flatten, folder_has_flat_child,
    paint_sticky_viewport, paint_tree_file_row, row_type_icon,
};

const DELETE_LIST_VISIBLE_ROWS: f32 = 7.0;

/// Expandable sticky tree of delete targets (and folder contents when opened).
///
/// Same rounded hairline plate + overlay scroll as create/move folder choosers.
/// Fixed N-row plate so expand/collapse does not resize the sheet.
///
/// **Files-only selection:** returns `false` without painting — the bold
/// summary already names the docs; a static one-row plate reads as a button.
/// Returns `true` when a tree was painted (at least one folder in the roots).
pub fn show_delete_tree(
    app: &ShellApp, ui: &mut Ui, t: &Theme, ids: &[Uuid],
    expanded: &mut std::collections::HashSet<Uuid>,
) -> bool {
    let flat = {
        let Some(ready) = app.session.ready() else {
            return false;
        };
        let files = ready.workspace.files.read().unwrap();
        let roots = top_level_delete_ids(&*files, ids);
        let any_folder = roots
            .iter()
            .any(|id| files.get_by_id(*id).is_some_and(|f| f.is_folder()));
        // No folders → summary-only confirm (no plate of static file rows).
        if !any_folder {
            return false;
        }
        let mut flat = Vec::new();
        for id in roots {
            flatten_delete(&*files, expanded, id, 0, &mut flat);
        }
        flat
    };
    if flat.is_empty() {
        return false;
    }

    let geom = RowGeom::uniform(flat.len(), ROW_H);
    let list_h = (DELETE_LIST_VISIBLE_ROWS * ROW_H).max(ROW_H);
    let bottom_pad = 0.0;
    let w = crate::components::ui_width(ui).max(1.0);

    // Outside hairline (not Frame Inside — sticky fills cover it). Stroke is
    let pin_top_r = crate::components::Radius::Control.pts();
    let scroll_id = Id::new("shell_delete_tree_scroll");
    let radius = crate::components::Radius::Control.corner();

    ui.allocate_ui_with_layout(vec2(w, list_h), Layout::top_down(egui::Align::Min), |ui| {
        ui.set_width(w);
        ui.set_height(list_h);
        ui.set_max_height(list_h);
        let (slot, _) = ui.allocate_exact_size(vec2(w, list_h), egui::Sense::hover());
        crate::components::paint_plate_stroke(ui, slot, radius, t.neutral());
        ui.scope_builder(egui::UiBuilder::new().max_rect(slot), |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.set_clip_rect(slot.intersect(ui.clip_rect()));
            with_overlay_scroll(ui, scroll_id, |ui| {
                let out = ScrollArea::vertical()
                    .id_salt("shell_delete_tree")
                    .max_height(list_h)
                    .min_scrolled_height(list_h)
                    .auto_shrink([false, false])
                    .show_viewport(ui, |ui, viewport| {
                        paint_sticky_viewport(
                            ui,
                            t,
                            &flat,
                            &geom,
                            viewport,
                            bottom_pad,
                            pin_top_r,
                            0.0, // plate owns edge; no extra content pad
                            |ui, t, row, paint_r, hit_r, elev, pin_vy| {
                                paint_delete_row(
                                    app, ui, t, row, paint_r, hit_r, elev, pin_vy, pin_top_r,
                                    expanded,
                                );
                            },
                        );
                    });
                ((), out.state.offset.y, out.id)
            });
        });
    });
    true
}

/// Selected ids with any selected ancestor dropped (folder covers its kids).
fn top_level_delete_ids(files: &impl FilesExt, ids: &[Uuid]) -> Vec<Uuid> {
    ids.iter()
        .copied()
        .filter(|&id| {
            !ids.iter()
                .any(|&other| other != id && is_strict_ancestor(files, other, id))
        })
        .collect()
}

fn is_strict_ancestor(files: &impl FilesExt, ancestor: Uuid, descendant: Uuid) -> bool {
    let mut cur = files.get_by_id(descendant).map(|f| f.parent);
    while let Some(p) = cur {
        if p == ancestor {
            return true;
        }
        let Some(f) = files.get_by_id(p) else {
            break;
        };
        if f.id == f.parent {
            break;
        }
        cur = Some(f.parent);
    }
    false
}

fn flatten_delete(
    files: &impl FilesExt, expanded: &std::collections::HashSet<Uuid>, id: Uuid, depth: usize,
    out: &mut Vec<FlatRow>,
) {
    // Same walk as Files nav (include self, recurse when expanded).
    flatten(files, expanded, id, depth, out, false);
}

fn paint_delete_row(
    app: &ShellApp, ui: &mut Ui, t: &Theme, row: FlatRow, paint_rect: Rect, hit_rect: Rect,
    elevated: bool, pin_vy: Option<f32>, pin_top_r: u8,
    expanded: &mut std::collections::HashSet<Uuid>,
) {
    let Some(ready) = app.session.ready() else {
        return;
    };
    let name = {
        let files = ready.workspace.files.read().unwrap();
        files
            .get_by_id(row.id)
            .map(|f| f.name.clone())
            .unwrap_or_else(|| "item".into())
    };
    let open = expanded.contains(&row.id);
    let icon = row_type_icon(&name, row.is_folder, open);
    let id = ui.id().with("delete_tree_row").with(row.id).with(elevated);
    // Folders expand; docs are static (no wash) — see module interactivity table.
    let chrome = TreeRowChrome::new(row.depth)
        .with_sheet_pin(elevated, pin_vy, pin_top_r)
        .interactive(row.is_folder);
    let resp = paint_tree_file_row(ui, t, name, icon, chrome, id, paint_rect, hit_rect, |r| r);
    if resp.clicked() && row.is_folder && !expanded.insert(row.id) {
        expanded.remove(&row.id);
    }
}

/// How many rows a sheet folder picker keeps visible by default.
const FOLDER_PICK_VISIBLE_ROWS: f32 = 10.0;

/// Default list viewport height for folder pickers (move / import / etc.).
pub fn folder_tree_default_height() -> f32 {
    FOLDER_PICK_VISIBLE_ROWS * ROW_H
}

/// Expand every ancestor of `id` (and the account root) so `id` is reachable
/// Expand ancestors so `id` is visible in a collapsed-by-default mini tree.
pub fn expand_ancestors_of(
    files: &impl FilesExt, id: Uuid, expanded: &mut std::collections::HashSet<Uuid>,
) {
    if let Some(root) = files.get_by_id(files.root().id) {
        expanded.insert(root.id);
    }
    let mut cur = Some(id);
    while let Some(cid) = cur {
        expanded.insert(cid);
        let Some(f) = files.get_by_id(cid) else {
            break;
        };
        if f.id == f.parent {
            break;
        }
        cur = Some(f.parent);
    }
}

/// True if `dest` is one of `moving` or lives under a moved folder (illegal dest).
pub fn is_forbidden_move_dest(files: &impl FilesExt, moving: &[Uuid], dest: Uuid) -> bool {
    if moving.contains(&dest) {
        return true;
    }
    for &m in moving {
        if files.get_by_id(m).is_some_and(|f| f.is_folder()) && is_strict_ancestor(files, m, dest) {
            return true;
        }
    }
    false
}

/// Temp flag: one-shot scroll-to-selected for a folder picker (`id_salt`).
pub fn folder_tree_scroll_key(id_salt: &str) -> Id {
    Id::new(("shell_folder_scroll", id_salt))
}

/// After collapsing an elevated sticky folder, pin scroll so the row keeps its
/// screen y (`folder`, sticky `vy` in viewport space).
fn folder_tree_collapse_scroll_key(id_salt: &str) -> Id {
    Id::new(("shell_folder_collapse_scroll", id_salt))
}

/// Sticky expandable **folder** tree for create / move / import pickers.
///
/// Returns a folder id when the user selects a valid destination. Folders with
/// children toggle open (root stays open). Same sticky chrome as delete/files.
///
/// On first show (until [`folder_tree_scroll_key`] is cleared), scrolls so the
/// selected row is centered in the **free** viewport *below* sticky ancestors —
/// plain center hides deep selections under the pin stack.
pub fn show_folder_tree(
    app: &ShellApp, ui: &mut Ui, t: &Theme, expanded: &mut std::collections::HashSet<Uuid>,
    selected: Option<Uuid>, moving: &[Uuid], id_salt: &str, list_h: f32,
) -> Option<Uuid> {
    let flat = {
        let ready = app.session.ready()?;
        let files = ready.workspace.files.read().unwrap();
        let root_id = files.root().id;
        // Root starts open from the sheet seed only — user can collapse it like
        // any other folder (sticky or in-flow). No per-frame force-expand.
        let mut flat = Vec::new();
        flatten_folders_only(&*files, expanded, root_id, 0, &mut flat);
        flat
    };
    if flat.is_empty() {
        return None;
    }

    let geom = RowGeom::uniform(flat.len(), ROW_H);
    // Fixed viewport height so collapsing root doesn't shrink the control to one
    // row. Trailing pad (= one viewport) lets any row scroll to the pin band —
    // required when unsticking a late sticky folder with few remaining children.
    let list_h = list_h.max(ROW_H);
    let bottom_pad = list_h;
    let scroll_total = geom.total + bottom_pad;
    let max_off = (scroll_total - list_h).max(0.0);

    // One-shot: place selected in free space under sticky ancestors.
    let scroll_key = folder_tree_scroll_key(id_salt);
    let need_scroll = ui
        .ctx()
        .data(|d| d.get_temp::<bool>(scroll_key))
        .unwrap_or(true);
    // Collapse-from-sticky: keep the row's screen y (content_y − offset = pin_vy).
    let collapse_key = folder_tree_collapse_scroll_key(id_salt);
    let collapse_pin = ui
        .ctx()
        .data_mut(|d| d.remove_temp::<(Uuid, f32)>(collapse_key));
    // Overlay bar (same as Files tree) — stock ScrollArea expands to a thick
    // solid bar and reads inconsistent in sheets.
    let scroll_id = Id::new(("shell_folder_tree_scroll", id_salt));
    let mut picked = None;
    with_overlay_scroll(ui, scroll_id, |ui| {
        let mut scroll = ScrollArea::vertical()
            .id_salt(id_salt)
            .max_height(list_h)
            .min_scrolled_height(list_h)
            .auto_shrink([false, false]);
        if let Some((fid, pin_vy)) = collapse_pin {
            if let Some(i) = flat.iter().position(|r| r.id == fid) {
                let offset = (geom.top(i) - pin_vy).clamp(0.0, max_off);
                scroll = scroll.vertical_scroll_offset(offset);
            }
        } else if need_scroll {
            if let Some(sel) = selected {
                if let Some(i) = flat.iter().position(|r| r.id == sel) {
                    // Deep rows pin every ancestor folder; those overlays steal the
                    // top of the viewport. Center in what's left under that band.
                    let sticky_h = sticky_band_above(&flat, &geom, i);
                    let free_h = (list_h - sticky_h).max(geom.height(i));
                    let target_vy = sticky_h + (free_h - geom.height(i)) * 0.5;
                    let offset = (geom.top(i) - target_vy).clamp(0.0, max_off);
                    scroll = scroll.vertical_scroll_offset(offset);
                    ui.ctx().data_mut(|d| d.insert_temp(scroll_key, false));
                }
                // else: selected not in flat yet — try again next frame
            } else {
                ui.ctx().data_mut(|d| d.insert_temp(scroll_key, false));
            }
        }

        // Match the rounded chooser Frame (Control radius on top pin corners).
        let pin_top_r = crate::components::Radius::Control.pts();
        let out = scroll.show_viewport(ui, |ui, viewport| {
            paint_sticky_viewport(
                ui,
                t,
                &flat,
                &geom,
                viewport,
                bottom_pad,
                pin_top_r,
                0.0, // plate owns edge
                |ui, t, row, paint_r, hit_r, elev, pin_vy| {
                    if let Some(id) = paint_folder_pick_row(
                        app, ui, t, row, paint_r, hit_r, elev, pin_vy, pin_top_r, expanded,
                        selected, moving, id_salt,
                    ) {
                        picked = Some(id);
                    }
                },
            );
        });
        ((), out.state.offset.y, out.id)
    });
    picked
}

/// Height of sticky ancestor headers when row `i` is in view (folder ancestors
/// only — matches [`sticky_layout`] for an expanded folder walk).
pub(crate) fn sticky_band_above(flat: &[FlatRow], geom: &RowGeom, i: usize) -> f32 {
    let depth = flat[i].depth;
    if depth == 0 {
        return 0.0;
    }
    // One pin slot per ancestor depth; each is that ancestor's row height.
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

fn flatten_folders_only(
    files: &impl FilesExt, expanded: &std::collections::HashSet<Uuid>, id: Uuid, depth: usize,
    out: &mut Vec<FlatRow>,
) {
    let Some(f) = files.get_by_id(id) else {
        return;
    };
    if !f.is_folder() {
        return;
    }
    out.push(FlatRow { id, depth, is_folder: true });
    if !expanded.contains(&id) {
        return;
    }
    for kid in files.children(id) {
        if kid.id == id || !kid.is_folder() {
            continue;
        }
        flatten_folders_only(files, expanded, kid.id, depth + 1, out);
    }
}

fn paint_folder_pick_row(
    app: &ShellApp, ui: &mut Ui, t: &Theme, row: FlatRow, paint_rect: Rect, hit_rect: Rect,
    elevated: bool, pin_vy: Option<f32>, pin_top_r: u8,
    expanded: &mut std::collections::HashSet<Uuid>, selected: Option<Uuid>, moving: &[Uuid],
    id_salt: &str,
) -> Option<Uuid> {
    let ready = app.session.ready()?;
    let (name, has_kids, forbidden) = {
        let files = ready.workspace.files.read().unwrap();
        let name = files
            .get_by_id(row.id)
            .map(|f| f.name.clone())
            .unwrap_or_else(|| "folder".into());
        let has_kids = files
            .children(row.id)
            .iter()
            .any(|k| k.is_folder() && k.id != row.id);
        let forbidden = is_forbidden_move_dest(&*files, moving, row.id);
        (name, has_kids, forbidden)
    };
    let open = expanded.contains(&row.id);
    // Folder-only walk — always interactive (select + expand).
    let icon = row_type_icon(&name, true, open);
    let is_sel = selected == Some(row.id) && !forbidden;
    let id = ui
        .id()
        .with(id_salt)
        .with("folder_pick_row")
        .with(row.id)
        .with(elevated);
    let chrome = TreeRowChrome::new(row.depth)
        .with_sheet_pin(elevated, pin_vy, pin_top_r)
        .selected(is_sel);
    let resp = paint_tree_file_row(ui, t, name, icon, chrome, id, paint_rect, hit_rect, |r| {
        r.sense(sense_click())
    });

    if !resp.clicked() {
        return None;
    }
    // Uniform for every folder (root included): sticky or in-flow — select and
    // toggle open/closed when there are children.
    if has_kids {
        let was_open = expanded.contains(&row.id);
        if !expanded.insert(row.id) {
            expanded.remove(&row.id);
        }
        // Collapsing an elevated sticky unsticks the row next frame; hold its
        // viewport y so the row doesn't jump under the pointer.
        if elevated && was_open && !expanded.contains(&row.id) {
            if let Some(vy) = pin_vy {
                ui.ctx().data_mut(|d| {
                    d.insert_temp(folder_tree_collapse_scroll_key(id_salt), (row.id, vy));
                });
            }
        }
    }
    if forbidden { None } else { Some(row.id) }
}
