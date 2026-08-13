use egui::{Area, Id, Order};
use lb::Uuid;
use workspace_rs::file_cache::FilesExt;

use crate::components::{
    Chip, ChipHue, EqualCells, Field, PersonRow, PersonTone, SheetFooterOpts, Space, Spacer, Theme,
    TypeRole, fixed_height_list, person_row_height, phosphor, sheet_dim, sheet_footer,
    sheet_panel_fit, sheet_title_muted, shortcut_return,
};

use super::ShellApp;
use super::action::Action as A;
use super::action::{Action, Modal, ShareLookup, ShareStaged};
use lb::model::file::ShareMode;

pub(crate) fn show_share(
    app: &mut ShellApp, ctx: &egui::Context, t: &Theme, queue: &mut Vec<Action>,
) {
    // Drain background username checks (never block the UI thread on network).
    super::apply::share_poll_network(app, ctx);

    let id = match &app.modal {
        Some(Modal::Share { id, .. }) => *id,
        _ => return,
    };

    let subject = super::sheets::file_name(app, id);
    let layer = egui::LayerId::new(Order::Foreground, Id::new("shell_share"));
    if sheet_dim(ctx, Id::new("shell_share_dim"), layer) {
        queue.push(A::CloseModal);
    }

    // Copy-cheap / display snapshots — Field edits Modal::Share.query in place.
    let (mode_buf_init, lookup, lookup_for, err, staged_snap) = match &app.modal {
        Some(Modal::Share { mode, lookup, lookup_for, err, staged, .. }) => {
            (*mode, lookup.clone(), lookup_for.clone(), err.clone(), staged.clone())
        }
        _ => return,
    };
    let mut mode_buf = mode_buf_init;
    let staged = staged_snap;
    let edit_id = Id::new("shell_share_field").with("edit");
    let need_focus = ctx.data(|d| {
        d.get_temp::<bool>(Id::new("shell_share_need_focus"))
            .unwrap_or(false)
    });

    // Network verify only: 100ms quiet after a local miss. Known usernames are
    // stamped Found in share_query (instant). Staged tokens verify off-thread.
    let (q_before, q_for_debounce) = match &app.modal {
        Some(Modal::Share { query, .. }) => (query.clone(), query.trim().to_owned()),
        _ => return,
    };
    if super::sheets::debounce_query(
        ctx,
        Id::new("shell_share_verify_due"),
        Id::new("shell_share_verify_q"),
        &q_for_debounce,
        &lookup_for,
    ) {
        queue.push(A::ShareVerify);
    }

    let access = share_access_rows(app, id);
    let access_n = access.len();

    // Share if any staged Found, or a settled Found field token.
    let field_ok = !q_for_debounce.is_empty()
        && q_for_debounce == lookup_for
        && matches!(lookup, ShareLookup::Found);
    let staged_found = staged
        .iter()
        .filter(|s| matches!(s.lookup, ShareLookup::Found))
        .count();
    let staged_checking = staged
        .iter()
        .any(|s| matches!(s.lookup, ShareLookup::Checking));
    let can_share = (staged_found > 0 || field_ok) && !staged_checking;

    let share_names = share_summary_names(&staged, q_for_debounce.as_str(), &lookup, &lookup_for);
    let show_summary = !share_names.is_empty();

    // Five full PersonRows: measure pitch, then lock the viewport (empty slots OK).
    const ACCESS_VISIBLE: f32 = 5.0;

    Area::new(Id::new("shell_share"))
        .order(Order::Foreground)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            sheet_panel_fit(ui, t, 380.0, |ui| {
                // Title only — file name lives in the commit summary, not the chrome.
                if sheet_title_muted(ui, t, "Share") {
                    queue.push(A::CloseModal);
                }

                // ── 1. Who already has access ────────────────────────────
                ui.add(Spacer::new(Space::Md));
                ui.label(
                    TypeRole::Body
                        .rich(format!("People with access ({access_n})"))
                        .color(t.neutral_fg_secondary()),
                );
                ui.add(Spacer::new(Space::Xs));
                let access_list_h = ACCESS_VISIBLE * person_row_height(ui, true);
                fixed_height_list(ui, t, Id::new("shell_share_access"), access_list_h, |ui| {
                    if access.is_empty() {
                        ui.label(
                            TypeRole::Body
                                .rich("Only you have access.")
                                .color(t.neutral_fg_secondary()),
                        );
                    } else {
                        for row in &access {
                            let status = if row.is_owner {
                                "Owner".to_owned()
                            } else if let Some(via) = &row.via {
                                format!("{} · Via {via}", row.mode_label())
                            } else {
                                row.mode_label().to_owned()
                            };
                            // Icon = permission role; inheritance stays in status (“Via …”).
                            let icon = if row.is_owner {
                                Some(phosphor::USER)
                            } else {
                                match row.mode {
                                    Some(ShareMode::Write) => Some(phosphor::PENCIL_CIRCLE),
                                    Some(ShareMode::Read) => Some(phosphor::EYE),
                                    None => Some(phosphor::USER),
                                }
                            };
                            let tone =
                                if row.is_owner { PersonTone::Ok } else { PersonTone::Neutral };
                            let _ = PersonRow::new(t, &row.username)
                                .icon(icon)
                                .status(status)
                                .tone(tone)
                                .show(ui);
                        }
                    }
                });

                // ── 2. Add people (comma / paste multi-stage) ─────────────
                ui.add(Spacer::new(Space::Md));
                ui.label(
                    TypeRole::Body
                        .rich("Add people")
                        .color(t.neutral_fg_secondary()),
                );
                ui.add(Spacer::new(Space::Xs));
                // Lookup icon: keep last result while typing; mute until debounce
                // settles (avoids USER ↔ check/x flicker). Settled = full accent/danger.
                let field_dirty = !q_for_debounce.is_empty() && q_for_debounce != lookup_for;
                let (lead_icon, lead_ink) = match &lookup {
                    ShareLookup::Found => (
                        phosphor::USER_CHECK,
                        if field_dirty { t.neutral_fg_secondary() } else { t.accent() },
                    ),
                    ShareLookup::NotFound | ShareLookup::Error(_) => (
                        phosphor::X_CIRCLE,
                        if field_dirty { t.neutral_fg_secondary() } else { t.danger() },
                    ),
                    // Idle: empty / not yet verified. Checking is staged-only.
                    ShareLookup::Idle | ShareLookup::Checking => {
                        (phosphor::USER, t.neutral_fg_secondary())
                    }
                };

                // Ghost + Tab: shortest known prefix (from pre-edit debounce snap).
                let complete_full =
                    super::apply::share_shortest_prefix_match(app, q_for_debounce.as_str());
                let ghost_suffix = complete_full
                    .as_ref()
                    .and_then(|full| share_completion_ghost(q_for_debounce.as_str(), full));

                // Field edits Modal::Share.query in place (in-place modal fields).
                let query_changed = {
                    let Some(Modal::Share { query, .. }) = &mut app.modal else {
                        return;
                    };
                    let mut field = Field::new(t, query)
                        .hint("Username")
                        .leading(lead_icon)
                        .leading_ink(lead_ink)
                        .clearable(true)
                        .id("shell_share_field");
                    if let Some(suffix) = ghost_suffix {
                        field = field.completion_suffix(suffix);
                    }
                    if let Some(full) = complete_full {
                        field = field.completion_full(full);
                    }
                    let _ = field.show(ui);
                    if need_focus {
                        ui.memory_mut(|m| m.request_focus(edit_id));
                        if ui.memory(|m| m.has_focus(edit_id)) {
                            ui.ctx().data_mut(|d| {
                                d.insert_temp(Id::new("shell_share_need_focus"), false);
                            });
                        }
                    }
                    *query != q_before
                };
                // Comma-stage + local-known stamp (apply reads modal.query).
                if query_changed {
                    queue.push(A::ShareQuery);
                }

                // Staged batch: wrap spacing from EqualCells gap only (one plan).
                if !staged.is_empty() {
                    ui.add(Spacer::new(Space::Xs));
                    let gap = EqualCells::gap_pts();
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(gap, gap);
                        for s in &staged {
                            let hue = match &s.lookup {
                                ShareLookup::Found | ShareLookup::Idle => ChipHue::Neutral,
                                ShareLookup::NotFound | ShareLookup::Error(_) => ChipHue::Red,
                                ShareLookup::Checking => ChipHue::Yellow,
                            };
                            let out = Chip::new(t, &s.username).hue(hue).dismissible().show(ui);
                            if out.dismissed {
                                queue.push(A::ShareUnstage(s.username.clone()));
                            }
                        }
                    });
                }
                if let Some(e) = err.as_deref().filter(|e| !e.is_empty()) {
                    ui.add(Spacer::new(Space::Xs));
                    ui.label(TypeRole::Body.rich(e).color(t.danger()));
                }

                ui.add(Spacer::new(Space::Sm));
                ui.label(
                    TypeRole::Body
                        .rich("Access")
                        .color(t.neutral_fg_secondary()),
                );
                ui.add(Spacer::new(Space::Xs));
                let labels = ["Can edit", "Can view"];
                if crate::components::segmented(ui, t, &labels, &mut mode_buf).changed() {
                    queue.push(A::ShareMode(mode_buf));
                }

                // Commit block (footer spacing): always Xl from Access; summary
                // when anyone is in the batch (stage or settled Found field);
                // Md groups copy with the footer.
                ui.add(Spacer::new(Space::Xl));
                if show_summary {
                    paint_share_summary(ui, t, &subject, &share_names, mode_buf == 0);
                    ui.add(Spacer::new(Space::Md));
                }

                // ── Footer: Cancel + primary Share ───────────────────────
                // ⌘⏎ commits; plain ⏎ stages the field token (ShareStageField).
                let foot = sheet_footer(
                    ui,
                    t,
                    "Share",
                    SheetFooterOpts::default()
                        .divider(false)
                        .accent(true)
                        .primary_enabled(can_share)
                        .primary_shortcut(shortcut_return()),
                );
                if foot.cancel {
                    queue.push(A::CloseModal);
                }
                if foot.primary {
                    queue.push(A::ShareInvite);
                }
            });
        });
}

/// Ghost after the typed buffer: remainder of `full` once `query` is a
/// case-insensitive prefix. Empty query / exact match → no ghost.
fn share_completion_ghost(query: &str, full: &str) -> Option<String> {
    if query.is_empty() {
        return None;
    }
    let q_chars: Vec<char> = query.chars().collect();
    let f_chars: Vec<char> = full.chars().collect();
    if f_chars.len() <= q_chars.len() {
        return None;
    }
    let prefix_ok = q_chars
        .iter()
        .zip(f_chars.iter())
        .all(|(a, b)| a.eq_ignore_ascii_case(b));
    if !prefix_ok {
        return None;
    }
    Some(f_chars[q_chars.len()..].iter().collect())
}

/// Found usernames for the commit summary (stage + settled field token).
///
/// Field counts only when verify has settled on that exact text (`lookup_for`)
/// — not while typing with a stale Found from a previous name.
fn share_summary_names(
    staged: &[ShareStaged], field: &str, field_lookup: &ShareLookup, lookup_for: &str,
) -> Vec<String> {
    let mut names: Vec<String> = staged
        .iter()
        .filter(|s| matches!(s.lookup, ShareLookup::Found))
        .map(|s| s.username.clone())
        .collect();
    let field_settled =
        !field.is_empty() && field == lookup_for && matches!(field_lookup, ShareLookup::Found);
    if field_settled && !names.iter().any(|n| n.eq_ignore_ascii_case(field)) {
        names.push(field.to_owned());
    }
    names
}

/// **file** will be shared with **alice** and **bob** with **edit access**.
///
/// Body fg; bold on the variable bits (subject, people, access). Glyphon for
/// emoji-safe names — same stack as create/move summaries.
fn paint_share_summary(ui: &mut egui::Ui, t: &Theme, subject: &str, names: &[String], edit: bool) {
    use workspace_rs::widgets::GlyphonLabel;

    let access = if edit { "edit access" } else { "view access" };
    let ink = t.neutral_fg();
    let fs = TypeRole::Body.size();
    let lh = TypeRole::Body.line_height();
    let max_w = crate::components::ui_width(ui).max(1.0);

    // (text, bold)
    let mut spans: Vec<(String, bool)> = Vec::new();
    spans.push((subject.to_owned(), true));
    spans.push((" will be shared with ".into(), false));

    match names {
        [] => {}
        [one] => spans.push((one.clone(), true)),
        [a, b] => {
            spans.push((a.clone(), true));
            spans.push((" and ".into(), false));
            spans.push((b.clone(), true));
        }
        many if many.len() <= 4 => {
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
        many => {
            spans.push((format!("{} people", many.len()), true));
        }
    }

    spans.push((" with ".into(), false));
    spans.push((access.into(), true));
    spans.push((".".into(), false));

    let rich: Vec<(&str, bool)> = spans.iter().map(|(s, b)| (s.as_str(), *b)).collect();
    ui.add(
        GlyphonLabel::new_rich(rich, ink)
            .font_size(fs)
            .line_height(lh)
            .max_width(max_w),
    );
}

#[derive(Clone, Debug)]
struct ShareAccessRow {
    username: String,
    mode: Option<ShareMode>,
    /// Folder path if access is only via an ancestor share.
    via: Option<String>,
    is_owner: bool,
}

impl ShareAccessRow {
    fn mode_label(&self) -> &'static str {
        match self.mode {
            Some(ShareMode::Write) => "Can edit",
            Some(ShareMode::Read) => "Can view",
            None if self.is_owner => "Owner",
            None => "Access",
        }
    }
}

/// Owner + direct shares on `id` + inherited shares from ancestor folders.
fn share_access_rows(app: &ShellApp, id: Uuid) -> Vec<ShareAccessRow> {
    let Some(ready) = app.session.ready() else {
        return Vec::new();
    };
    let files = ready.workspace.files.read().unwrap();
    let Some(file) = files.get_by_id(id) else {
        return Vec::new();
    };
    let me = ready
        .workspace
        .core
        .get_account()
        .map(|a| a.username)
        .unwrap_or_default();

    let mut rows: Vec<ShareAccessRow> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

    // Owner (shown as "you" when it's the signed-in account).
    let owner = file.owner.clone();
    seen.insert(owner.to_lowercase());
    rows.push(ShareAccessRow {
        username: if owner.eq_ignore_ascii_case(&me) { "you".into() } else { owner },
        mode: None,
        via: None,
        is_owner: true,
    });

    // Direct shares on this file.
    for s in &file.shares {
        let key = s.shared_with.to_lowercase();
        if seen.contains(&key) {
            continue;
        }
        seen.insert(key);
        rows.push(ShareAccessRow {
            username: s.shared_with.clone(),
            mode: Some(s.mode),
            via: None,
            is_owner: false,
        });
    }

    // Inherited: walk parents; first time we see a sharee wins as "via" that folder.
    let mut cur = if file.is_root() { None } else { Some(file.parent) };
    while let Some(pid) = cur {
        let Some(p) = files.get_by_id(pid) else {
            break;
        };
        let via_path = {
            let path = files.path(pid);
            if path.is_empty() || path == "/" {
                "/".to_owned()
            } else {
                let t = path.trim_matches('/');
                format!("/{t}/")
            }
        };
        for s in &p.shares {
            let key = s.shared_with.to_lowercase();
            if seen.contains(&key) {
                continue;
            }
            seen.insert(key);
            rows.push(ShareAccessRow {
                username: s.shared_with.clone(),
                mode: Some(s.mode),
                via: Some(via_path.clone()),
                is_owner: false,
            });
        }
        if p.is_root() {
            break;
        }
        cur = Some(p.parent);
    }

    // Owner → Can edit → Can view; alphabetical within each band.
    rows.sort_by(|a, b| {
        fn band(r: &ShareAccessRow) -> u8 {
            if r.is_owner {
                0
            } else {
                match r.mode {
                    Some(ShareMode::Write) => 1,
                    Some(ShareMode::Read) => 2,
                    None => 3,
                }
            }
        }
        band(a)
            .cmp(&band(b))
            .then_with(|| a.username.to_lowercase().cmp(&b.username.to_lowercase()))
    });
    rows
}
