use egui::{Area, Id, Order};
use lb::Uuid;
use workspace_rs::file_cache::FilesExt;

use crate::components::{
    SheetFooterOpts, Space, Spacer, Theme, TypeRole, sheet_dim, sheet_footer, sheet_panel_fit,
    sheet_title_muted, shortcut_enter, shortcut_return,
};

use super::ShellApp;
use super::action::Action as A;
use super::action::Action;

pub(crate) fn folder_path_slash(app: &ShellApp, id: Option<Uuid>) -> String {
    app.session
        .ready()
        .map(|r| {
            let files = r.workspace.files.read().unwrap();
            let id = id.unwrap_or_else(|| files.root().id);
            let p = files.path(id);
            if p.is_empty() || p == "/" {
                "/".to_owned()
            } else {
                let t = p.trim_matches('/');
                format!("/{t}/")
            }
        })
        .unwrap_or_else(|| "/".into())
}

/// Move sheet summary: **name** / count will be moved to **path** (+ size like delete).
///
/// Layout preference (same idea as create summary):
/// 1. One line only if the **full** path fits with the prose (never pre-condense
///    the path into a one-line residual — that always “fits” as `/…/leaf/`).
/// 2. Else prose on line 1, path on line 2 at full sheet width; middle-ellipsis
///    only when the path alone exceeds that width. Size shares line 2 when it
///    fits after the path; otherwise path keeps the full width.
pub(crate) fn move_summary_line(
    ui: &mut egui::Ui, t: &Theme, app: &ShellApp, ids: &[Uuid], dest: Option<Uuid>,
) {
    use workspace_rs::widgets::GlyphonLabel;

    let Some(dest) = dest else {
        return;
    };

    let max_w = crate::components::ui_width(ui).max(1.0);
    let fs = TypeRole::Body.size();
    let lh = TypeRole::Body.line_height();
    let ink = t.neutral_fg();

    let mw = |ui: &egui::Ui, text: &str, bold: bool| -> f32 {
        GlyphonLabel::new_rich(vec![(text, bold)], ink)
            .font_size(fs)
            .line_height(lh)
            .measure(ui)
            .x
    };

    let dest_path = folder_path_slash(app, Some(dest));
    let (single_name, n_items, bytes) = move_cascade_stats(app, ids);
    let size = super::sheets::delete_size_paren(bytes);
    let mid = " will be moved to ";
    // Size paren + sentence period, or just "." when size is omitted.
    let tail = format!("{size}.");
    let path_w = mw(ui, &dest_path, true);
    let mid_w = mw(ui, mid, false);
    let tail_w = mw(ui, &tail, false);

    // Path line: fit path to a budget that **already reserves** the suffix
    // (size or "."). Never paint path+suffix into max_w with Clip — that
    // chops the leaf (`dev-journal/` → `dev-`).
    let paint_path_line = |ui: &mut egui::Ui| {
        let period_w = mw(ui, ".", false);
        // Prefer path + size on one line; else path + period; size on next line.
        let (path_budget, suffix, size_below) = if path_w + tail_w <= max_w {
            ((max_w - tail_w).max(8.0), tail.as_str(), false)
        } else {
            ((max_w - period_w).max(8.0), ".", !size.is_empty())
        };
        let path_shown = fit_slash_path(ui, &dest_path, path_budget, fs, lh, ink);
        ui.add(
            GlyphonLabel::new_rich(vec![(&path_shown, true), (suffix, false)], ink)
                .font_size(fs)
                .line_height(lh)
                // No max_width Clip: string is already budgeted to fit.
                .max_width(f32::MAX),
        );
        if size_below {
            ui.add(
                GlyphonLabel::new(size.trim(), t.neutral_fg_secondary())
                    .font_size(fs)
                    .line_height(lh),
            );
        }
    };

    if let Some(name) = single_name {
        let name_w = mw(ui, &name, true);
        // Decision uses **full** path width — not a pre-ellipsized residual.
        let one_line = name_w + mid_w + path_w + tail_w <= max_w;
        if one_line {
            ui.add(
                GlyphonLabel::new_rich(
                    vec![(&name, true), (mid, false), (&dest_path, true), (&tail, false)],
                    ink,
                )
                .font_size(fs)
                .line_height(lh)
                .max_width(f32::MAX),
            );
        } else {
            // Line 1: name + “ will be moved to ”
            let name_budget = (max_w - mid_w).max(8.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
                ui.add(
                    GlyphonLabel::new_rich(vec![(&name, true)], ink)
                        .font_size(fs)
                        .line_height(lh)
                        .max_width(name_budget)
                        .text_overflow(workspace_rs::widgets::TextOverflow::EndEllipsis),
                );
                ui.add(GlyphonLabel::new(mid, ink).font_size(fs).line_height(lh));
            });
            // Line 2: full-width destination path.
            paint_path_line(ui);
        }
    } else {
        let head = format!(
            "These {} will be moved to ",
            super::sheets::delete_count_noun(n_items, "item", "items")
        );
        let head_w = mw(ui, &head, false);
        let one_line = head_w + path_w + tail_w <= max_w;
        if one_line {
            ui.add(
                GlyphonLabel::new_rich(
                    vec![(&head, false), (&dest_path, true), (&tail, false)],
                    ink,
                )
                .font_size(fs)
                .line_height(lh)
                .max_width(f32::MAX),
            );
        } else {
            ui.add(
                GlyphonLabel::new(&head, ink)
                    .font_size(fs)
                    .line_height(lh)
                    .max_width(max_w)
                    .text_overflow(workspace_rs::widgets::TextOverflow::EndEllipsis),
            );
            paint_path_line(ui);
        }
    }
}

/// Single display name when one root is moving; else `None` + total cascade count.
fn move_cascade_stats(app: &ShellApp, ids: &[Uuid]) -> (Option<String>, usize, u64) {
    let Some(ready) = app.session.ready() else {
        return (None, ids.len().max(1), 0);
    };
    let files = ready.workspace.files.read().unwrap();
    let roots: Vec<Uuid> = ids
        .iter()
        .copied()
        .filter(|&id| {
            !ids.iter().any(|&other| {
                other != id && super::sheets::delete_is_strict_ancestor(&*files, other, id)
            })
        })
        .collect();
    if roots.is_empty() {
        return (None, 1, 0);
    }

    let mut cascade = 0usize;
    let mut bytes = 0u64;
    for id in &roots {
        cascade += 1 + files.descendents(*id).len();
        bytes += files.size_bytes_recursive.get(id).copied().unwrap_or(0);
    }

    let single =
        if roots.len() == 1 { files.get_by_id(roots[0]).map(|f| f.name.clone()) } else { None };
    (single, cascade, bytes)
}

/// Flowing summary via glyphon (file names / folder paths can include emoji).
///
/// Same path layout rule as [`move_summary_line`]: one line only if the **full**
/// path fits with the prose; else path on its own line at full sheet width;
/// middle-ellipsis only when the path alone exceeds that width.
pub(crate) fn fit_slash_path(
    ui: &egui::Ui, path: &str, max_w: f32, font_size: f32, line_height: f32, color: egui::Color32,
) -> String {
    use workspace_rs::widgets::GlyphonLabel;

    let measure = |s: &str| {
        GlyphonLabel::new_rich(vec![(s, true)], color)
            .font_size(font_size)
            .line_height(line_height)
            .measure(ui)
            .x
    };
    let max_w = max_w.max(1.0);
    if path.is_empty() || measure(path) <= max_w {
        return path.to_owned();
    }
    let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
    if parts.is_empty() {
        return "/".to_owned();
    }

    let last = parts[parts.len() - 1];
    let n = parts.len();

    // Drop middle segments one at a time; always keep the leaf whole while possible.
    if n >= 2 {
        for keep_left in (0..n - 1).rev() {
            let cand = if keep_left == 0 {
                format!("/…/{last}/")
            } else {
                format!("/{}/…/{last}/", parts[..keep_left].join("/"))
            };
            if measure(&cand) <= max_w {
                return cand;
            }
        }
    }

    // Leaf alone still too wide — end-ellipsis the leaf (guaranteed ≤ max_w).
    let with_leaf = |leaf: &str| -> String {
        if n == 1 { format!("/{leaf}/") } else { format!("/…/{leaf}/") }
    };

    if measure(&with_leaf(last)) <= max_w {
        return with_leaf(last);
    }

    const ELLIP: &str = "…";
    let chars: Vec<char> = last.chars().collect();
    if chars.is_empty() {
        return if n == 1 { "/".into() } else { "/…/".into() };
    }
    // Smallest leaf chrome
    if measure(&with_leaf(ELLIP)) > max_w {
        // Even `/…/…/` is too wide — return shortest and accept overflow only here.
        return if n == 1 { "/…/".into() } else { "/…/…/".into() };
    }

    let mut lo = 0usize;
    let mut hi = chars.len();
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        let mut leaf: String = chars[..mid].iter().collect();
        leaf.push_str(ELLIP);
        if measure(&with_leaf(&leaf)) <= max_w {
            lo = mid;
        } else {
            hi = mid.saturating_sub(1);
        }
    }
    let mut leaf: String = chars[..lo].iter().collect();
    leaf.push_str(ELLIP);
    // Final clamp (measure edge cases).
    while measure(&with_leaf(&leaf)) > max_w {
        let mut v: Vec<char> = leaf.chars().collect();
        if v.len() <= ELLIP.chars().count() {
            return with_leaf(ELLIP);
        }
        // Pop ellipsis, pop one char, re-append ellipsis.
        for _ in 0..ELLIP.chars().count() {
            v.pop();
        }
        if v.is_empty() {
            return with_leaf(ELLIP);
        }
        v.pop();
        leaf = v.into_iter().collect();
        leaf.push_str(ELLIP);
    }
    with_leaf(&leaf)
}

pub(crate) fn show_move(
    app: &ShellApp, ctx: &egui::Context, t: &Theme, queue: &mut Vec<Action>, ids: &[Uuid],
    dest: Option<Uuid>, _unused: bool,
) {
    // Identity lives in the summary under the tree (create/delete style), not a
    // second title line — keeps the plate quiet while selection is clear.
    folder_picker_sheet(
        app,
        ctx,
        t,
        queue,
        "shell_move",
        "Move",
        "",
        // Same register as create location (“Choose a folder for the new file.”)
        // — concrete noun + purpose, not abstract “destination”.
        "Choose a folder to move to.",
        "Move",
        dest,
        ids,
        FolderPickKind::Move,
    );
}

pub(crate) fn show_accept_share(
    app: &ShellApp, ctx: &egui::Context, t: &Theme, queue: &mut Vec<Action>, id: Uuid, name: &str,
    dest: Option<Uuid>,
) {
    // Identity only in the commit summary (Move-style) — no subject under title.
    folder_picker_sheet(
        app,
        ctx,
        t,
        queue,
        "shell_accept",
        "Save",
        name, // summary only; not painted as a second title line
        // Parallel Move / Create: concrete folder + purpose.
        "Choose a folder to save into.",
        "Save",
        dest,
        &[],
        FolderPickKind::AcceptShare,
    );
    let _ = id;
}

pub(crate) fn show_decline_share(
    ctx: &egui::Context, t: &Theme, queue: &mut Vec<Action>, id: Uuid, name: &str,
) {
    let layer = egui::LayerId::new(Order::Foreground, Id::new("shell_decline_share"));
    if sheet_dim(ctx, Id::new("shell_decline_share_dim"), layer) {
        queue.push(A::CloseModal);
    }

    Area::new(Id::new("shell_decline_share"))
        .order(Order::Foreground)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            // Pure confirm (delete): muted title · Md · bold summary · Md · danger footer.
            // Identity lives in the summary — no second title/subject line.
            sheet_panel_fit(ui, t, 340.0, |ui| {
                if sheet_title_muted(ui, t, "Decline") {
                    queue.push(A::CloseModal);
                }
                ui.add(Spacer::new(Space::Md));
                paint_decline_summary(ui, t, name);
                ui.add(Spacer::new(Space::Md));
                let foot = sheet_footer(
                    ui,
                    t,
                    "Decline",
                    SheetFooterOpts::default()
                        .danger(true)
                        .divider(false)
                        .primary_shortcut(shortcut_return()),
                );
                if foot.cancel {
                    queue.push(A::CloseModal);
                }
                if foot.primary {
                    queue.push(A::ConfirmDeclineShare(id));
                }
            });
        });
}

/// You will lose access to **name**.
fn paint_decline_summary(ui: &mut egui::Ui, t: &Theme, name: &str) {
    use workspace_rs::widgets::GlyphonLabel;
    let ink = t.neutral_fg();
    let fs = TypeRole::Body.size();
    let lh = TypeRole::Body.line_height();
    let max_w = crate::components::ui_width(ui).max(1.0);
    ui.add(
        GlyphonLabel::new_rich(
            vec![("You will lose access to ", false), (name, true), (".", false)],
            ink,
        )
        .font_size(fs)
        .line_height(lh)
        .max_width(max_w),
    );
}

pub(crate) fn show_import_parent(
    app: &ShellApp, ctx: &egui::Context, t: &Theme, queue: &mut Vec<Action>,
    paths: &[std::path::PathBuf], dest: Option<Uuid>,
) {
    // Summary subject only (no second title line) — same as Move / Accept.
    let subject = match paths.len() {
        0 => "files".into(),
        1 => paths[0]
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "file".into()),
        n => format!("{n} items"),
    };
    folder_picker_sheet(
        app,
        ctx,
        t,
        queue,
        "shell_import_parent",
        "Import",
        &subject,
        "Choose a folder to import into.",
        "Import",
        dest,
        &[],
        FolderPickKind::ImportParent,
    );
}

#[derive(Clone, Copy)]
enum FolderPickKind {
    Move,
    AcceptShare,
    ImportParent,
}

fn folder_picker_sheet(
    app: &ShellApp, ctx: &egui::Context, t: &Theme, queue: &mut Vec<Action>, salt: &str,
    action: &str, subject: &str, hint: &str, primary: &str, dest: Option<Uuid>, exclude: &[Uuid],
    kind: FolderPickKind,
) {
    let layer = egui::LayerId::new(Order::Foreground, Id::new(salt));
    if sheet_dim(ctx, Id::new(format!("{salt}_dim")), layer) {
        queue.push(A::CloseModal);
    }

    // Expand state shared for this picker salt; seed once toward `dest` / exclude parents.
    let exp_id = Id::new(("shell_folder_pick_exp", salt));
    let mut expanded: std::collections::HashSet<Uuid> =
        ctx.data(|d| d.get_temp(exp_id)).unwrap_or_default();
    if expanded.is_empty() {
        if let Some(ready) = app.session.ready() {
            let files = ready.workspace.files.read().unwrap();
            expanded.insert(files.root().id);
            if let Some(d) = dest {
                super::tree::expand_ancestors_of(&*files, d, &mut expanded);
            }
            // Expand ancestors so each moved item is visible in the dest tree.
            for id in exclude {
                super::tree::expand_ancestors_of(&*files, *id, &mut expanded);
            }
        }
    }

    Area::new(Id::new(salt))
        .order(Order::Foreground)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            sheet_panel_fit(ui, t, 360.0, |ui| {
                if sheet_title_muted(ui, t, action) {
                    queue.push(A::CloseModal);
                }
                // Identity lives in the commit summary only (`subject` is for that
                // line — never a second title under the muted chrome).
                ui.add(Spacer::new(Space::Md));
                ui.label(TypeRole::Body.rich(hint).color(t.neutral_fg()));
                ui.add(Spacer::new(Space::Sm));
                // Flush sticky fills + Outside hairline (not Frame Inside).
                let tree_h = super::tree::folder_tree_default_height();
                let tw = crate::components::ui_width(ui);
                let (slot, _) =
                    ui.allocate_exact_size(egui::vec2(tw, tree_h), egui::Sense::hover());
                crate::components::paint_plate_stroke(
                    ui,
                    slot,
                    crate::components::Radius::Control.corner(),
                    t.neutral(),
                );
                ui.scope_builder(egui::UiBuilder::new().max_rect(slot), |ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    ui.set_clip_rect(slot.intersect(ui.clip_rect()));
                    if let Some(id) = super::tree::show_folder_tree(
                        app,
                        ui,
                        t,
                        &mut expanded,
                        dest,
                        exclude,
                        salt,
                        tree_h,
                    ) {
                        match kind {
                            FolderPickKind::Move => queue.push(A::MoveSelect(id)),
                            FolderPickKind::AcceptShare => queue.push(A::AcceptShareDest(id)),
                            FolderPickKind::ImportParent => queue.push(A::ImportParentSelect(id)),
                        }
                    }
                });
                // Commit copy when a folder is chosen (footer spacing: Xl · copy · Md · footer).
                // Move has its own cascade summary; Accept/Import use a simple dest line.
                let show_summary = dest.is_some();
                if show_summary {
                    ui.add(Spacer::new(Space::Xl));
                    match kind {
                        FolderPickKind::Move => move_summary_line(ui, t, app, exclude, dest),
                        FolderPickKind::AcceptShare => {
                            dest_action_summary(ui, t, app, subject, "will be saved to", dest)
                        }
                        FolderPickKind::ImportParent => {
                            dest_action_summary(ui, t, app, subject, "will be imported into", dest)
                        }
                    }
                }
                ui.add(Spacer::new(Space::Md));
                let foot = sheet_footer(
                    ui,
                    t,
                    primary,
                    SheetFooterOpts::default()
                        .divider(false)
                        .primary_enabled(dest.is_some())
                        .primary_shortcut(shortcut_enter()),
                );
                if foot.cancel {
                    queue.push(A::CloseModal);
                }
                if foot.primary {
                    match kind {
                        FolderPickKind::Move => queue.push(A::ConfirmMove),
                        FolderPickKind::AcceptShare => queue.push(A::ConfirmAcceptShare),
                        FolderPickKind::ImportParent => queue.push(A::ConfirmImportParent),
                    }
                }
            });
        });

    ctx.data_mut(|d| d.insert_temp(exp_id, expanded));
}

/// **subject** {verb} **/path/**. — Accept / Import commit line (move has its own).
fn dest_action_summary(
    ui: &mut egui::Ui, t: &Theme, app: &ShellApp, subject: &str, verb: &str, dest: Option<Uuid>,
) {
    use workspace_rs::widgets::GlyphonLabel;

    let Some(dest) = dest else {
        return;
    };
    let path = folder_path_slash(app, Some(dest));
    let ink = t.neutral_fg();
    let fs = TypeRole::Body.size();
    let lh = TypeRole::Body.line_height();
    let max_w = crate::components::ui_width(ui).max(1.0);
    let mid = format!(" {verb} ");
    ui.add(
        GlyphonLabel::new_rich(
            vec![(subject, true), (mid.as_str(), false), (path.as_str(), true), (".", false)],
            ink,
        )
        .font_size(fs)
        .line_height(lh)
        .max_width(max_w),
    );
}
