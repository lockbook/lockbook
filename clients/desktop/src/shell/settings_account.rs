//! Settings → Account (key, plan, Stripe, QR, logout / delete).

use egui::{Id, Ui};

use crate::components::{
    Button, Field, Radius, SheetFooterOpts, Space, Spacer, Theme, TypeRole, control_height,
    footnote, form_group, form_row, form_row_detail, form_segmented, form_value, section_label,
    sheet_band_centered, sheet_footer, shortcut_return, ui_width,
};
use crate::shell::ShellApp;
use crate::shell::action::{Action, Action as A, UpgradeStage};
use crate::shell::prefs::AccountPanel;

fn plan_standing(app: &ShellApp) -> super::account_plan::AccountStanding {
    use super::account_plan::AccountStanding;
    let cap = app
        .session
        .ready()
        .and_then(|r| r.status.space_used.as_ref())
        .map(|u| u.data_cap.exact);
    let info = app.session.ready().and_then(|r| r.sub_info.as_ref());
    AccountStanding::from_subscription_and_cap(info, cap)
}

pub(crate) fn page_account(app: &mut ShellApp, ui: &mut Ui, t: &Theme, queue: &mut Vec<Action>) {
    // In-content panels (phrase / QR / manage). Esc / Back → HideAccountKey.
    match &app.account_panel {
        AccountPanel::Phrase => {
            page_account_phrase(app, ui, t, queue);
            return;
        }
        AccountPanel::Qr => {
            page_account_qr(app, ui, t, queue);
            return;
        }
        AccountPanel::Logout { .. } => {
            page_account_logout(app, ui, t, queue);
            return;
        }
        AccountPanel::DeleteAccount { .. } => {
            page_account_delete(app, ui, t, queue);
            return;
        }
        AccountPanel::CancelSub => {
            page_account_cancel_sub(app, ui, t, queue);
            return;
        }
        AccountPanel::Upgrade { .. } => {
            page_account_upgrade(app, ui, t, queue);
            return;
        }
        AccountPanel::Closed => {}
    }

    // ── Account key (single plate; page title is already Account) ────────
    form_group(ui, t, |ui| {
        // Copy · Reveal · Show QR. Detail = never-share (link-previews pattern).
        form_row_detail(ui, t, "Account key", "Never share — grants anyone full access.", |ui| {
            if Button::quiet(t, "Copy")
                .copy_feedback(egui::Id::unique("shell_copy_phrase"))
                .show(ui)
                .clicked()
            {
                queue.push(A::CopyPhrase);
            }
            ui.add(Spacer::new(Space::Sm).fill_cross(control_height()));
            if Button::quiet(t, "Reveal").show(ui).clicked() {
                queue.push(A::RevealPhrase);
            }
            ui.add(Spacer::new(Space::Sm).fill_cross(control_height()));
            if Button::quiet(t, "Show QR").show(ui).clicked() {
                queue.push(A::OpenAccountQr);
            }
        });
    });

    // ── Plan (tier + storage + upgrade / renewal) ───────────────────────
    ui.add(Spacer::new(Space::Lg));
    page_plan_body(app, ui, t, queue);

    // ── Log out · cancel sub · delete — one bottom cluster ──────────────
    ui.add(Spacer::new(Space::Lg));
    section_label(ui, t, "Manage");
    form_group(ui, t, |ui| {
        form_row(ui, t, "Log out", |ui| {
            if Button::quiet(t, "Log out…").danger().show(ui).clicked() {
                queue.push(A::OpenLogout);
            }
        });
        let standing = plan_standing(app);
        if standing.can_cancel() && standing.source != Some("Stripe") {
            form_row(ui, t, "Subscription", |ui| {
                if Button::quiet(t, "Cancel…").danger().show(ui).clicked() {
                    queue.push(A::CancelSubscription);
                }
            });
        }
        form_row(ui, t, "Delete account", |ui| {
            if Button::quiet(t, "Delete…").danger().show(ui).clicked() {
                queue.push(A::OpenDeleteAccount);
            }
        });
    });
}

/// In-settings phrase view (body · mono plate · Back · Copy). Esc → HideAccountKey.
fn page_account_phrase(app: &ShellApp, ui: &mut Ui, t: &Theme, queue: &mut Vec<Action>) {
    // Parallel to QR: “Scan this QR… to sign in.”
    ui.label(
        TypeRole::Body
            .rich(
                "Write these 24 words down or enter them in Lockbook on another device to sign in.",
            )
            .color(t.neutral_fg()),
    );
    ui.add(Spacer::new(Space::Md));

    let phrase = app.phrase_cache.as_deref().unwrap_or("Preparing phrase…");
    // Elevated plate + pad — natural height, no tight form_group squeeze.
    crate::components::plate_content_shadow(
        ui,
        t.neutral_bg_secondary(),
        crate::components::pin_shadow(t),
        Radius::Control.corner(),
        |ui| {
            ui.set_width(ui_width(ui));
            ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
            crate::components::with_pad_fit(ui, Space::Md, |ui| {
                ui.label(TypeRole::Mono.rich(phrase).color(t.neutral_fg()));
            });
        },
    );

    ui.add(Spacer::new(Space::Md));
    let foot = sheet_footer(
        ui,
        t,
        "Copy",
        SheetFooterOpts::default()
            .divider(false)
            .cancel_label("Back")
            .copy_feedback("shell_copy_phrase"),
    );
    if foot.cancel {
        queue.push(A::HideAccountKey);
    }
    if foot.primary {
        queue.push(A::CopyPhrase);
    }
}

/// In-settings log out confirm. Esc / Back → HideAccountKey.
fn page_account_logout(app: &mut ShellApp, ui: &mut Ui, t: &Theme, queue: &mut Vec<Action>) {
    ui.label(
        TypeRole::Body
            .rich(
                "Your key and all data on this device will be removed. You’ll need your phrase or compact key to sign back in.",
            )
            .color(t.neutral_fg()),
    );
    ui.add(Spacer::new(Space::Md));
    let acked_now = {
        let AccountPanel::Logout { acked } = &mut app.account_panel else {
            return;
        };
        crate::components::ack_row(
            ui,
            t,
            "I am signed in on another device or have access to a backup of my phrase or compact key.",
            acked,
        );
        *acked
    };
    ui.add(Spacer::new(Space::Md));
    let foot = sheet_footer(
        ui,
        t,
        "Log out",
        SheetFooterOpts::default()
            .danger(true)
            .divider(false)
            .primary_enabled(acked_now)
            .primary_shortcut(shortcut_return()),
    );
    if foot.cancel {
        queue.push(A::HideAccountKey);
    }
    if foot.primary {
        queue.push(A::ConfirmLogout);
    }
}

/// In-settings delete account (type username). Esc / Back → HideAccountKey.
fn page_account_delete(app: &mut ShellApp, ui: &mut Ui, t: &Theme, queue: &mut Vec<Action>) {
    let username = app
        .session
        .ready()
        .map(|r| r.workspace.account.username.clone())
        .unwrap_or_default();
    ui.label(
        TypeRole::Body
            .rich("This account will be permanently deleted on the server. This cannot be undone.")
            .color(t.neutral_fg()),
    );
    ui.add(Spacer::new(Space::Md));
    {
        use workspace_rs::widgets::GlyphonLabel;
        let ink = t.neutral_fg();
        let fs = TypeRole::Body.size();
        let lh = TypeRole::Body.line_height();
        let max_w = ui_width(ui).max(1.0);
        ui.add(
            GlyphonLabel::new_rich(
                vec![("Type ", false), (username.as_str(), true), (" to confirm.", false)],
                ink,
            )
            .font_size(fs)
            .line_height(lh)
            .max_width(max_w),
        );
    }
    ui.add(Spacer::new(Space::Xs));
    let edit_id = Id::unique("shell_delete_account_field").with("edit");
    let need_focus = ui.ctx().data(|d| {
        d.get_temp::<bool>(Id::unique("shell_delete_account_need_focus"))
            .unwrap_or(false)
    });
    let can_commit = {
        let AccountPanel::DeleteAccount { typed } = &mut app.account_panel else {
            return;
        };
        let _ = Field::new(t, typed)
            .hint("Username")
            .id(egui::Id::unique("shell_delete_account_field"))
            .show(ui);
        if need_focus {
            ui.memory_mut(|m| m.request_focus(edit_id));
            if ui.memory(|m| m.has_focus(edit_id)) {
                ui.ctx().data_mut(|d| {
                    d.insert_temp(Id::unique("shell_delete_account_need_focus"), false);
                });
            }
        }
        !username.is_empty() && typed.trim() == username
    };
    ui.add(Spacer::new(Space::Md));
    let foot = sheet_footer(
        ui,
        t,
        "Delete account",
        SheetFooterOpts::default()
            .danger(true)
            .divider(false)
            .primary_enabled(can_commit)
            .primary_shortcut(shortcut_return()),
    );
    if foot.cancel {
        queue.push(A::HideAccountKey);
    }
    if foot.primary {
        queue.push(A::ConfirmDeleteAccount);
    }
}

const PRICE_MONTHLY: &str = "$2.99 / month";
const PRICE_YEARLY: &str = "$30 / year";

/// Body copy with **bold** runs, wrapped to the column.
fn rich_body(ui: &mut Ui, t: &Theme, runs: Vec<(&str, bool)>) {
    use workspace_rs::widgets::GlyphonLabel;
    ui.add(
        GlyphonLabel::new_rich(runs, t.neutral_fg())
            .font_size(TypeRole::Body.size())
            .line_height(TypeRole::Body.line_height())
            .max_width(ui_width(ui).max(1.0)),
    );
}

/// Upgrade as an in-Settings page: choose a plan, pay on Stripe's page, done.
fn page_account_upgrade(app: &mut ShellApp, ui: &mut Ui, t: &Theme, queue: &mut Vec<Action>) {
    let AccountPanel::Upgrade { stage, yearly } = app.account_panel else {
        return;
    };
    let usage = app
        .session
        .ready()
        .and_then(|r| r.status.space_used.as_ref())
        .map(|u| {
            (
                u.server_usage.readable.clone(),
                u.data_cap.readable.clone(),
                u.server_usage.exact > u.data_cap.exact,
            )
        });

    match stage {
        UpgradeStage::Choose => {
            match &usage {
                Some((used, cap, true)) => rich_body(
                    ui,
                    t,
                    vec![
                        ("Your files take up ", false),
                        (used, true),
                        (" and Free has room for ", false),
                        (cap, true),
                        (". Premium has room for ", false),
                        ("30 GB", true),
                        (".", false),
                    ],
                ),
                _ => rich_body(
                    ui,
                    t,
                    vec![
                        ("Premium has room for ", false),
                        ("30 GB", true),
                        (" of notes and files.", false),
                    ],
                ),
            }
            ui.add(Spacer::new(Space::Md));
            form_group(ui, t, |ui| {
                let mut sel = yearly as usize;
                if form_segmented(ui, t, "Billing", &["Monthly", "Yearly"], &mut sel).changed() {
                    queue.push(A::UpgradeYearly(sel == 1));
                }
                if yearly {
                    form_row_detail(ui, t, "Price", "Two months free.", |ui| {
                        ui.label(
                            TypeRole::Body
                                .rich(PRICE_YEARLY)
                                .color(t.neutral_fg_secondary()),
                        );
                    });
                } else {
                    form_value(ui, t, "Price", PRICE_MONTHLY);
                }
                form_value(ui, t, "Storage", "30 GB");
            });
            footnote(ui, t, "You pay on Stripe’s page in your browser. Cancel anytime.");
            ui.add(Spacer::new(Space::Md));
            let foot = sheet_footer(
                ui,
                t,
                "Continue in browser",
                SheetFooterOpts::default()
                    .divider(false)
                    .accent(true)
                    .cancel_label("Back")
                    .primary_shortcut(shortcut_return()),
            );
            if foot.cancel {
                queue.push(A::UpgradeBack);
            }
            if foot.primary {
                queue.push(A::UpgradeCheckout);
            }
        }
        UpgradeStage::Browser => {
            sheet_band_centered(ui, 80.0, |ui| {
                ui.spinner();
                ui.add(Spacer::new(Space::Sm));
                ui.label(
                    TypeRole::Body
                        .rich("Finish paying in your browser.")
                        .color(t.neutral_fg_secondary()),
                );
            });
            ui.add(Spacer::new(Space::Md));
            let foot = sheet_footer(
                ui,
                t,
                "Open the page again",
                SheetFooterOpts::default()
                    .divider(false)
                    .quiet_primary(true)
                    .cancel_label("Back"),
            );
            if foot.cancel {
                queue.push(A::UpgradeBack);
            }
            if foot.primary {
                queue.push(A::UpgradeCheckout);
            }
        }
        UpgradeStage::Done => {
            rich_body(
                ui,
                t,
                vec![
                    ("You’re on Premium. You have room for ", false),
                    ("30 GB", true),
                    (".", false),
                ],
            );
            ui.add(Spacer::new(Space::Md));
            let foot = sheet_footer(
                ui,
                t,
                "",
                SheetFooterOpts::default()
                    .divider(false)
                    .back_only()
                    .cancel_label("Done"),
            );
            if foot.cancel {
                queue.push(A::UpgradeDone);
            }
        }
    }
}

/// In-settings cancel subscription. Esc / Back → HideAccountKey.
fn page_account_cancel_sub(app: &ShellApp, ui: &mut Ui, t: &Theme, queue: &mut Vec<Action>) {
    let period = app
        .session
        .ready()
        .and_then(|r| r.sub_info.as_ref())
        .map(|info| super::account_plan::format_period_end(info.period_end));
    let body = match period {
        Some(date) => format!("You’ll keep Premium until {date}, then return to Free."),
        None => "You’ll keep Premium until the end of the billing period, then return to Free."
            .to_owned(),
    };
    ui.label(TypeRole::Body.rich(body).color(t.neutral_fg()));
    ui.add(Spacer::new(Space::Md));
    let foot = sheet_footer(
        ui,
        t,
        "Cancel subscription",
        SheetFooterOpts::default()
            .danger(true)
            .divider(false)
            .primary_shortcut(shortcut_return()),
    );
    if foot.cancel {
        queue.push(A::HideAccountKey);
    }
    if foot.primary {
        queue.push(A::ConfirmCancelSub);
    }
}

/// In-settings QR view — same column as phrase (body · code · Back). Esc → HideAccountKey.
fn page_account_qr(app: &ShellApp, ui: &mut Ui, t: &Theme, queue: &mut Vec<Action>) {
    ui.label(
        TypeRole::Body
            .rich("Scan this QR with Lockbook on another device to sign in.")
            .color(t.neutral_fg()),
    );
    ui.add(Spacer::new(Space::Md));

    const QR_SIDE: f32 = 280.0;
    let cache = app
        .account_qr
        .lock()
        .ok()
        .map(|g| g.clone())
        .unwrap_or_default();
    match &cache {
        super::AccountQrCache::Idle | super::AccountQrCache::Loading => {
            sheet_band_centered(ui, 80.0, |ui| {
                ui.spinner();
                ui.add(Spacer::new(Space::Sm));
                ui.label(
                    TypeRole::Body
                        .rich("Preparing QR…")
                        .color(t.neutral_fg_secondary()),
                );
            });
        }
        super::AccountQrCache::Ready(png) => {
            sheet_band_centered(ui, QR_SIDE, |ui| {
                paint_account_qr_image(ui, png, QR_SIDE);
            });
        }
        super::AccountQrCache::Err(e) => {
            ui.label(TypeRole::Body.rich(e).color(t.danger()));
        }
    }

    ui.add(Spacer::new(Space::Md));
    let foot = sheet_footer(ui, t, "", SheetFooterOpts::default().divider(false).back_only());
    if foot.cancel {
        queue.push(A::HideAccountKey);
    }
}

/// Decode PNG → texture for the in-Settings QR panel.
fn paint_account_qr_image(ui: &mut Ui, png: &[u8], side: f32) {
    #[derive(Clone)]
    struct QrTex(egui::TextureHandle);
    let tex_key = Id::unique("shell_account_qr_texture");
    let len_key = Id::unique("shell_account_qr_texture_len");
    let cached_len = ui.ctx().data(|d| d.get_temp::<usize>(len_key));
    let reuse =
        cached_len == Some(png.len()) && ui.ctx().data(|d| d.get_temp::<QrTex>(tex_key).is_some());
    let tex = if reuse {
        ui.ctx()
            .data(|d| d.get_temp::<QrTex>(tex_key).map(|q| q.0))
            .expect("reuse checked")
    } else {
        match decode_png_color_image(png) {
            Ok(color) => {
                let handle = ui.ctx().load_texture(
                    format!("shell_account_qr_{}", png.len()),
                    color,
                    egui::TextureOptions::LINEAR,
                );
                ui.ctx().data_mut(|d| {
                    d.insert_temp(len_key, png.len());
                    d.insert_temp(tex_key, QrTex(handle.clone()));
                });
                handle
            }
            Err(e) => {
                ui.label(TypeRole::Body.rich(e).color(ui.visuals().error_fg_color));
                return;
            }
        }
    };
    ui.add(egui::Image::from_texture((tex.id(), egui::vec2(side, side))));
}

fn decode_png_color_image(png: &[u8]) -> Result<egui::ColorImage, String> {
    let img = image::load_from_memory(png)
        .map_err(|e| format!("Could not decode QR image: {e}"))?
        .to_rgba8();
    let size = [img.width() as usize, img.height() as usize];
    Ok(egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw()))
}

/// Plan / storage — one plate. Paying: payment/status values. Not paying:
/// Payment row carries Upgrade in the trailing slot.
/// Cancel lives with Log out / Delete under Manage.
fn page_plan_body(app: &mut ShellApp, ui: &mut Ui, t: &Theme, queue: &mut Vec<Action>) {
    // SI readable from lb (1000-based) so premium shows 30 GB not ~27.9 GiB.
    let storage = app
        .session
        .ready()
        .and_then(|r| r.status.space_used.as_ref())
        .map(|u| format!("{} of {}", u.server_usage.readable, u.data_cap.readable))
        .unwrap_or_else(|| "—".into());
    let standing = plan_standing(app);
    let tier = standing.tier.label();
    let paying = standing.tier == super::account_plan::AccountTier::Premium;

    section_label(ui, t, "Plan & storage");
    form_group(ui, t, |ui| {
        form_value(ui, t, "Tier", tier);
        form_value(ui, t, "Storage", &storage);
        if paying {
            // Standing detail (renew / access until) + source line.
            if standing.source == Some("Stripe") && standing.can_cancel() {
                let detail = standing.detail.as_deref().unwrap_or("");
                form_row_detail(ui, t, "Payment", detail, |ui| {
                    if Button::quiet(t, "Manage").show(ui).clicked() {
                        queue.push(A::ManageBilling);
                    }
                });
            } else if let Some(detail) = standing.detail.as_deref() {
                if let Some(pay) = standing.payment_line() {
                    form_row_detail(ui, t, "Payment", detail, |ui| {
                        ui.label(TypeRole::Body.rich(pay).color(t.neutral_fg_secondary()));
                    });
                } else {
                    form_value(ui, t, "Status", detail);
                }
            } else if let Some(pay) = standing.payment_line() {
                form_value(ui, t, "Payment", &pay);
            }
        } else {
            // Free / canceled: Payment row is the upgrade affordance (not a second plate).
            let detail = standing
                .detail
                .as_deref()
                .unwrap_or("Premium has room for 30 GB from $2.99 a month.");
            form_row_detail(ui, t, "Payment", detail, |ui| {
                if Button::primary(t, "Upgrade").show(ui).clicked() {
                    queue.push(A::OpenUpgrade);
                }
            });
        }
    });
}
