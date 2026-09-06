//! Sticky expandable folder tree — create / move / import / search pickers.

use std::collections::HashSet;

use egui::{Id, Rect, ScrollArea, Ui};
use lb_rs::Uuid;

use crate::file_cache::FilesExt;
use crate::style::chrome::{Radius, display_file_name};
use crate::style::color::Theme;
use crate::style::interact::sense_click;
use crate::style::overlay_scroll::with_overlay_scroll;
use crate::style::sticky::{
    FlatRow, RowGeom, TreeRowChrome, paint_sticky_viewport, paint_tree_file_row, row_type_icon,
    sticky_band_above,
};
use crate::style::tree_metrics::ROW_H;

/// How many rows a folder picker keeps visible by default.
const FOLDER_PICK_VISIBLE_ROWS: f32 = 10.0;

/// Default list viewport height for folder pickers (move / import / search).
pub fn folder_tree_default_height() -> f32 {
    FOLDER_PICK_VISIBLE_ROWS * ROW_H
}

/// Expand every ancestor of `id` (and the account root) so `id` is visible
/// in a collapsed-by-default mini tree.
pub fn expand_ancestors_of(files: &impl FilesExt, id: Uuid, expanded: &mut HashSet<Uuid>) {
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

/// True if `ancestor` is a strict ancestor of `descendant`.
pub fn is_strict_ancestor(files: &impl FilesExt, ancestor: Uuid, descendant: Uuid) -> bool {
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

/// Sticky expandable **folder** tree for create / move / import / search pickers.
///
/// Returns a folder id when the user selects a valid destination. Folders with
/// children toggle open (root stays open). Same sticky chrome as the Files tree.
///
/// On first show (until [`folder_tree_scroll_key`] is cleared), scrolls so the
/// selected row is centered in the **free** viewport *below* sticky ancestors —
/// plain center hides deep selections under the pin stack.
#[allow(clippy::too_many_arguments)]
pub fn show_folder_tree(
    ui: &mut Ui, t: &Theme, files: &impl FilesExt, expanded: &mut HashSet<Uuid>,
    selected: Option<Uuid>, moving: &[Uuid], id_salt: &str, list_h: f32,
) -> Option<Uuid> {
    let root_id = files.root().id;
    let mut flat = Vec::new();
    flatten_folders_only(files, expanded, root_id, 0, &mut flat);
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

    let scroll_key = folder_tree_scroll_key(id_salt);
    let need_scroll = ui
        .ctx()
        .data(|d| d.get_temp::<bool>(scroll_key))
        .unwrap_or(true);
    let collapse_key = folder_tree_collapse_scroll_key(id_salt);
    let collapse_pin = ui
        .ctx()
        .data_mut(|d| d.remove_temp::<(Uuid, f32)>(collapse_key));
    let scroll_id = Id::new(("folder_tree_scroll", id_salt));
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
                    let sticky_h = sticky_band_above(&flat, &geom, i);
                    let free_h = (list_h - sticky_h).max(geom.height(i));
                    let target_vy = sticky_h + (free_h - geom.height(i)) * 0.5;
                    let offset = (geom.top(i) - target_vy).clamp(0.0, max_off);
                    scroll = scroll.vertical_scroll_offset(offset);
                    ui.ctx().data_mut(|d| d.insert_temp(scroll_key, false));
                }
            } else {
                ui.ctx().data_mut(|d| d.insert_temp(scroll_key, false));
            }
        }

        let pin_top_r = Radius::Control.pts();
        let out = scroll.show_viewport(ui, |ui, viewport| {
            paint_sticky_viewport(
                ui,
                t,
                &flat,
                &geom,
                viewport,
                bottom_pad,
                pin_top_r,
                0.0,
                |ui, t, row, paint_r, hit_r, elev, pin_vy| {
                    if let Some(id) = paint_folder_pick_row(
                        ui, t, files, row, paint_r, hit_r, elev, pin_vy, pin_top_r, expanded,
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

/// Folder tree in a flush plate (Outside hairline). Sticky fills bleed to the
/// control edge — same chrome as the move sheet.
#[allow(clippy::too_many_arguments)]
pub fn show_folder_tree_plate(
    ui: &mut Ui, t: &Theme, files: &impl FilesExt, expanded: &mut HashSet<Uuid>,
    selected: Option<Uuid>, moving: &[Uuid], id_salt: &str, list_h: f32,
) -> Option<Uuid> {
    let list_h = list_h.max(ROW_H);
    let w = super::layout::ui_width(ui).max(1.0);
    let (slot, _) = ui.allocate_exact_size(egui::vec2(w, list_h), egui::Sense::hover());
    super::chrome::paint_plate_stroke(ui, slot, Radius::Control.corner(), t.neutral());
    let mut picked = None;
    ui.scope_builder(egui::UiBuilder::new().max_rect(slot), |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        ui.set_clip_rect(slot.intersect(ui.clip_rect()));
        picked = show_folder_tree(ui, t, files, expanded, selected, moving, id_salt, list_h);
    });
    picked
}

/// Result of [`show_folder_sheet`].
#[derive(Clone, Copy, Debug, Default)]
pub struct FolderSheetOut {
    pub dismiss: bool,
    pub confirm: bool,
    pub picked: Option<Uuid>,
}

/// Move-style folder sheet: dim + centered plate + sticky tree + footer.
///
/// Clicking a row selects (and expands); [`FolderSheetOut::confirm`] commits.
/// `after_tree` is the summary band under the list (may be empty).
#[allow(clippy::too_many_arguments)]
pub fn show_folder_sheet(
    ctx: &egui::Context, t: &Theme, files: &impl FilesExt, expanded: &mut HashSet<Uuid>,
    dest: Option<Uuid>, moving: &[Uuid], id_salt: &str, title: &str, hint: &str, primary: &str,
    footer: super::sheet::SheetFooterOpts, after_tree: impl FnOnce(&mut Ui),
) -> FolderSheetOut {
    use super::sheet::{sheet_dim, sheet_footer, sheet_panel_fit, sheet_title_muted};
    use super::space::Space;
    use super::spacer::Spacer;
    use super::typography::TypeRole;

    let mut out = FolderSheetOut::default();
    let layer = egui::LayerId::new(egui::Order::Foreground, Id::new(id_salt));
    if sheet_dim(ctx, Id::new((id_salt, "dim")), layer) {
        out.dismiss = true;
    }

    egui::Area::new(Id::new(id_salt))
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            sheet_panel_fit(ui, t, 360.0, |ui| {
                if sheet_title_muted(ui, t, title) {
                    out.dismiss = true;
                }
                ui.add(Spacer::new(Space::Md));
                ui.label(TypeRole::Body.rich(hint).color(t.neutral_fg()));
                ui.add(Spacer::new(Space::Sm));
                let tree_h = folder_tree_default_height();
                if let Some(id) =
                    show_folder_tree_plate(ui, t, files, expanded, dest, moving, id_salt, tree_h)
                {
                    out.picked = Some(id);
                }
                after_tree(ui);
                ui.add(Spacer::new(Space::Md));
                let foot = sheet_footer(ui, t, primary, footer.primary_enabled(dest.is_some()));
                if foot.cancel {
                    out.dismiss = true;
                }
                if foot.primary {
                    out.confirm = true;
                }
            });
        });
    out
}

fn flatten_folders_only(
    files: &impl FilesExt, expanded: &HashSet<Uuid>, id: Uuid, depth: usize, out: &mut Vec<FlatRow>,
) {
    let Some(f) = files.get_by_id(id) else {
        return;
    };
    if !f.is_folder() {
        return;
    }
    out.push(FlatRow { id, depth, is_folder: true, kids_empty: false });
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

#[allow(clippy::too_many_arguments)]
fn paint_folder_pick_row(
    ui: &mut Ui, t: &Theme, files: &impl FilesExt, row: FlatRow, paint_rect: Rect, hit_rect: Rect,
    elevated: bool, pin_vy: Option<f32>, pin_top_r: u8, expanded: &mut HashSet<Uuid>,
    selected: Option<Uuid>, moving: &[Uuid], id_salt: &str,
) -> Option<Uuid> {
    let name = files
        .get_by_id(row.id)
        .map(|f| display_file_name(&f.name).to_owned())
        .unwrap_or_else(|| "folder".into());
    let has_kids = files
        .children(row.id)
        .iter()
        .any(|k| k.is_folder() && k.id != row.id);
    let forbidden = is_forbidden_move_dest(files, moving, row.id);
    let open = expanded.contains(&row.id);
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
    if has_kids {
        let was_open = expanded.contains(&row.id);
        if !expanded.insert(row.id) {
            expanded.remove(&row.id);
        }
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
