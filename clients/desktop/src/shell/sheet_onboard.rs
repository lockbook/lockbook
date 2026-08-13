use egui::{Align, Area, Id, Layout, Order};

use crate::components::{Button, Field, SheetFooterOpts, Space, Spacer, Theme, TypeRole, phosphor, segmented, sheet_band_centered, sheet_dim, sheet_footer, sheet_panel_fit, shortcut_cmd_i, shortcut_cmd_n, shortcut_return};

use super::ShellApp;
use super::action::Action as A;
use super::action::{
    Action, Modal, OnboardImportKind, OnboardLookup, OnboardMode,
};

pub(crate) fn show_onboard(app: &mut ShellApp, ctx: &egui::Context, t: &Theme, queue: &mut Vec<Action>) {
    let (mode, uname_lookup, uname_lookup_for, import_kind, busy, err) = match &app.modal {
        Some(Modal::Onboard {
            mode,
            uname_lookup,
            uname_lookup_for,
            import_kind,
            busy,
            err,
            ..
        }) => (
            *mode,
            uname_lookup.clone(),
            uname_lookup_for.clone(),
            *import_kind,
            *busy,
            err.clone(),
        ),
        _ => return,
    };

    let layer = egui::LayerId::new(Order::Foreground, Id::new("shell_onboard"));
    // Don't dismiss create/import by dim-click while busy.
    if !busy && sheet_dim(ctx, Id::new("shell_onboard_dim"), layer) {
        // Stay on onboard when signed out — only cancel sub-modes.
        if mode != OnboardMode::Choice {
            queue.push(A::OnboardSetMode(OnboardMode::Choice));
        }
    }

    let mut kind_i = import_kind.index();

    if matches!(mode, OnboardMode::Create) {
        let q_trim = match &app.modal {
            Some(Modal::Onboard { uname, .. }) => uname.trim().to_owned(),
            _ => String::new(),
        };
        if super::sheets::debounce_query(
            ctx,
            Id::new("onboard_uname_verify_due"),
            Id::new("onboard_uname_verify_q"),
            &q_trim,
            &uname_lookup_for,
        ) {
            queue.push(A::OnboardVerifyUname);
        }
    }

    // Wider plate when the 24-word grid is up.
    let panel_w = if matches!(mode, OnboardMode::Import)
        && matches!(import_kind, OnboardImportKind::Phrase)
    {
        440.0
    } else {
        380.0
    };

    // Create: Found = taken, NotFound = available.
    let uname_snap = match &app.modal {
        Some(Modal::Onboard { uname, .. }) => uname.clone(),
        _ => String::new(),
    };
    let field_dirty =
        !uname_snap.trim().is_empty() && !uname_snap.trim().eq_ignore_ascii_case(&uname_lookup_for);
    let uname_available = !uname_snap.trim().is_empty()
        && uname_snap.trim().eq_ignore_ascii_case(&uname_lookup_for)
        && matches!(uname_lookup, OnboardLookup::Available);

    Area::new(Id::new("shell_onboard"))
        .order(Order::Foreground)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            // Welcome title is brand, not a task sheet — keep strong Title (no muted X).
            // Form sub-modes use Create/Rename field rhythm: secondary label · Xs · field · Md · footer.
            sheet_panel_fit(ui, t, panel_w, |ui| {
                // Classic onboard header is just "Lockbook" (not "Welcome to…").
                ui.label(
                    TypeRole::Title
                        .rich("Lockbook")
                        .strong()
                        .color(t.neutral_fg()),
                );
                ui.add(Spacer::new(Space::Md));
                match mode {
                    OnboardMode::Choice => {
                        ui.label(
                            TypeRole::Heading
                                .rich("Notes that stay yours.")
                                .color(t.neutral_fg()),
                        );
                        ui.add(Spacer::new(Space::Sm));
                        ui.label(
                            TypeRole::Body
                                .rich(
                                    "An open-source notebook for markdown notes and SVG drawings — encrypted on your device and shared only by invitation.",
                                )
                                .color(t.neutral_fg_secondary()),
                        );
                        ui.add(Spacer::new(Space::Lg));
                        // Sheet footer rhythm: quiet Import left · primary Create right.
                        // ⌘I / ⌘N badges; keys wired in `process_keys` on Choice.
                        let row_w = crate::components::ui_width(ui);
                        let row_h = crate::components::control_height();
                        let gap = Space::Sm;
                        ui.horizontal(|ui| {
                            ui.set_min_height(row_h);
                            ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
                            let import = Button::quiet(t, "Import account")
                                .shortcut(shortcut_cmd_i())
                                .height(row_h)
                                .enabled(!busy)
                                .show(ui);
                            if import.clicked() {
                                queue.push(A::OnboardSetMode(OnboardMode::Import));
                            }
                            let import_w = import.rect.width();
                            ui.add(Spacer::new(gap).fill_cross(row_h));
                            let primary_max =
                                (row_w - import_w - gap.pts()).max(Space::Xl.pts() * 2.4);
                            ui.allocate_ui_with_layout(
                                egui::vec2(primary_max, row_h),
                                Layout::right_to_left(Align::Center),
                                |ui| {
                                    ui.set_max_width(primary_max);
                                    if Button::primary(t, "Create account")
                                        .shortcut(shortcut_cmd_n())
                                        .height(row_h)
                                        .max_width(primary_max)
                                        .enabled(!busy)
                                        .show(ui)
                                        .clicked()
                                    {
                                        queue.push(A::OnboardSetMode(OnboardMode::Create));
                                    }
                                },
                            );
                        });
                    }
                    OnboardMode::Create => {
                        ui.label(
                            TypeRole::Body
                                .rich("Username")
                                .color(t.neutral_fg_secondary()),
                        );
                        ui.add(Spacer::new(Space::Xs));
                        let (lead_icon, lead_ink) = match &uname_lookup {
                            OnboardLookup::Available => (
                                phosphor::USER_CHECK,
                                if field_dirty {
                                    t.neutral_fg_secondary()
                                } else {
                                    t.accent()
                                },
                            ),
                            OnboardLookup::Taken | OnboardLookup::Error(_) => (
                                phosphor::X_CIRCLE,
                                if field_dirty {
                                    t.neutral_fg_secondary()
                                } else {
                                    t.danger()
                                },
                            ),
                            OnboardLookup::Idle | OnboardLookup::Checking => {
                                (phosphor::USER, t.neutral_fg_secondary())
                            }
                        };
                        let edit_id = Id::new("onboard_uname").with("edit");
                        let need_focus = ui.ctx().data(|d| {
                            d.get_temp::<bool>(Id::new("onboard_uname_need_focus"))
                                .unwrap_or(false)
                        });
                        {
                            let Some(Modal::Onboard { uname, err, .. }) = &mut app.modal else {
                                return;
                            };
                            let before = uname.clone();
                            let _ = Field::new(t, uname)
                                .hint("username")
                                .id("onboard_uname")
                                .leading(lead_icon)
                                .leading_ink(lead_ink)
                                .clearable(true)
                                .show(ui);
                            if need_focus {
                                ui.memory_mut(|m| m.request_focus(edit_id));
                                if ui.memory(|m| m.has_focus(edit_id)) {
                                    ui.ctx().data_mut(|d| {
                                        d.insert_temp(Id::new("onboard_uname_need_focus"), false);
                                    });
                                }
                            }
                            if *uname != before {
                                *err = None;
                            }
                        }
                        // Always reserve status row height so footer doesn’t jump.
                        ui.add(Spacer::new(Space::Xs));
                        let uname_now = match &app.modal {
                            Some(Modal::Onboard { uname, .. }) => uname.as_str(),
                            _ => "",
                        };
                        let (status, color) = if uname_now.trim().is_empty() {
                            ("", t.neutral_fg_secondary())
                        } else if field_dirty {
                            ("Checking…", t.neutral_fg_secondary())
                        } else {
                            match &uname_lookup {
                                OnboardLookup::Available => ("Available", t.accent()),
                                OnboardLookup::Taken => ("Username taken", t.danger()),
                                OnboardLookup::Error(e) => (e.as_str(), t.danger()),
                                OnboardLookup::Idle | OnboardLookup::Checking => {
                                    ("", t.neutral_fg_secondary())
                                }
                            }
                        };
                        let status_h = TypeRole::Body.line_height();
                        let (slot, _) = ui.allocate_exact_size(
                            egui::vec2(crate::components::ui_width(ui).max(1.0), status_h),
                            egui::Sense::hover(),
                        );
                        if !status.is_empty() {
                            let g = ui.painter().layout_no_wrap(
                                status.to_owned(),
                                TypeRole::Body.font_id(),
                                color,
                            );
                            ui.painter().galley(
                                egui::pos2(slot.left(), slot.center().y - g.size().y / 2.0),
                                g,
                                color,
                            );
                        }
                        if let Some(e) = err {
                            ui.add(Spacer::new(Space::Xs));
                            ui.label(TypeRole::Body.rich(e).color(t.danger()));
                        }
                        ui.add(Spacer::new(Space::Md));
                        let foot = sheet_footer(
                            ui,
                            t,
                            if busy { "Working…" } else { "Create" },
                            SheetFooterOpts::default()
                                .divider(false)
                                .primary_enabled(!busy && uname_available)
                                .primary_shortcut(shortcut_return()),
                        );
                        if foot.cancel {
                            queue.push(A::OnboardSetMode(OnboardMode::Choice));
                        }
                        if foot.primary {
                            queue.push(A::OnboardSubmit { show_error: true });
                        }
                    }
                    OnboardMode::Import => {
                        // Compact key (default) vs structured 24-word phrase (web3-style grid).
                        sheet_band_centered(ui, crate::components::segmented_h(), |ui| {
                            if segmented(
                                ui,
                                t,
                                &["Compact key", "Phrase"],
                                &mut kind_i,
                            )
                            .changed()
                            {
                                queue.push(A::OnboardImportKind(
                                    OnboardImportKind::from_index(kind_i),
                                ));
                            }
                        });
                        ui.add(Spacer::new(Space::Md));
                        match OnboardImportKind::from_index(kind_i) {
                            OnboardImportKind::CompactKey => {
                                ui.label(
                                    TypeRole::Body
                                        .rich("Compact key")
                                        .color(t.neutral_fg_secondary()),
                                );
                                ui.add(Spacer::new(Space::Xs));
                                let edit_id = Id::new("onboard_compact").with("edit");
                                let need_focus = ui.ctx().data(|d| {
                                    d.get_temp::<bool>(Id::new("onboard_compact_need_focus"))
                                        .unwrap_or(false)
                                });
                                {
                                    let Some(Modal::Onboard { compact, .. }) = &mut app.modal
                                    else {
                                        return;
                                    };
                                    let _ = Field::new(t, compact)
                                        .hint("Paste your compact account key")
                                        .id("onboard_compact")
                                        .password(true)
                                        .show(ui);
                                    if need_focus {
                                        ui.memory_mut(|m| m.request_focus(edit_id));
                                        if ui.memory(|m| m.has_focus(edit_id)) {
                                            ui.ctx().data_mut(|d| {
                                                d.insert_temp(
                                                    Id::new("onboard_compact_need_focus"),
                                                    false,
                                                );
                                            });
                                        }
                                    }
                                }
                            }
                            OnboardImportKind::Phrase => {
                                ui.label(
                                    TypeRole::Body
                                        .rich("24-word phrase")
                                        .color(t.neutral_fg_secondary()),
                                );
                                ui.add(Spacer::new(Space::Xs));
                                {
                                    let Some(Modal::Onboard { words, .. }) = &mut app.modal else {
                                        return;
                                    };
                                    if words.len() < 24 {
                                        words.resize(24, String::new());
                                    }
                                    onboard_phrase_grid(ui, t, words);
                                }
                            }
                        }
                        if let Some(e) = err {
                            ui.add(Spacer::new(Space::Xs));
                            ui.label(TypeRole::Body.rich(e).color(t.danger()));
                        }
                        ui.add(Spacer::new(Space::Md));
                        let can_import = match (&app.modal, OnboardImportKind::from_index(kind_i)) {
                            (
                                Some(Modal::Onboard { compact, .. }),
                                OnboardImportKind::CompactKey,
                            ) => !compact.trim().is_empty(),
                            (Some(Modal::Onboard { words, .. }), OnboardImportKind::Phrase) => {
                                words.iter().filter(|w| !w.trim().is_empty()).count() == 24
                            }
                            _ => false,
                        };
                        // Auto-submit once per secret when valid. Failures stay silent
                        // until the secret changes or the user hits Import manually.
                        if can_import && !busy {
                            let secret = match (&app.modal, OnboardImportKind::from_index(kind_i)) {
                                (
                                    Some(Modal::Onboard { compact, .. }),
                                    OnboardImportKind::CompactKey,
                                ) => compact.trim().to_owned(),
                                (Some(Modal::Onboard { words, .. }), OnboardImportKind::Phrase) => {
                                    words
                                        .iter()
                                        .map(|w| w.trim())
                                        .filter(|w| !w.is_empty())
                                        .collect::<Vec<_>>()
                                        .join(" ")
                                }
                                _ => String::new(),
                            };
                            let last = ui.ctx().data(|d| {
                                d.get_temp::<String>(Id::new("onboard_auto_submit_secret"))
                            });
                            if last.as_deref() != Some(secret.as_str()) {
                                ui.ctx().data_mut(|d| {
                                    d.insert_temp(
                                        Id::new("onboard_auto_submit_secret"),
                                        secret,
                                    );
                                });
                                queue.push(A::OnboardSubmit { show_error: false });
                            }
                        }
                        let foot = sheet_footer(
                            ui,
                            t,
                            if busy { "Working…" } else { "Import" },
                            SheetFooterOpts::default()
                                .divider(false)
                                .primary_enabled(!busy && can_import)
                                .primary_shortcut(shortcut_return()),
                        );
                        if foot.cancel {
                            queue.push(A::OnboardSetMode(OnboardMode::Choice));
                        }
                        if foot.primary {
                            queue.push(A::OnboardSubmit { show_error: true });
                        }
                    }
                }
            });
        });
}

/// 4×6 numbered word slots (BIP-39 / web3 recovery-style entry).
/// Paste of a full phrase into any cell distributes across the grid and focuses
/// the **last** filled slot (so that word is revealed, not the paste source).
fn onboard_phrase_grid(ui: &mut egui::Ui, t: &Theme, words: &mut [String]) {
    const COLS: usize = 4;
    let n = words.len().min(24);
    let gap = Space::Xs.pts();
    let max_w = crate::components::ui_width(ui).max(1.0);
    let col_w = ((max_w - gap * (COLS as f32 - 1.0)) / COLS as f32).max(40.0);
    let row_h = crate::components::control_height();
    let focus_idx = ui
        .ctx()
        .data(|d| d.get_temp::<usize>(Id::new("onboard_word_need_focus")));
    // Paste into cell `i` may request focus on the last distributed word.
    let mut focus_after_paste: Option<usize> = None;

    let rows = n.div_ceil(COLS);
    for row in 0..rows {
        if row > 0 {
            ui.add(Spacer::new(Space::Xs));
        }
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
            ui.set_min_height(row_h);
            ui.set_max_height(row_h);
            for col in 0..COLS {
                let i = row * COLS + col;
                if i >= n {
                    break;
                }
                if col > 0 {
                    ui.add(Spacer::new(Space::Xs));
                }
                // Fixed band per cell so col 0 doesn’t grow from label metrics.
                ui.allocate_ui_with_layout(
                    egui::vec2(col_w, row_h),
                    Layout::left_to_right(Align::Center),
                    |ui| {
                        ui.set_width(col_w);
                        ui.set_min_height(row_h);
                        ui.set_max_height(row_h);
                        ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);

                        // Index (01–24) — painter, not `ui.label` (avoids extra pitch).
                        let idx = format!("{:02}", i + 1);
                        let idx_g = ui.painter().layout_no_wrap(
                            idx,
                            TypeRole::Mono.font_id(),
                            t.neutral_fg_secondary(),
                        );
                        let idx_w = idx_g.size().x + Space::Xxs.pts();
                        let (ir, _) =
                            ui.allocate_exact_size(egui::vec2(idx_w, row_h), egui::Sense::hover());
                        ui.painter().galley(
                            egui::pos2(ir.left(), ir.center().y - idx_g.size().y / 2.0),
                            idx_g,
                            t.neutral_fg_secondary(),
                        );

                        let field_w = (col_w - idx_w).max(20.0);
                        let id_salt = format!("onboard_word_{i}");
                        let edit_id = Id::new(&id_salt).with("edit");
                        // After multi-word paste we focus the last slot with caret at end.
                        let cursor_end = focus_idx == Some(i);
                        let _ = Field::new(t, &mut words[i])
                            .id(id_salt)
                            .width(field_w)
                            // Reveal only the focused word; others stay masked.
                            .password_when_unfocused(true)
                            .cursor_at_end_on_focus(cursor_end)
                            // Tab / Shift+Tab walk the 24 slots (not claim-for-complete).
                            .tab_navigates(true)
                            .show(ui);
                        if focus_idx == Some(i) {
                            ui.memory_mut(|m| m.request_focus(edit_id));
                            if ui.memory(|m| m.has_focus(edit_id)) {
                                ui.ctx().data_mut(|d| {
                                    d.remove_temp::<usize>(Id::new("onboard_word_need_focus"));
                                });
                            }
                        }
                        // Paste of full phrase → fill the grid (wallet pattern).
                        let parts: Vec<&str> = words[i]
                            .split([' ', ',', '\n', '\t'])
                            .map(str::trim)
                            .filter(|s| !s.is_empty())
                            .collect();
                        if parts.len() > 1 {
                            let fill_n = parts.len().min(24);
                            let owned: Vec<String> =
                                parts.iter().take(fill_n).map(|p| (*p).to_owned()).collect();
                            for (j, next) in owned.into_iter().enumerate() {
                                words[j] = next;
                            }
                            // Prefer last filled word focused/revealed (not paste source).
                            focus_after_paste = Some(fill_n.saturating_sub(1));
                        }
                        // Single-word edits stay in `words[i]` via Field (no dual buffer).
                    },
                );
            }
        });
    }

    if let Some(last) = focus_after_paste {
        let edit_id = Id::new(format!("onboard_word_{last}")).with("edit");
        let len = words.get(last).map(|s| s.len()).unwrap_or(0);
        ui.memory_mut(|m| m.request_focus(edit_id));
        workspace_rs::widgets::GlyphonTextEdit::place_cursor_at_end(ui, edit_id, len);
        ui.ctx().data_mut(|d| {
            d.insert_temp(Id::new("onboard_word_need_focus"), last);
        });
    }
}
