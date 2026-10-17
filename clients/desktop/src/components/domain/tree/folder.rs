//! Delete-preview and folder-picker trees (sheet-facing).

use egui::{Id, Layout, Rect, ScrollArea, Ui, vec2};
use lb::Uuid;
use workspace_rs::file_cache::FilesExt;

use crate::components::{ROW_H, Theme, with_overlay_scroll};
use crate::shell::ShellApp;

use super::{
    FlatRow, RowGeom, TreeRowChrome, flatten, paint_sticky_viewport, paint_tree_file_row,
    row_type_icon,
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
            !ids.iter().any(|&other| {
                other != id && workspace_rs::style::is_strict_ancestor(files, other, id)
            })
        })
        .collect()
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

pub use workspace_rs::style::{
    expand_ancestors_of, folder_tree_default_height, folder_tree_scroll_key, is_forbidden_move_dest,
};

/// Sticky expandable folder tree for create / move / import pickers.
///
/// Thin host wrapper: files come from the session; paint lives in workspace.
pub fn show_folder_tree(
    app: &ShellApp, ui: &mut Ui, t: &Theme, expanded: &mut std::collections::HashSet<Uuid>,
    selected: Option<Uuid>, moving: &[Uuid], id_salt: &str, list_h: f32,
) -> Option<Uuid> {
    let ready = app.session.ready()?;
    let files = ready.workspace.files.read().unwrap();
    workspace_rs::style::show_folder_tree(
        ui, t, &*files, expanded, selected, moving, id_salt, list_h,
    )
}
