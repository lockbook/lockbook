//! Recents sidebar pane.

use egui::{Id, Rect, ScrollArea, Sense, Ui, pos2, vec2};
use lb::Uuid;
use workspace_rs::file_cache::FilesExt;

use crate::components::{
    FileRow, LIST_PAD, SECTION_GAP, SECTION_HEAD_GAP, Theme, TypeRole,
    context_menu, phosphor, with_overlay_scroll,
};
use crate::shell::ShellApp;
use crate::shell::action::Action;
use crate::shell::prefs::recents_bucket;

use super::{
    FileCmd, RowGeom, TreeRowChrome, empty_state,
    paint_tree_file_row, row_type_icon,
};

pub fn show_recents(app: &mut ShellApp, ui: &mut Ui, t: &Theme, queue: &mut Vec<Action>) {
    app.ensure_recents_cache();
    if app.session.ready().is_none() {
        return;
    }
    // Clone row ids/meta once; paint path is virtualized (was full-list every frame).
    let docs = app.recents_cache.rows.clone();

    if docs.is_empty() {
        empty_state(ui, t, "No recent documents");
        return;
    }

    let (selected_ids, cursor) = app
        .session
        .ready()
        .map(|r| (r.selected.clone(), r.cursor))
        .unwrap_or_default();

    // Layout strip: section headers + two-line docs. Heights declared up front
    // (same contract as Shared / tree `RowGeom`) — not measured by painting.
    let doc_h = FileRow::height_for(true);
    let mut items: Vec<RecentItem> = Vec::with_capacity(docs.len() + 8);
    let mut last_bucket = "";
    for (i, (_, _, modified, _, _)) in docs.iter().enumerate() {
        let bucket = recents_bucket(*modified);
        if bucket != last_bucket {
            let had_prior = !last_bucket.is_empty();
            items.push(RecentItem::Section { title: bucket, h: recents_section_h(had_prior) });
            last_bucket = bucket;
        }
        items.push(RecentItem::Doc { idx: i, h: doc_h });
    }
    let heights: Vec<f32> = items.iter().map(|it| it.h()).collect();
    let geom = RowGeom::from_heights(&heights);

    let pad = LIST_PAD.pts();
    let scroll_id = Id::new("shell_recents_scroll");
    with_overlay_scroll(ui, scroll_id, |ui| {
        let out = ScrollArea::vertical()
            .id_salt("shell_recents_scroll")
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

                // Visible doc ids → parents (one lock, only rows we paint).
                let mut visible_doc_ids: Vec<Uuid> = Vec::new();
                for (i, item) in items.iter().enumerate() {
                    let y0 = pad + geom.top(i);
                    let y1 = y0 + item.h();
                    if y1 < offset || y0 > view_bot {
                        continue;
                    }
                    if let RecentItem::Doc { idx, .. } = item {
                        visible_doc_ids.push(docs[*idx].0);
                    }
                }
                let parents: std::collections::HashMap<Uuid, Uuid> = app
                    .session
                    .ready()
                    .map(|r| {
                        let files = r.workspace.files.read().unwrap();
                        visible_doc_ids
                            .iter()
                            .filter_map(|id| files.get_by_id(*id).map(|f| (*id, f.parent)))
                            .collect()
                    })
                    .unwrap_or_default();

                for (i, item) in items.iter().enumerate() {
                    let y0 = pad + geom.top(i);
                    let y1 = y0 + item.h();
                    if y1 < offset || y0 > view_bot {
                        continue;
                    }
                    let top = content_min.y + y0;
                    match item {
                        RecentItem::Section { title, h } => {
                            let mut y = top;
                            if *h > TypeRole::Body.line_height() + SECTION_HEAD_GAP.pts() + 0.5 {
                                y += SECTION_GAP.pts();
                            }
                            // Match `list_section_header` (strong body, muted).
                            let g = ui.painter().layout_no_wrap(
                                (*title).to_owned(),
                                egui::FontId::new(
                                    TypeRole::Body.size(),
                                    egui::FontFamily::Name(std::sync::Arc::from("Bold")),
                                ),
                                t.neutral_fg_secondary(),
                            );
                            ui.painter()
                                .galley(pos2(text_left, y), g, t.neutral_fg_secondary());
                        }
                        RecentItem::Doc { idx, h } => {
                            let (id, name, _mod, crumbs, pinned) = &docs[*idx];
                            let selected = selected_ids.contains(id) || cursor == Some(*id);
                            let path = if crumbs.is_empty() { "Home" } else { crumbs.as_str() };
                            let rect = Rect::from_min_size(pos2(text_left, top), vec2(inner_w, *h));
                            let row_id = Id::new("shell_recents_row").with(*id);
                            // Docs only — always interactive (open).
                            let chrome = TreeRowChrome::new(0).selected(selected);
                            let resp = paint_tree_file_row(
                                ui,
                                t,
                                name.clone(),
                                row_type_icon(name, false, false),
                                chrome,
                                row_id,
                                rect,
                                rect,
                                |r| r.subtitle(path),
                            );
                            if resp.clicked() {
                                queue.push(Action::OpenFile(*id));
                            }
                            let parent = parents.get(id).copied();
                            let pinned = *pinned;
                            if let Some(cmd) = context_menu::show(&resp, t, |e| {
                                e.item(phosphor::ARROW_SQUARE_OUT, "Open", FileCmd::Open);
                                e.item(
                                    phosphor::APP_WINDOW,
                                    "Open in new tab",
                                    FileCmd::OpenNewTab,
                                );
                                e.separator();
                                e.item(phosphor::NOTE_PENCIL, "Create…", FileCmd::Create);
                                e.separator();
                                e.item(phosphor::PENCIL, "Rename…", FileCmd::Rename);
                                e.item(
                                    phosphor::PUSH_PIN,
                                    if pinned { "Unpin" } else { "Pin" },
                                    FileCmd::Pin,
                                );
                                e.item(phosphor::FOLDERS, "Move…", FileCmd::Move);
                                e.item(phosphor::COPY, "Duplicate", FileCmd::Duplicate);
                                e.separator();
                                e.item(phosphor::USERS, "Share…", FileCmd::Share);
                                e.item(phosphor::LINK, "Copy link", FileCmd::CopyLink);
                                e.item(phosphor::DOWNLOAD_SIMPLE, "Export…", FileCmd::Export);
                                e.separator();
                                e.item_danger(phosphor::TRASH, "Delete…", FileCmd::Delete);
                            }) {
                                match cmd {
                                    FileCmd::Open => queue.push(Action::OpenFile(*id)),
                                    FileCmd::OpenNewTab => queue.push(Action::OpenFileNewTab(*id)),
                                    FileCmd::Create => {
                                        queue.push(Action::OpenCreate { parent, is_folder: false })
                                    }
                                    FileCmd::Rename => queue.push(Action::OpenRename(*id)),
                                    FileCmd::Pin => queue.push(Action::TogglePin(*id)),
                                    FileCmd::Move => queue.push(Action::OpenMove(vec![*id])),
                                    FileCmd::Duplicate => queue.push(Action::Duplicate(vec![*id])),
                                    FileCmd::Share => queue.push(Action::OpenShare(*id)),
                                    FileCmd::CopyLink => queue.push(Action::CopyLink(*id)),
                                    FileCmd::Export => queue.push(Action::Export(vec![*id])),
                                    FileCmd::Delete => queue.push(Action::OpenDelete(vec![*id])),
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            });
        ((), out.state.offset.y, out.id)
    });
}

/// Pitch for a Recents section label (matches [`SECTION_GAP`] + header).
fn recents_section_h(had_prior: bool) -> f32 {
    let head = TypeRole::Body.line_height() + SECTION_HEAD_GAP.pts();
    if had_prior { SECTION_GAP.pts() + head } else { head }
}

#[derive(Clone, Copy)]
enum RecentItem {
    Section { title: &'static str, h: f32 },
    Doc { idx: usize, h: f32 },
}

impl RecentItem {
    fn h(self) -> f32 {
        match self {
            Self::Section { h, .. } | Self::Doc { h, .. } => h,
        }
    }
}

