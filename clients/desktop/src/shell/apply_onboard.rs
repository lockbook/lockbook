//! Onboard create / import.

use std::sync::mpsc;
use std::thread;

use egui::Context;
use workspace_rs::file_cache::FileCache;

use super::ShellApp;
use super::action::{Modal, OnboardImportKind, OnboardLookup, OnboardMode};
use super::apply_share::spawn_username_exists;
use super::session::Session;

pub(crate) fn onboard_import_focus(ctx: &Context, app: &ShellApp) {
    let kind = match &app.modal {
        Some(Modal::Onboard { import_kind, .. }) => *import_kind,
        _ => OnboardImportKind::CompactKey,
    };
    match kind {
        OnboardImportKind::CompactKey => {
            ctx.data_mut(|d| d.insert_temp(egui::Id::new("onboard_compact_need_focus"), true));
        }
        OnboardImportKind::Phrase => {
            ctx.data_mut(|d| d.insert_temp(egui::Id::new("onboard_word_need_focus"), 0usize));
        }
    }
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
    let q = match &app.modal {
        Some(Modal::Onboard {
            mode: OnboardMode::Create,
            uname,
            uname_lookup,
            uname_lookup_for,
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
            q
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

/// Join compact buffer or 24-word slots into the secret string `import_account` expects.
fn onboard_import_secret(
    import_kind: OnboardImportKind, compact: &str, words: &[String],
) -> String {
    match import_kind {
        OnboardImportKind::CompactKey => compact.trim().to_owned(),
        OnboardImportKind::Phrase => words
            .iter()
            .map(|w| w.trim())
            .filter(|w| !w.is_empty())
            .collect::<Vec<_>>()
            .join(" "),
    }
}

pub(crate) fn onboard_submit(app: &mut ShellApp, ctx: &Context, show_error: bool) {
    let (mode, uname, import_kind, import_secret, lookup_ok) = match &app.modal {
        Some(Modal::Onboard {
            mode,
            uname,
            import_kind,
            compact,
            words,
            uname_lookup,
            uname_lookup_for,
            ..
        }) => {
            let u = uname.trim();
            let lookup_ok = u.eq_ignore_ascii_case(uname_lookup_for)
                && matches!(uname_lookup, OnboardLookup::Available);
            (
                *mode,
                uname.clone(),
                *import_kind,
                onboard_import_secret(*import_kind, compact, words),
                lookup_ok,
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
                Some(match import_kind {
                    OnboardImportKind::CompactKey => "Compact key required".into(),
                    OnboardImportKind::Phrase => "Enter your 24-word phrase".into(),
                })
            } else if matches!(import_kind, OnboardImportKind::Phrase)
                && import_secret.split_whitespace().count() != 24
            {
                Some("Phrase must be 24 words".into())
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
        use super::session::{CoreLoad, set_load_status};

        let api = std::env::var("API_URL").unwrap_or_else(|_| lb::DEFAULT_API_LOCATION.to_string());

        let account_res = match mode {
            OnboardMode::Create => {
                set_load_status(&status, "Creating account…");
                ctx.request_repaint();
                core.create_account(uname.trim(), &api, true)
                    .map(|_| ())
                    .map_err(|e| format!("{e:?}"))
            }
            OnboardMode::Import => {
                set_load_status(&status, "Importing account…");
                ctx.request_repaint();
                core.import_account(&import_secret, Some(&api))
                    .map(|_| ())
                    .map_err(|e| format!("{e:?}"))
            }
            OnboardMode::Choice => Err("Pick Create or Import".into()),
        };

        if let Err(e) = account_res {
            let _ = tx.send(CoreLoad::OnboardFailed { core, err: e });
            ctx.request_repaint();
            return;
        }

        set_load_status(&status, "Syncing your files…");
        ctx.request_repaint();
        // Preview spinner: `LOCKBOOK_SLOW_SYNC_MS=4000 cargo run -p lockbook-desktop`
        if let Ok(ms) = std::env::var("LOCKBOOK_SLOW_SYNC_MS") {
            if let Ok(ms) = ms.parse::<u64>() {
                if ms > 0 {
                    std::thread::sleep(std::time::Duration::from_millis(ms));
                }
            }
        }
        if let Err(e) = core.sync() {
            let _ = tx.send(CoreLoad::OnboardFailed { core, err: format!("Sync failed: {e:?}") });
            ctx.request_repaint();
            return;
        }

        set_load_status(&status, "Opening workspace…");
        ctx.request_repaint();
        match FileCache::new(&core) {
            Ok(files) => {
                let _ = tx.send(CoreLoad::Ready { core, files });
            }
            Err(e) => {
                let _ = tx.send(CoreLoad::OnboardFailed {
                    core,
                    err: format!("Couldn’t load files: {e:?}"),
                });
            }
        }
        ctx.request_repaint();
    });
}
