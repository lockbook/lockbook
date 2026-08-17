//! Share sheet: stage, verify, invite.

use std::thread;

use egui::Context;

use super::ShellApp;
use super::action::{Modal, ShareLookup, ShareStaged};
use super::ops::rebuild_cache;
use lb::model::file::ShareMode;

fn share_split_tokens(q: &str) -> (Vec<String>, String) {
    if !q.contains(',') {
        return (Vec::new(), q.to_owned());
    }
    let ends_with_sep = q.trim_end().ends_with(',');
    let parts: Vec<&str> = q.split(',').collect();
    let mut complete = Vec::new();
    let mut remainder = String::new();
    if ends_with_sep {
        for p in parts {
            let t = p.trim();
            if !t.is_empty() {
                complete.push(t.to_owned());
            }
        }
    } else {
        let last = parts.len().saturating_sub(1);
        for (i, p) in parts.iter().enumerate() {
            if i == last {
                remainder = (*p).trim_start().to_owned();
            } else {
                let t = p.trim();
                if !t.is_empty() {
                    complete.push(t.to_owned());
                }
            }
        }
    }
    (complete, remainder)
}

/// Local roster only (in-memory cache — never `block_on` / network).
fn share_local_known(app: &ShellApp, q: &str) -> bool {
    let q = q.trim();
    if q.is_empty() {
        return false;
    }
    app.session
        .ready()
        .map(|r| r.known_usernames.iter().any(|u| u.eq_ignore_ascii_case(q)))
        .unwrap_or(false)
}

/// Shortest known username with `query` as a case-insensitive prefix.
///
/// Preferring the **shortest** match means typing more characters that still
/// match the current suggestion cannot jump to a longer sibling (e.g. `alice`
/// stays preferred over `alicesmith` as you type `a…e`). Falls out of the rule;
/// no post-hoc sticky.
pub(crate) fn share_shortest_prefix_match(app: &ShellApp, query: &str) -> Option<String> {
    let names = &app.session.ready()?.known_usernames;
    shortest_prefix_match(query, names.iter().map(|s| s.as_str()))
}

// ── Network verify (never on the UI thread) ─────────────────────────────────

#[derive(Clone, Debug)]
pub(crate) struct ShareNetDone {
    /// Query / staged username this result is for.
    name: String,
    lookup: ShareLookup,
    /// `true` → update staged row; `false` → field lookup.
    for_stage: bool,
}

pub(crate) fn share_verify_done_key() -> egui::Id {
    egui::Id::new("shell_share_verify_done")
}

pub(crate) fn share_verify_inflight_key() -> egui::Id {
    egui::Id::new("shell_share_verify_inflight")
}

/// Apply finished network checks (call from paint / apply each frame).
pub(crate) fn share_poll_network(app: &mut ShellApp, ctx: &Context) {
    let done: Vec<ShareNetDone> = ctx
        .data_mut(|d| d.remove_temp::<Vec<ShareNetDone>>(share_verify_done_key()))
        .unwrap_or_default();
    if done.is_empty() {
        return;
    }
    let mut remember: Vec<String> = Vec::new();
    if let Some(Modal::Share { query, lookup, lookup_for, staged, .. }) = &mut app.modal {
        for item in done {
            if item.for_stage {
                if let Some(row) = staged
                    .iter_mut()
                    .find(|s| s.username.eq_ignore_ascii_case(&item.name))
                {
                    row.lookup = item.lookup.clone();
                }
            } else if query.trim() == item.name {
                *lookup = item.lookup.clone();
                *lookup_for = item.name.clone();
            }
            if matches!(item.lookup, ShareLookup::Found) {
                remember.push(item.name);
            }
        }
    }
    if let Some(r) = app.session.ready_mut() {
        for name in remember {
            r.remember_username(&name);
        }
    }
}

pub(crate) fn spawn_username_exists(
    ctx: &Context, core: lb::blocking::Lb, name: String,
    finish: impl FnOnce(&Context, String, Result<bool, String>) + Send + 'static,
) {
    let ctx = ctx.clone();
    thread::spawn(move || {
        let result = core.username_exists(&name).map_err(|e| e.to_string());
        finish(&ctx, name, result);
        ctx.request_repaint();
    });
}

fn share_spawn_network(ctx: &Context, core: lb::blocking::Lb, name: String, for_stage: bool) {
    spawn_username_exists(ctx, core, name, move |ctx, name, result| {
        let lookup = match result {
            Ok(true) => ShareLookup::Found,
            Ok(false) => ShareLookup::NotFound,
            Err(e) => ShareLookup::Error(e),
        };
        let done_name = name.clone();
        ctx.data_mut(|d| {
            d.get_temp_mut_or_default::<Vec<ShareNetDone>>(share_verify_done_key())
                .push(ShareNetDone { name, lookup, for_stage });
            if !for_stage {
                let clear = d.get_temp::<String>(share_verify_inflight_key()).as_deref()
                    == Some(done_name.as_str());
                if clear {
                    d.remove::<String>(share_verify_inflight_key());
                }
            }
        });
    });
}

/// Pure ranking for [`share_shortest_prefix_match`].
fn shortest_prefix_match<'a>(query: &str, names: impl Iterator<Item = &'a str>) -> Option<String> {
    let q = query.trim();
    if q.is_empty() {
        return None;
    }
    let q_low = q.to_ascii_lowercase();
    let mut best: Option<&'a str> = None;
    for name in names {
        let n_low = name.to_ascii_lowercase();
        if !n_low.starts_with(&q_low) {
            continue;
        }
        match best {
            None => best = Some(name),
            Some(b) => {
                let shorter = name.len() < b.len();
                let same_len_earlier = name.len() == b.len() && n_low < b.to_ascii_lowercase();
                if shorter || same_len_earlier {
                    best = Some(name);
                }
            }
        }
    }
    best.map(|s| s.to_owned())
}

/// Stage a username (dedupe by case-insensitive name). Local hit → Found;
/// else Checking + background network (never block the UI thread).
fn share_stage_push(app: &mut ShellApp, ctx: &Context, name: String) {
    let name = name.trim().to_owned();
    if name.is_empty() {
        return;
    }
    let already = matches!(
        &app.modal,
        Some(Modal::Share { staged, .. })
            if staged.iter().any(|s| s.username.eq_ignore_ascii_case(&name))
    );
    if already {
        return;
    }
    let local = share_local_known(app, &name);
    let lookup = if local { ShareLookup::Found } else { ShareLookup::Checking };
    let core = app.session.ready().map(|r| r.workspace.core.clone());
    if let Some(Modal::Share { staged, .. }) = &mut app.modal {
        staged.push(ShareStaged { username: name.clone(), lookup });
    }
    if !local {
        if let Some(core) = core {
            share_spawn_network(ctx, core, name, true);
        } else if let Some(Modal::Share { staged, .. }) = &mut app.modal {
            if let Some(row) = staged.last_mut() {
                row.lookup = ShareLookup::Error("Not signed in".into());
            }
        }
    }
}

pub(crate) fn share_query(app: &mut ShellApp, ctx: &Context) {
    let q = match &app.modal {
        Some(Modal::Share { query, .. }) => query.clone(),
        _ => return,
    };
    let (complete, remainder) = share_split_tokens(&q);
    for name in complete {
        share_stage_push(app, ctx, name);
    }
    let next = remainder.trim().to_owned();
    if next.is_empty() {
        if let Some(Modal::Share { query, lookup, lookup_for, err, .. }) = &mut app.modal {
            *err = None;
            *query = remainder;
            *lookup = ShareLookup::Idle;
            lookup_for.clear();
        }
        return;
    }

    // Known locals resolve immediately (no debounce). Unknowns keep the prior
    // icon muted until paint's debounced network ShareVerify.
    let local_found = share_local_known(app, &next);
    if let Some(Modal::Share { query, lookup, lookup_for, err, .. }) = &mut app.modal {
        *err = None;
        *query = remainder;
        if local_found {
            *lookup = ShareLookup::Found;
            *lookup_for = next;
        }
        // else leave lookup as last terminal; lookup_for stays so paint sees dirty.
    }
}

/// ⏎ in the username field: move the current token into the stage (comma-like).
pub(crate) fn share_stage_field(app: &mut ShellApp, ctx: &Context) {
    let user = match &app.modal {
        Some(Modal::Share { query, .. }) => {
            let t = query.trim().to_owned();
            if t.is_empty() {
                return;
            }
            t
        }
        _ => return,
    };
    // Also split any commas the user left in the buffer.
    let (complete, rem) = share_split_tokens(&user);
    let had_complete = !complete.is_empty();
    for name in complete {
        share_stage_push(app, ctx, name);
    }
    if !rem.trim().is_empty() {
        share_stage_push(app, ctx, rem);
    } else if !had_complete {
        // No commas — stage the whole field.
        share_stage_push(app, ctx, user);
    }
    if let Some(Modal::Share { query, lookup, lookup_for, err, .. }) = &mut app.modal {
        query.clear();
        *lookup = ShareLookup::Idle;
        lookup_for.clear();
        *err = None;
    }
    ctx.data_mut(|d| d.insert_temp(egui::Id::new("shell_share_need_focus"), true));
}

/// Debounced field verify. Local hits are free; network runs on a worker thread.
pub(crate) fn share_verify(app: &mut ShellApp, ctx: &Context) {
    share_poll_network(app, ctx);
    let q = match &app.modal {
        Some(Modal::Share { query, lookup_for, lookup, .. }) => {
            let q = query.trim().to_owned();
            if !q.is_empty()
                && q == *lookup_for
                && matches!(
                    lookup,
                    ShareLookup::Found | ShareLookup::NotFound | ShareLookup::Error(_)
                )
            {
                return;
            }
            q
        }
        _ => return,
    };
    if q.is_empty() {
        if let Some(Modal::Share { lookup, lookup_for, .. }) = &mut app.modal {
            *lookup = ShareLookup::Idle;
            lookup_for.clear();
        }
        return;
    }

    if share_local_known(app, &q) {
        if let Some(Modal::Share { lookup, lookup_for, .. }) = &mut app.modal {
            *lookup = ShareLookup::Found;
            *lookup_for = q;
        }
        return;
    }

    let Some(core) = app.session.ready().map(|r| r.workspace.core.clone()) else {
        if let Some(Modal::Share { lookup, lookup_for, .. }) = &mut app.modal {
            *lookup = ShareLookup::Error("Not signed in".into());
            *lookup_for = q;
        }
        return;
    };

    // One in-flight network check at a time (latest query wins via poll match).
    let inflight = ctx.data(|d| d.get_temp::<String>(share_verify_inflight_key()));
    if inflight.as_deref() == Some(q.as_str()) {
        return;
    }
    ctx.data_mut(|d| d.insert_temp(share_verify_inflight_key(), q.clone()));
    share_spawn_network(ctx, core, q, false);
}

pub(crate) fn share_invite(app: &mut ShellApp, ctx: &Context) {
    // Flush a trailing field token into the stage first.
    let field_user = match &app.modal {
        Some(Modal::Share { query, .. }) => {
            let t = query.trim().to_owned();
            if t.is_empty() { None } else { Some(t) }
        }
        _ => return,
    };
    if let Some(u) = field_user {
        share_stage_push(app, ctx, u);
        if let Some(Modal::Share { query, lookup, lookup_for, .. }) = &mut app.modal {
            query.clear();
            *lookup = ShareLookup::Idle;
            lookup_for.clear();
        }
    }

    let (id, mode, targets, blocked) = {
        let Some(Modal::Share { id, mode, staged, .. }) = &app.modal else {
            return;
        };
        if staged.is_empty() {
            (*id, *mode, Vec::new(), Some("Enter a username"))
        } else if staged
            .iter()
            .any(|s| matches!(s.lookup, ShareLookup::Checking))
        {
            (*id, *mode, Vec::new(), Some("Still checking usernames…"))
        } else {
            let found: Vec<String> = staged
                .iter()
                .filter(|s| matches!(s.lookup, ShareLookup::Found))
                .map(|s| s.username.clone())
                .collect();
            if found.is_empty() {
                let msg = if staged
                    .iter()
                    .any(|s| matches!(s.lookup, ShareLookup::NotFound))
                {
                    "No Lockbook user with that name"
                } else {
                    "Could not verify those usernames"
                };
                (*id, *mode, Vec::new(), Some(msg))
            } else {
                (*id, *mode, found, None)
            }
        }
    };
    if let Some(msg) = blocked {
        if let Some(Modal::Share { err, .. }) = &mut app.modal {
            *err = Some(msg.into());
        }
        return;
    }

    let share_mode = if mode == 0 { ShareMode::Write } else { ShareMode::Read };

    let mut ok: Vec<String> = Vec::new();
    let mut fail: Vec<(String, String)> = Vec::new();
    {
        let Some(r) = app.session.ready_mut() else {
            return;
        };
        for user in &targets {
            match r.workspace.core.share_file(id, user, share_mode) {
                Ok(()) => ok.push(user.clone()),
                Err(e) => fail.push((user.clone(), e.to_string())),
            }
        }
        if !ok.is_empty() {
            rebuild_cache(r);
            r.refresh_status();
        }
    }

    if let Some(Modal::Share { staged, query, lookup, lookup_for, err, .. }) = &mut app.modal {
        // Drop people we successfully shared with; keep failures for retry/edit.
        staged.retain(|s| !ok.iter().any(|u| u.eq_ignore_ascii_case(&s.username)));
        // Mark hard failures as NotFound when the server said so.
        for (user, msg) in &fail {
            if let Some(s) = staged
                .iter_mut()
                .find(|s| s.username.eq_ignore_ascii_case(user))
            {
                let low = msg.to_lowercase();
                if low.contains("not found") || low.contains("nonexistent") {
                    s.lookup = ShareLookup::NotFound;
                } else {
                    s.lookup = ShareLookup::Error(msg.clone());
                }
            }
        }
        query.clear();
        *lookup = ShareLookup::Idle;
        lookup_for.clear();
        *err = if fail.is_empty() {
            None
        } else if ok.is_empty() {
            Some(fail[0].1.clone())
        } else {
            Some(format!(
                "Shared with {}; failed for {}",
                ok.len(),
                fail.iter()
                    .map(|(u, _)| u.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ))
        };
    }

    ctx.data_mut(|d| d.insert_temp(egui::Id::new("shell_share_need_focus"), true));
}

#[cfg(test)]
mod share_complete_tests {
    use super::shortest_prefix_match;

    #[test]
    fn shortest_prefix_stays_stable_as_you_type() {
        let names = ["alice", "alicesmith", "bob"];
        assert_eq!(shortest_prefix_match("al", names.into_iter()).as_deref(), Some("alice"));
        assert_eq!(shortest_prefix_match("alic", names.into_iter()).as_deref(), Some("alice"));
        assert_eq!(shortest_prefix_match("alice", names.into_iter()).as_deref(), Some("alice"));
        assert_eq!(
            shortest_prefix_match("alices", names.into_iter()).as_deref(),
            Some("alicesmith")
        );
    }
}
