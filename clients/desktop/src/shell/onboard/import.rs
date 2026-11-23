//! Step 2 & 3 (Sign in) — 3×8 phrase grid, compact key mode, and in-place
//! `"Welcome back, {username}"` initial sync screen.

use egui::{Align, Id, Layout, Rect, Sense, Ui, pos2, vec2};

use crate::components::{
    Button, Field, Radius, Space, Spacer, Theme, TypeRole, control_height, icon_button, phosphor,
    shortcut_enter, shortcut_esc, ui_width,
};
use crate::shell::ShellApp;
use crate::shell::action::{Action as A, OnboardStep};

use super::phrase_grid::{all_words_filled, grid_height, show_import_grid};
use super::{centered_column, measure_text_h, paint_logo};

const GRID_COL_W: f32 = 570.0;
const COMPACT_COL_W: f32 = 410.0;
const LOGO_SIDE: f32 = 44.0;
const GRID_SUBTITLE: &str =
    "Enter or paste your 24-word secret key. Each word masks once you move to the next slot.";
const COMPACT_SUBTITLE: &str = "Paste your compact account key.";

pub(super) fn show_import(app: &mut ShellApp, ui: &mut Ui, t: &Theme, queue: &mut Vec<A>) {
    let compact = app.onboard.as_ref().is_some_and(|s| s.import_compact);
    if compact {
        show_compact(app, ui, t, queue);
    } else {
        show_phrase_grid(app, ui, t, queue);
    }
}

fn show_phrase_grid(app: &mut ShellApp, ui: &mut Ui, t: &Theme, queue: &mut Vec<A>) {
    let (busy, err) = match &app.onboard {
        Some(s) => (s.busy, s.err.clone()),
        None => return,
    };

    let err_for_h = err.clone();
    centered_column(
        ui,
        GRID_COL_W,
        |ui, w| {
            let err_h = err_for_h
                .as_deref()
                .map_or(0.0, |e| Space::Sm.pts() + measure_text_h(ui, TypeRole::Body.rich(e), w));
            measure_text_h(ui, TypeRole::Title.rich("Sign in").strong(), w)
                + Space::Sm.pts()
                + measure_text_h(ui, TypeRole::Body.rich(GRID_SUBTITLE), w)
                + Space::Lg.pts()
                + grid_height()
                + err_h
                + Space::Lg.pts()
                + control_height()
        },
        |ui| {
            ui.label(
                TypeRole::Title
                    .rich("Sign in")
                    .strong()
                    .color(t.neutral_fg()),
            );
            ui.add(Spacer::new(Space::Sm));
            ui.label(
                TypeRole::Body
                    .rich(GRID_SUBTITLE)
                    .color(t.neutral_fg_secondary()),
            );
            ui.add(Spacer::new(Space::Lg));

            let mut request_focus = ui.ctx().data(|d| {
                d.get_temp::<bool>(Id::unique("onboard_import_need_focus"))
                    .unwrap_or(false)
            });

            let grid_res = {
                let Some(state) = &mut app.onboard else {
                    return;
                };
                let res = show_import_grid(
                    ui,
                    t,
                    Id::unique("onboard_import_grid"),
                    &mut state.import_words,
                    &mut state.import_focus,
                    &mut request_focus,
                );
                if res.changed {
                    state.err = None;
                }
                if let Some(compact) = &res.compact_key_pasted {
                    state.import_compact = true;
                    state.compact_key = compact.clone();
                    state.err = None;
                }
                res
            };

            ui.ctx().data_mut(|d| {
                d.insert_temp(Id::unique("onboard_import_need_focus"), request_focus);
            });

            if !busy && (grid_res.compact_key_pasted.is_some() || grid_res.pasted_full_phrase) {
                queue.push(A::OnboardSubmit { show_error: true });
            }

            if let Some(e) = &err {
                ui.add(Spacer::new(Space::Sm));
                ui.label(TypeRole::Body.rich(e).color(t.danger()));
            }

            ui.add(Spacer::new(Space::Lg));
            let all_filled = app
                .onboard
                .as_ref()
                .is_some_and(|s| all_words_filled(&s.import_words));

            import_footer(
                ui,
                t,
                busy,
                all_filled,
                "Compact key",
                queue,
                A::OnboardToggleCompact(true),
            );
        },
    );
}

fn show_compact(app: &mut ShellApp, ui: &mut Ui, t: &Theme, queue: &mut Vec<A>) {
    let (busy, err, visible) = match &app.onboard {
        Some(s) => (s.busy, s.err.clone(), s.compact_visible),
        None => return,
    };

    let err_for_h = err.clone();
    centered_column(
        ui,
        COMPACT_COL_W,
        |ui, w| {
            let err_h = err_for_h
                .as_deref()
                .map_or(0.0, |e| Space::Sm.pts() + measure_text_h(ui, TypeRole::Body.rich(e), w));
            measure_text_h(ui, TypeRole::Title.rich("Sign in").strong(), w)
                + Space::Sm.pts()
                + measure_text_h(ui, TypeRole::Body.rich(COMPACT_SUBTITLE), w)
                + Space::Lg.pts()
                + control_height()
                + err_h
                + Space::Lg.pts()
                + control_height()
        },
        |ui| {
            ui.label(
                TypeRole::Title
                    .rich("Sign in")
                    .strong()
                    .color(t.neutral_fg()),
            );
            ui.add(Spacer::new(Space::Sm));
            ui.label(
                TypeRole::Body
                    .rich(COMPACT_SUBTITLE)
                    .color(t.neutral_fg_secondary()),
            );
            ui.add(Spacer::new(Space::Lg));

            let edit_id = Id::unique("onboard_account_key").with("edit");
            let need_focus = ui.ctx().data(|d| {
                d.get_temp::<bool>(Id::unique("onboard_account_key_need_focus"))
                    .unwrap_or(false)
            });
            let row_w = ui_width(ui);
            let eye_w = control_height();
            let field_w = (row_w - eye_w - Space::Xs.pts()).max(120.0);

            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                {
                    let Some(state) = &mut app.onboard else {
                        return;
                    };
                    let before = state.compact_key.clone();
                    let _ = Field::new(t, &mut state.compact_key)
                        .hint("Compact key")
                        .width(field_w)
                        .id(Id::unique("onboard_account_key"))
                        .password(!visible)
                        .show(ui);
                    if need_focus {
                        ui.memory_mut(|m| m.request_focus(edit_id));
                        if ui.memory(|m| m.has_focus(edit_id)) {
                            ui.ctx().data_mut(|d| {
                                d.insert_temp(Id::unique("onboard_account_key_need_focus"), false);
                            });
                        }
                    }
                    if state.compact_key != before {
                        state.err = None;
                    }
                }
                ui.add(Spacer::new(Space::Xs).fill_cross(eye_w));
                let eye_icon = if visible { phosphor::EYE_SLASH } else { phosphor::EYE };
                if icon_button(ui, t, eye_icon, visible, t.neutral_bg()).clicked() {
                    if let Some(state) = &mut app.onboard {
                        state.compact_visible = !state.compact_visible;
                    }
                }
            });

            let secret = app
                .onboard
                .as_ref()
                .map(|s| s.compact_key.trim().to_owned())
                .unwrap_or_default();
            let can_import = !secret.is_empty();

            // Auto-submit silently when compact key changes (deduped via `last_auto_submit_hash`,
            // shared with `submit_import` so a compact key pasted into the 24-word grid does not
            // re-submit silently and wipe its own error).
            if can_import && !busy {
                let secret_hash = super::secret_hash(&secret);
                let already_tried =
                    app.onboard.as_ref().and_then(|s| s.last_auto_submit_hash) == Some(secret_hash);
                if !already_tried {
                    if let Some(state) = &mut app.onboard {
                        state.last_auto_submit_hash = Some(secret_hash);
                    }
                    queue.push(A::OnboardSubmit { show_error: false });
                }
            }

            if let Some(e) = &err {
                ui.add(Spacer::new(Space::Sm));
                ui.label(TypeRole::Body.rich(e).color(t.danger()));
            }

            ui.add(Spacer::new(Space::Lg));
            import_footer(
                ui,
                t,
                busy,
                can_import,
                "24 words",
                queue,
                A::OnboardToggleCompact(false),
            );
        },
    );
}

fn import_footer(
    ui: &mut Ui, t: &Theme, busy: bool, can_submit: bool, toggle_label: &str, queue: &mut Vec<A>,
    toggle_action: A,
) {
    let row_w = ui_width(ui);
    let row_h = control_height();
    ui.horizontal(|ui| {
        ui.set_min_height(row_h);
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        let back = Button::quiet(t, "Back")
            .shortcut(shortcut_esc())
            .height(row_h)
            .enabled(!busy)
            .show(ui);
        if back.clicked() {
            queue.push(A::OnboardSetStep(OnboardStep::Welcome));
        }
        let left_w = back.rect.width();
        let right_w = (row_w - left_w - Space::Sm.pts()).max(120.0);
        ui.add(Spacer::new(Space::Sm).fill_cross(row_h));
        ui.allocate_ui_with_layout(
            vec2(right_w, row_h),
            Layout::right_to_left(Align::Center),
            |ui| {
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                let mut primary = Button::primary(t, if busy { "Signing in…" } else { "Sign in" })
                    .height(row_h)
                    .enabled(!busy && can_submit);
                if !busy && can_submit {
                    primary = primary.shortcut(shortcut_enter());
                }
                if primary.show(ui).clicked() {
                    queue.push(A::OnboardSubmit { show_error: true });
                }
                ui.add(Spacer::new(Space::Sm).fill_cross(row_h));
                if Button::quiet(t, toggle_label)
                    .height(row_h)
                    .enabled(!busy)
                    .show(ui)
                    .clicked()
                {
                    queue.push(toggle_action);
                }
            },
        );
    });
}

/// Step 3 (Sign in) — In-place `"Welcome back, {username}"` sync screen.
pub(super) fn show_syncing(app: &mut ShellApp, ui: &mut Ui, t: &Theme, queue: &mut Vec<A>) {
    let (username, status, err) = match &app.onboard {
        Some(s) => (s.sync_username.clone(), s.sync_status.clone(), s.err.clone()),
        None => return,
    };

    let heading = if username.is_empty() {
        "Welcome back".to_owned()
    } else {
        format!("Welcome back, {username}")
    };
    let subtitle = if err.is_some() {
        "Couldn’t finish syncing your files.".to_owned()
    } else if status.is_empty() {
        "Syncing your encrypted files…".to_owned()
    } else {
        status
    };
    let heading_for_h = heading.clone();
    let subtitle_for_h = subtitle.clone();
    let err_for_h = err.clone();

    centered_column(
        ui,
        COMPACT_COL_W,
        |ui, w| {
            let tail_h = match err_for_h.as_deref() {
                Some(e) => {
                    measure_text_h(ui, TypeRole::Body.rich(e), w)
                        + Space::Md.pts()
                        + control_height()
                }
                None => 4.0,
            };
            LOGO_SIDE
                + Space::Lg.pts()
                + measure_text_h(ui, TypeRole::Title.rich(&heading_for_h).strong(), w)
                + Space::Sm.pts()
                + measure_text_h(ui, TypeRole::Body.rich(&subtitle_for_h), w)
                + Space::Lg.pts()
                + tail_h
        },
        |ui| {
            paint_logo(ui, LOGO_SIDE);
            ui.add(Spacer::new(Space::Lg));
            ui.label(TypeRole::Title.rich(heading).strong().color(t.neutral_fg()));
            ui.add(Spacer::new(Space::Sm));
            ui.label(
                TypeRole::Body
                    .rich(subtitle)
                    .color(t.neutral_fg_secondary()),
            );
            ui.add(Spacer::new(Space::Lg));

            if let Some(e) = &err {
                ui.label(TypeRole::Body.rich(e).color(t.danger()));
                ui.add(Spacer::new(Space::Md));
                let row_w = ui_width(ui);
                let row_h = control_height();
                ui.allocate_ui_with_layout(
                    vec2(row_w, row_h),
                    Layout::right_to_left(Align::Center),
                    |ui| {
                        if Button::primary(t, "Retry")
                            .shortcut(shortcut_enter())
                            .height(row_h)
                            .show(ui)
                            .clicked()
                        {
                            queue.push(A::OnboardRetryImportSync);
                        }
                    },
                );
            } else {
                paint_progress_bar(ui, t);
                ui.ctx().request_repaint();
            }
        },
    );
}

fn paint_progress_bar(ui: &mut Ui, t: &Theme) {
    let w = ui_width(ui).max(1.0);
    let h = 4.0;
    let (track, _) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
    ui.painter()
        .rect_filled(track, Radius::Sm.corner(), t.neutral_bg_secondary());

    // Smooth indeterminate sweep along the track.
    let time = ui.input(|i| i.time) as f32;
    let cycle = (time * 0.65) % 1.0;
    let bar_w = (w * 0.34).max(48.0);
    let start_x = track.left() - bar_w + (w + bar_w) * cycle;
    let bar = Rect::from_min_size(pos2(start_x, track.top()), vec2(bar_w, h)).intersect(track);
    if bar.width() > 0.0 {
        ui.painter()
            .rect_filled(bar, Radius::Sm.corner(), t.accent());
    }
}
