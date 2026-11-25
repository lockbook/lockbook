//! Step 2 (Create) — Choose a username.

use egui::{Id, Ui, pos2, vec2};

use crate::components::{
    Field, SheetFooterOpts, Space, Spacer, Theme, TypeRole, control_height, phosphor, sheet_footer,
    shortcut_enter, ui_width,
};
use crate::shell::ShellApp;
use crate::shell::action::{Action as A, OnboardLookup, OnboardStep};
use crate::shell::apply_onboard::uname_check_matches_core;

use super::{centered_column, measure_text_h};

const COL_W: f32 = 410.0;
const TITLE: &str = "Choose a username";
const SUBTITLE: &str = "We don’t use an email or password. Pick a username and a secret key will be generated on this device for you.";

pub(super) fn show(app: &mut ShellApp, ui: &mut Ui, t: &Theme, queue: &mut Vec<A>) {
    let (uname_snap, uname_lookup, uname_lookup_for, busy, err, api_url) = match &app.onboard {
        Some(s) => (
            s.uname.clone(),
            s.uname_lookup.clone(),
            s.uname_lookup_for.clone(),
            s.busy,
            s.err.clone(),
            s.api_url.clone(),
        ),
        None => return,
    };

    let q_trim = uname_snap.trim().to_owned();
    if crate::shell::sheets::debounce_query(
        ui.ctx(),
        Id::unique("onboard_uname_verify_due"),
        Id::unique("onboard_uname_verify_q"),
        &q_trim,
        &uname_lookup_for,
    ) {
        queue.push(A::OnboardVerifyUname);
    }

    let field_dirty = !q_trim.is_empty() && !q_trim.eq_ignore_ascii_case(&uname_lookup_for);
    let uname_rejected =
        !field_dirty && matches!(uname_lookup, OnboardLookup::Taken | OnboardLookup::Error(_));
    let can_create = !q_trim.is_empty() && !uname_rejected;

    let status_h = TypeRole::Body.line_height();
    let err_for_h = err.clone();
    centered_column(
        ui,
        COL_W,
        |ui, w| {
            let err_h = err_for_h
                .as_deref()
                .map_or(0.0, |e| Space::Xs.pts() + measure_text_h(ui, TypeRole::Body.rich(e), w));
            measure_text_h(ui, TypeRole::Title.rich(TITLE).strong(), w)
                + Space::Sm.pts()
                + measure_text_h(ui, TypeRole::Body.rich(SUBTITLE), w)
                + Space::Lg.pts()
                + control_height()
                + Space::Xs.pts()
                + status_h
                + err_h
                + Space::Md.pts()
                + control_height()
        },
        |ui| {
            ui.label(TypeRole::Title.rich(TITLE).strong().color(t.neutral_fg()));
            ui.add(Spacer::new(Space::Sm));
            ui.label(
                TypeRole::Body
                    .rich(SUBTITLE)
                    .color(t.neutral_fg_secondary()),
            );
            ui.add(Spacer::new(Space::Lg));

            let (lead_icon, lead_ink) = match &uname_lookup {
                OnboardLookup::Available => (
                    phosphor::USER_CHECK,
                    if field_dirty { t.neutral_fg_secondary() } else { t.accent() },
                ),
                OnboardLookup::Taken | OnboardLookup::Error(_) => (
                    phosphor::X_CIRCLE,
                    if field_dirty { t.neutral_fg_secondary() } else { t.danger() },
                ),
                OnboardLookup::Idle | OnboardLookup::Checking { .. } => {
                    (phosphor::USER, t.neutral_fg_secondary())
                }
            };

            let edit_id = Id::unique("onboard_uname").with("edit");
            let need_focus = ui.ctx().data(|d| {
                d.get_temp::<bool>(Id::unique("onboard_uname_need_focus"))
                    .unwrap_or(false)
            });
            {
                let Some(state) = &mut app.onboard else {
                    return;
                };
                let before = state.uname.clone();
                let _ = Field::new(t, &mut state.uname)
                    .hint("username")
                    .id(Id::unique("onboard_uname"))
                    .leading(lead_icon)
                    .leading_ink(lead_ink)
                    .clearable(true)
                    .show(ui);
                if need_focus {
                    ui.memory_mut(|m| m.request_focus(edit_id));
                    if ui.memory(|m| m.has_focus(edit_id)) {
                        ui.ctx().data_mut(|d| {
                            d.insert_temp(Id::unique("onboard_uname_need_focus"), false);
                        });
                    }
                }
                if state.uname != before {
                    state.err = None;
                }
            }

            // Fixed-height status line so the footer never jumps.
            ui.add(Spacer::new(Space::Xs));
            let uname_now = app.onboard.as_ref().map(|s| s.uname.as_str()).unwrap_or("");
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
                    OnboardLookup::Checking { .. } => ("Checking…", t.neutral_fg_secondary()),
                    OnboardLookup::Idle => ("", t.neutral_fg_secondary()),
                }
            };
            let status_h = TypeRole::Body.line_height();
            let (slot, _) =
                ui.allocate_exact_size(vec2(ui_width(ui).max(1.0), status_h), egui::Sense::hover());
            if !status.is_empty() {
                let g =
                    ui.painter()
                        .layout_no_wrap(status.to_owned(), TypeRole::Body.font_id(), color);
                ui.painter().galley(
                    pos2(slot.left(), slot.center().y - g.size().y / 2.0),
                    g,
                    color,
                );
            }

            if let Some(e) = &err {
                ui.add(Spacer::new(Space::Xs));
                ui.label(TypeRole::Body.rich(e).color(t.danger()));
            }

            ui.add(Spacer::new(Space::Md));
            let foot = sheet_footer(
                ui,
                t,
                if busy { "Creating…" } else { "Create" },
                SheetFooterOpts::default()
                    .divider(false)
                    .cancel_label("Back")
                    .primary_enabled(!busy && can_create)
                    .primary_shortcut(shortcut_enter()),
            );
            if foot.cancel && !busy {
                queue.push(A::OnboardSetStep(OnboardStep::Welcome));
            }
            if foot.primary {
                queue.push(A::OnboardSubmit);
            }
        },
    );
}
