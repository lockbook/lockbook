//! Modal sheets: delete, share, create, move, help, onboard.

use egui::{Area, Id, Order};
use lb::Uuid;
use workspace_rs::file_cache::FilesExt;

use crate::components::{
    Button, Field, SheetFooterOpts, Space, Spacer, Theme, TypeRole, display_file_name, phosphor,
    sheet_dim, sheet_footer, sheet_panel_fit, sheet_title_muted, shortcut_enter, shortcut_return,
};

use super::ShellApp;
use super::action::Action as A;
use super::action::{Action, Modal};
use super::ops::is_saved_share;
use super::settings;

/// True when `q` has been stable for 100ms and verify should run.
pub(crate) fn debounce_query(
    ctx: &egui::Context, due_key: Id, q_key: Id, q: &str, settled_for: &str,
) -> bool {
    let now = ctx.input(|i| i.time);
    if q.is_empty() || q.eq_ignore_ascii_case(settled_for) {
        ctx.data_mut(|d| {
            d.remove::<f64>(due_key);
            d.remove::<String>(q_key);
        });
        return false;
    }
    let scheduled_q = ctx.data(|d| d.get_temp::<String>(q_key));
    if scheduled_q.as_deref() != Some(q) {
        ctx.data_mut(|d| {
            d.insert_temp(due_key, now + 0.1);
            d.insert_temp(q_key, q.to_owned());
        });
        ctx.request_repaint_after(std::time::Duration::from_millis(20));
        return false;
    }
    let due = ctx.data(|d| d.get_temp::<f64>(due_key)).unwrap_or(now);
    if now >= due {
        ctx.data_mut(|d| {
            d.remove::<f64>(due_key);
            d.remove::<String>(q_key);
        });
        true
    } else {
        ctx.request_repaint_after(std::time::Duration::from_millis(20));
        false
    }
}

/// Discriminant only — never clone modal text fields (in-place modal fields).
#[derive(Clone, Copy)]
enum ModalKind {
    Settings,
    Delete,
    Share,
    Create,
    Move,
    Rename,
    AcceptShare,
    DeclineShare,
    ImportParent,
    Help,
    Onboard,
}

pub fn show_modals(app: &mut ShellApp, ctx: &egui::Context, t: &Theme, queue: &mut Vec<Action>) {
    let kind = match &app.modal {
        None => return,
        Some(Modal::Settings { .. }) => ModalKind::Settings,
        Some(Modal::Delete { .. }) => ModalKind::Delete,
        Some(Modal::Share { .. }) => ModalKind::Share,
        Some(Modal::Create { .. }) => ModalKind::Create,
        Some(Modal::Move { .. }) => ModalKind::Move,
        Some(Modal::Rename { .. }) => ModalKind::Rename,
        Some(Modal::AcceptShare { .. }) => ModalKind::AcceptShare,
        Some(Modal::DeclineShare { .. }) => ModalKind::DeclineShare,
        Some(Modal::ImportParent { .. }) => ModalKind::ImportParent,
        Some(Modal::Help) => ModalKind::Help,
        Some(Modal::Onboard { .. }) => ModalKind::Onboard,
    };

    match kind {
        ModalKind::Settings => {
            if matches!(app.account_panel, super::prefs::AccountPanel::Upgrade { .. }) {
                super::apply::poll_upgrade(app, ctx);
            }
            settings::show(app, ctx, t, queue);
        }
        ModalKind::Delete => {
            let ids = match &app.modal {
                Some(Modal::Delete { ids }) => ids.clone(),
                _ => return,
            };
            show_delete(app, ctx, t, queue, &ids);
        }
        ModalKind::Share => super::sheet_share::show_share(app, ctx, t, queue),
        ModalKind::Create => super::sheet_create::show_create(app, ctx, t, queue),
        ModalKind::Move => {
            let (ids, dest) = match &app.modal {
                Some(Modal::Move { ids, dest }) => (ids.clone(), *dest),
                _ => return,
            };
            super::sheet_folder::show_move(app, ctx, t, queue, &ids, dest, false);
        }
        ModalKind::Rename => show_rename(app, ctx, t, queue),
        ModalKind::AcceptShare => {
            let (id, name, dest) = match &app.modal {
                Some(Modal::AcceptShare { id, name, dest }) => (*id, name.clone(), *dest),
                _ => return,
            };
            super::sheet_folder::show_accept_share(app, ctx, t, queue, id, &name, dest);
        }
        ModalKind::DeclineShare => {
            let (id, name) = match &app.modal {
                Some(Modal::DeclineShare { id, name }) => (*id, name.clone()),
                _ => return,
            };
            super::sheet_folder::show_decline_share(ctx, t, queue, id, &name);
        }
        ModalKind::ImportParent => {
            let (paths, dest) = match &app.modal {
                Some(Modal::ImportParent { paths, dest }) => (paths.clone(), *dest),
                _ => return,
            };
            super::sheet_folder::show_import_parent(app, ctx, t, queue, &paths, dest);
        }
        ModalKind::Help => show_help(ctx, t, queue),
        ModalKind::Onboard => {
            super::apply::onboard_poll_uname(app, ctx);
            super::sheet_onboard::show_onboard(app, ctx, t, queue);
        }
    }
}

pub(crate) fn file_name(app: &ShellApp, id: Uuid) -> String {
    app.session
        .ready()
        .and_then(|r| {
            r.workspace
                .files
                .read()
                .unwrap()
                .get_by_id(id)
                .map(|f| display_file_name(&f.name).to_owned())
        })
        .unwrap_or_else(|| "item".into())
}

fn show_delete(
    app: &ShellApp, ctx: &egui::Context, t: &Theme, queue: &mut Vec<Action>, ids: &[Uuid],
) {
    // Expand state lives in temp memory (keyed by selection) so folders can
    // open without a Modal field / Action for every click.
    let exp_id = Id::new("shell_delete_expanded").with(ids);
    let mut expanded: std::collections::HashSet<Uuid> =
        ctx.data(|d| d.get_temp(exp_id)).unwrap_or_default();

    let parts = delete_parts(app, ids);
    let remove_only = parts.owned.is_empty() && !parts.shares.is_empty();

    let layer = egui::LayerId::new(Order::Foreground, Id::new("shell_delete"));
    if sheet_dim(ctx, Id::new("shell_delete_dim"), layer) {
        queue.push(A::CloseModal);
    }

    Area::new(Id::new("shell_delete"))
        .order(Order::Foreground)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            sheet_panel_fit(ui, t, 320.0, |ui| {
                // Pure confirm: optional folder tree + copy + danger footer.
                // Files-only: summary alone (tree would be static rows in a plate).
                // Share links: Remove (not Delete) — opposite of Save to your files.
                let title = if remove_only { "Remove" } else { "Delete" };
                if sheet_title_muted(ui, t, title) {
                    queue.push(A::CloseModal);
                }
                ui.add(Spacer::new(Space::Md));
                if !parts.owned_ids.is_empty()
                    && super::tree::show_delete_tree(app, ui, t, &parts.owned_ids, &mut expanded)
                {
                    ui.add(Spacer::new(Space::Md));
                }
                paint_delete_parts(ui, t, &parts);
                ui.add(Spacer::new(Space::Md));
                let foot = sheet_footer(
                    ui,
                    t,
                    if remove_only { "Remove" } else { "Delete" },
                    SheetFooterOpts::default()
                        .danger(!remove_only)
                        .divider(false)
                        .primary_shortcut(shortcut_return()),
                );
                if foot.cancel {
                    queue.push(A::CloseModal);
                }
                if foot.primary {
                    queue.push(A::ConfirmDelete);
                }
            });
        });

    ctx.data_mut(|d| d.insert_temp(exp_id, expanded));
}

struct DeleteParts {
    owned_ids: Vec<Uuid>,
    owned: Vec<(String, bool)>,
    shares: Vec<(String, bool)>,
}

fn paint_delete_parts(ui: &mut egui::Ui, t: &Theme, parts: &DeleteParts) {
    let mixed = !parts.owned.is_empty() && !parts.shares.is_empty();
    if mixed {
        paint_delete_para(ui, t, &parts.owned);
        ui.add(Spacer::new(Space::Md));
        paint_delete_para(ui, t, &parts.shares);
    } else if !parts.owned.is_empty() {
        paint_delete_para(ui, t, &parts.owned);
    } else if !parts.shares.is_empty() {
        paint_delete_para(ui, t, &parts.shares);
    }
}

fn paint_delete_para(ui: &mut egui::Ui, t: &Theme, spans: &[(String, bool)]) {
    use workspace_rs::widgets::GlyphonLabel;
    if spans.is_empty() {
        return;
    }
    let ink = t.neutral_fg();
    let fs = TypeRole::Body.size();
    let lh = TypeRole::Body.line_height();
    let max_w = crate::components::ui_width(ui).max(1.0);
    let rich: Vec<(&str, bool)> = spans.iter().map(|(s, b)| (s.as_str(), *b)).collect();
    ui.add(
        GlyphonLabel::new_rich(rich, ink)
            .font_size(fs)
            .line_height(lh)
            .max_width(max_w),
    );
}

fn delete_parts(app: &ShellApp, ids: &[Uuid]) -> DeleteParts {
    let undo = "This cannot be undone.";
    let empty = DeleteParts {
        owned_ids: Vec::new(),
        owned: vec![(undo.into(), false)],
        shares: Vec::new(),
    };
    let Some(ready) = app.session.ready() else {
        return empty;
    };
    let me = ready.workspace.account.username.as_str();
    let files = ready.workspace.files.read().unwrap();

    // Folder covers its selected kids — same roots as the delete tree.
    let roots: Vec<Uuid> = ids
        .iter()
        .copied()
        .filter(|&id| {
            !ids.iter()
                .any(|&other| other != id && delete_is_strict_ancestor(&*files, other, id))
        })
        .collect();
    if roots.is_empty() {
        return empty;
    }

    let mut cascade = 0usize;
    let mut bytes = 0u64;
    let mut any_folder = false;
    let mut owned_ids = Vec::new();
    let mut names: Vec<String> = Vec::with_capacity(roots.len());
    let mut link_names: Vec<String> = Vec::new();
    for id in &roots {
        let Some(f) = files.get_by_id(*id) else {
            continue;
        };
        let shown = display_file_name(&f.name).to_owned();
        if is_saved_share(f, me) {
            link_names.push(shown);
            continue;
        }
        let is_folder = f.is_folder();
        any_folder |= is_folder;
        cascade += 1 + files.descendents(*id).len();
        bytes += files.size_bytes_recursive.get(id).copied().unwrap_or(0);
        names.push(shown);
        owned_ids.push(*id);
    }

    let shares = if link_names.is_empty() { Vec::new() } else { delete_link_spans(&link_names) };
    if names.is_empty() {
        return DeleteParts { owned_ids, owned: Vec::new(), shares };
    }

    // Descendants only (excludes the folder row itself).
    let inside = cascade.saturating_sub(1);
    let size = delete_size_paren(bytes);
    let mut spans: Vec<(String, bool)> = Vec::new();

    match (names.len(), any_folder, cascade) {
        (1, false, _) => {
            // **Linux** will be permanently deleted (12.4 MB).
            if let Some(n) = names.first() {
                spans.push((n.clone(), true));
                spans.push((" will be permanently deleted".into(), false));
            } else {
                spans.push(("This file will be permanently deleted".into(), false));
            }
            if !size.is_empty() {
                spans.push((size.clone(), true));
            }
            spans.push((".".into(), false));
        }
        (1, true, 1) => {
            // **Notes** will be permanently deleted. (empty folder)
            if let Some(n) = names.first() {
                spans.push((n.clone(), true));
                spans.push((" will be permanently deleted".into(), false));
            } else {
                spans.push(("This empty folder will be permanently deleted".into(), false));
            }
            if !size.is_empty() {
                spans.push((size.clone(), true));
            }
            spans.push((".".into(), false));
        }
        (1, true, _) => {
            // **Notes** and **12 items** in it will be permanently deleted.
            if let Some(n) = names.first() {
                spans.push((n.clone(), true));
                spans.push((" and ".into(), false));
            } else {
                spans.push(("This folder and ".into(), false));
            }
            spans.push((delete_count_noun(inside, "item", "items"), true));
            spans.push((" in it will be permanently deleted".into(), false));
            if !size.is_empty() {
                spans.push((size.clone(), true));
            }
            spans.push((".".into(), false));
        }
        // Few named roots, no cascade (docs and/or empty folders): list names.
        (n, _, c) if n <= 4 && c == names.len() && names.len() == n => {
            delete_push_name_list(&mut spans, &names);
            spans.push((" will be permanently deleted".into(), false));
            if !size.is_empty() {
                spans.push((size.clone(), true));
            }
            spans.push((".".into(), false));
        }
        // Docs-only multi (too many to name): **3 files** …
        (_, false, n) => {
            spans.push((delete_count_noun(n, "file", "files"), true));
            spans.push((" will be permanently deleted".into(), false));
            if !size.is_empty() {
                spans.push((size.clone(), true));
            }
            spans.push((".".into(), false));
        }
        // Multi with folder(s), no extra descendants.
        (_, true, n) if n == names.len() => {
            spans.push((delete_count_noun(n, "item", "items"), true));
            spans.push((" will be permanently deleted".into(), false));
            if !size.is_empty() {
                spans.push((size.clone(), true));
            }
            spans.push((".".into(), false));
        }
        // Cascade: **8 items** will be permanently deleted, including folder contents.
        (_, true, n) => {
            spans.push((delete_count_noun(n, "item", "items"), true));
            spans.push((" will be permanently deleted, including folder contents".into(), false));
            if !size.is_empty() {
                spans.push((size.clone(), true));
            }
            spans.push((".".into(), false));
        }
    }

    spans.push((" ".into(), false));
    spans.push((undo.into(), false));
    DeleteParts { owned_ids, owned: spans, shares }
}

/// Removing an accepted share from Files — not a real delete.
fn delete_link_spans(names: &[String]) -> Vec<(String, bool)> {
    let mut spans = Vec::new();
    delete_push_name_list(&mut spans, names);
    spans.push((
        " will be removed from files. You'll still have access through Shared with me.".into(),
        false,
    ));
    spans
}

/// **a**, **b**, and **c** (share-style list, bold names).
fn delete_push_name_list(spans: &mut Vec<(String, bool)>, names: &[String]) {
    match names {
        [] => {}
        [one] => spans.push((one.clone(), true)),
        [a, b] => {
            spans.push((a.clone(), true));
            spans.push((" and ".into(), false));
            spans.push((b.clone(), true));
        }
        many => {
            for (i, n) in many.iter().enumerate() {
                if i > 0 {
                    if i + 1 == many.len() {
                        spans.push((", and ".into(), false));
                    } else {
                        spans.push((", ".into(), false));
                    }
                }
                spans.push((n.clone(), true));
            }
        }
    }
}

/// ` (12.4 MB)` when size is ≥ 1 MB; otherwise empty.
pub(crate) fn delete_size_paren(bytes: u64) -> String {
    if bytes >= 1_000_000 {
        format!(" ({})", super::prefs::format_bytes(bytes))
    } else {
        String::new()
    }
}

pub(crate) fn delete_count_noun(n: usize, one: &str, many: &str) -> String {
    if n == 1 { format!("1 {one}") } else { format!("{n} {many}") }
}

pub(crate) fn delete_is_strict_ancestor(
    files: &impl FilesExt, ancestor: Uuid, descendant: Uuid,
) -> bool {
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

fn show_rename(app: &mut ShellApp, ctx: &egui::Context, t: &Theme, queue: &mut Vec<Action>) {
    let layer = egui::LayerId::new(Order::Foreground, Id::new("shell_rename"));
    if sheet_dim(ctx, Id::new("shell_rename_dim"), layer) {
        queue.push(A::CloseModal);
    }
    // Must match Field::id("shell_rename_field") → host.with("edit").
    let edit_id = Id::new("shell_rename_field").with("edit");
    let need_focus = ctx.data(|d| {
        d.get_temp::<bool>(Id::new("shell_rename_need_focus"))
            .unwrap_or(false)
    });

    let (id, ext_owned) = match &app.modal {
        Some(Modal::Rename { id, ext, .. }) => (*id, ext.clone()),
        _ => return,
    };
    let ext = ext_owned.as_deref();

    Area::new(Id::new("shell_rename"))
        .order(Order::Foreground)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            sheet_panel_fit(ui, t, 320.0, |ui| {
                if sheet_title_muted(ui, t, "Rename") {
                    queue.push(A::CloseModal);
                }
                ui.add(Spacer::new(Space::Md));
                // Stem editable in place on Modal::Rename.name (in-place modal fields).
                {
                    let Some(Modal::Rename { name, .. }) = &mut app.modal else {
                        return;
                    };
                    let mut field = Field::new(t, name)
                        .hint("Name")
                        .id("shell_rename_field")
                        .select_all_on_focus(true);
                    if let Some(e) = ext.filter(|e| !e.is_empty()) {
                        field = field.trailing_static(e);
                    }
                    let _ = field.show(ui);
                    if need_focus {
                        ui.memory_mut(|m| m.request_focus(edit_id));
                        if ui.memory(|m| m.has_focus(edit_id)) {
                            ui.ctx().data_mut(|d| {
                                d.insert_temp(Id::new("shell_rename_need_focus"), false);
                            });
                        }
                    }
                }
                // Snapshot name for validation only (not the Field buffer).
                let (name_snap, ext_snap) = match &app.modal {
                    Some(Modal::Rename { name, ext, .. }) => (name.clone(), ext.clone()),
                    _ => return,
                };
                let live =
                    super::apply::rename_live_status(app, id, &name_snap, ext_snap.as_deref());
                let can_commit = live.can_commit;
                let live_err = live.error;
                if let Some(err) = live_err.as_deref().filter(|e| !e.is_empty()) {
                    ui.add(Spacer::new(Space::Xs));
                    ui.label(TypeRole::Body.rich(err).color(t.danger()));
                }
                ui.add(Spacer::new(Space::Md));
                let foot = sheet_footer(
                    ui,
                    t,
                    "Rename",
                    SheetFooterOpts::default()
                        .primary_enabled(can_commit)
                        .divider(false)
                        .primary_shortcut(shortcut_enter()),
                );
                if foot.cancel {
                    queue.push(A::CloseModal);
                }
                if foot.primary {
                    queue.push(A::ConfirmRename);
                }
            });
        });
}

fn show_help(ctx: &egui::Context, t: &Theme, queue: &mut Vec<Action>) {
    let layer = egui::LayerId::new(Order::Foreground, Id::new("shell_help"));
    if sheet_dim(ctx, Id::new("shell_help_dim"), layer) {
        queue.push(A::CloseModal);
    }

    // Keep in lockstep with `ShellApp::process_keys`. Product chrome only —
    // no Esc/⌘↩ (sheet chrome shows those), no tree keys (editor owns focus).
    let rows = [
        ("⌘N", "Create"),
        ("⌘O", "Search"),
        ("⌘,", "Settings"),
        ("⌘/", "Shortcuts"),
        ("⌘E", "Toggle sidebar"),
    ];

    Area::new(Id::new("shell_help"))
        .order(Order::Foreground)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            // Title · Md · key rows (control pitch) · Lg · Quiet dismiss.
            // Keys use body-size type in kbd plates — Mono 12pt was too small
            // for the primary content of this sheet.
            sheet_panel_fit(ui, t, 360.0, |ui| {
                if sheet_title_muted(ui, t, "Shortcuts") {
                    queue.push(A::CloseModal);
                }
                ui.add(Spacer::new(Space::Md));
                for (i, (key, label)) in rows.iter().enumerate() {
                    if i > 0 {
                        ui.add(Spacer::new(Space::Sm));
                    }
                    help_shortcut_row(ui, t, key, label);
                }
                ui.add(Spacer::new(Space::Lg));
                // Single dismiss (not Cancel/Primary — help is not a decision).
                if Button::quiet(t, "Close")
                    .shortcut(crate::components::shortcut_esc())
                    .max_width(crate::components::ui_width(ui))
                    .show(ui)
                    .clicked()
                {
                    queue.push(A::CloseModal);
                }
            });
        });
}

/// One shortcuts row: kbd plate (left) · action label (right), control height.
///
/// Chord parts match button badges: Phosphor ⌘ + mono letter with
/// `PART_GAP` (Xxs / 2pt) — not a single `⌘N` string (those glue tight).
fn help_shortcut_row(ui: &mut egui::Ui, t: &Theme, key: &str, label: &str) {
    let row_h = crate::components::control_height();
    let max_w = crate::components::ui_width(ui).max(1.0);
    let (row, _) = ui.allocate_exact_size(egui::vec2(max_w, row_h), egui::Sense::hover());
    let ink = t.neutral_fg();

    // All product help chords are ⌘ + one char (N, O, ,, /, E, R).
    let rest = key.trim_start_matches('⌘');
    let cmd_g = ui.painter().layout_no_wrap(
        phosphor::COMMAND.into(),
        crate::components::phosphor_ui_font_id(),
        ink,
    );
    let letter_font = egui::FontId::new(TypeRole::Body.size(), egui::FontFamily::Monospace);
    let letter_g = ui
        .painter()
        .layout_no_wrap(rest.to_owned(), letter_font, ink);
    // Same as button kbd parts (`control::PART_GAP` / `PAD_X`).
    let part_gap = Space::Xxs.pts();
    let kbd_pad_x = Space::Sm.pts();
    let cmd_w = cmd_g.size().x;
    let cmd_h = cmd_g.size().y;
    let letter_w = letter_g.size().x;
    let letter_h = letter_g.size().y;
    let content_w = cmd_w + part_gap + letter_w;
    let kbd_w = (content_w + kbd_pad_x * 2.0).max(row_h);
    let kbd_rect = egui::Rect::from_min_size(
        egui::pos2(row.left(), row.center().y - row_h / 2.0),
        egui::vec2(kbd_w, row_h),
    );
    ui.painter().rect(
        kbd_rect,
        crate::components::Radius::Control.corner(),
        t.neutral_bg_secondary(),
        egui::Stroke::new(crate::components::STROKE_HAIRLINE, t.neutral()),
        egui::StrokeKind::Inside,
    );
    // Center the ⌘ + gap + letter block in the plate. Both parts use
    // layout-box **mid** (⌘↩ energy) — not button mono bottom-align, which
    // is for tiny `esc` next to a body label and sits the letter low here.
    let mut x = kbd_rect.center().x - content_w / 2.0;
    let cy = kbd_rect.center().y;
    ui.painter()
        .galley(egui::pos2(x, cy - cmd_h / 2.0), cmd_g, ink);
    x += cmd_w + part_gap;
    ui.painter()
        .galley(egui::pos2(x, cy - letter_h / 2.0), letter_g, ink);

    // Action name — body fg, vertically mid on the row.
    let label_galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), TypeRole::Body.font_id(), ink);
    let label_x = row.right() - label_galley.size().x;
    ui.painter().galley(
        egui::pos2(
            label_x.max(kbd_rect.right() + Space::Sm.pts()),
            cy - label_galley.size().y / 2.0,
        ),
        label_galley,
        ink,
    );
}

#[cfg(test)]
mod delete_link_copy_tests {
    use super::{delete_link_spans, delete_push_name_list};

    fn text(spans: &[(String, bool)]) -> String {
        spans.iter().map(|(s, _)| s.as_str()).collect()
    }

    #[test]
    fn single_link_is_not_permanent_delete() {
        let t = text(&delete_link_spans(&["Notes".into()]));
        assert_eq!(
            t,
            "Notes will be removed from files. You'll still have access through Shared with me."
        );
        assert!(!t.to_ascii_lowercase().contains("delete"));
        assert!(!t.contains("lose access"));
        assert!(!t.contains("cannot be undone"));
    }

    #[test]
    fn multiple_links_pluralize() {
        let t = text(&delete_link_spans(&["A".into(), "B".into()]));
        assert_eq!(
            t,
            "A and B will be removed from files. You'll still have access through Shared with me."
        );
        assert!(!t.to_ascii_lowercase().contains("delete"));
    }

    #[test]
    fn name_list_three() {
        let mut spans = Vec::new();
        delete_push_name_list(&mut spans, &["a".into(), "b".into(), "c".into()]);
        assert_eq!(text(&spans), "a, b, and c");
    }
}
