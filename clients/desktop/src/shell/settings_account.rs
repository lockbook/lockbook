//! Settings → Account (key, plan, Stripe, QR, logout / delete).

use egui::{Id, Stroke, Ui};

use crate::components::{
    Button, FG_HOVER, Field, Radius, STROKE_HAIRLINE, SheetFooterOpts, Space, Spacer, Theme,
    TypeRole, control_height, footnote, form_group, form_row, form_row_detail, form_value,
    phosphor, phosphor_ui_font_id, plate_content, section_label, sense_click, sheet_band_centered,
    sheet_footer, shortcut_enter, shortcut_return, ui_width,
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
                .copy_feedback("shell_copy_phrase")
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
        if plan_standing(app).can_cancel() {
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
    plate_content(ui, t.neutral_bg_secondary(), t.neutral(), Radius::Control.corner(), |ui| {
        ui.set_width(ui_width(ui));
        ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
        crate::components::with_pad_fit(ui, Space::Md, |ui| {
            ui.label(TypeRole::Mono.rich(phrase).color(t.neutral_fg()));
        });
    });

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
        logout_ack_row(ui, t, acked);
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
    let edit_id = Id::new("shell_delete_account_field").with("edit");
    let need_focus = ui.ctx().data(|d| {
        d.get_temp::<bool>(Id::new("shell_delete_account_need_focus"))
            .unwrap_or(false)
    });
    let can_commit = {
        let AccountPanel::DeleteAccount { typed } = &mut app.account_panel else {
            return;
        };
        let _ = Field::new(t, typed)
            .hint("Username")
            .id("shell_delete_account_field")
            .show(ui);
        if need_focus {
            ui.memory_mut(|m| m.request_focus(edit_id));
            if ui.memory(|m| m.has_focus(edit_id)) {
                ui.ctx().data_mut(|d| {
                    d.insert_temp(Id::new("shell_delete_account_need_focus"), false);
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

/// Stripe upgrade as an in-Settings page (not a sheet).
fn page_account_upgrade(app: &mut ShellApp, ui: &mut Ui, t: &Theme, queue: &mut Vec<Action>) {
    let (stage, error, done, number_tail) = match &app.account_panel {
        AccountPanel::Upgrade { stage, error, done, number, .. } => {
            (*stage, error.clone(), done.clone(), number.clone())
        }
        _ => return,
    };

    match stage {
        UpgradeStage::EnterCard => {
            ui.label(
                TypeRole::Body
                    .rich("$2.99 / month for 30 GB of storage. Cancel anytime.")
                    .color(t.neutral_fg()),
            );
            ui.add(Spacer::new(Space::Md));
            let need_focus = ui.ctx().data(|d| {
                d.get_temp::<bool>(Id::new("shell_upgrade_need_focus"))
                    .unwrap_or(false)
            });
            {
                let AccountPanel::Upgrade { number, exp, cvc, error: card_err, .. } =
                    &mut app.account_panel
                else {
                    return;
                };
                let n0 = number.clone();
                let e0 = exp.clone();
                let c0 = cvc.clone();
                let _ = Field::new(t, number)
                    .hint("Card number")
                    .id("upgrade_number")
                    .rewrite(|s, c, a| {
                        let formatted = format_card_number(s);
                        let nc = map_digit_cursor(s, c, &formatted);
                        let na = map_digit_cursor(s, a, &formatted);
                        (formatted, nc, na)
                    })
                    .show(ui);
                if need_focus {
                    ui.memory_mut(|m| m.request_focus(Id::new("upgrade_number").with("edit")));
                    ui.ctx().data_mut(|d| {
                        d.insert_temp(Id::new("shell_upgrade_need_focus"), false);
                    });
                }
                ui.add(Spacer::new(Space::Sm));
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
                    let row_w = ui_width(ui);
                    let gap = Space::Sm;
                    let field_h = control_height();
                    let w = ((row_w - gap.pts()) / 2.0).max(1.0);
                    let _ = Field::new(t, exp)
                        .hint("MM/YY")
                        .width(w)
                        .id("upgrade_exp")
                        .rewrite(|s, c, a| {
                            let formatted = format_card_exp(s);
                            let nc = map_digit_cursor(s, c, &formatted);
                            let na = map_digit_cursor(s, a, &formatted);
                            (formatted, nc, na)
                        })
                        .show(ui);
                    ui.add(Spacer::new(gap).fill_cross(field_h));
                    let _ = Field::new(t, cvc)
                        .hint("CVC")
                        .width(w)
                        .password(true)
                        .id("upgrade_cvc")
                        .rewrite(|s, c, a| {
                            let formatted = format_card_cvc(s);
                            let nc = map_digit_cursor(s, c, &formatted);
                            let na = map_digit_cursor(s, a, &formatted);
                            (formatted, nc, na)
                        })
                        .show(ui);
                });
                if *number != n0 || *exp != e0 || *cvc != c0 {
                    *card_err = None;
                }
            }
            if let Some(err) = &error {
                ui.add(Spacer::new(Space::Sm));
                ui.label(TypeRole::Body.rich(err).color(t.danger()));
            }
            ui.add(Spacer::new(Space::Md));
            let foot = sheet_footer(
                ui,
                t,
                "Continue",
                SheetFooterOpts::default()
                    .divider(false)
                    .primary_shortcut(shortcut_enter()),
            );
            if foot.cancel {
                queue.push(A::UpgradeBack);
            }
            if foot.primary {
                queue.push(A::UpgradeNext);
            }
        }
        UpgradeStage::Confirm => {
            let last4 =
                if number_tail.len() >= 4 { &number_tail[number_tail.len() - 4..] } else { "????" };
            ui.label(
                TypeRole::Body
                    .rich(format!(
                        "Charge card ending in {last4} $2.99 / month for 30 GB of storage?"
                    ))
                    .color(t.neutral_fg()),
            );
            ui.add(Spacer::new(Space::Md));
            let foot = sheet_footer(
                ui,
                t,
                "Pay $2.99",
                SheetFooterOpts::default()
                    .divider(false)
                    .accent(true)
                    .primary_shortcut(shortcut_return()),
            );
            if foot.cancel {
                queue.push(A::UpgradeBack);
            }
            if foot.primary {
                queue.push(A::UpgradeConfirmPay);
            }
        }
        UpgradeStage::Paying => match &done {
            None => {
                sheet_band_centered(ui, 80.0, |ui| {
                    ui.spinner();
                    ui.add(Spacer::new(Space::Sm));
                    ui.label(
                        TypeRole::Body
                            .rich("Processing payment…")
                            .color(t.neutral_fg_secondary()),
                    );
                });
            }
            Some(Ok(())) => {
                ui.label(
                    TypeRole::Body
                        .rich("Subscription successful.")
                        .color(t.neutral_fg()),
                );
                ui.add(Spacer::new(Space::Md));
                let foot = sheet_footer(
                    ui,
                    t,
                    "Done",
                    SheetFooterOpts::default()
                        .divider(false)
                        .primary_shortcut(shortcut_enter()),
                );
                if foot.cancel || foot.primary {
                    queue.push(A::UpgradeDone);
                }
            }
            Some(Err(e)) => {
                ui.label(TypeRole::Body.rich(e).color(t.danger()));
                ui.add(Spacer::new(Space::Md));
                let foot =
                    sheet_footer(ui, t, "Try again", SheetFooterOpts::default().divider(false));
                if foot.cancel || foot.primary {
                    queue.push(A::UpgradeBack);
                }
            }
        },
    }
}

/// Digits only, grouped for display: Amex `34`/`37` → 4-6-5, else 4-4-4-4.
fn format_card_number(s: &str) -> String {
    let digits: String = s.chars().filter(|c| c.is_ascii_digit()).collect();
    let amex = digits.starts_with("34") || digits.starts_with("37");
    let max = if amex { 15 } else { 16 };
    let digits = &digits[..digits.len().min(max)];
    let mut out = String::with_capacity(digits.len() + 3);
    for (i, c) in digits.chars().enumerate() {
        if amex {
            if i == 4 || i == 10 {
                out.push(' ');
            }
        } else if i > 0 && i % 4 == 0 {
            out.push(' ');
        }
        out.push(c);
    }
    out
}

/// Digits only (≤4), slash after month: `MM/YY`.
fn format_card_exp(s: &str) -> String {
    let digits: String = s.chars().filter(|c| c.is_ascii_digit()).collect();
    let digits = &digits[..digits.len().min(4)];
    if digits.len() <= 2 {
        digits.to_string()
    } else {
        format!("{}/{}", &digits[..2], &digits[2..])
    }
}

/// Digits only, max 4 (Amex CVC).
fn format_card_cvc(s: &str) -> String {
    s.chars().filter(|c| c.is_ascii_digit()).take(4).collect()
}

/// Keep caret on the same digit index after grouping/stripping non-digits.
fn map_digit_cursor(old: &str, old_cursor: usize, new: &str) -> usize {
    let digits_before = old[..old_cursor.min(old.len())]
        .chars()
        .filter(|c| c.is_ascii_digit())
        .count();
    let mut seen = 0usize;
    for (i, c) in new.char_indices() {
        if seen == digits_before {
            return i;
        }
        if c.is_ascii_digit() {
            seen += 1;
        }
    }
    new.len()
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

/// Checkbox + wrapping copy; whole row toggles (logout confirm).
fn logout_ack_row(ui: &mut Ui, t: &Theme, on: &mut bool) {
    let label =
        "I am signed in on another device or have access to a backup of my phrase or compact key.";
    let box_s = TypeRole::Body.line_height().min(control_height() * 0.85);
    let gap = Space::Sm.pts();
    let max_w = ui_width(ui).max(1.0);
    let text_w = (max_w - box_s - gap).max(1.0);
    let galley =
        ui.painter()
            .layout(label.to_owned(), TypeRole::Body.font_id(), t.neutral_fg(), text_w);
    let row_h = galley.size().y.max(box_s);
    let (row, resp) = ui.allocate_exact_size(egui::vec2(max_w, row_h), sense_click());
    if resp.clicked() {
        *on = !*on;
    }
    let over = ui.ctx().rect_contains_pointer(ui.layer_id(), row);
    let box_rect = egui::Rect::from_min_size(
        egui::pos2(row.left(), row.center().y - box_s / 2.0),
        egui::vec2(box_s, box_s),
    );
    let ground = t.neutral_bg();
    let fill = if *on {
        t.accent()
    } else if over {
        t.wash_toward_neutral_fg(ground, FG_HOVER)
    } else {
        ground
    };
    ui.painter().rect(
        box_rect,
        Radius::Sm.corner(),
        fill,
        Stroke::new(STROKE_HAIRLINE, if *on { t.accent() } else { t.neutral() }),
        egui::StrokeKind::Inside,
    );
    if *on {
        let check = phosphor::CHECK;
        let ig = ui
            .painter()
            .layout_no_wrap(check.into(), phosphor_ui_font_id(), t.neutral_bg());
        ui.painter().galley(
            egui::pos2(
                box_rect.center().x - ig.size().x / 2.0,
                box_rect.center().y - ig.size().y / 2.0,
            ),
            ig,
            t.neutral_bg(),
        );
    }
    let text_pos = egui::pos2(row.left() + box_s + gap, row.top());
    ui.painter().galley(text_pos, galley, t.neutral_fg());
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
    let tex_key = Id::new("shell_account_qr_texture");
    let len_key = Id::new("shell_account_qr_texture_len");
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
            if let Some(detail) = standing.detail.as_deref() {
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
                .unwrap_or("Premium unlocks 30 GB for $2.99 / month.");
            form_row_detail(ui, t, "Payment", detail, |ui| {
                if Button::primary(t, "Upgrade").show(ui).clicked() {
                    queue.push(A::OpenUpgrade);
                }
            });
        }
    });
    if !paying {
        footnote(
            ui,
            t,
            "Card checkout is processed by Stripe. Review Terms of Service before upgrading.",
        );
    }
}
