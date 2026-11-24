//! Account: logout, delete, QR, upgrade, debug dump.

use std::fs;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use egui::Context;
use lb::model::api::{FREE_TIER_USAGE_SIZE, SubscriptionInfo};
use lb::model::errors::LbErrKind;
use lb::service::usage::UsageMetrics;

use super::DebugInfoCache;
use super::ShellApp;
use super::action::{Modal, SettingsCat, UpgradeStage};
use super::prefs::AccountPanel;

/// How often the Browser stage asks the server whether the payment landed.
const UPGRADE_POLL: Duration = Duration::from_secs(3);
/// Stop background polling if a Checkout tab is abandoned for 15 minutes.
const UPGRADE_POLL_TIMEOUT: Duration = Duration::from_secs(15 * 60);

struct BillingRefreshResult {
    usage: Option<UsageMetrics>,
    /// `Some(sub)` = request succeeded (`sub` may be `None` for Free); `None` = transient error.
    sub_info: Option<Option<SubscriptionInfo>>,
}

enum CheckoutResult {
    Opened,
    AlreadyPremium,
    Failed(String),
}

/// Typed state for async billing refreshes, Stripe Checkout, and Billing Portal.
#[derive(Default)]
pub(crate) struct BillingState {
    refresh_rx: Option<mpsc::Receiver<BillingRefreshResult>>,
    checkout_rx: Option<mpsc::Receiver<CheckoutResult>>,
    portal_rx: Option<mpsc::Receiver<Result<String, String>>>,
    portal_awaiting_return: bool,
    portal_was_focused: bool,
    browser_poll_started: Option<Instant>,
    last_poll: Option<Instant>,
}

impl BillingState {
    pub(crate) fn stop_browser_poll(&mut self) {
        self.browser_poll_started = None;
        self.last_poll = None;
    }
}

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

/// Refreshes usage cap and subscription info off the UI thread (deduped while in flight).
pub(crate) fn spawn_billing_refresh(app: &mut ShellApp, ctx: &Context) {
    if app.billing.refresh_rx.is_some() {
        return;
    }
    let Some(r) = app.session.ready() else {
        return;
    };
    let (tx, rx) = mpsc::channel();
    app.billing.refresh_rx = Some(rx);
    app.billing.last_poll = Some(Instant::now());

    let core = r.workspace.core.clone();
    let ctx = ctx.clone();
    thread::spawn(move || {
        let usage = core.refresh_usage().ok();
        let sub_info = core.get_subscription_info().ok();
        let _ = tx.send(BillingRefreshResult { usage, sub_info });
        ctx.request_repaint();
    });
}

/// Opens Stripe Checkout in the browser; the page then waits for Premium.
pub(crate) fn upgrade_open_checkout(app: &mut ShellApp, ctx: &Context) {
    if app.billing.checkout_rx.is_some() {
        return;
    }
    let AccountPanel::Upgrade { stage } = &mut app.account_panel else {
        return;
    };
    *stage = UpgradeStage::Browser;
    app.billing.browser_poll_started = Some(Instant::now());
    app.billing.last_poll = None;

    let Some(r) = app.session.ready() else {
        return;
    };
    let (tx, rx) = mpsc::channel();
    app.billing.checkout_rx = Some(rx);

    let core = r.workspace.core.clone();
    let ctx = ctx.clone();
    thread::spawn(move || {
        let res = match core.stripe_checkout_url() {
            Ok(url) => {
                ctx.open_url(egui::OpenUrl::new_tab(url));
                CheckoutResult::Opened
            }
            // Server self-healed any live subscription; the refresh below finds Premium.
            Err(e) if e.kind == LbErrKind::AlreadyPremium => CheckoutResult::AlreadyPremium,
            Err(e) => CheckoutResult::Failed(format!("Couldn’t open checkout: {e}")),
        };
        let _ = tx.send(res);
        ctx.request_repaint();
    });
}

/// Opens Stripe's billing page, where the account changes its card, gets receipts, or cancels.
pub(crate) fn open_billing_portal(app: &mut ShellApp, ctx: &Context) {
    if app.billing.portal_rx.is_some() {
        return;
    }
    let Some(r) = app.session.ready() else {
        return;
    };
    let (tx, rx) = mpsc::channel();
    app.billing.portal_rx = Some(rx);
    app.billing.portal_awaiting_return = true;
    app.billing.portal_was_focused = true;

    let core = r.workspace.core.clone();
    let ctx = ctx.clone();
    thread::spawn(move || {
        let res = core
            .stripe_billing_portal_url()
            .map_err(|e| format!("Couldn’t open billing: {e}"));
        let _ = tx.send(res);
        ctx.request_repaint();
    });
}

/// Applies background billing refreshes, refreshes once on return from the billing portal,
/// and polls while the Upgrade Browser stage is visible and waiting for Premium.
pub(crate) fn poll_upgrade(app: &mut ShellApp, ctx: &Context) {
    // 1. Drain completed billing refresh.
    if let Some(rx) = &app.billing.refresh_rx {
        match rx.try_recv() {
            Ok(res) => {
                app.billing.refresh_rx = None;
                if let Some(r) = app.session.ready_mut() {
                    let sub_ok = res.sub_info.is_some();
                    if let Some(sub_info) = res.sub_info {
                        r.sub_info = sub_info;
                    }
                    // `refresh_status()` pulls the fresh usage cap that `core.refresh_usage()` wrote.
                    if res.usage.is_some() || sub_ok {
                        r.refresh_status();
                    }
                }
            }
            Err(mpsc::TryRecvError::Disconnected) => app.billing.refresh_rx = None,
            Err(mpsc::TryRecvError::Empty) => {}
        }
    }

    // 2. Drain completed portal-URL request.
    if let Some(rx) = &app.billing.portal_rx {
        match rx.try_recv() {
            Ok(Ok(url)) => {
                app.billing.portal_rx = None;
                ctx.open_url(egui::OpenUrl::new_tab(url));
            }
            Ok(Err(msg)) => {
                app.billing.portal_rx = None;
                app.billing.portal_awaiting_return = false;
                app.toasts.error(msg);
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                app.billing.portal_rx = None;
                app.billing.portal_awaiting_return = false;
            }
            Err(mpsc::TryRecvError::Empty) => {}
        }
    }

    // 3. Drain completed checkout-URL request.
    if let Some(rx) = &app.billing.checkout_rx {
        match rx.try_recv() {
            Ok(CheckoutResult::Opened) => app.billing.checkout_rx = None,
            Ok(CheckoutResult::AlreadyPremium) => {
                app.billing.checkout_rx = None;
                app.billing.last_poll = None;
                spawn_billing_refresh(app, ctx);
            }
            Ok(CheckoutResult::Failed(msg)) => {
                app.billing.checkout_rx = None;
                app.toasts.error(msg);
                if let AccountPanel::Upgrade { stage: stage @ UpgradeStage::Browser } =
                    &mut app.account_panel
                {
                    *stage = UpgradeStage::Choose;
                    app.billing.stop_browser_poll();
                }
            }
            Err(mpsc::TryRecvError::Disconnected) => app.billing.checkout_rx = None,
            Err(mpsc::TryRecvError::Empty) => {}
        }
    }

    // 4. Refresh once when returning to Lockbook from the Billing Portal.
    if app.billing.portal_awaiting_return {
        let focused = ctx.input(|i| i.focused);
        if !focused {
            app.billing.portal_was_focused = false;
        } else if !app.billing.portal_was_focused {
            app.billing.portal_awaiting_return = false;
            spawn_billing_refresh(app, ctx);
        }
    }

    // 5. Only poll while Settings → Account → Upgrade(Browser) is actually on screen.
    let browser_visible = matches!(app.modal, Some(Modal::Settings { cat: SettingsCat::Account }))
        && matches!(app.account_panel, AccountPanel::Upgrade { stage: UpgradeStage::Browser });
    if !browser_visible {
        return;
    }

    let premium = app
        .session
        .ready()
        .and_then(|r| r.status.usage.data_cap.as_ref())
        .is_some_and(|cap| cap.exact > FREE_TIER_USAGE_SIZE);
    if premium {
        if let AccountPanel::Upgrade { stage } = &mut app.account_panel {
            *stage = UpgradeStage::Done;
        }
        app.billing.stop_browser_poll();
        // Whatever the old cap held back uploads now.
        app.push(super::action::Action::RequestSync);
        return;
    }

    let Some(started) = app.billing.browser_poll_started else {
        return;
    };
    if started.elapsed() >= UPGRADE_POLL_TIMEOUT {
        app.billing.stop_browser_poll();
        return;
    }

    if app
        .billing
        .last_poll
        .is_none_or(|t| t.elapsed() >= UPGRADE_POLL)
    {
        spawn_billing_refresh(app, ctx);
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
