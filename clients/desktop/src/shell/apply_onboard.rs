//! Onboard create / import.

use std::sync::mpsc;
use std::thread;

use egui::Context;
use workspace_rs::file_cache::FileCache;

use super::ShellApp;
use super::action::{Modal, OnboardLookup, OnboardMode};
use super::apply_share::spawn_username_exists;
use super::session::{self, CoreLoad, OnboardFail, Session};

/// Seed the onboard URL field: `API_URL` when it isn’t the hosted default.
pub(crate) fn initial_api_url() -> String {
    match std::env::var("API_URL") {
        Ok(s) => {
            let t = s.trim();
            if t.is_empty() || t == lb::DEFAULT_API_LOCATION { String::new() } else { t.to_owned() }
        }
        Err(_) => String::new(),
    }
}

/// Empty / whitespace → hosted default.
pub(crate) fn resolved_api_url(api_url: &str) -> String {
    let t = api_url.trim();
    if t.is_empty() { lb::DEFAULT_API_LOCATION.to_string() } else { t.to_owned() }
}

/// Hostname (or host:port) for the welcome caption.
pub(crate) fn display_server_host(api_url: &str) -> String {
    let url = resolved_api_url(api_url);
    url.trim()
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/')
        .to_owned()
}

fn env_or_default_api_url() -> String {
    std::env::var("API_URL").unwrap_or_else(|_| lb::DEFAULT_API_LOCATION.to_string())
}

/// Live username check hits env/default inside core; skip it when the form URL differs.
pub(crate) fn uname_check_matches_core(api_url: &str) -> bool {
    resolved_api_url(api_url) == env_or_default_api_url()
}

pub(crate) fn onboard_modal(mode: OnboardMode, err: Option<String>) -> Modal {
    onboard_form(mode, String::new(), String::new(), initial_api_url(), err)
}

pub(crate) fn onboard_form(
    mode: OnboardMode, uname: String, account_key: String, api_url: String, err: Option<String>,
) -> Modal {
    Modal::Onboard {
        mode,
        uname,
        uname_lookup: OnboardLookup::Idle,
        uname_lookup_for: String::new(),
        account_key,
        api_url,
        busy: false,
        err,
    }
}

pub(crate) fn onboard_server_editing_key() -> egui::Id {
    egui::Id::new("onboard_server_editing")
}

pub(crate) fn onboard_server_snap_key() -> egui::Id {
    egui::Id::new("onboard_server_snap")
}

pub(crate) fn onboard_import_focus(ctx: &Context) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new("onboard_account_key_need_focus"), true));
}

fn onboard_uname_verify_done_key() -> egui::Id {
    egui::Id::new("shell_onboard_uname_verify_done")
}

fn onboard_uname_inflight_key() -> egui::Id {
    egui::Id::new("shell_onboard_uname_verify_inflight")
}

/// Local format rules (mirrors server `username_is_valid`).
fn onboard_uname_format_ok(username: &str) -> bool {
    use lb::model::account::MAX_USERNAME_LENGTH;
    let u = username.trim().to_lowercase();
    !u.is_empty()
        && u.len() <= MAX_USERNAME_LENGTH
        && u.chars().all(|c| {
            c.is_ascii_lowercase()
                || c.is_ascii_digit()
                || c == '-'
                || c == '_'
                || c == '.'
                || c == '@'
        })
}

/// Apply finished create-username network checks.
pub(crate) fn onboard_poll_uname(app: &mut ShellApp, ctx: &Context) {
    let done: Option<(String, OnboardLookup)> =
        ctx.data_mut(|d| d.remove_temp::<(String, OnboardLookup)>(onboard_uname_verify_done_key()));
    let Some((name, lookup)) = done else {
        return;
    };
    if let Some(Modal::Onboard { uname, uname_lookup, uname_lookup_for, .. }) = &mut app.modal {
        if uname.trim().eq_ignore_ascii_case(&name) {
            *uname_lookup = lookup;
            *uname_lookup_for = name;
        }
    }
}

/// Debounced availability check via `username_exists` (works signed-out).
pub(crate) fn onboard_verify_uname(app: &mut ShellApp, ctx: &Context) {
    onboard_poll_uname(app, ctx);
    let (q, api_url) = match &app.modal {
        Some(Modal::Onboard {
            mode: OnboardMode::Create,
            uname,
            uname_lookup,
            uname_lookup_for,
            api_url,
            ..
        }) => {
            let q = uname.trim().to_owned();
            if !q.is_empty()
                && q.eq_ignore_ascii_case(uname_lookup_for)
                && matches!(
                    uname_lookup,
                    OnboardLookup::Available | OnboardLookup::Taken | OnboardLookup::Error(_)
                )
            {
                return;
            }
            (q, api_url.clone())
        }
        _ => return,
    };
    if q.is_empty() {
        if let Some(Modal::Onboard { uname_lookup, uname_lookup_for, .. }) = &mut app.modal {
            *uname_lookup = OnboardLookup::Idle;
            uname_lookup_for.clear();
        }
        return;
    }
    if !onboard_uname_format_ok(&q) {
        if let Some(Modal::Onboard { uname_lookup, uname_lookup_for, .. }) = &mut app.modal {
            *uname_lookup = OnboardLookup::Error("Invalid username".into());
            *uname_lookup_for = q;
        }
        return;
    }
    if !uname_check_matches_core(&api_url) {
        if let Some(Modal::Onboard { uname_lookup, uname_lookup_for, .. }) = &mut app.modal {
            *uname_lookup = OnboardLookup::Available;
            *uname_lookup_for = q;
        }
        return;
    }

    let core = match &app.session {
        Session::SignedOut { core } => core.clone(),
        Session::Ready(r) => r.workspace.core.clone(),
        _ => {
            if let Some(Modal::Onboard { uname_lookup, uname_lookup_for, .. }) = &mut app.modal {
                *uname_lookup = OnboardLookup::Error("Core not ready".into());
                *uname_lookup_for = q;
            }
            return;
        }
    };

    let inflight = ctx.data(|d| d.get_temp::<String>(onboard_uname_inflight_key()));
    if inflight.as_deref() == Some(q.as_str()) {
        return;
    }
    ctx.data_mut(|d| d.insert_temp(onboard_uname_inflight_key(), q.clone()));

    spawn_username_exists(ctx, core, q, move |ctx, q_net, result| {
        let lookup = match result {
            Ok(true) => OnboardLookup::Taken,
            Ok(false) => OnboardLookup::Available,
            Err(e) => OnboardLookup::Error(e),
        };
        let done_name = q_net.clone();
        ctx.data_mut(|d| {
            d.insert_temp(onboard_uname_verify_done_key(), (q_net, lookup));
            let clear = d
                .get_temp::<String>(onboard_uname_inflight_key())
                .as_deref()
                == Some(done_name.as_str());
            if clear {
                d.remove::<String>(onboard_uname_inflight_key());
            }
        });
    });
}

/// Flatten paste so both compact keys and phrases work.
///
/// 24 tokens → phrase (spaces kept). Otherwise drop whitespace so a wrapped
/// compact key still base64-decodes.
fn onboard_import_secret(account_key: &str) -> String {
    let words: Vec<&str> = account_key.split_whitespace().collect();
    if words.len() == 24 { words.join(" ") } else { words.concat() }
}

pub(crate) enum AutoOnboard {
    Ready {
        core: lb::blocking::Lb,
        files: FileCache,
        sub_info: Option<lb::model::api::SubscriptionInfo>,
    },
    Failed,
}

/// Apply a finished auto-import. Failures are silent — the form stays put.
pub(crate) fn poll_auto_import(app: &mut ShellApp, ctx: &Context) {
    let Some(rx) = &app.onboard_auto_rx else {
        return;
    };
    let result = match rx.try_recv() {
        Ok(r) => r,
        Err(mpsc::TryRecvError::Empty) => return,
        Err(mpsc::TryRecvError::Disconnected) => {
            app.onboard_auto_rx = None;
            if let Some(Modal::Onboard { busy, .. }) = &mut app.modal {
                *busy = false;
            }
            return;
        }
    };
    app.onboard_auto_rx = None;
    match result {
        AutoOnboard::Ready { core, files, sub_info } => {
            app.modal = None;
            app.lb_rx = None;
            app.recents_cache = Default::default();
            app.shared_cache = Default::default();
            app.session = Session::Ready(Box::new(session::Ready::new(core, files, ctx, sub_info)));
        }
        AutoOnboard::Failed => {
            if let Some(Modal::Onboard { busy, .. }) = &mut app.modal {
                *busy = false;
            }
        }
    }
}

pub(crate) fn onboard_submit(app: &mut ShellApp, ctx: &Context, show_error: bool) {
    if app.onboard_auto_rx.is_some() {
        return;
    }
    let (mode, uname, account_key, import_secret, lookup_ok, api, api_url) = match &app.modal {
        Some(Modal::Onboard {
            mode,
            uname,
            account_key,
            uname_lookup,
            uname_lookup_for,
            api_url,
            ..
        }) => {
            let u = uname.trim();
            let lookup_ok = u.eq_ignore_ascii_case(uname_lookup_for)
                && matches!(uname_lookup, OnboardLookup::Available);
            (
                *mode,
                uname.clone(),
                account_key.clone(),
                onboard_import_secret(account_key),
                lookup_ok,
                resolved_api_url(api_url),
                api_url.clone(),
            )
        }
        _ => return,
    };
    // Cheap validation stays on the UI thread so errors stay on the form.
    let local_err: Option<String> = match mode {
        OnboardMode::Create => {
            let u = uname.trim();
            if u.is_empty() {
                Some("Username required".into())
            } else if !lookup_ok {
                Some("Pick an available username".into())
            } else {
                None
            }
        }
        OnboardMode::Import => {
            if import_secret.is_empty() {
                Some("Account key required".into())
            } else {
                None
            }
        }
        OnboardMode::Choice => Some("Pick Create or Import".into()),
    };
    if let Some(e) = local_err {
        if let Some(Modal::Onboard { busy, err, .. }) = &mut app.modal {
            *busy = false;
            if show_error {
                *err = Some(e);
            } else {
                *err = None;
            }
        }
        return;
    }

    // Auto-import: try on every change. Failures are silent until Import.
    if !show_error {
        if mode != OnboardMode::Import {
            return;
        }
        let Some(core) = app.session.signed_out_core().cloned() else {
            return;
        };
        if let Some(Modal::Onboard { busy, err, .. }) = &mut app.modal {
            *busy = true;
            *err = None;
        }
        let (tx, rx) = mpsc::channel();
        app.onboard_auto_rx = Some(rx);
        let ctx = ctx.clone();
        thread::spawn(move || {
            match onboard_account(&core, mode, &uname, &import_secret, &api) {
                Ok(()) => {
                    // Import already wrote the account. Open even if sync fails
                    // so we don't leave a signed-in core on the import form.
                    let load = match onboard_sync_and_cache(core, None, &ctx) {
                        Ok(CoreLoad::Ready { core, files, sub_info }) => {
                            AutoOnboard::Ready { core, files, sub_info }
                        }
                        Ok(_) => AutoOnboard::Failed,
                        Err((core, _)) => match FileCache::new(&core) {
                            Ok(files) => {
                                let sub_info = core.get_subscription_info().ok().flatten();
                                AutoOnboard::Ready { core, files, sub_info }
                            }
                            Err(_) => AutoOnboard::Failed,
                        },
                    };
                    let _ = tx.send(load);
                }
                Err(_) => {
                    let _ = tx.send(AutoOnboard::Failed);
                }
            }
            ctx.request_repaint();
        });
        return;
    }

    let fail = OnboardFail { err: String::new(), mode, uname: uname.clone(), account_key, api_url };

    // Take the live `Lb` off the session and finish create/import + initial
    let core = match std::mem::replace(
        &mut app.session,
        Session::Error("signing in".into()), // temporary; replaced below
    ) {
        Session::SignedOut { core } => core,
        Session::Ready(r) => r.workspace.core.clone(),
        other => {
            app.session = other;
            return;
        }
    };

    let (tx, rx) = mpsc::channel();
    let status = super::session::load_status(match mode {
        OnboardMode::Create => "Creating account…",
        OnboardMode::Import => "Importing account…",
        OnboardMode::Choice => "Signing in…",
    });
    app.lb_rx = None;
    app.modal = None;
    app.recents_cache = Default::default();
    app.shared_cache = Default::default();
    app.session =
        Session::Loading { kind: super::session::LoadKind::Onboard, status: status.clone(), rx };

    let ctx = ctx.clone();
    thread::spawn(move || {
        if let Err(e) = onboard_account(&core, mode, &uname, &import_secret, &api) {
            let _ = tx.send(CoreLoad::OnboardFailed { core, fail: OnboardFail { err: e, ..fail } });
            ctx.request_repaint();
            return;
        }
        match onboard_sync_and_cache(core, Some(&status), &ctx) {
            Ok(load) => {
                let _ = tx.send(load);
            }
            Err((core, err)) => {
                let _ =
                    tx.send(CoreLoad::OnboardFailed { core, fail: OnboardFail { err, ..fail } });
            }
        }
        ctx.request_repaint();
    });
}

fn onboard_account(
    core: &lb::blocking::Lb, mode: OnboardMode, uname: &str, import_secret: &str, api: &str,
) -> Result<(), String> {
    match mode {
        OnboardMode::Create => core
            .create_account(uname.trim(), api, true)
            .map(|_| ())
            .map_err(|e| e.to_string()),
        OnboardMode::Import => core
            .import_account(import_secret, Some(api))
            .map(|_| ())
            .map_err(|e| e.to_string()),
        OnboardMode::Choice => Err("Pick Create or Import".into()),
    }
}

fn onboard_sync_and_cache(
    core: lb::blocking::Lb, status: Option<&session::LoadStatus>, ctx: &Context,
) -> Result<CoreLoad, (lb::blocking::Lb, String)> {
    use super::session::set_load_status;
    if let Some(status) = status {
        set_load_status(status, "Syncing your files…");
        ctx.request_repaint();
    }
    // Preview spinner: `LOCKBOOK_SLOW_SYNC_MS=4000 cargo run -p lockbook-desktop`
    if let Ok(ms) = std::env::var("LOCKBOOK_SLOW_SYNC_MS") {
        if let Ok(ms) = ms.parse::<u64>() {
            if ms > 0 {
                std::thread::sleep(std::time::Duration::from_millis(ms));
            }
        }
    }
    if let Err(e) = core.sync() {
        return Err((core, format!("Sync failed: {e}")));
    }
    if let Some(status) = status {
        set_load_status(status, "Opening workspace…");
        ctx.request_repaint();
    }
    match FileCache::new(&core) {
        Ok(files) => Ok(session::prepare_ready(core, files)),
        Err(e) => Err((core, format!("Couldn’t load files: {e}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::{display_server_host, onboard_import_secret, resolved_api_url};

    #[test]
    fn empty_url_is_hosted_default() {
        assert_eq!(resolved_api_url(""), lb::DEFAULT_API_LOCATION);
        assert_eq!(resolved_api_url("  "), lb::DEFAULT_API_LOCATION);
        assert_eq!(resolved_api_url("http://localhost:8000"), "http://localhost:8000");
        assert_eq!(display_server_host(""), "app.lockbook.net");
        assert_eq!(display_server_host("http://localhost:8000"), "localhost:8000");
    }

    #[test]
    fn import_secret_phrase_keeps_spaces_compact_strips() {
        let phrase = (0..24)
            .map(|i| format!("w{i}"))
            .collect::<Vec<_>>()
            .join(" ");
        assert_eq!(onboard_import_secret(&phrase), phrase);
        assert_eq!(onboard_import_secret("abcd\n efgh"), "abcdefgh");
        assert_eq!(onboard_import_secret("  abcd  "), "abcd");
    }
}
