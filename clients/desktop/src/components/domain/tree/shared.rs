//! Shared-with-me sidebar pane.

use egui::{Id, Layout, Rect, ScrollArea, Ui, pos2, vec2};
use lb::Uuid;
use workspace_rs::file_cache::FilesExt;

use crate::components::interact::sense_click;
use crate::components::{
    FileRow, LIST_PAD, Space, Spacer, Theme, TypeRole, context_menu, control_height,
    icon_button_hit, list_section_header, phosphor, tip_text, with_overlay_scroll,
};
use crate::shell::ShellApp;
use crate::shell::action::Action;

use super::{
    FileCmd, FlatRow, RowGeom, TreeRowChrome, empty_state, paint_sticky_viewport,
    paint_tree_file_row, row_type_icon,
};

/// One visible row in Shared with me (share root or descendant under an expanded folder).
#[derive(Clone, Debug)]
struct SharedFlat {
    id: Uuid,
    name: String,
    depth: usize,
    is_folder: bool,
    /// Share-root only: who shared it (two-line subtitle + Save button).
    from: Option<String>,
}

pub fn show_shared(app: &mut ShellApp, ui: &mut Ui, t: &Theme, queue: &mut Vec<Action>) {
    app.ensure_shared_cache();
    let pending = app.shared_cache.pending.clone();

    if pending.is_empty() {
        empty_state(ui, t, "Nothing shared with you yet");
        return;
    }

    // Flatten roots + expanded descendants (FileCache includes pending share trees).
    let shared_flat = {
        let Some(ready) = app.session.ready() else {
            return;
        };
        let files = ready.workspace.files.read().unwrap();
        let mut flat = Vec::new();
        for (id, name, from, is_folder, _) in &pending {
            flatten_shared(
                &*files,
                &ready.expanded,
                *id,
                name.clone(),
                0,
                *is_folder,
                Some(from.clone()),
                &mut flat,
            );
        }
        flat
    };

    // Sticky virtualizer: share roots are two-line; descendants single-line.
    let geom = RowGeom::from_heights(
        &shared_flat
            .iter()
            .map(|r| FileRow::height_for(r.from.is_some()))
            .collect::<Vec<_>>(),
    );
    let flat: Vec<FlatRow> = shared_flat
        .iter()
        .map(|r| FlatRow { id: r.id, depth: r.depth, is_folder: r.is_folder, kids_empty: false })
        .collect();
    // Parallel meta by index (ids unique within a share forest for paint).
    let meta = shared_flat;

    // Same head/body split as the sidebar chrome: header is a top panel with
    // real height this frame; scroll is CentralPanel and gets the remainder.
    // Nested vertical/horizontal + ScrollArea was only getting a content-sized
    // sliver, so the virtualizer viewport stayed ~one row tall.
    ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
    egui::TopBottomPanel::top("shell_shared_head")
        .resizable(false)
        .show_separator_line(false)
        .frame(egui::Frame::new().inner_margin(0.0))
        .show_inside(ui, |ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            ui.add(Spacer::new(LIST_PAD));
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                ui.add(Spacer::new(LIST_PAD));
                ui.vertical(|ui| {
                    list_section_header(ui, t, "Shared with me");
                });
            });
        });

    egui::CentralPanel::default()
        .frame(egui::Frame::new().inner_margin(0.0))
        .show_inside(ui, |ui| {
            let body = ui.max_rect();
            ui.set_clip_rect(body.intersect(ui.clip_rect()));
            ui.set_min_size(body.size());
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            // Same fill contract as Files tree — full remainder is the viewport.
            let scroll_id = Id::new("shell_shared_scroll");
            with_overlay_scroll(ui, scroll_id, |ui| {
                let out = ScrollArea::vertical()
                    .id_salt("shell_shared_scroll")
                    .auto_shrink([false, false])
                    .show_viewport(ui, |ui, viewport| {
                        let bottom_pad = viewport.height() * 0.5;
                        let _bg = paint_sticky_viewport(
                            ui,
                            t,
                            &flat,
                            &geom,
                            viewport,
                            bottom_pad,
                            0,
                            LIST_PAD.pts(),
                            |ui, t, row, paint_r, hit_r, elev, _pin_vy| {
                                let m = meta.iter().find(|m| m.id == row.id).expect("shared meta");
                                let inset = if elev { LIST_PAD.pts() } else { 0.0 };
                                paint_shared_row(app, ui, t, queue, m, paint_r, hit_r, elev, inset);
                            },
                        );
                    });
                ((), out.state.offset.y, out.id)
            });
        });
}

/// Paint one Shared-with-me row into `paint_rect` (in-flow or sticky).
fn paint_shared_row(
    app: &ShellApp, ui: &mut Ui, t: &Theme, queue: &mut Vec<Action>, row: &SharedFlat,
    paint_rect: Rect, hit_rect: Rect, elevated: bool, content_inset: f32,
) {
    let expanded = app
        .session
        .ready()
        .map(|r| r.expanded.contains(&row.id))
        .unwrap_or(false);
    let icon = row_type_icon(&row.name, row.is_folder, expanded);
    let kids_empty = shared_folder_empty(app, row.id, row.is_folder);
    let is_root = row.from.is_some();

    // Shared: every row is interactive (folders expand, docs open). Roots add
    // Save control + Decline menu — not display-only like delete preview.
    let chrome = TreeRowChrome::new(row.depth)
        .elevated(elevated)
        .content_inset(content_inset);

    if is_root {
        let from = row.from.as_deref().unwrap_or("someone");
        let subtitle = format!("From {from}");
        // Dense hit: glyph stays body phosphor; wash smaller than full control.
        let side = Space::Xs.pts();
        let hit = (TypeRole::Body.size() + side * 2.0)
            .min(control_height())
            .min((paint_rect.height() - side * 2.0).max(1.0))
            .max(1.0);
        // Band for Save: [gap · hit · gap] inside the content inset.
        let trail = hit + side * 2.0;
        // Carve Save out of the row hit so the row does not hover under the
        // button (mixed grounds / double wash was the animation color bug).
        let mut row_hit = hit_rect;
        row_hit.max.x = row_hit
            .max
            .x
            .min(paint_rect.right() - content_inset - trail)
            .max(row_hit.min.x);
        // Ground = plate under the mark (idle). Row is not hovered over the
        // button, so this matches what's actually painted behind it.
        let ground = if elevated { t.neutral_bg_secondary() } else { t.neutral_bg() };
        let id = ui.id().with("shared_root").with(row.id);
        let resp = paint_tree_file_row(
            ui,
            t,
            row.name.clone(),
            icon,
            chrome,
            id,
            paint_rect,
            row_hit,
            |r| {
                r.subtitle(subtitle)
                    .trail_reserve(trail)
                    .sense(sense_click())
            },
        );

        // Contained in the trail band: right air `side`, vertical center in row.
        let btn_right = paint_rect.right() - content_inset - side;
        let btn_rect = Rect::from_min_size(
            pos2(btn_right - hit, paint_rect.center().y - hit / 2.0),
            vec2(hit, hit),
        );
        let mut accept = false;
        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(btn_rect)
                .layout(Layout::centered_and_justified(egui::Direction::TopDown)),
            |ui| {
                let r = icon_button_hit(ui, t, phosphor::FOLDER_PLUS, false, ground, hit);
                tip_text(ui.ctx(), &r, "Save to your files…");
                if r.clicked() {
                    accept = true;
                }
            },
        );
        if accept {
            queue.push(Action::OpenAcceptShare { id: row.id, name: row.name.clone() });
        } else if resp.clicked() {
            if row.is_folder {
                queue.push(Action::ToggleExpand(row.id));
            } else {
                queue.push(Action::OpenFile(row.id));
            }
        }
        if resp.double_clicked() && !row.is_folder {
            queue.push(Action::OpenFileNewTab(row.id));
        }
        if let Some(cmd) = context_menu::show(&resp, t, |e| {
            if !row.is_folder {
                e.item(phosphor::ARROW_SQUARE_OUT, "Open", FileCmd::Open);
                e.item(phosphor::APP_WINDOW, "Open in new tab", FileCmd::OpenNewTab);
                e.separator();
            }
            if row.is_folder && !kids_empty {
                e.item(phosphor::CARET_DOWN, "Expand all", FileCmd::ExpandAll);
                e.item(phosphor::CARET_LEFT, "Collapse all", FileCmd::CollapseAll);
                e.separator();
            }
            e.item(phosphor::FOLDER_PLUS, "Save to your files…", FileCmd::OrganizeShare);
            e.separator();
            e.item_danger(phosphor::TRASH, "Decline…", FileCmd::DeclineShare);
        }) {
            match cmd {
                FileCmd::Open => queue.push(Action::OpenFile(row.id)),
                FileCmd::OpenNewTab => queue.push(Action::OpenFileNewTab(row.id)),
                FileCmd::ExpandAll => queue.push(Action::ExpandSubtree(row.id)),
                FileCmd::CollapseAll => queue.push(Action::CollapseSubtree(row.id)),
                FileCmd::OrganizeShare => {
                    queue.push(Action::OpenAcceptShare { id: row.id, name: row.name.clone() })
                }
                FileCmd::DeclineShare => {
                    queue.push(Action::OpenDeclineShare { id: row.id, name: row.name.clone() })
                }
                _ => {}
            }
        }
    } else {
        let id = ui.id().with("shared_child").with(row.id);
        let resp = paint_tree_file_row(
            ui,
            t,
            row.name.clone(),
            icon,
            chrome,
            id,
            paint_rect,
            hit_rect,
            |r| r.sense(sense_click()),
        );
        if resp.clicked() {
            if row.is_folder {
                queue.push(Action::ToggleExpand(row.id));
            } else {
                queue.push(Action::OpenFile(row.id));
            }
        }
        if resp.double_clicked() && !row.is_folder {
            queue.push(Action::OpenFileNewTab(row.id));
        }
        if let Some(cmd) = context_menu::show(&resp, t, |e| {
            if !row.is_folder {
                e.item(phosphor::ARROW_SQUARE_OUT, "Open", FileCmd::Open);
                e.item(phosphor::APP_WINDOW, "Open in new tab", FileCmd::OpenNewTab);
            } else if !kids_empty {
                e.item(phosphor::CARET_DOWN, "Expand all", FileCmd::ExpandAll);
                e.item(phosphor::CARET_LEFT, "Collapse all", FileCmd::CollapseAll);
            }
        }) {
            match cmd {
                FileCmd::Open => queue.push(Action::OpenFile(row.id)),
                FileCmd::OpenNewTab => queue.push(Action::OpenFileNewTab(row.id)),
                FileCmd::ExpandAll => queue.push(Action::ExpandSubtree(row.id)),
                FileCmd::CollapseAll => queue.push(Action::CollapseSubtree(row.id)),
                _ => {}
            }
        }
    }
}

fn shared_folder_empty(app: &ShellApp, id: Uuid, is_folder: bool) -> bool {
    if !is_folder {
        return true;
    }
    app.session
        .ready()
        .map(|r| r.workspace.files.read().unwrap().children(id).is_empty())
        .unwrap_or(true)
}

/// DFS under a share root (or any folder), respecting [`Ready::expanded`].
fn flatten_shared(
    files: &impl FilesExt, expanded: &std::collections::HashSet<Uuid>, id: Uuid, name: String,
    depth: usize, is_folder: bool, from: Option<String>, out: &mut Vec<SharedFlat>,
) {
    out.push(SharedFlat { id, name, depth, is_folder, from });
    if !is_folder || !expanded.contains(&id) {
        return;
    }
    for kid in files.children(id) {
        if kid.id == id {
            continue;
        }
        flatten_shared(
            files,
            expanded,
            kid.id,
            kid.name.clone(),
            depth + 1,
            kid.is_folder(),
            None,
            out,
        );
    }
}
