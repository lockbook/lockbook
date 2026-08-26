//! Single admission door: `Action` → mutate session / workspace / modals.

pub(crate) use super::apply_account::poll_upgrade;
use super::apply_account::*;
pub(crate) use super::apply_onboard::onboard_poll_uname;
use super::apply_onboard::*;
use super::apply_share::*;
pub(crate) use super::apply_share::{share_poll_network, share_shortest_prefix_match};

use std::path::PathBuf;
use std::thread;

use egui::Context;
use lb::Uuid;
use lb::model::file_metadata::FileType;
use rfd::FileDialog;
use workspace_rs::file_cache::FilesExt;

use super::ShellApp;
use super::action::{
    Action as A, CreateKind, CreateLoc, Modal, OnboardLookup, OnboardMode, SettingsCat,
    ShareLookup, UpgradeStage,
};
use super::ops::{is_pinned, refresh_pinned};
use super::prefs::AccountPanel;
use super::session::Ready;
use crate::components::{set_mode_preference, set_theme_family};
use tracing::instrument;

#[instrument(level = "trace", skip_all, fields(action = action.name()))]
pub fn apply(app: &mut ShellApp, ctx: &Context, action: A) {
    let _sample = lb::service::perf::Sample::new();
    match action {
        A::SelectPane(p) => {
            if app.pane == p && app.sidebar_open && !app.settings.zen_mode {
                app.sidebar_open = false;
            } else {
                app.pane = p;
                app.sidebar_open = true;
                if app.settings.zen_mode {
                    let _ = app.settings.write_zen_mode(false);
                }
            }
            if let Some(r) = app.session.ready_mut() {
                r.workspace.sidebar_open = app.sidebar_open;
            }
            persist_zen(app);
        }
        A::ToggleSidebar => {
            if app.settings.zen_mode {
                let _ = app.settings.write_zen_mode(false);
                app.sidebar_open = true;
            } else {
                app.sidebar_open = !app.sidebar_open;
            }
            if let Some(r) = app.session.ready_mut() {
                r.workspace.sidebar_open = app.sidebar_open;
            }
            persist_zen(app);
        }
        A::SelectFile(id) => {
            if let Some(r) = app.session.ready_mut() {
                r.select_only(id);
            }
        }
        A::SetSelection(ids) => {
            if let Some(r) = app.session.ready_mut() {
                r.selected = ids.iter().copied().collect();
                r.cursor = ids.last().copied();
            }
        }
        A::ToggleSelect(id) => {
            if let Some(r) = app.session.ready_mut() {
                r.toggle_select(id);
            }
        }
        A::SelectRange(id) => {
            if let Some(r) = app.session.ready_mut() {
                r.select_range_to(id);
            }
        }
        A::ToggleExpand(id) => {
            if let Some(r) = app.session.ready_mut() {
                if !r.expanded.insert(id) {
                    r.expanded.remove(&id);
                }
            }
        }
        A::ExpandSubtree(id) => expand_or_collapse(app, id, true),
        A::CollapseSubtree(id) => expand_or_collapse(app, id, false),
        A::OpenFile(id) => open_documents(app, &[id], false),
        A::OpenFileNewTab(id) => open_documents(app, &[id], true),
        A::OpenDocuments { ids, new_tab } => open_documents(app, &ids, new_tab),
        A::SelectTab(i) => {
            if let Some(r) = app.session.ready_mut() {
                // Prefer by-index; if content is still loading, still force the slot.
                if !r.workspace.make_current(i) {
                    if let Some(slot) = r.workspace.tab_strip.get(i).cloned() {
                        // Force current even when promote missed a frame.
                        r.workspace.current_tab = Some(slot.dest);
                        r.workspace.current_tab_changed = true;
                        r.workspace.ctx.request_repaint();
                    }
                }
                if let Some(id) = r.workspace.current_tab_id() {
                    r.select_only(id);
                    super::reveal_and_scroll(r, id);
                } else if let Some(slot) = r.workspace.tab_strip.get(i) {
                    if let Some(id) = slot.dest.backing_file() {
                        r.select_only(id);
                        super::reveal_and_scroll(r, id);
                    }
                }
            }
        }
        A::CloseTab(i) => {
            if let Some(r) = app.session.ready_mut() {
                if i < r.workspace.tab_strip.len() {
                    r.workspace.close_tab(i);
                }
                sync_selection_after_tab_close(r);
            }
        }
        A::CloseOtherTabs(keep) => {
            if let Some(r) = app.session.ready_mut() {
                r.workspace.close_other_tabs(keep);
                sync_selection_after_tab_close(r);
            }
        }
        A::CloseTabsToLeft(i) => {
            if let Some(r) = app.session.ready_mut() {
                r.workspace.close_tabs_to_left(i);
                sync_selection_after_tab_close(r);
            }
        }
        A::CloseTabsToRight(i) => {
            if let Some(r) = app.session.ready_mut() {
                r.workspace.close_tabs_to_right(i);
                sync_selection_after_tab_close(r);
            }
        }
        A::CloseAllTabs => {
            if let Some(r) = app.session.ready_mut() {
                while !r.workspace.tab_strip.is_empty() {
                    r.workspace.close_tab(0);
                }
                sync_selection_after_tab_close(r);
            }
        }
        A::ReopenClosedTab => {
            if let Some(r) = app.session.ready_mut() {
                r.workspace.reopen_closed_tab();
                if let Some(id) = r.workspace.current_tab_id() {
                    r.select_only(id);
                    super::reveal_and_scroll(r, id);
                }
            }
        }
        A::ReorderTab { src, dst } => {
            if let Some(r) = app.session.ready_mut() {
                let n = r.workspace.tab_strip.len();
                if src >= n || dst > n || dst == src || dst == src + 1 {
                    // no-op
                } else {
                    let tab = r.workspace.tab_strip.remove(src);
                    let insert_at = if dst > src { dst - 1 } else { dst };
                    r.workspace
                        .tab_strip
                        .insert(insert_at.min(r.workspace.tab_strip.len()), tab);
                }
            }
        }
        A::OpenSettings => {
            app.modal = Some(Modal::Settings { cat: SettingsCat::Account });
        }
        A::CloseModal => app.modal = None,
        A::SetSettingsCat(cat) => {
            if let Some(Modal::Settings { cat: c }) = &mut app.modal {
                *c = cat;
            }
        }
        A::OpenDelete(ids) => {
            if !ids.is_empty() {
                app.modal = Some(Modal::Delete { ids });
            }
        }
        A::ConfirmDelete => {
            if let Some(Modal::Delete { ids }) = app.modal.take() {
                if let Some(r) = app.session.ready_mut() {
                    for id in &ids {
                        r.workspace.delete_file(*id);
                        r.selected.remove(id);
                    }
                    if let Some(id) = r.workspace.current_tab_id() {
                        r.select_only(id);
                    } else {
                        r.cursor = None;
                    }
                }
            }
        }
        A::OpenShare(id) => {
            if let Some(r) = app.session.ready_mut() {
                r.refresh_known_usernames();
            }
            app.modal = Some(Modal::Share {
                id,
                query: String::new(),
                mode: 0,
                staged: Vec::new(),
                lookup: ShareLookup::Idle,
                lookup_for: String::new(),
                err: None,
            });
            ctx.data_mut(|d| {
                d.insert_temp(egui::Id::new("shell_share_need_focus"), true);
                d.remove::<String>(share_verify_inflight_key());
                d.remove::<Vec<ShareNetDone>>(share_verify_done_key());
            });
        }
        A::ShareQuery => share_query(app, ctx),
        A::ShareMode(m) => {
            if let Some(Modal::Share { mode, .. }) = &mut app.modal {
                *mode = m;
            }
        }
        A::ShareVerify => share_verify(app, ctx),
        A::ShareStageField => share_stage_field(app, ctx),
        A::ShareUnstage(user) => {
            if let Some(Modal::Share { staged, err, .. }) = &mut app.modal {
                staged.retain(|s| !s.username.eq_ignore_ascii_case(&user));
                *err = None;
            }
        }
        A::ShareInvite => share_invite(app, ctx),
        A::OpenCreate { parent, is_folder } => open_create_sheet(app, ctx, parent, is_folder),
        A::CreateSetKind(kind) => {
            let (parent, dirty) = match &app.modal {
                Some(Modal::Create { parent, name_dirty, .. }) => (*parent, *name_dirty),
                _ => return,
            };
            let suggested =
                if dirty { None } else { Some(suggested_create_name(app, parent, kind)) };
            if let Some(Modal::Create { kind: k, name, error, .. }) = &mut app.modal {
                *k = kind;
                *error = None;
                if let Some(s) = suggested {
                    *name = s;
                }
            }
        }
        A::CreateSetLoc(loc) => {
            let (alongside, kind, dirty, cur_parent) = match &app.modal {
                Some(Modal::Create { alongside, kind, name_dirty, parent, .. }) => {
                    (alongside.clone(), *kind, *name_dirty, *parent)
                }
                _ => return,
            };
            let root = app
                .session
                .ready()
                .map(|r| r.workspace.files.read().unwrap().root().id);
            let parent = match loc {
                CreateLoc::Root => root,
                CreateLoc::Alongside => alongside.as_ref().map(|(id, _)| *id).or(root),
                CreateLoc::Custom => cur_parent.or(root),
            };
            let suggested =
                if dirty { None } else { Some(suggested_create_name(app, parent, kind)) };
            if let Some(Modal::Create { loc: l, parent: p, picking, name, error, .. }) =
                &mut app.modal
            {
                *l = loc;
                *p = parent;
                // Simple location pick collapses the advanced browser.
                if !matches!(loc, CreateLoc::Custom) {
                    *picking = false;
                }
                *error = None;
                if let Some(s) = suggested {
                    *name = s;
                }
            }
        }
        A::CreateSetPicking(on) => {
            if let Some(Modal::Create { picking, .. }) = &mut app.modal {
                *picking = on;
            }
        }
        A::CreatePickFolder(id) => {
            let (kind, dirty, root) = match &app.modal {
                Some(Modal::Create { kind, name_dirty, .. }) => {
                    let root = app
                        .session
                        .ready()
                        .map(|r| r.workspace.files.read().unwrap().root().id);
                    (*kind, *name_dirty, root)
                }
                _ => return,
            };
            let suggested =
                if dirty { None } else { Some(suggested_create_name(app, Some(id), kind)) };
            if let Some(Modal::Create { picking, parent, loc, name, error, .. }) = &mut app.modal {
                // Collapse the browser after a pick.
                *picking = false;
                *loc = if root == Some(id) { CreateLoc::Root } else { CreateLoc::Custom };
                *parent = Some(id);
                *error = None;
                if let Some(s) = suggested {
                    *name = s;
                }
            }
        }
        A::ConfirmCreate => confirm_create(app),
        A::OpenMove(ids) => {
            if !ids.is_empty() {
                // Prefer a shared parent as the initial dest so the tree can
                // expand to where the selection currently lives (not always root).
                let dest = app.session.ready().map(|r| {
                    let files = r.workspace.files.read().unwrap();
                    let parents: Vec<Uuid> = ids
                        .iter()
                        .filter_map(|id| files.get_by_id(*id).map(|f| f.parent))
                        .collect();
                    let shared = parents
                        .first()
                        .copied()
                        .filter(|p| parents.iter().all(|q| q == p));
                    shared.unwrap_or_else(|| files.root().id)
                });
                // Fresh expand + scroll-to-selected for this move session.
                ctx.data_mut(|d| {
                    d.remove::<std::collections::HashSet<Uuid>>(egui::Id::new((
                        "shell_folder_pick_exp",
                        "shell_move",
                    )));
                    d.remove::<bool>(super::tree::folder_tree_scroll_key("shell_move"));
                });
                app.modal = Some(Modal::Move { ids, dest });
            }
        }
        A::MoveSelect(id) => {
            if let Some(Modal::Move { dest, .. }) = &mut app.modal {
                *dest = Some(id);
            }
        }
        A::ConfirmMove => {
            if let Some(Modal::Move { ids, dest }) = app.modal.take() {
                if let (Some(d), Some(r)) = (dest, app.session.ready_mut()) {
                    for id in &ids {
                        r.workspace.move_file((*id, d));
                    }
                    r.expanded.insert(d);
                }
            }
        }
        A::OpenRename(id) => {
            let (stem, ext) = app
                .session
                .ready()
                .and_then(|r| {
                    r.workspace
                        .files
                        .read()
                        .unwrap()
                        .get_by_id(id)
                        .map(|f| rename_split_stem_ext(&f.name, f.is_folder()))
                })
                .unwrap_or_else(|| (String::new(), None));
            // Drop markdown focus so the next frame's rename field can claim keys
            // (editor re-grabs only when nothing else is focused).
            if let Some(r) = app.session.ready_mut() {
                if let Some(md) = r.workspace.current_tab_markdown_mut() {
                    md.surrender_focus(ctx);
                }
            }
            app.modal = Some(Modal::Rename { id, name: stem, ext });
            ctx.data_mut(|d| d.insert_temp(egui::Id::new("shell_rename_need_focus"), true));
        }
        A::ConfirmRename => {
            if let Some(Modal::Rename { id, name, ext }) = app.modal.take() {
                let full = rename_join_name(name.trim(), ext.as_deref());
                // Primary is disabled when invalid; Enter is gated the same way.
                // If we still get here, toast instead of silently no-op.
                if full.is_empty() || rename_validate_name(&full).is_err() {
                    app.toasts.error("Invalid name");
                    return;
                }
                if let Some(r) = app.session.ready_mut() {
                    if rename_name_taken(r, id, &full) {
                        app.toasts.error("A file with that name already exists");
                        return;
                    }
                    // Core failure lands on workspace.failure_messages → toast in editor.
                    r.workspace.rename_file((id, full), true);
                }
            }
        }
        A::OpenAcceptShare { id, name } => {
            let dest = app
                .session
                .ready()
                .map(|r| r.workspace.files.read().unwrap().root().id);
            ctx.data_mut(|d| {
                d.remove::<std::collections::HashSet<Uuid>>(egui::Id::new((
                    "shell_folder_pick_exp",
                    "shell_accept",
                )));
                d.remove::<bool>(super::tree::folder_tree_scroll_key("shell_accept"));
            });
            app.modal = Some(Modal::AcceptShare { id, name, dest });
        }
        A::AcceptShareDest(id) => {
            if let Some(Modal::AcceptShare { dest, .. }) = &mut app.modal {
                *dest = Some(id);
            }
        }
        A::ConfirmAcceptShare => {
            if let Some(Modal::AcceptShare { id, name, dest }) = app.modal.take() {
                if let (Some(parent), Some(r)) = (dest, app.session.ready_mut()) {
                    match r.workspace.core.create_file(
                        &name,
                        &parent,
                        FileType::Link { target: id },
                    ) {
                        Ok(_) => {
                            r.expanded.insert(parent);
                        }
                        Err(e) => {
                            app.toasts.error(format!("Couldn’t accept share: {e}"));
                        }
                    }
                }
            }
        }
        A::OpenDeclineShare { id, name } => {
            app.modal = Some(Modal::DeclineShare { id, name });
        }
        A::ConfirmDeclineShare(id) => {
            if let Some(r) = app.session.ready_mut() {
                match r.workspace.core.delete_pending_share(&id) {
                    Ok(()) => {
                        app.modal = None;
                    }
                    Err(e) => {
                        app.toasts.error(format!("Couldn’t decline share: {e}"));
                    }
                }
            }
        }
        A::OpenHelp => app.modal = Some(Modal::Help),
        A::OnboardSetMode(m) => {
            if let Some(Modal::Onboard { mode, err, uname_lookup, uname_lookup_for, .. }) =
                &mut app.modal
            {
                *mode = m;
                *err = None;
                if matches!(m, OnboardMode::Create) {
                    *uname_lookup = OnboardLookup::Idle;
                    uname_lookup_for.clear();
                }
            }
            // Default-focus the field when entering a form step (Create/Rename pattern).
            match m {
                OnboardMode::Create => {
                    ctx.data_mut(|d| {
                        d.insert_temp(egui::Id::new("onboard_uname_need_focus"), true)
                    });
                }
                OnboardMode::Import => {
                    onboard_import_focus(ctx, app);
                }
                OnboardMode::Choice => {}
            }
        }
        A::OnboardVerifyUname => onboard_verify_uname(app, ctx),
        A::OnboardImportKind(k) => {
            if let Some(Modal::Onboard { import_kind, err, .. }) = &mut app.modal {
                *import_kind = k;
                *err = None;
            }
            onboard_import_focus(ctx, app);
        }
        A::OnboardSubmit { show_error } => onboard_submit(app, ctx, show_error),
        A::RequestSync => request_sync(app, ctx),
        A::TogglePin(id) => toggle_pins(app, &[id]),
        A::TogglePinMany(ids) => toggle_pins(app, &ids),
        A::Cut(ids) => {
            if let Some(r) = app.session.ready_mut() {
                r.clipboard = super::session::FileClipboard { ids, cut: true };
            }
        }
        A::Copy(ids) => {
            if let Some(r) = app.session.ready_mut() {
                r.clipboard = super::session::FileClipboard { ids, cut: false };
            }
        }
        A::Paste => paste_clip(app, None),
        A::MoveInto { ids, parent } => {
            if let Some(r) = app.session.ready_mut() {
                for id in &ids {
                    if *id != parent {
                        r.workspace.move_file((*id, parent));
                    }
                }
                r.expanded.insert(parent);
            }
        }
        A::Duplicate(ids) => duplicate_files(app, &ids),
        A::Export(ids) => export_files(app, &ids),
        A::CopyLink(id) => {
            if let Some(r) = app.session.ready() {
                if let Ok(url) = r.workspace.core.get_file_link_url(id) {
                    ctx.copy_text(url);
                }
            }
        }
        A::Import => import_pick(app, ctx),
        A::ImportPaths { paths, parent } => import_paths(app, ctx, paths, parent),
        A::OpenImportParent { paths } => open_import_parent_sheet(app, ctx, paths),
        A::ImportParentSelect(id) => {
            if let Some(Modal::ImportParent { dest, .. }) = &mut app.modal {
                *dest = Some(id);
            }
        }
        A::ConfirmImportParent => {
            if let Some(Modal::ImportParent { paths, dest: Some(parent) }) = app.modal.take() {
                import_paths(app, ctx, paths, parent);
            }
        }
        A::OpenSearch => {
            if let Some(r) = app.session.ready_mut() {
                r.workspace.upsert_search(None);
            }
        }
        A::CancelSubscription => {
            ensure_settings_account(app);
            app.account_panel = AccountPanel::CancelSub;
            app.phrase_cache = None;
        }
        A::SetThemeMode(pref) => {
            app.settings.theme_mode = pref;
            set_mode_preference(ctx, pref);
            let _ = app.settings.to_file();
        }
        A::SetThemeFamily(fam) => {
            app.settings.theme_name = fam.name().to_owned();
            set_theme_family(ctx, fam);
            let _ = app.settings.to_file();
        }
        // Workspace prefs: single store on `workspace.cfg`.
        A::SetPrefLinkPreviews(v) => {
            if let Some(r) = app.session.ready_mut() {
                r.workspace.cfg.set_contact_linked_sites(v);
            }
        }
        A::SetPrefSidebarUsage(v) => {
            app.settings.sidebar_usage = v;
            let _ = app.settings.to_file();
        }

        A::SetPrefAllowWayland(v) => {
            #[cfg(target_os = "linux")]
            {
                app.settings.allow_wayland = v;
                let _ = app.settings.to_file();
            }
            let _ = v;
        }
        A::RevealPhrase => {
            ensure_settings_account(app);
            app.account_panel = AccountPanel::Phrase;
            if let Some(r) = app.session.ready() {
                if let Ok(p) = r.workspace.core.export_account_phrase() {
                    app.phrase_cache = Some(p);
                }
            }
        }
        A::OpenAccountQr => {
            ensure_settings_account(app);
            app.account_panel = AccountPanel::Qr;
            app.phrase_cache = None;
            spawn_account_qr(app, ctx);
        }
        A::HideAccountKey => {
            app.close_account_panel();
            app.phrase_cache = None;
        }
        A::CopyPhrase => {
            if let Some(p) = &app.phrase_cache {
                ctx.copy_text(p.clone());
            } else if let Some(r) = app.session.ready() {
                if let Ok(p) = r.workspace.core.export_account_phrase() {
                    ctx.copy_text(p.clone());
                    app.phrase_cache = Some(p);
                }
            }
        }
        A::RevealDebugInfo => {
            app.debug_info_revealed = true;
            spawn_debug_info(app, ctx, false);
        }
        A::HideDebugInfo => {
            app.debug_info_revealed = false;
        }
        A::EnsureDebugInfo => spawn_debug_info(app, ctx, false),
        A::RefreshDebugInfo => spawn_debug_info(app, ctx, true),
        A::CopyDebugInfo => {
            let text = app.debug_info.lock().ok().and_then(|g| match &*g {
                super::DebugInfoCache::Ready(s) => Some(s.clone()),
                _ => None,
            });
            if let Some(s) = text {
                if !s.is_empty() {
                    ctx.copy_text(s);
                }
            }
        }
        A::OpenUpgrade => {
            // Stay in Settings → Account in-content (not a separate sheet).
            ensure_settings_account(app);
            app.account_panel = AccountPanel::Upgrade {
                stage: UpgradeStage::EnterCard,
                number: String::new(),
                exp: String::new(),
                cvc: String::new(),
                error: None,
                done: None,
            };
            app.phrase_cache = None;
            ctx.data_mut(|d| d.insert_temp(egui::Id::new("shell_upgrade_need_focus"), true));
        }
        A::UpgradeBack => {
            match &app.account_panel {
                AccountPanel::Upgrade { stage: UpgradeStage::Confirm, .. } => {
                    if let AccountPanel::Upgrade { stage, error, .. } = &mut app.account_panel {
                        *stage = UpgradeStage::EnterCard;
                        *error = None;
                    }
                }
                // Payment failed — edit card again.
                AccountPanel::Upgrade {
                    stage: UpgradeStage::Paying, done: Some(Err(_)), ..
                } => {
                    if let AccountPanel::Upgrade { stage, done, error, .. } = &mut app.account_panel
                    {
                        *stage = UpgradeStage::EnterCard;
                        *done = None;
                        *error = None;
                    }
                }
                AccountPanel::Upgrade { stage: UpgradeStage::EnterCard, .. }
                | AccountPanel::Upgrade {
                    stage: UpgradeStage::Paying, done: Some(Ok(())), ..
                } => {
                    app.close_account_panel();
                }
                // Mid-charge: ignore back.
                _ => {}
            }
        }
        A::UpgradeNext => upgrade_validate_and_confirm(app),
        A::UpgradeConfirmPay => upgrade_start_pay(app, ctx),
        A::UpgradeDone => {
            app.close_account_panel();
        }
        A::OpenLogout => {
            ensure_settings_account(app);
            app.account_panel = AccountPanel::Logout { acked: false };
            app.phrase_cache = None;
        }
        A::LogoutAck(on) => {
            if let AccountPanel::Logout { acked } = &mut app.account_panel {
                *acked = on;
            }
        }
        A::ConfirmLogout => {
            let ok = matches!(&app.account_panel, AccountPanel::Logout { acked: true });
            if !ok {
                return;
            }
            do_logout(app, ctx);
        }
        A::OpenDeleteAccount => {
            ensure_settings_account(app);
            app.account_panel = AccountPanel::DeleteAccount { typed: String::new() };
            app.phrase_cache = None;
            ctx.data_mut(|d| d.insert_temp(egui::Id::new("shell_delete_account_need_focus"), true));
        }
        A::ConfirmDeleteAccount => {
            let username = app
                .session
                .ready()
                .map(|r| r.workspace.account.username.clone())
                .unwrap_or_default();
            let typed = match &app.account_panel {
                AccountPanel::DeleteAccount { typed } => typed.trim().to_owned(),
                _ => String::new(),
            };
            if typed != username {
                return;
            }
            do_delete_account(app, ctx);
        }
        A::ConfirmCancelSub => {
            // Cancel → refresh usage + sub_info (Stripe standing is cap-based).
            app.close_account_panel();
            let Some(r) = app.session.ready_mut() else {
                return;
            };
            match r.workspace.core.cancel_subscription() {
                Ok(()) => {
                    let usage = r.workspace.core.get_usage().ok();
                    if let Some(u) = usage.clone() {
                        r.status.space_used = Some(u);
                    }
                    r.sub_info = r.workspace.core.get_subscription_info().ok().flatten();
                }
                Err(e) => {
                    let msg = format!("{e}");
                    let lower = msg.to_lowercase();
                    let already = lower.contains("not premium")
                        || lower.contains("notpremium")
                        || lower.contains("already canceled")
                        || lower.contains("alreadycanceled");
                    if already {
                        let usage = r.workspace.core.get_usage().ok();
                        if let Some(u) = usage {
                            r.status.space_used = Some(u);
                        }
                        r.sub_info = r.workspace.core.get_subscription_info().ok().flatten();
                    }
                }
            }
        }
        A::Create => {
            // Sidebar Create chip / ⌘N — create sheet, default Note at focus parent.
            let parent = focused_create_parent(app);
            open_create_sheet(app, ctx, parent, false);
        }
        A::SaveAll => {
            if let Some(r) = app.session.ready_mut() {
                r.workspace.save_all_tabs();
            }
        }
    }
}

/// After closing tabs: selection follows workspace current file, or clears.
fn sync_selection_after_tab_close(r: &mut Ready) {
    if let Some(id) = r.workspace.current_tab_id() {
        r.select_only(id);
    } else {
        r.cursor = None;
        r.selected.clear();
    }
}

fn focused_create_parent(app: &ShellApp) -> Option<Uuid> {
    let r = app.session.ready()?;
    if let Some(id) = r.cursor {
        let files = r.workspace.files.read().unwrap();
        return files
            .get_by_id(id)
            .map(|f| if f.is_folder() { f.id } else { f.parent });
    }
    r.workspace
        .current_tab_id()
        .and_then(|id| {
            let files = r.workspace.files.read().unwrap();
            files.get_by_id(id).map(|f| f.parent)
        })
        .or_else(|| Some(r.workspace.files.read().unwrap().root().id))
}

fn open_create_sheet(
    app: &mut ShellApp, ctx: &Context, parent_hint: Option<Uuid>, prefer_folder: bool,
) {
    let Some(r) = app.session.ready() else {
        return;
    };
    let files = r.workspace.files.read().unwrap();
    let root = files.root().id;
    // Alongside plate = tree cursor when that row is a document (same parent).
    // Tab focus usually mirrors selection (`SelectTab` → select_only + reveal), but
    // a tree click without open can diverge — invocation context is the sidebar.
    let (alongside_parent, alongside_label) = r
        .cursor
        .and_then(|id| {
            let f = files.get_by_id(id)?;
            if f.is_folder() { None } else { Some((f.parent, f.name.clone())) }
        })
        .map(|(p, n)| (Some(p), Some(n)))
        .unwrap_or((None, None));

    let hint = parent_hint.unwrap_or(root);
    let (loc, parent) = if hint == root {
        (CreateLoc::Root, Some(root))
    } else if alongside_parent == Some(hint) {
        (CreateLoc::Alongside, Some(hint))
    } else {
        (CreateLoc::Custom, Some(hint))
    };

    let kind = if prefer_folder { CreateKind::Folder } else { CreateKind::Note };
    drop(files);
    let name = suggested_create_name(app, parent, kind);
    let alongside = match (alongside_parent, alongside_label) {
        (Some(id), Some(label)) => Some((id, label)),
        _ => None,
    };
    app.modal = Some(Modal::Create {
        name,
        kind,
        parent,
        loc,
        alongside,
        picking: false,
        error: None,
        name_dirty: false,
    });
    // Focus name field + select-all; clear height lock + folder wizard state.
    ctx.data_mut(|d| {
        d.insert_temp(egui::Id::new("shell_create_need_focus"), true);
        d.remove::<std::collections::HashSet<Uuid>>(egui::Id::new("shell_create_folder_exp"));
        d.remove::<bool>(super::tree::folder_tree_scroll_key("shell_create_folder_tree"));
        d.remove::<f32>(egui::Id::new("shell_create_inner_h"));
    });
}

// ── Rename helpers (stem + static extension, proactive validation) ───────────

/// Split display name into editable stem + static trailing extension.
///
/// Folders and names without a mid-dot keep the full string as stem.
/// Leading-dot names (`.gitignore`) stay whole — no extension plate.
pub(crate) fn rename_split_stem_ext(full: &str, is_folder: bool) -> (String, Option<String>) {
    if is_folder {
        return (full.to_owned(), None);
    }
    match full.rfind('.') {
        Some(i) if i > 0 && i + 1 < full.len() => {
            (full[..i].to_owned(), Some(full[i..].to_owned()))
        }
        _ => (full.to_owned(), None),
    }
}

/// Rejoin stem + optional extension (create-style: skip append if stem already ends with it).
pub(crate) fn rename_join_name(stem: &str, ext: Option<&str>) -> String {
    let stem = stem.trim();
    match ext {
        Some(e) if !e.is_empty() && !stem.ends_with(e) => format!("{stem}{e}"),
        _ => stem.to_owned(),
    }
}

/// Core filename rules (`validate::file_name` + length).
pub(crate) fn rename_validate_name(full: &str) -> Result<(), String> {
    if full.is_empty() {
        return Err("Name required".into());
    }
    if full.contains('/') {
        return Err("A file name cannot contain slashes".into());
    }
    if full.len() > lb::model::filename::MAX_FILENAME_LENGTH {
        return Err("That file name is too long".into());
    }
    Ok(())
}

/// Another sibling under the same parent already has `full` (case-sensitive, core path rules).
pub(crate) fn rename_name_taken(r: &Ready, id: Uuid, full: &str) -> bool {
    let files = r.workspace.files.read().unwrap();
    let Some(f) = files.get_by_id(id) else {
        return false;
    };
    files
        .children(f.parent)
        .into_iter()
        .any(|c| c.id != id && c.name == full)
}

/// Live check for the rename sheet (error copy + whether primary is enabled).
pub(crate) fn rename_live_status(
    app: &ShellApp, id: Uuid, stem: &str, ext: Option<&str>,
) -> RenameLive {
    let full = rename_join_name(stem, ext);
    let original = app.session.ready().and_then(|r| {
        r.workspace
            .files
            .read()
            .unwrap()
            .get_by_id(id)
            .map(|f| f.name.clone())
    });
    if let Err(msg) = rename_validate_name(&full) {
        return RenameLive { error: Some(msg), can_commit: false };
    }
    if original.as_deref() == Some(full.as_str()) {
        return RenameLive { error: None, can_commit: false };
    }
    if let Some(r) = app.session.ready() {
        if rename_name_taken(r, id, &full) {
            return RenameLive {
                error: Some("A file with that name already exists".into()),
                can_commit: false,
            };
        }
    }
    RenameLive { error: None, can_commit: true }
}

pub(crate) struct RenameLive {
    pub error: Option<String>,
    pub can_commit: bool,
}

/// Suggested create name: local calendar date + unique variant
/// under `parent` via [`lb::model::filename::NameComponents`].
///
/// Note: NameComponents currently treats the day of `YYYY-MM-DD` as a variant
/// (`2026-08-05` → `2026-08-5`). Accept that until NameComponents is fixed.
fn suggested_create_name(app: &ShellApp, parent: Option<Uuid>, kind: CreateKind) -> String {
    let date = chrono::Local::now().format("%Y-%m-%d").to_string();
    let ext = kind.ext().unwrap_or("");
    let desired = format!("{date}{ext}");
    let Some(r) = app.session.ready() else {
        return date;
    };
    let files = r.workspace.files.read().unwrap();
    let parent = parent.unwrap_or_else(|| files.root().id);
    let kids: Vec<_> = files.children(parent).into_iter().cloned().collect();
    let mut nc = lb::model::filename::NameComponents::from(&desired);
    nc.next_in_children(kids);
    let full = nc.to_name();
    // Sheet shows stem; extension is a static trailing label.
    if let Some(e) = kind.ext() { full.strip_suffix(e).unwrap_or(&full).to_owned() } else { full }
}

#[instrument(level = "trace", skip_all)]
fn confirm_create(app: &mut ShellApp) {
    let (name, kind, parent) = match &app.modal {
        Some(Modal::Create { name, kind, parent, .. }) => (name.trim().to_owned(), *kind, *parent),
        _ => return,
    };
    if name.is_empty() {
        if let Some(Modal::Create { error, .. }) = &mut app.modal {
            *error = Some("Name required".into());
        }
        return;
    }
    let is_folder = kind == CreateKind::Folder;
    let full_name = match kind.ext() {
        Some(ext) if !name.ends_with(ext) => format!("{name}{ext}"),
        _ => name,
    };
    let outcome = {
        let Some(r) = app.session.ready_mut() else {
            return;
        };
        let parent_id = parent.unwrap_or_else(|| r.workspace.effective_focused_parent());
        let result = if is_folder {
            r.workspace
                .core
                .create_file(&full_name, &parent_id, FileType::Folder)
        } else {
            r.workspace
                .core
                .create_file(&full_name, &parent_id, FileType::Document)
        };
        match result {
            Ok(file) => {
                r.expanded.insert(parent_id);
                r.select_only(file.id);
                if !is_folder {
                    r.workspace.open_file(file.id, true, true);
                }
                Ok(file.name)
            }
            Err(e) => Err(format!("{e:?}")),
        }
    };
    match outcome {
        Ok(_) => {
            app.modal = None;
        }
        Err(e) => {
            if let Some(Modal::Create { error, .. }) = &mut app.modal {
                *error = Some(e);
            }
        }
    }
}

/// Open documents. Folders in `ids` are skipped except a sole folder (toggle expand).
/// Multi open without `new_tab`: first reuses tab path, rest open as new tabs.
#[instrument(level = "trace", skip_all)]
fn open_documents(app: &mut ShellApp, ids: &[Uuid], new_tab: bool) {
    let Some(r) = app.session.ready_mut() else {
        return;
    };
    if ids.is_empty() {
        return;
    }
    // Single folder (e.g. Enter): toggle expand — not a document open.
    if ids.len() == 1 {
        let id = ids[0];
        let is_folder = r
            .workspace
            .files
            .read()
            .unwrap()
            .get_by_id(id)
            .map(|f| f.is_folder())
            .unwrap_or(false);
        if is_folder {
            r.select_only(id);
            if !r.expanded.insert(id) {
                r.expanded.remove(&id);
            }
            r.workspace.focused_parent = Some(id);
            return;
        }
    }
    let docs: Vec<Uuid> = {
        let files = r.workspace.files.read().unwrap();
        ids.iter()
            .copied()
            .filter(|id| {
                files
                    .get_by_id(*id)
                    .map(|f| f.is_document())
                    .unwrap_or(false)
            })
            .collect()
    };
    if docs.is_empty() {
        return;
    }
    for (i, id) in docs.iter().enumerate() {
        let nt = new_tab || i > 0;
        r.workspace.open_file(*id, true, nt);
    }
    let last = *docs.last().unwrap();
    if docs.len() > 1 {
        r.selected = docs.iter().copied().collect();
        r.cursor = Some(last);
    } else {
        r.select_only(last);
    }
    super::reveal_and_scroll(r, last);
}

fn toggle_pins(app: &mut ShellApp, ids: &[Uuid]) {
    if let Some(r) = app.session.ready_mut() {
        for &id in ids {
            let was = is_pinned(r, id);
            let res =
                if was { r.workspace.core.unpin_file(id) } else { r.workspace.core.pin_file(id) };
            let _ = res;
        }
        refresh_pinned(r);
    }
}

#[instrument(level = "trace", skip_all)]
fn duplicate_files(app: &mut ShellApp, ids: &[Uuid]) {
    let mut created = Vec::new();
    if let Some(r) = app.session.ready_mut() {
        for id in ids {
            match r.workspace.core.duplicate_file(id) {
                Ok(f) => created.push(f),
                Err(_) => {
                    // Folders → FileNotDocument; skip quietly when multi.
                }
            }
        }
        if !created.is_empty() {
            let last = created.last().unwrap().id;
            for (i, f) in created.iter().enumerate() {
                if f.is_document() {
                    r.workspace
                        .open_file(f.id, true, i > 0 || created.len() == 1);
                }
            }
            if created.len() > 1 {
                r.selected = created.iter().map(|f| f.id).collect();
                r.cursor = Some(last);
            } else {
                r.select_only(last);
            }
        }
    }
}

fn expand_or_collapse(app: &mut ShellApp, id: Uuid, expand: bool) {
    let Some(r) = app.session.ready_mut() else {
        return;
    };
    let mut stack = vec![id];
    let files = r.workspace.files.read().unwrap();
    while let Some(cur) = stack.pop() {
        if expand {
            r.expanded.insert(cur);
        } else {
            r.expanded.remove(&cur);
        }
        for kid in files.children(cur) {
            if kid.is_folder() {
                stack.push(kid.id);
            }
        }
    }
}

fn paste_clip(app: &mut ShellApp, dest_override: Option<Uuid>) {
    let Some(r) = app.session.ready_mut() else {
        return;
    };
    let dest = dest_override
        .map(|d| {
            // If dest is a document, paste into its parent (recents menus).
            let files = r.workspace.files.read().unwrap();
            files
                .get_by_id(d)
                .map(|f| if f.is_folder() { f.id } else { f.parent })
                .unwrap_or(d)
        })
        .unwrap_or_else(|| {
            r.cursor
                .and_then(|id| {
                    let files = r.workspace.files.read().unwrap();
                    files
                        .get_by_id(id)
                        .map(|f| if f.is_folder() { f.id } else { f.parent })
                })
                .unwrap_or_else(|| r.workspace.files.read().unwrap().root().id)
        });
    let clip = r.clipboard.clone();
    if clip.ids.is_empty() {
        return;
    }
    if clip.cut {
        for id in &clip.ids {
            if *id != dest {
                r.workspace.move_file((*id, dest));
            }
        }
        r.clipboard = Default::default();
        r.expanded.insert(dest);
    } else {
        for id in &clip.ids {
            if let Ok(f) = r.workspace.core.duplicate_file(id) {
                if f.parent != dest {
                    let _ = r.workspace.core.move_file(&f.id, &dest);
                }
            }
        }
        r.expanded.insert(dest);
    }
}

fn export_files(app: &mut ShellApp, ids: &[Uuid]) {
    if ids.is_empty() {
        return;
    }
    let Some(r) = app.session.ready() else {
        return;
    };
    let files: Vec<_> = {
        let cache = r.workspace.files.read().unwrap();
        ids.iter()
            .filter_map(|id| cache.get_by_id(*id).cloned())
            .collect()
    };
    if files.is_empty() {
        return;
    }
    let Some(dest) = FileDialog::new().pick_folder() else {
        return;
    };
    let core = r.workspace.core.clone();
    let mut failed = 0usize;
    for f in &files {
        if let Err(e) = core.export_files(f.id, dest.clone(), true, &None) {
            failed += 1;
            eprintln!("export {}: {e:?}", f.name);
        }
    }
    if failed > 0 {
        let msg = if failed == 1 {
            "Couldn’t export 1 file".into()
        } else {
            format!("Couldn’t export {failed} files")
        };
        app.toasts.error(msg);
    } else {
        app.toasts.info(format!(
            "Exported {} {}",
            files.len(),
            if files.len() == 1 { "item" } else { "items" }
        ));
    }
}

fn import_pick(app: &mut ShellApp, ctx: &Context) {
    // Sidebar Import chip: system file picker → same destination sheet as drop.
    let Some(paths) = FileDialog::new().pick_files() else {
        return;
    };
    if paths.is_empty() {
        return;
    }
    open_import_parent_sheet(app, ctx, paths);
}

/// Open the Import folder-picker sheet. Seeds `dest` from the cursor folder
/// (or its parent / root) so chip and drop share one path.
fn open_import_parent_sheet(app: &mut ShellApp, ctx: &Context, paths: Vec<PathBuf>) {
    let dest = app.session.ready().map(|r| {
        r.cursor
            .and_then(|id| {
                let files = r.workspace.files.read().unwrap();
                files
                    .get_by_id(id)
                    .map(|f| if f.is_folder() { f.id } else { f.parent })
            })
            .unwrap_or_else(|| r.workspace.files.read().unwrap().root().id)
    });
    ctx.data_mut(|d| {
        d.remove::<std::collections::HashSet<Uuid>>(egui::Id::new((
            "shell_folder_pick_exp",
            "shell_import_parent",
        )));
        d.remove::<bool>(super::tree::folder_tree_scroll_key("shell_import_parent"));
    });
    app.modal = Some(Modal::ImportParent { paths, dest });
}

fn import_paths(app: &mut ShellApp, ctx: &Context, paths: Vec<PathBuf>, parent: Uuid) {
    let Some(r) = app.session.ready() else {
        return;
    };
    let core = r.workspace.core.clone();
    let ctx = ctx.clone();
    let n = paths.len();
    let toast_inbox = app.toasts.inbox.clone();
    if let Some(r) = app.session.ready_mut() {
        r.expanded.insert(parent);
        r.status_msg = format!("Importing {n}…");
    }
    thread::spawn(move || {
        if let Err(e) = core.import_files(&paths, parent, &|_| {}) {
            eprintln!("import failed: {e:?}");
            toast_inbox.error(format!("Import failed: {e}"));
        }
        let _ = n;
        ctx.request_repaint();
    });
}

fn request_sync(app: &mut ShellApp, ctx: &Context) {
    if let Some(r) = app.session.ready_mut() {
        r.syncing = true;
        r.status_msg = "Syncing…".into();
        let core = r.workspace.core.clone();
        let ctx = ctx.clone();
        thread::spawn(move || {
            let _ = core.sync();
            ctx.request_repaint();
        });
    }
}
fn persist_zen(app: &mut ShellApp) {
    // Disk: zen = sidebar collapsed at next launch.
    let _ = app.settings.write_zen_mode(!app.sidebar_open);
}
