use egui::{Align, Area, Id, Layout, Order};

use crate::components::{
    Button, Field, Radius, SheetFooterOpts, Space, Spacer, Theme, TypeRole, control_height,
    phosphor, plate_content, sense_click, sheet_dim, sheet_footer, sheet_panel_fit, shortcut_cmd_i,
    shortcut_cmd_n, shortcut_return, ui_width, with_pad_fit,
};

use super::ShellApp;
use super::action::Action as A;
use super::action::{Action, Modal, OnboardLookup, OnboardMode};
use super::apply_onboard::{
    display_server_host, onboard_server_editing_key, onboard_server_snap_key,
    uname_check_matches_core,
};

pub(crate) fn show_onboard(
    app: &mut ShellApp, ctx: &egui::Context, t: &Theme, queue: &mut Vec<Action>,
) {
    let (mode, uname_lookup, uname_lookup_for, busy, err, api_url) = match &app.modal {
        Some(Modal::Onboard {
            mode, uname_lookup, uname_lookup_for, busy, err, api_url, ..
        }) => (
            *mode,
            uname_lookup.clone(),
            uname_lookup_for.clone(),
            *busy,
            err.clone(),
            api_url.clone(),
        ),
        _ => return,
    };

    let layer = egui::LayerId::new(Order::Foreground, Id::new("shell_onboard"));
    // Don't dismiss create/import by dim-click while busy, or the backup step at all.
    if !busy && sheet_dim(ctx, Id::new("shell_onboard_dim"), layer) {
        // Stay on onboard when signed out — only cancel sub-modes.
        if mode != OnboardMode::Choice && mode != OnboardMode::Backup {
            queue.push(A::OnboardSetMode(OnboardMode::Choice));
        }
    }

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

    const PANEL_W: f32 = 380.0;

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
            sheet_panel_fit(ui, t, PANEL_W, |ui| {
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
                        ui.add(Spacer::new(Space::Md));
                        onboard_server_row(app, ui, t, &api_url);
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
                                OnboardLookup::Available => {
                                    if uname_check_matches_core(&api_url) {
                                        ("Available", t.accent())
                                    } else {
                                        ("", t.neutral_fg_secondary())
                                    }
                                }
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
                        ui.label(
                            TypeRole::Body
                                .rich("Account key")
                                .color(t.neutral_fg_secondary()),
                        );
                        ui.add(Spacer::new(Space::Xs));
                        let edit_id = Id::new("onboard_account_key").with("edit");
                        let need_focus = ui.ctx().data(|d| {
                            d.get_temp::<bool>(Id::new("onboard_account_key_need_focus"))
                                .unwrap_or(false)
                        });
                        {
                            let Some(Modal::Onboard { account_key, err, .. }) = &mut app.modal
                            else {
                                return;
                            };
                            let before = account_key.clone();
                            let _ = Field::new(t, account_key)
                                .hint("Phrase or compact key")
                                .id("onboard_account_key")
                                .password(true)
                                .show(ui);
                            if need_focus {
                                ui.memory_mut(|m| m.request_focus(edit_id));
                                if ui.memory(|m| m.has_focus(edit_id)) {
                                    ui.ctx().data_mut(|d| {
                                        d.insert_temp(
                                            Id::new("onboard_account_key_need_focus"),
                                            false,
                                        );
                                    });
                                }
                            }
                            if *account_key != before {
                                *err = None;
                            }
                        }
                        if let Some(e) = err {
                            ui.add(Spacer::new(Space::Xs));
                            ui.label(TypeRole::Body.rich(e).color(t.danger()));
                        }
                        ui.add(Spacer::new(Space::Md));
                        let secret = match &app.modal {
                            Some(Modal::Onboard { account_key, .. }) => {
                                account_key.trim().to_owned()
                            }
                            _ => String::new(),
                        };
                        let can_import = !secret.is_empty();
                        // Auto-submit on every change. Failures stay silent
                        // until the user hits Import.
                        if can_import && !busy {
                            let last = ui.ctx().data(|d| {
                                d.get_temp::<String>(Id::new("onboard_auto_submit_secret"))
                            });
                            if last.as_deref() != Some(secret.as_str()) {
                                ui.ctx().data_mut(|d| {
                                    d.insert_temp(Id::new("onboard_auto_submit_secret"), secret);
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
                    OnboardMode::Backup => {
                        onboard_backup(app, ui, t, queue);
                    }
                }
            });
        });
}

fn onboard_backup(app: &mut ShellApp, ui: &mut egui::Ui, t: &Theme, queue: &mut Vec<Action>) {
    ui.label(
        TypeRole::Heading
            .rich("Your secret key")
            .color(t.neutral_fg()),
    );
    ui.add(Spacer::new(Space::Sm));
    ui.label(
        TypeRole::Body
            .rich(
                "This 24-word phrase is your password — sign in on another device, write it down, or save it in a password manager.",
            )
            .color(t.neutral_fg()),
    );
    ui.add(Spacer::new(Space::Sm));
    ui.label(
        TypeRole::Body
            .rich(
                "Anyone with it can read your notes. If you lose your last copy, we can’t help you recover your files. You can always find it in Settings.",
            )
            .color(t.neutral_fg_secondary()),
    );
    ui.add(Spacer::new(Space::Md));

    if app
        .phrase_cache
        .as_deref()
        .is_none_or(|p| p.split_whitespace().count() != 24)
    {
        if let Some(r) = app.session.ready() {
            if let Ok(p) = r.workspace.core.export_account_phrase() {
                app.phrase_cache = Some(p);
            }
        }
    }
    let phrase = app.phrase_cache.as_deref().unwrap_or("");
    let phrase_ok = phrase.split_whitespace().count() == 24;
    plate_content(ui, t.neutral_bg_secondary(), t.neutral(), Radius::Control.corner(), |ui| {
        ui.set_width(ui_width(ui));
        ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
        with_pad_fit(ui, Space::Md, |ui| {
            onboard_phrase_columns(ui, t, phrase);
        });
    });

    ui.add(Spacer::new(Space::Md));
    {
        let Some(Modal::Onboard { key_stored, .. }) = &mut app.modal else {
            return;
        };
        crate::components::ack_row(ui, t, "I’ve stored my secret key in a safe place.", key_stored);
    }
    let stored = match &app.modal {
        Some(Modal::Onboard { key_stored, .. }) => *key_stored,
        _ => false,
    };

    ui.add(Spacer::new(Space::Md));
    let row_w = ui_width(ui);
    let row_h = control_height();
    let gap = Space::Sm;
    ui.horizontal(|ui| {
        ui.set_min_height(row_h);
        ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
        let copy = Button::quiet(t, "Copy")
            .copy_feedback("shell_onboard_copy_phrase")
            .height(row_h)
            .show(ui);
        if copy.clicked() {
            queue.push(A::CopyPhrase);
        }
        let copy_w = copy.rect.width();
        ui.add(Spacer::new(gap).fill_cross(row_h));
        let primary_max = (row_w - copy_w - gap.pts()).max(Space::Xl.pts() * 2.4);
        ui.allocate_ui_with_layout(
            egui::vec2(primary_max, row_h),
            Layout::right_to_left(Align::Center),
            |ui| {
                ui.set_max_width(primary_max);
                if Button::primary(t, "Done")
                    .shortcut(shortcut_return())
                    .height(row_h)
                    .max_width(primary_max)
                    .enabled(stored && phrase_ok)
                    .show(ui)
                    .clicked()
                {
                    queue.push(A::OnboardFinishBackup);
                }
            },
        );
    });
}

fn onboard_phrase_columns(ui: &mut egui::Ui, t: &Theme, phrase: &str) {
    let words: Vec<&str> = phrase.split_whitespace().collect();
    if words.len() != 24 {
        ui.label(
            TypeRole::Mono
                .rich(if phrase.is_empty() { "Preparing phrase…" } else { phrase })
                .color(t.neutral_fg()),
        );
        return;
    }
    let col_w = (ui_width(ui) / 2.0).max(1.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
        ui.vertical(|ui| {
            ui.set_width(col_w);
            for (i, word) in words.iter().take(12).enumerate() {
                onboard_phrase_word(ui, t, i + 1, word);
            }
        });
        ui.vertical(|ui| {
            ui.set_width(col_w);
            for (i, word) in words.iter().skip(12).enumerate() {
                onboard_phrase_word(ui, t, i + 13, word);
            }
        });
    });
}

fn onboard_phrase_word(ui: &mut egui::Ui, t: &Theme, n: usize, word: &str) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(Space::Xs.pts(), 0.0);
        ui.label(TypeRole::Mono.rich(format!("{n}.")).color(t.accent()));
        ui.label(TypeRole::Mono.rich(word).color(t.neutral_fg()));
    });
}

fn onboard_server_edit_id() -> Id {
    Id::new("onboard_server").with("edit")
}

fn onboard_server_need_focus_key() -> Id {
    Id::new("onboard_server_need_focus")
}

/// Rest: hostname caption. Click: field. Blur / Enter commit; Esc reverts; × defaults.
fn onboard_server_row(app: &mut ShellApp, ui: &mut egui::Ui, t: &Theme, api_url: &str) {
    let editing_key = onboard_server_editing_key();
    let snap_key = onboard_server_snap_key();
    let mut editing = ui
        .ctx()
        .data(|d| d.get_temp::<bool>(editing_key))
        .unwrap_or(false);
    let mut need_focus = ui
        .ctx()
        .data(|d| d.get_temp::<bool>(onboard_server_need_focus_key()))
        .unwrap_or(false);

    if editing {
        {
            let Some(Modal::Onboard { api_url, .. }) = &mut app.modal else {
                return;
            };
            let _ = Field::new(t, api_url)
                .hint(lb::DEFAULT_API_LOCATION)
                .id("onboard_server")
                .clearable(true)
                .sticky(false)
                .show(ui);
            if need_focus {
                ui.memory_mut(|m| m.request_focus(onboard_server_edit_id()));
                if ui.memory(|m| m.has_focus(onboard_server_edit_id())) {
                    need_focus = false;
                }
            }
        }
        let focused = ui.memory(|m| m.has_focus(onboard_server_edit_id()));
        if !need_focus && !focused {
            editing = false;
        }
    } else if onboard_server_caption(ui, t, &display_server_host(api_url)) {
        editing = true;
        need_focus = true;
        ui.ctx()
            .data_mut(|d| d.insert_temp(snap_key, api_url.to_owned()));
    }

    ui.ctx().data_mut(|d| {
        d.insert_temp(editing_key, editing);
        d.insert_temp(onboard_server_need_focus_key(), need_focus);
    });
}

fn onboard_server_caption(ui: &mut egui::Ui, t: &Theme, host: &str) -> bool {
    let rest = t.neutral_fg_secondary();
    let g = ui
        .painter()
        .layout_no_wrap(host.to_owned(), TypeRole::Body.font_id(), rest);
    let h = crate::components::control_height();
    let w = crate::components::ui_width(ui).max(1.0);
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, h), sense_click());
    let over = ui.ctx().rect_contains_pointer(ui.layer_id(), rect);
    let ink = if over { t.neutral_fg() } else { rest };
    ui.painter()
        .galley(egui::pos2(rect.left(), rect.center().y - g.size().y / 2.0), g, ink);
    resp.clicked()
}
