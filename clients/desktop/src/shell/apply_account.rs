//! Account: logout, delete, QR, upgrade, debug dump.

use std::fs;
use std::thread;

use egui::Context;

use super::DebugInfoCache;
use super::ShellApp;
use super::action::{Modal, SettingsCat, UpgradeStage};
use super::prefs::AccountPanel;

pub(crate) fn ensure_settings_account(app: &mut ShellApp) {
    if !matches!(app.modal, Some(Modal::Settings { .. })) {
        app.modal = Some(Modal::Settings { cat: SettingsCat::Account });
    } else if let Some(Modal::Settings { cat }) = &mut app.modal {
        *cat = SettingsCat::Account;
    }
}

/// Classic logout: wipe local data and **exit**. In-process re-init races
/// `lb_bg_worker` / tokio workers holding the DB — not worth the shared-surface
/// changes in workspace/lb-rs. Next launch opens onboard.
pub(crate) fn do_logout(app: &mut ShellApp, _ctx: &Context) {
    app.modal = None;
    app.close_account_panel();
    if let Some(r) = app.session.ready_mut() {
        r.workspace.save_all_tabs();
        let path = r.workspace.core.get_config().writeable_path.clone();
        let _ = fs::remove_dir_all(path);
    }
    std::process::exit(0);
}

/// Server delete (best-effort), wipe local data, exit.
pub(crate) fn do_delete_account(app: &mut ShellApp, _ctx: &Context) {
    app.modal = None;
    app.close_account_panel();
    if let Some(r) = app.session.ready_mut() {
        r.workspace.save_all_tabs();
        let path = r.workspace.core.get_config().writeable_path.clone();
        let core = r.workspace.core.clone();
        let _ = core.delete_account();
        drop(core);
        let _ = fs::remove_dir_all(path);
    }
    std::process::exit(0);
}

pub(crate) fn spawn_account_qr(app: &mut ShellApp, ctx: &Context) {
    {
        let Ok(mut g) = app.account_qr.lock() else {
            return;
        };
        // Reuse PNG for the session (same account).
        if matches!(&*g, super::AccountQrCache::Loading | super::AccountQrCache::Ready(_)) {
            return;
        }
        *g = super::AccountQrCache::Loading;
    }
    let Some(r) = app.session.ready() else {
        if let Ok(mut g) = app.account_qr.lock() {
            *g = super::AccountQrCache::Err("Not signed in".into());
        }
        return;
    };
    let core = r.workspace.core.clone();
    let slot = app.account_qr.clone();
    let ctx = ctx.clone();
    thread::spawn(move || {
        let result = core.export_account_qr().map_err(|e| format!("{e:?}"));
        if let Ok(mut g) = slot.lock() {
            *g = match result {
                Ok(png) => super::AccountQrCache::Ready(png),
                Err(e) => super::AccountQrCache::Err(e),
            };
        }
        ctx.request_repaint();
    });
}

fn upgrade_card_digits(s: &str) -> String {
    s.chars().filter(|c| c.is_ascii_digit()).collect()
}

/// Parse `MM/YY` (or bare digits) → (month 1–12, full year).
fn parse_card_exp(exp: &str) -> Option<(i32, i32)> {
    let d = upgrade_card_digits(exp);
    if d.len() != 4 {
        return None;
    }
    let month: i32 = d[..2].parse().ok()?;
    if !(1..=12).contains(&month) {
        return None;
    }
    let yy: i32 = d[2..].parse().ok()?;
    Some((month, 2000 + yy))
}

pub(crate) fn upgrade_validate_and_confirm(app: &mut ShellApp) {
    let AccountPanel::Upgrade { stage: UpgradeStage::EnterCard, number, exp, cvc, .. } =
        &app.account_panel
    else {
        return;
    };
    let number = upgrade_card_digits(number);
    if number.is_empty() || number.len() < 12 {
        if let AccountPanel::Upgrade { error, .. } = &mut app.account_panel {
            *error = Some("Enter a valid card number".into());
        }
        return;
    }
    let Some((exp_month, exp_year)) = parse_card_exp(exp) else {
        if let AccountPanel::Upgrade { error, .. } = &mut app.account_panel {
            *error = Some("Invalid expiry (MM/YY)".into());
        }
        return;
    };
    let cvc = cvc.trim().to_string();
    if cvc.len() < 3 {
        if let AccountPanel::Upgrade { error, .. } = &mut app.account_panel {
            *error = Some("Enter CVC".into());
        }
        return;
    }
    // Normalize buffers + advance (digits-only number for API; MM/YY for display).
    if let AccountPanel::Upgrade { stage, number: n, exp: e, cvc: c, error, .. } =
        &mut app.account_panel
    {
        *n = number;
        *e = format!("{exp_month:02}/{:02}", exp_year % 100);
        *c = cvc;
        *error = None;
        *stage = UpgradeStage::Confirm;
    }
}

pub(crate) fn upgrade_start_pay(app: &mut ShellApp, ctx: &Context) {
    let (number, exp_month, exp_year, cvc) = match &app.account_panel {
        AccountPanel::Upgrade { stage: UpgradeStage::Confirm, number, exp, cvc, .. } => {
            let (m, y) = parse_card_exp(exp).unwrap_or((0, 0));
            (upgrade_card_digits(number), m, y, cvc.clone())
        }
        _ => return,
    };
    if let AccountPanel::Upgrade { stage, done, error, .. } = &mut app.account_panel {
        *stage = UpgradeStage::Paying;
        *done = None;
        *error = None;
    }
    let Some(r) = app.session.ready() else {
        if let AccountPanel::Upgrade { done, .. } = &mut app.account_panel {
            *done = Some(Err("Not signed in".into()));
        }
        return;
    };
    let core = r.workspace.core.clone();
    let ctx = ctx.clone();
    // Result channel via temp Id — worker can't mut ShellApp.
    // Wrapper: `remove_temp` requires Default (Result doesn't implement it).
    let result_id = egui::Id::new("shell_upgrade_result");
    ctx.data_mut(|d| d.remove::<UpgradePayResult>(result_id));
    thread::spawn(move || {
        use lb::model::api::{PaymentMethod, StripeAccountTier};
        let method = PaymentMethod::NewCard { number, exp_month, exp_year, cvc };
        let out = core
            .upgrade_account_stripe(StripeAccountTier::Premium(method))
            .map_err(|e| format!("{e:?}"));
        ctx.data_mut(|d| d.insert_temp(result_id, UpgradePayResult(Some(out))));
        ctx.request_repaint();
    });
}

#[derive(Clone, Default)]
struct UpgradePayResult(Option<Result<(), String>>);

/// Poll Stripe upgrade worker + refresh sub/usage on success.
pub(crate) fn poll_upgrade(app: &mut ShellApp, ctx: &Context) {
    let AccountPanel::Upgrade { stage: UpgradeStage::Paying, done: None, .. } = &app.account_panel
    else {
        return;
    };
    let result_id = egui::Id::new("shell_upgrade_result");
    let Some(UpgradePayResult(Some(out))) =
        ctx.data_mut(|d| d.remove_temp::<UpgradePayResult>(result_id))
    else {
        return;
    };
    if out.is_ok() {
        if let Some(r) = app.session.ready_mut() {
            let usage = r.workspace.core.get_usage().ok();
            if let Some(u) = usage {
                r.status.space_used = Some(u);
            }
            r.sub_info = r.workspace.core.get_subscription_info().ok().flatten();
            r.refresh_status();
        }
    }
    if let AccountPanel::Upgrade { done, .. } = &mut app.account_panel {
        *done = Some(out);
    }
}

/// Background `Lb::debug_info` (can take a beat).
pub(crate) fn spawn_debug_info(app: &mut ShellApp, ctx: &Context, force: bool) {
    {
        let Ok(mut g) = app.debug_info.lock() else {
            return;
        };
        match &*g {
            DebugInfoCache::Loading => return,
            DebugInfoCache::Ready(_) if !force => return,
            DebugInfoCache::Idle | DebugInfoCache::Ready(_) => {
                *g = DebugInfoCache::Loading;
            }
        }
    }

    let Some(r) = app.session.ready() else {
        if let Ok(mut g) = app.debug_info.lock() {
            *g = DebugInfoCache::Ready("Error retrieving debug info: not signed in".into());
        }
        return;
    };

    let core = r.workspace.core.clone();
    let slot = app.debug_info.clone();
    let ctx = ctx.clone();
    let os_info = format!(
        "{} {} / lockbook-desktop {}",
        std::env::consts::OS,
        std::env::consts::ARCH,
        env!("CARGO_PKG_VERSION")
    );
    thread::spawn(move || {
        use lb::service::debug::DebugInfoDisplay as _;
        let s = core.debug_info(os_info).to_string();
        if let Ok(mut g) = slot.lock() {
            *g = DebugInfoCache::Ready(s);
        }
        ctx.request_repaint();
    });
}
