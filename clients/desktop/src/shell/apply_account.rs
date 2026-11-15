//! Account: logout, delete, QR, upgrade, debug dump.

use std::fs;
use std::thread;

use egui::Context;
use lb::model::api::{FREE_TIER_USAGE_SIZE, StripePlan, SubscriptionInfo};
use lb::model::errors::LbErrKind;
use lb::service::usage::UsageMetrics;
use std::time::Duration;

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

fn checkout_failed_id() -> egui::Id {
    egui::Id::unique("shell_upgrade_checkout_failed")
}

/// Account standing fetched while the Browser stage waits for Premium.
#[derive(Clone, Default)]
struct UpgradeRefresh(Option<(Option<UsageMetrics>, Option<SubscriptionInfo>)>);

fn refresh_id() -> egui::Id {
    egui::Id::unique("shell_upgrade_refresh")
}

/// How often the Browser stage asks the server whether the payment landed.
const UPGRADE_POLL: Duration = Duration::from_secs(3);

/// Opens Stripe Checkout for the chosen plan in the browser; the page then waits for Premium.
pub(crate) fn upgrade_open_checkout(app: &mut ShellApp, ctx: &Context) {
    let AccountPanel::Upgrade { stage, yearly } = &mut app.account_panel else {
        return;
    };
    *stage = UpgradeStage::Browser;
    let plan = if *yearly { StripePlan::Yearly } else { StripePlan::Monthly };
    let Some(r) = app.session.ready() else {
        return;
    };
    let core = r.workspace.core.clone();
    let toasts = app.toasts.inbox.clone();
    let ctx = ctx.clone();
    thread::spawn(move || {
        match core.stripe_checkout_url(plan) {
            Ok(url) => ctx.open_url(egui::OpenUrl::new_tab(url)),
            // the refresh below finds the Premium this account already has
            Err(e) if e.kind == LbErrKind::AlreadyPremium => {}
            Err(e) => {
                toasts.error(format!("Couldn’t open checkout: {e}"));
                ctx.data_mut(|d| d.insert_temp(checkout_failed_id(), true));
            }
        }
        ctx.request_repaint();
    });
}

/// Opens Stripe's billing page, where the account changes its card, gets receipts, or cancels.
pub(crate) fn open_billing_portal(app: &mut ShellApp, ctx: &Context) {
    let Some(r) = app.session.ready() else {
        return;
    };
    let core = r.workspace.core.clone();
    let toasts = app.toasts.inbox.clone();
    let ctx = ctx.clone();
    thread::spawn(move || {
        match core.stripe_billing_portal_url() {
            Ok(url) => ctx.open_url(egui::OpenUrl::new_tab(url)),
            Err(e) => toasts.error(format!("Couldn’t open billing: {e}")),
        }
        ctx.request_repaint();
    });
}

/// While the Browser stage waits, re-reads the account every few seconds and turns to Done once
/// it is Premium.
pub(crate) fn poll_upgrade(app: &mut ShellApp, ctx: &Context) {
    let AccountPanel::Upgrade { stage: UpgradeStage::Browser, .. } = &app.account_panel else {
        return;
    };

    if ctx
        .data_mut(|d| d.remove_temp::<bool>(checkout_failed_id()))
        .unwrap_or(false)
    {
        if let AccountPanel::Upgrade { stage, .. } = &mut app.account_panel {
            *stage = UpgradeStage::Choose;
        }
        return;
    }

    if let Some(UpgradeRefresh(Some((usage, sub_info)))) =
        ctx.data_mut(|d| d.remove_temp::<UpgradeRefresh>(refresh_id()))
    {
        if let Some(r) = app.session.ready_mut() {
            if usage.is_some() {
                r.status.space_used = usage;
            }
            r.sub_info = sub_info;
            r.refresh_status();
        }
    }

    let premium = app
        .session
        .ready()
        .and_then(|r| r.status.space_used.as_ref())
        .is_some_and(|u| u.data_cap.exact > FREE_TIER_USAGE_SIZE);
    if premium {
        if let AccountPanel::Upgrade { stage, .. } = &mut app.account_panel {
            *stage = UpgradeStage::Done;
        }
        // whatever the old cap held back uploads now
        app.push(super::action::Action::RequestSync);
        return;
    }

    let now = ctx.input(|i| i.time);
    let last_id = egui::Id::unique("shell_upgrade_last_poll");
    let in_flight_id = egui::Id::unique("shell_upgrade_poll_in_flight");
    let last = ctx.data(|d| d.get_temp::<f64>(last_id));
    let in_flight = ctx
        .data(|d| d.get_temp::<bool>(in_flight_id))
        .unwrap_or(false);
    if !in_flight && last.is_none_or(|t| now - t >= UPGRADE_POLL.as_secs_f64()) {
        if let Some(r) = app.session.ready() {
            ctx.data_mut(|d| {
                d.insert_temp(last_id, now);
                d.insert_temp(in_flight_id, true);
            });
            let core = r.workspace.core.clone();
            let ctx = ctx.clone();
            thread::spawn(move || {
                let usage = core.get_usage().ok();
                let sub_info = core.get_subscription_info().ok().flatten();
                ctx.data_mut(|d| {
                    d.insert_temp(refresh_id(), UpgradeRefresh(Some((usage, sub_info))));
                    d.insert_temp(in_flight_id, false);
                });
                ctx.request_repaint();
            });
        }
    }
    ctx.request_repaint_after(UPGRADE_POLL);
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
    thread::spawn(move || {
        use lb::service::debug::DebugInfoDisplay as _;
        let s = core.debug_info().to_string();
        if let Ok(mut g) = slot.lock() {
            *g = DebugInfoCache::Ready(s);
        }
        ctx.request_repaint();
    });
}
