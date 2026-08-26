//! Shared-with-me sidebar pane.

use egui::{Id, Layout, Rect, ScrollArea, Sense, Ui, pos2, vec2};
use lb::Uuid;
use workspace_rs::file_cache::FilesExt;

use crate::components::interact::sense_click;
use crate::components::{
    FileRow, LIST_PAD, SECTION_GAP, SECTION_HEAD_GAP, Space, Theme, TypeRole, context_menu,
    control_height, icon_button_hit, paint_list_section, phosphor, tip_text, with_overlay_scroll,
};
use crate::shell::ShellApp;
use crate::shell::action::Action;
use crate::shell::prefs::relative_modified;

use super::{FileCmd, RowGeom, TreeRowChrome, empty_state, paint_tree_file_row, row_type_icon};

/// One visible row in Shared with me (share root or descendant under an expanded folder).
#[derive(Clone, Debug)]
struct SharedFlat {
    id: Uuid,
    name: String,
    depth: usize,
    is_folder: bool,
    /// Share-root only: Save / Decline. Descendants under an expanded folder are not.
    is_root: bool,
    /// Share-root second line (`last_modified`; Share has no timestamp).
    subtitle: Option<String>,
}

pub fn show_shared(app: &mut ShellApp, ui: &mut Ui, t: &Theme, queue: &mut Vec<Action>) {
    app.ensure_shared_cache();
    let pending = app.shared_cache.pending.clone();

    if pending.is_empty() {
        empty_state(ui, t, "Nothing shared with you yet");
        return;
    }

    // Flatten roots + expanded descendants, grouped by sharer (same section
    // contract as Recents time buckets). FileCache includes pending share trees.
    let mut items: Vec<SharedItem> = Vec::with_capacity(pending.len() + 8);
    {
        let Some(ready) = app.session.ready() else {
            return;
        };
        let files = ready.workspace.files.read().unwrap();
        let mut last_from = "";
        for (id, name, from, is_folder, modified) in &pending {
            if !from.eq_ignore_ascii_case(last_from) {
                let had_prior = !last_from.is_empty();
                items.push(SharedItem::Section {
                    title: from.clone(),
                    h: shared_section_h(had_prior),
                });
                last_from = from.as_str();
            }
            let subtitle = {
                let s = relative_modified(*modified as i64);
                (!s.is_empty()).then_some(s)
            };
            flatten_shared(
                &*files,
                &ready.expanded,
                *id,
                name.clone(),
                0,
                *is_folder,
                true,
                subtitle,
                &mut items,
            );
        }
    }
    let heights: Vec<f32> = items.iter().map(|it| it.h()).collect();
    let geom = RowGeom::from_heights(&heights);

    let pad = LIST_PAD.pts();
    let scroll_id = Id::new("shell_shared_scroll");
    with_overlay_scroll(ui, scroll_id, |ui| {
        let out = ScrollArea::vertical()
            .id_salt("shell_shared_scroll")
            .auto_shrink([false, false])
            .show_viewport(ui, |ui, viewport| {
                let content_min = ui.max_rect().min;
                let view_screen =
                    Rect::from_min_size(content_min + viewport.min.to_vec2(), viewport.size());
                let view_clip = view_screen.intersect(ui.clip_rect());
                ui.set_clip_rect(view_clip);

                let total_h = geom.total + pad * 2.0;
                ui.allocate_exact_size(
                    vec2(view_screen.width(), total_h.max(view_screen.height())),
                    Sense::hover(),
                );

                let offset = viewport.min.y;
                let view_bot = offset + viewport.height();
                let inner_w = (view_screen.width() - pad * 2.0).max(1.0);
                let text_left = view_screen.left() + pad;

                for (i, item) in items.iter().enumerate() {
                    let y0 = pad + geom.top(i);
                    let y1 = y0 + item.h();
                    if y1 < offset || y0 > view_bot {
                        continue;
                    }
                    let top = content_min.y + y0;
                    match item {
                        SharedItem::Section { title, h } => {
                            paint_shared_section(ui, t, title, text_left, top, *h);
                        }
                        SharedItem::File { row, h } => {
                            let rect = Rect::from_min_size(pos2(text_left, top), vec2(inner_w, *h));
                            paint_shared_row(app, ui, t, queue, row, rect);
                        }
                    }
                }
            });
        ((), out.state.offset.y, out.id)
    });
}

fn paint_shared_section(ui: &Ui, t: &Theme, title: &str, x: f32, top: f32, h: f32) {
    let head = TypeRole::Body.line_height() + SECTION_HEAD_GAP.pts();
    let mut y = top;
    if h > head + 0.5 {
        y += h - head;
    }
    paint_list_section(ui, t, title, pos2(x, y));
}

/// Paint one Shared-with-me file row (share root or descendant).
fn paint_shared_row(
    app: &ShellApp, ui: &mut Ui, t: &Theme, queue: &mut Vec<Action>, row: &SharedFlat, rect: Rect,
) {
    let expanded = app
        .session
        .ready()
        .map(|r| r.expanded.contains(&row.id))
        .unwrap_or(false);
    let icon = row_type_icon(&row.name, row.is_folder, expanded);
    let kids_empty = shared_folder_empty(app, row.id, row.is_folder);

    // Shared: every row is interactive (folders expand, docs open). Roots add
    // Save control + Decline menu — not display-only like delete preview.
    let chrome = TreeRowChrome::new(row.depth);

    if row.is_root {
        // Dense hit: glyph stays body phosphor; wash smaller than full control.
        let side = Space::Xs.pts();
        let hit = (TypeRole::Body.size() + side * 2.0)
            .min(control_height())
            .min((rect.height() - side * 2.0).max(1.0))
            .max(1.0);
        // Band for Save: [gap · hit · gap] inside the content inset.
        let trail = hit + side * 2.0;
        // Carve Save out of the row hit so the row does not hover under the
        // button (mixed grounds / double wash was the animation color bug).
        let mut row_hit = rect;
        row_hit.max.x = row_hit.max.x.min(rect.right() - trail).max(row_hit.min.x);
        let ground = t.neutral_bg();
        let id = ui.id().with("shared_root").with(row.id);
        let subtitle = row.subtitle.clone().unwrap_or_default();
        let resp =
            paint_tree_file_row(ui, t, row.name.clone(), icon, chrome, id, rect, row_hit, |r| {
                r.subtitle(subtitle)
                    .trail_reserve(trail)
                    .sense(sense_click())
            });

        // Contained in the trail band: right air `side`, vertical center in row.
        let btn_right = rect.right() - side;
        let btn_rect =
            Rect::from_min_size(pos2(btn_right - hit, rect.center().y - hit / 2.0), vec2(hit, hit));
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
        if resp.middle_clicked() && !row.is_folder {
            queue.push(Action::OpenFileNewTab(row.id));
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
        let resp =
            paint_tree_file_row(ui, t, row.name.clone(), icon, chrome, id, rect, rect, |r| {
                r.sense(sense_click())
            });
        if resp.clicked() {
            if row.is_folder {
                queue.push(Action::ToggleExpand(row.id));
            } else {
                queue.push(Action::OpenFile(row.id));
            }
        }
        if resp.middle_clicked() && !row.is_folder {
            queue.push(Action::OpenFileNewTab(row.id));
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
    depth: usize, is_folder: bool, is_root: bool, subtitle: Option<String>,
    out: &mut Vec<SharedItem>,
) {
    let h = FileRow::height_for(subtitle.is_some());
    out.push(SharedItem::File {
        row: SharedFlat { id, name, depth, is_folder, is_root, subtitle },
        h,
    });
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
            false,
            None,
            out,
        );
    }
}

/// Pitch for a Shared section label (matches [`SECTION_GAP`] + header).
fn shared_section_h(had_prior: bool) -> f32 {
    let head = TypeRole::Body.line_height() + SECTION_HEAD_GAP.pts();
    if had_prior { SECTION_GAP.pts() + head } else { head }
}

enum SharedItem {
    Section { title: String, h: f32 },
    File { row: SharedFlat, h: f32 },
}

impl SharedItem {
    fn h(&self) -> f32 {
        match self {
            Self::Section { h, .. } | Self::File { h, .. } => *h,
        }
    }
}
