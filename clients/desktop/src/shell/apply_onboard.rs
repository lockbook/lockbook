//! Onboard create / import workers and username verification.

use std::sync::mpsc;
use std::thread;

use egui::Context;
use lb::blocking::Lb;
use lb::experiments::{OnboardLook, cohort_in};
use lb::model::api::SubscriptionInfo;
use workspace_rs::file_cache::FileCache;

use super::ShellApp;
use super::action::{Action, OnboardLookup, OnboardStep};
use super::apply_share::spawn_username_exists;
use super::nudge::Nudges;
use super::onboard::PendingCrossImport;
use super::onboard::phrase_grid::{
    all_words_filled, parse_pasted_words, request_import_slot_caret_end, words_joined,
};
use super::session::{self, Session};

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

pub(crate) fn onboard_server_editing_key() -> egui::Id {
    egui::Id::unique("onboard_server_editing")
}

pub(crate) fn onboard_server_snap_key() -> egui::Id {
    egui::Id::unique("onboard_server_snap")
}

pub(crate) fn onboard_import_focus(ctx: &Context, compact: bool) {
    if compact {
        ctx.data_mut(|d| d.insert_temp(egui::Id::unique("onboard_account_key_need_focus"), true));
    } else {
        ctx.data_mut(|d| d.insert_temp(egui::Id::unique("onboard_import_need_focus"), true));
    }
}

fn onboard_uname_verify_done_key() -> egui::Id {
    egui::Id::unique("shell_onboard_uname_verify_done")
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

fn record_uname_verify_done(ctx: &Context, seq: u64, name: String, lookup: OnboardLookup) {
    ctx.data_mut(|d| {
        let key = onboard_uname_verify_done_key();
        let keep_new = d
            .get_temp::<(u64, String, OnboardLookup)>(key)
            .is_none_or(|(prev_seq, _, _)| seq >= prev_seq);
        if keep_new {
            d.insert_temp(key, (seq, name, lookup));
        }
    });
}

/// Apply finished create-username network checks.
pub(crate) fn onboard_poll_uname(app: &mut ShellApp, ctx: &Context) {
    let done: Option<(u64, String, OnboardLookup)> = ctx.data_mut(|d| {
        d.remove_temp::<(u64, String, OnboardLookup)>(onboard_uname_verify_done_key())
    });
    let Some((seq, name, lookup)) = done else {
        return;
    };
    if let Some(state) = &mut app.onboard {
        if seq == state.uname_lookup_seq && state.uname_lookup_for.eq_ignore_ascii_case(&name) {
            let submit = state.step == OnboardStep::Create
                && lookup == OnboardLookup::Available
                && matches!(state.uname_lookup, OnboardLookup::Checking { then_submit: true })
                && state.uname.trim().eq_ignore_ascii_case(&name);
            state.uname_lookup = lookup;
            if submit {
                app.queue.push(Action::OnboardSubmit);
                ctx.request_repaint();
            }
        }
    }
}

/// Debounced availability check via `username_exists` (works signed-out).
pub(crate) fn onboard_verify_uname(app: &mut ShellApp, ctx: &Context) {
    onboard_poll_uname(app, ctx);
    let (q, api_url) = match &app.onboard {
        Some(s) if s.step == OnboardStep::Create => {
            let q = s.uname.trim().to_owned();
            if !q.is_empty()
                && q.eq_ignore_ascii_case(&s.uname_lookup_for)
                && s.uname_lookup != OnboardLookup::Idle
            {
                return;
            }
            (q, s.api_url.clone())
        }
        _ => return,
    };
    if q.is_empty() {
        if let Some(s) = &mut app.onboard {
            s.uname_lookup_seq = s.uname_lookup_seq.wrapping_add(1);
            s.uname_lookup = OnboardLookup::Idle;
            s.uname_lookup_for.clear();
        }
        return;
    }
    if !onboard_uname_format_ok(&q) {
        if let Some(s) = &mut app.onboard {
            s.uname_lookup_seq = s.uname_lookup_seq.wrapping_add(1);
            s.uname_lookup = OnboardLookup::Error("Invalid username".into());
            s.uname_lookup_for = q;
        }
        return;
    }
    if !uname_check_matches_core(&api_url) {
        if let Some(s) = &mut app.onboard {
            s.uname_lookup_seq = s.uname_lookup_seq.wrapping_add(1);
            s.uname_lookup = OnboardLookup::Available;
            s.uname_lookup_for = q;
        }
        return;
    }

    let core = match &app.session {
        Session::SignedOut { core } => core.clone(),
        Session::Ready(r) => r.workspace.core.clone(),
        _ => {
            if let Some(s) = &mut app.onboard {
                s.uname_lookup_seq = s.uname_lookup_seq.wrapping_add(1);
                s.uname_lookup = OnboardLookup::Error("Core not ready".into());
                s.uname_lookup_for = q;
            }
            return;
        }
    };

    let seq = if let Some(s) = &mut app.onboard {
        s.uname_lookup_seq = s.uname_lookup_seq.wrapping_add(1);
        s.uname_lookup = OnboardLookup::Checking { then_submit: false };
        s.uname_lookup_for = q.clone();
        s.uname_lookup_seq
    } else {
        return;
    };

    spawn_username_exists(ctx, core, q, move |ctx, q_net, result| {
        let lookup = match result {
            Ok(true) => OnboardLookup::Taken,
            Ok(false) => OnboardLookup::Available,
            Err(e) => OnboardLookup::Error(e),
        };
        record_uname_verify_done(ctx, seq, q_net, lookup);
    });
}

/// Flatten paste so both compact keys and phrases work.
///
/// 24 tokens (or a numbered / comma-separated 24-word phrase) → phrase
/// (spaces kept). Otherwise drop whitespace so a wrapped compact key still
/// base64-decodes.
pub(crate) fn onboard_import_secret(account_key: &str) -> String {
    let parsed = parse_pasted_words(account_key);
    if parsed.len() == 24
        && parsed
            .iter()
            .filter(|w| lb::model::account::is_valid_phrase_word(w))
            .count()
            >= 2
    {
        return parsed.join(" ");
    }
    let words: Vec<&str> = account_key.split_whitespace().collect();
    if words.len() == 24 { words.join(" ") } else { words.concat() }
}

/// Messages sent from the background onboard create / import worker.
pub(crate) enum OnboardWorkerMsg {
    /// Create succeeded + initial cache built → land on empty welcome view with backup checklist.
    CreateReady {
        core: Lb,
        files: FileCache,
        sub_info: Option<SubscriptionInfo>,
    },
    CreateFailed(String),
    /// `import_account` succeeded and returned `username` — transition to in-place syncing screen.
    ImportAuthenticated {
        username: String,
    },
    ImportStatus(String),
    /// Import sync + cache finished → open empty landing page for privacy (no nudge).
    ImportReady {
        core: Lb,
        files: FileCache,
        sub_info: Option<SubscriptionInfo>,
    },
    /// `import_account` itself failed (invalid key / wrong server).
    ImportFailed(String),
    /// Account was imported, but initial `core.sync()` failed — keep on syncing screen with Retry.
    ImportSyncFailed(String),
}

/// Restore saved theme accent & note font if any `Choose a look` hover preview is active.
pub(crate) fn clear_look_preview(app: &mut ShellApp, ctx: &Context) {
    let (restore_accent, restore_font) = match &mut app.onboard {
        Some(s) => (s.preview_accent.take().is_some(), s.preview_font.take().is_some()),
        None => (false, false),
    };
    if restore_accent {
        app.show_saved_theme(ctx);
    }
    if restore_font {
        workspace_rs::font::choose(
            app.settings.text_font.as_deref(),
            app.settings.code_font.as_deref(),
        );
    }
    if restore_accent || restore_font {
        ctx.request_repaint();
    }
}

/// Drain background create / import messages.
pub(crate) fn poll_onboard_worker(app: &mut ShellApp, ctx: &Context) {
    let mut msgs = Vec::new();
    let mut disconnected = false;
    if let Some(rx) = &app.onboard_worker_rx {
        loop {
            match rx.try_recv() {
                Ok(m) => msgs.push(m),
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    disconnected = true;
                    break;
                }
            }
        }
    } else {
        return;
    }

    let mut saw_terminal = false;
    for msg in msgs {
        match msg {
            OnboardWorkerMsg::CreateReady { core, files, sub_info } => {
                saw_terminal = true;
                clear_look_preview(app, ctx);
                app.onboard_worker_rx = None;
                app.onboard = None;
                app.onboard_look_label = None;
                app.lb_rx = None;
                app.recents_cache = Default::default();
                app.shared_cache = Default::default();
                let ready = session::Ready::new(core, files, ctx, sub_info);
                let mut nudges = Nudges::load(&ready);
                nudges.created();
                app.nudges = Some(nudges);
                app.session = Session::Ready(Box::new(ready));
            }
            OnboardWorkerMsg::CreateFailed(err) => {
                saw_terminal = true;
                clear_look_preview(app, ctx);
                app.onboard_worker_rx = None;
                let mut returned_to_create = false;
                if let Some(s) = &mut app.onboard {
                    s.busy = false;
                    // Errors from `Choose a look` (e.g. username taken or custom server rejection)
                    // need the Create username input so the user can fix it.
                    if s.step == OnboardStep::Look {
                        s.step = OnboardStep::Create;
                        s.uname_lookup = OnboardLookup::Idle;
                        s.uname_lookup_for.clear();
                        returned_to_create = true;
                    }
                    s.err = Some(err);
                }
                if returned_to_create {
                    ctx.data_mut(|d| {
                        d.insert_temp(egui::Id::unique("onboard_uname_need_focus"), true);
                    });
                }
            }
            OnboardWorkerMsg::ImportAuthenticated { username } => {
                if let Some(s) = &mut app.onboard {
                    s.pending_cross_import = None;
                    s.step = OnboardStep::ImportSyncing;
                    s.sync_username = username;
                    s.sync_status = "Syncing your encrypted files…".into();
                    s.err = None;
                    s.busy = true;
                }
            }
            OnboardWorkerMsg::ImportStatus(status) => {
                if let Some(s) = &mut app.onboard {
                    s.sync_status = status;
                }
            }
            OnboardWorkerMsg::ImportReady { core, files, sub_info } => {
                saw_terminal = true;
                app.onboard_worker_rx = None;
                app.onboard = None;
                app.onboard_look_label = None;
                app.lb_rx = None;
                app.recents_cache = Default::default();
                app.shared_cache = Default::default();
                // Sign in lands on the empty landing page ("No file open") for privacy,
                // with no backup nudge (never calls `nudges.created`).
                let ready = session::Ready::new(core, files, ctx, sub_info);
                app.nudges = Some(Nudges::load(&ready));
                app.session = Session::Ready(Box::new(ready));
            }
            OnboardWorkerMsg::ImportFailed(err) => {
                saw_terminal = true;
                app.onboard_worker_rx = None;
                let mut switch_focus: Option<bool> = None;
                if let Some(s) = &mut app.onboard {
                    s.busy = false;
                    if let Some(cross) = s.pending_cross_import.take() {
                        match cross {
                            PendingCrossImport::Compact(compact) => {
                                s.import_compact = true;
                                s.last_auto_submit_hash =
                                    Some(super::onboard::secret_hash(compact.trim()));
                                s.compact_key = compact;
                                switch_focus = Some(true);
                            }
                            PendingCrossImport::Phrase { words, focus } => {
                                s.import_compact = false;
                                s.compact_key.clear();
                                s.import_words = words;
                                s.import_focus = focus;
                                request_import_slot_caret_end(ctx, focus, &s.import_words[focus]);
                                switch_focus = Some(false);
                            }
                        }
                    }
                    s.err = Some(err);
                }
                if let Some(compact) = switch_focus {
                    onboard_import_focus(ctx, compact);
                }
            }
            OnboardWorkerMsg::ImportSyncFailed(err) => {
                saw_terminal = true;
                app.onboard_worker_rx = None;
                if let Some(s) = &mut app.onboard {
                    s.busy = false;
                    s.step = OnboardStep::ImportSyncing;
                    s.err = Some(err);
                }
            }
        }
    }

    if disconnected {
        app.onboard_worker_rx = None;
        if !saw_terminal {
            if let Some(s) = &mut app.onboard {
                s.busy = false;
                s.pending_cross_import = None;
                if s.step == OnboardStep::ImportSyncing && s.err.is_none() {
                    s.err = Some("Sync was interrupted. Please try again.".into());
                }
            }
        }
    }
}

/// Handle `Action::OnboardSubmit` on Create or Import.
pub(crate) fn onboard_submit(app: &mut ShellApp, ctx: &Context) {
    if app.onboard_worker_rx.is_some() {
        return;
    }
    let step = match &app.onboard {
        Some(s) => s.step,
        None => return,
    };

    if step == OnboardStep::Create {
        onboard_verify_uname(app, ctx);
    }

    match step {
        OnboardStep::Create => submit_create(app, ctx),
        OnboardStep::Import => submit_import(app, ctx),
        OnboardStep::Look => onboard_finish_look(app, ctx),
        OnboardStep::Welcome | OnboardStep::ImportSyncing => {}
    }
}

fn submit_create(app: &mut ShellApp, ctx: &Context) {
    let (uname, lookup_ok, api) = match &app.onboard {
        Some(s) => {
            let u = s.uname.trim().to_owned();
            let lookup_ok = u.eq_ignore_ascii_case(&s.uname_lookup_for)
                && matches!(s.uname_lookup, OnboardLookup::Available);
            (u, lookup_ok, resolved_api_url(&s.api_url))
        }
        None => return,
    };

    if uname.is_empty() {
        if let Some(s) = &mut app.onboard {
            s.busy = false;
            s.err = Some("Username required".into());
        }
        return;
    }
    if !lookup_ok {
        if let Some(s) = &mut app.onboard {
            if let OnboardLookup::Checking { then_submit } = &mut s.uname_lookup {
                *then_submit = true;
            }
            s.err = None;
        }
        return;
    }

    // Create-only experiment: show `Choose a look` before creating the account
    // so Back cleanly returns to editing the username and Continue has zero spinner.
    // Lowercase to match `create_account_with_key`'s canonical stored username.
    let cohort_uname = uname.to_lowercase();
    if cohort_in::<OnboardLook>(OnboardLook::EXPERIMENT, &cohort_uname) == OnboardLook::Show {
        if let Some(s) = &mut app.onboard {
            s.step = OnboardStep::Look;
            s.err = None;
            s.busy = false;
        }
        return;
    }

    spawn_create_account(app, ctx, uname, api);
}

/// Complete `Choose a look` and create the account.
pub(crate) fn onboard_finish_look(app: &mut ShellApp, ctx: &Context) {
    if app.onboard_worker_rx.is_some() {
        return;
    }
    clear_look_preview(app, ctx);
    let (uname, api) = match &app.onboard {
        Some(s) => (s.uname.trim().to_owned(), resolved_api_url(&s.api_url)),
        None => return,
    };
    if uname.is_empty() {
        if let Some(s) = &mut app.onboard {
            s.step = OnboardStep::Create;
            s.err = Some("Username required".into());
        }
        return;
    }
    spawn_create_account(app, ctx, uname, api);
}

fn spawn_create_account(app: &mut ShellApp, ctx: &Context, uname: String, api: String) {
    let Some(core) = app.session.signed_out_core().cloned() else {
        return;
    };
    if let Some(s) = &mut app.onboard {
        s.busy = true;
        s.err = None;
    }
    let (tx, rx) = mpsc::channel();
    app.onboard_worker_rx = Some(rx);
    let ctx = ctx.clone();

    thread::spawn(move || {
        if let Err(e) = core.create_account(&uname, &api, false) {
            let _ = tx.send(OnboardWorkerMsg::CreateFailed(e.to_string()));
            ctx.request_repaint();
            return;
        }
        maybe_slow_sync();
        if let Err(e) = core.sync() {
            // Account already exists locally; still open if cache builds, else surface error.
            if let Ok(files) = FileCache::new(&core) {
                let sub_info = core.get_subscription_info().ok().flatten();
                let _ = tx.send(OnboardWorkerMsg::CreateReady { core, files, sub_info });
            } else {
                let _ = tx.send(OnboardWorkerMsg::CreateFailed(format!("Sync failed: {e}")));
            }
            ctx.request_repaint();
            return;
        }
        match FileCache::new(&core) {
            Ok(files) => {
                let sub_info = core.get_subscription_info().ok().flatten();
                let _ = tx.send(OnboardWorkerMsg::CreateReady { core, files, sub_info });
            }
            Err(e) => {
                let _ =
                    tx.send(OnboardWorkerMsg::CreateFailed(format!("Couldn’t load files: {e}")));
            }
        }
        ctx.request_repaint();
    });
}

fn submit_import(app: &mut ShellApp, ctx: &Context) {
    let (secret, api) = match &app.onboard {
        Some(s) => {
            let secret = match &s.pending_cross_import {
                Some(PendingCrossImport::Compact(key)) => onboard_import_secret(key),
                Some(PendingCrossImport::Phrase { words, .. }) => words_joined(words),
                None if s.import_compact => onboard_import_secret(&s.compact_key),
                None if all_words_filled(&s.import_words) => words_joined(&s.import_words),
                None => String::new(),
            };
            (secret, resolved_api_url(&s.api_url))
        }
        None => return,
    };

    if secret.is_empty() {
        if let Some(s) = &mut app.onboard {
            s.busy = false;
            s.pending_cross_import = None;
            s.err = Some(if s.import_compact {
                "Account key required".into()
            } else {
                "Enter all 24 words to sign in".into()
            });
        }
        return;
    }

    let Some(core) = app.session.signed_out_core().cloned() else {
        return;
    };
    if let Some(s) = &mut app.onboard {
        if s.pending_cross_import.is_none() && s.import_compact {
            s.last_auto_submit_hash = Some(super::onboard::secret_hash(s.compact_key.trim()));
        }
        s.busy = true;
        s.err = None;
    }

    let (tx, rx) = mpsc::channel();
    app.onboard_worker_rx = Some(rx);
    let ctx = ctx.clone();

    thread::spawn(move || {
        let account = match core.import_account(&secret, Some(&api)) {
            Ok(acc) => acc,
            Err(e) => {
                let _ = tx.send(OnboardWorkerMsg::ImportFailed(e.to_string()));
                ctx.request_repaint();
                return;
            }
        };

        // Authenticated — immediately transition UI to "Welcome back, {username}".
        let _ = tx.send(OnboardWorkerMsg::ImportAuthenticated { username: account.username });
        ctx.request_repaint();

        run_import_sync(core, &tx, &ctx);
    });
}

/// Retry initial sync when `import_account` already succeeded on `core`.
pub(crate) fn onboard_retry_import_sync(app: &mut ShellApp, ctx: &Context) {
    if app.onboard_worker_rx.is_some() {
        return;
    }
    let Some(core) = app.session.signed_out_core().cloned() else {
        return;
    };
    if let Some(s) = &mut app.onboard {
        s.busy = true;
        s.err = None;
        s.sync_status = "Syncing your encrypted files…".into();
    }
    let (tx, rx) = mpsc::channel();
    app.onboard_worker_rx = Some(rx);
    let ctx = ctx.clone();

    thread::spawn(move || {
        run_import_sync(core, &tx, &ctx);
    });
}

fn run_import_sync(core: Lb, tx: &mpsc::Sender<OnboardWorkerMsg>, ctx: &Context) {
    maybe_slow_sync();
    if let Err(e) = core.sync() {
        let _ = tx.send(OnboardWorkerMsg::ImportSyncFailed(format!("Sync failed: {e}")));
        ctx.request_repaint();
        return;
    }
    let _ = tx.send(OnboardWorkerMsg::ImportStatus("Opening workspace…".into()));
    ctx.request_repaint();
    match FileCache::new(&core) {
        Ok(files) => {
            let sub_info = core.get_subscription_info().ok().flatten();
            let _ = tx.send(OnboardWorkerMsg::ImportReady { core, files, sub_info });
        }
        Err(e) => {
            let _ =
                tx.send(OnboardWorkerMsg::ImportSyncFailed(format!("Couldn’t load files: {e}")));
        }
    }
    ctx.request_repaint();
}

fn maybe_slow_sync() {
    #[cfg(debug_assertions)]
    if let Ok(ms) = std::env::var("LOCKBOOK_SLOW_SYNC_MS") {
        if let Ok(ms) = ms.parse::<u64>() {
            if ms > 0 {
                std::thread::sleep(std::time::Duration::from_millis(ms));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use lb::experiments::{OnboardLook, cohort_in};

    use super::super::ShellApp;
    use super::super::action::{Action, OnboardLookup, OnboardStep};
    use super::super::apply::apply;
    use super::super::onboard::OnboardState;
    use super::{
        OnboardWorkerMsg, display_server_host, onboard_import_secret, onboard_poll_uname,
        poll_onboard_worker, record_uname_verify_done, resolved_api_url,
    };
    use crate::components::ThemeExt;

    #[test]
    fn empty_url_is_hosted_default() {
        assert_eq!(resolved_api_url(""), lb::DEFAULT_API_LOCATION);
        assert_eq!(resolved_api_url("  "), lb::DEFAULT_API_LOCATION);
        assert_eq!(resolved_api_url("http://localhost:8000"), "http://localhost:8000");
        assert_eq!(display_server_host(""), "app.lockbook.net");
        assert_eq!(display_server_host("http://localhost:8000"), "localhost:8000");
    }

    #[test]
    fn return_during_check_waits_then_submits() {
        let ctx = egui::Context::default();
        let mut app = ShellApp::default();
        let mut state = OnboardState::new();
        state.step = OnboardStep::Create;
        state.uname = "alice".into();
        state.uname_lookup = OnboardLookup::Checking { then_submit: false };
        state.uname_lookup_for = "alice".into();
        state.uname_lookup_seq = 1;
        app.onboard = Some(state);

        apply(&mut app, &ctx, Action::OnboardSubmit);
        record_uname_verify_done(&ctx, 1, "alice".to_owned(), OnboardLookup::Available);
        onboard_poll_uname(&mut app, &ctx);

        let state = app.onboard.as_ref().unwrap();
        assert_eq!((&state.uname_lookup, &state.err), (&OnboardLookup::Available, &None));
        assert!(matches!(app.queue[..], [Action::OnboardSubmit]));
    }

    #[test]
    fn check_settles_while_field_differs() {
        let ctx = egui::Context::default();
        let mut app = ShellApp::default();
        let mut state = OnboardState::new();
        state.step = OnboardStep::Create;
        state.uname = "alicex".into();
        state.uname_lookup = OnboardLookup::Checking { then_submit: true };
        state.uname_lookup_for = "alice".into();
        state.uname_lookup_seq = 1;
        app.onboard = Some(state);

        record_uname_verify_done(&ctx, 1, "alice".to_owned(), OnboardLookup::Available);
        onboard_poll_uname(&mut app, &ctx);

        let state = app.onboard.as_ref().unwrap();
        assert_eq!(state.uname_lookup, OnboardLookup::Available);
        assert!(app.queue.is_empty());
    }

    #[test]
    fn stale_uname_check_does_not_overwrite_newer_result() {
        let ctx = egui::Context::default();
        let mut app = ShellApp::default();
        let mut state = OnboardState::new();
        state.step = OnboardStep::Create;
        state.uname = "alice".into();
        state.uname_lookup = OnboardLookup::Checking { then_submit: true };
        state.uname_lookup_for = "alice".into();
        state.uname_lookup_seq = 2;
        app.onboard = Some(state);

        // Newer check (seq 2) finishes first, then an older slow check (seq 1) completes before poll.
        record_uname_verify_done(&ctx, 2, "alice".to_owned(), OnboardLookup::Available);
        record_uname_verify_done(&ctx, 1, "alic".to_owned(), OnboardLookup::Taken);
        onboard_poll_uname(&mut app, &ctx);

        let state = app.onboard.as_ref().unwrap();
        assert_eq!(state.uname_lookup, OnboardLookup::Available);
        assert!(matches!(app.queue[..], [Action::OnboardSubmit]));
    }

    #[test]
    fn create_failed_from_look_returns_to_create() {
        let ctx = egui::Context::default();
        crate::components::install(&ctx);
        let mut app = ShellApp::default();
        app.show_saved_theme(&ctx);
        let saved_accent = ctx.get_lb_theme().accent_hue;
        let preview = if saved_accent == workspace_rs::theme::palette_v2::Palette::Magenta {
            workspace_rs::theme::palette_v2::Palette::Green
        } else {
            workspace_rs::theme::palette_v2::Palette::Magenta
        };

        let mut state = OnboardState::new();
        state.step = OnboardStep::Look;
        state.uname = "alice".into();
        state.uname_lookup = OnboardLookup::Available;
        state.uname_lookup_for = "alice".into();
        state.busy = true;
        app.onboard = Some(state);
        apply(&mut app, &ctx, Action::PreviewThemeAccent(Some(preview)));
        assert_eq!(ctx.get_lb_theme().accent_hue, preview);

        let (tx, rx) = mpsc::channel();
        tx.send(OnboardWorkerMsg::CreateFailed("Username taken".into()))
            .unwrap();
        app.onboard_worker_rx = Some(rx);

        poll_onboard_worker(&mut app, &ctx);

        let state = app.onboard.as_ref().unwrap();
        assert_eq!(state.step, OnboardStep::Create);
        assert!(!state.busy);
        assert_eq!(state.uname_lookup, OnboardLookup::Idle);
        assert_eq!(state.err.as_deref(), Some("Username taken"));
        assert_eq!(state.preview_accent, None);
        assert_eq!(ctx.get_lb_theme().accent_hue, saved_accent);
    }

    #[test]
    fn onboard_look_cohort_is_case_insensitive() {
        // Pick a username whose lowercase hashes to Show while its UPPERCASE hashes to Skip,
        // so this test fails if `submit_create` forgets `.to_lowercase()`.
        let lower = (0..200)
            .map(|i| format!("user{i}"))
            .find(|u| {
                cohort_in::<OnboardLook>(OnboardLook::EXPERIMENT, u) == OnboardLook::Show
                    && cohort_in::<OnboardLook>(OnboardLook::EXPERIMENT, &u.to_uppercase())
                        == OnboardLook::Skip
            })
            .expect("expected a case-divergent Show/Skip username");
        let upper = lower.to_uppercase();

        let ctx = egui::Context::default();
        let mut app = ShellApp::default();
        let mut state = OnboardState::new();
        state.step = OnboardStep::Create;
        state.uname = upper.clone();
        state.uname_lookup = OnboardLookup::Available;
        state.uname_lookup_for = upper;
        app.onboard = Some(state);

        apply(&mut app, &ctx, Action::OnboardSubmit);
        assert_eq!(app.onboard.as_ref().unwrap().step, OnboardStep::Look);
    }

    #[test]
    fn import_secret_phrase_keeps_spaces_compact_strips() {
        let phrase = (0..24)
            .map(|i| format!("w{i}"))
            .collect::<Vec<_>>()
            .join(" ");
        assert_eq!(onboard_import_secret(&phrase), phrase);
        let numbered = (1..=24)
            .map(|i| format!("{i}. Abandon,"))
            .collect::<Vec<_>>()
            .join("\n");
        let expected_bip39 = vec!["abandon"; 24].join(" ");
        assert_eq!(onboard_import_secret(&numbered), expected_bip39);
        // A numbered 24-word phrase with a typo still normalizes as a phrase
        // so lb-rs returns the phrase error rather than compact-key corruption.
        let mut numbered_typo: Vec<String> = (1..=24).map(|i| format!("{i}. abandon")).collect();
        numbered_typo[3] = "4. notaword".into();
        let expected_typo = (0..24)
            .map(|i| if i == 3 { "notaword" } else { "abandon" })
            .collect::<Vec<_>>()
            .join(" ");
        assert_eq!(onboard_import_secret(&numbered_typo.join("\n")), expected_typo);
        assert_eq!(onboard_import_secret("abcd\n efgh"), "abcdefgh");
        assert_eq!(onboard_import_secret("  abcd  "), "abcd");
    }

    #[test]
    fn failed_cross_format_paste_switches_to_target_view_with_error() {
        let ctx = egui::Context::default();

        // Compact key pasted on 24-word grid → failure switches to compact view with key + error.
        let mut app = ShellApp::default();
        let mut st = OnboardState::new();
        st.step = OnboardStep::Import;
        st.import_compact = false;
        st.busy = true;
        st.pending_cross_import = Some(super::PendingCrossImport::Compact(
            "nvo7SItwXYmoxxzmOCUrNJiw85V8CwJ+SXb8cOHPIlo=".into(),
        ));
        app.onboard = Some(st);

        let (tx, rx) = mpsc::channel();
        tx.send(OnboardWorkerMsg::ImportFailed("Could not reach server".into()))
            .unwrap();
        app.onboard_worker_rx = Some(rx);
        poll_onboard_worker(&mut app, &ctx);

        let s = app.onboard.as_ref().unwrap();
        assert!(s.import_compact);
        assert!(!s.busy);
        assert_eq!(s.compact_key, "nvo7SItwXYmoxxzmOCUrNJiw85V8CwJ+SXb8cOHPIlo=");
        assert!(s.pending_cross_import.is_none());
        assert!(s.last_auto_submit_hash.is_some());
        assert_eq!(s.err.as_deref(), Some("Could not reach server"));

        // 24-word phrase pasted on compact screen → failure switches to 24-word grid with words + error.
        let mut app = ShellApp::default();
        let mut st = OnboardState::new();
        st.step = OnboardStep::Import;
        st.import_compact = true;
        st.busy = true;
        let words: [String; 24] = std::array::from_fn(|_| "abandon".to_owned());
        st.pending_cross_import = Some(super::PendingCrossImport::Phrase { words, focus: 23 });
        app.onboard = Some(st);

        let (tx, rx) = mpsc::channel();
        tx.send(OnboardWorkerMsg::ImportFailed("Your private key phrase is wrong".into()))
            .unwrap();
        app.onboard_worker_rx = Some(rx);
        poll_onboard_worker(&mut app, &ctx);

        let s = app.onboard.as_ref().unwrap();
        assert!(!s.import_compact);
        assert!(!s.busy);
        assert_eq!(s.import_words[0], "abandon");
        assert_eq!(s.import_focus, 23);
        assert!(s.pending_cross_import.is_none());
        assert_eq!(s.err.as_deref(), Some("Your private key phrase is wrong"));
    }
}
