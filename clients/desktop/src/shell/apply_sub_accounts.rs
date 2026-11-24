use std::thread;

use egui::Context;
use lb::blocking::Lb;

use super::ShellApp;
use super::apply_account::ensure_settings_account;
use super::prefs::AccountPanel;
use super::sub_accounts::{Done, SubAccountKeys, is_sub_account};

pub(crate) fn new_label_focus_key() -> egui::Id {
    egui::Id::unique("shell_new_sub_account_need_focus")
}

fn spawn(app: &ShellApp, ctx: &Context, work: impl FnOnce(Lb) -> Done + Send + 'static) -> bool {
    let Some(r) = app.session.ready() else {
        return false;
    };
    if is_sub_account(&r.workspace.account.username) {
        return false;
    }
    let core = r.workspace.core.clone();
    let tx = app.sub_accounts.tx.clone();
    let ctx = ctx.clone();
    thread::spawn(move || {
        let _ = tx.send(work(core));
        ctx.request_repaint();
    });
    true
}

pub(crate) fn load_sub_accounts(app: &ShellApp, ctx: &Context) {
    spawn(app, ctx, |core| Done::Listed(core.list_sub_accounts().map_err(|e| e.to_string())));
}

pub(crate) fn poll_sub_accounts(app: &mut ShellApp, ctx: &Context) {
    let mut reload = false;
    while let Ok(done) = app.sub_accounts.rx.try_recv() {
        match done {
            Done::Listed(Ok(list)) => {
                app.sub_accounts.list = list;
                app.sub_accounts.loaded = true;
                app.sub_accounts.load_error = None;
            }
            Done::Listed(Err(e)) => app.sub_accounts.load_error = Some(e),
            Done::Created(result) => {
                app.sub_accounts.working = false;
                match result {
                    Ok(_) => {
                        reload = true;
                        if matches!(app.account_panel, AccountPanel::NewSubAccount { .. }) {
                            app.account_panel = AccountPanel::SubAccounts;
                        }
                    }
                    Err(e) => app.sub_accounts.error = Some(e),
                }
            }
            Done::Deleted { username, result } => {
                app.sub_accounts.working = false;
                let confirming = matches!(
                    &app.account_panel,
                    AccountPanel::DeleteSubAccount { username: open } if *open == username
                );
                match result {
                    Ok(()) => {
                        reload = true;
                        app.sub_accounts.list.retain(|s| s.username != username);
                        if confirming {
                            app.account_panel = AccountPanel::SubAccounts;
                        }
                    }
                    Err(e) => {
                        app.sub_accounts.error = Some(e);
                        if confirming {
                            app.account_panel = AccountPanel::SubAccount { username };
                        }
                    }
                }
            }
            Done::Keys { username, result } => {
                app.sub_accounts.working = false;
                let open = matches!(
                    &app.account_panel,
                    AccountPanel::SubAccount { username: open } if *open == username
                );
                match result {
                    Ok((key, phrase)) if open => {
                        app.sub_accounts.keys =
                            Some(SubAccountKeys { username: username.clone(), key, phrase });
                        app.account_panel = AccountPanel::SubAccountKeys { username };
                    }
                    Ok(_) => {}
                    Err(e) => app.sub_accounts.error = Some(e),
                }
            }
        }
    }
    if reload {
        load_sub_accounts(app, ctx);
    }
}

pub(crate) fn open_sub_accounts(app: &mut ShellApp, ctx: &Context) {
    ensure_settings_account(app);
    app.sub_accounts.error = None;
    app.sub_accounts.keys = None;
    app.account_panel = AccountPanel::SubAccounts;
    load_sub_accounts(app, ctx);
}

pub(crate) fn open_new_sub_account(app: &mut ShellApp, ctx: &Context) {
    ensure_settings_account(app);
    app.sub_accounts.error = None;
    app.account_panel = AccountPanel::NewSubAccount { label: String::new() };
    ctx.data_mut(|d| d.insert_temp(new_label_focus_key(), true));
}

pub(crate) fn confirm_new_sub_account(app: &mut ShellApp, ctx: &Context) {
    let AccountPanel::NewSubAccount { label } = &app.account_panel else {
        return;
    };
    let label = label.trim().to_owned();
    if label.is_empty() || app.sub_accounts.working {
        return;
    }
    app.sub_accounts.error = None;
    app.sub_accounts.working = spawn(app, ctx, move |core| {
        Done::Created(core.create_sub_account(&label).map_err(|e| e.to_string()))
    });
}

pub(crate) fn open_sub_account(app: &mut ShellApp, username: String) {
    ensure_settings_account(app);
    app.sub_accounts.error = None;
    app.sub_accounts.keys = None;
    app.account_panel = AccountPanel::SubAccount { username };
}

pub(crate) fn reveal_sub_account_keys(app: &mut ShellApp, ctx: &Context) {
    let AccountPanel::SubAccount { username } = &app.account_panel else {
        return;
    };
    if app.sub_accounts.working {
        return;
    }
    let username = username.clone();
    app.sub_accounts.error = None;
    app.sub_accounts.working = spawn(app, ctx, move |core| {
        let result = core
            .export_sub_account_private_key(&username)
            .and_then(|key| Ok((key, core.export_sub_account_phrase(&username)?)))
            .map_err(|e| e.to_string());
        Done::Keys { username, result }
    });
}

pub(crate) fn open_delete_sub_account(app: &mut ShellApp) {
    let AccountPanel::SubAccount { username } = &app.account_panel else {
        return;
    };
    let username = username.clone();
    app.sub_accounts.error = None;
    app.account_panel = AccountPanel::DeleteSubAccount { username };
}

pub(crate) fn confirm_delete_sub_account(app: &mut ShellApp, ctx: &Context) {
    let AccountPanel::DeleteSubAccount { username } = &app.account_panel else {
        return;
    };
    if app.sub_accounts.working {
        return;
    }
    let username = username.clone();
    app.sub_accounts.error = None;
    app.sub_accounts.working = spawn(app, ctx, move |core| {
        let result = core
            .delete_sub_account(&username)
            .map_err(|e| e.to_string());
        Done::Deleted { username, result }
    });
}

pub(crate) fn sub_accounts_back(app: &mut ShellApp) {
    app.sub_accounts.error = None;
    app.sub_accounts.keys = None;
    app.account_panel = match std::mem::take(&mut app.account_panel) {
        AccountPanel::NewSubAccount { .. } | AccountPanel::SubAccount { .. } => {
            AccountPanel::SubAccounts
        }
        AccountPanel::SubAccountKeys { username } | AccountPanel::DeleteSubAccount { username } => {
            AccountPanel::SubAccount { username }
        }
        _ => AccountPanel::Closed,
    };
}

pub(crate) fn copy_sub_account_key(app: &ShellApp, ctx: &Context, phrase: bool) {
    if let Some(keys) = &app.sub_accounts.keys {
        ctx.copy_text(if phrase { keys.phrase.clone() } else { keys.key.clone() });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(panel: AccountPanel) -> ShellApp {
        ShellApp { account_panel: panel, ..Default::default() }
    }

    fn keys(username: &str) -> SubAccountKeys {
        SubAccountKeys { username: username.into(), key: "key".into(), phrase: "phrase".into() }
    }

    fn deliver(app: &mut ShellApp, done: Done) {
        app.sub_accounts.working = true;
        app.sub_accounts.tx.send(done).unwrap();
        poll_sub_accounts(app, &Context::default());
        assert!(!app.sub_accounts.working);
    }

    #[test]
    fn back_steps_one_level() {
        let username = || "jane+site".to_owned();
        let steps = [
            (AccountPanel::NewSubAccount { label: "site".into() }, AccountPanel::SubAccounts),
            (AccountPanel::SubAccount { username: username() }, AccountPanel::SubAccounts),
            (
                AccountPanel::SubAccountKeys { username: username() },
                AccountPanel::SubAccount { username: username() },
            ),
            (
                AccountPanel::DeleteSubAccount { username: username() },
                AccountPanel::SubAccount { username: username() },
            ),
            (AccountPanel::SubAccounts, AccountPanel::Closed),
        ];
        for (from, to) in steps {
            let mut app = app(from);
            app.sub_accounts.keys = Some(keys("jane+site"));
            app.sub_accounts.error = Some("failed".into());
            sub_accounts_back(&mut app);
            assert_eq!(app.account_panel, to);
            assert!(app.sub_accounts.keys.is_none());
            assert!(app.sub_accounts.error.is_none());
        }
    }

    #[test]
    fn created_returns_to_the_list() {
        let mut app = app(AccountPanel::NewSubAccount { label: "site".into() });
        deliver(&mut app, Done::Created(Ok("jane+site".into())));
        assert_eq!(app.account_panel, AccountPanel::SubAccounts);
        assert!(app.sub_accounts.error.is_none());
    }

    #[test]
    fn failed_create_stays_on_the_prompt() {
        let prompt = AccountPanel::NewSubAccount { label: "site".into() };
        let mut app = app(prompt.clone());
        deliver(&mut app, Done::Created(Err("That username is not available".into())));
        assert_eq!(app.account_panel, prompt);
        assert_eq!(app.sub_accounts.error.as_deref(), Some("That username is not available"));
    }

    #[test]
    fn keys_show_only_for_the_open_sub_account() {
        let exported = || Done::Keys {
            username: "jane+site".into(),
            result: Ok(("key".into(), "phrase".into())),
        };

        let mut open = app(AccountPanel::SubAccount { username: "jane+site".into() });
        deliver(&mut open, exported());
        assert_eq!(
            open.account_panel,
            AccountPanel::SubAccountKeys { username: "jane+site".into() }
        );
        assert_eq!(open.sub_accounts.keys.as_ref().map(|k| k.phrase.as_str()), Some("phrase"));

        let mut left = app(AccountPanel::SubAccounts);
        deliver(&mut left, exported());
        assert_eq!(left.account_panel, AccountPanel::SubAccounts);
        assert!(left.sub_accounts.keys.is_none());
    }

    #[test]
    fn delete_leaves_the_confirmation() {
        let confirm = || AccountPanel::DeleteSubAccount { username: "jane+site".into() };

        let mut deleted = app(confirm());
        deliver(&mut deleted, Done::Deleted { username: "jane+site".into(), result: Ok(()) });
        assert_eq!(deleted.account_panel, AccountPanel::SubAccounts);

        let mut failed = app(confirm());
        let result = Err("Could not reach server".into());
        deliver(&mut failed, Done::Deleted { username: "jane+site".into(), result });
        assert_eq!(failed.account_panel, AccountPanel::SubAccount { username: "jane+site".into() });
        assert_eq!(failed.sub_accounts.error.as_deref(), Some("Could not reach server"));
    }

    #[test]
    fn nothing_starts_without_an_account() {
        let mut app = app(AccountPanel::NewSubAccount { label: "site".into() });
        confirm_new_sub_account(&mut app, &Context::default());
        assert!(!app.sub_accounts.working);
    }
}
