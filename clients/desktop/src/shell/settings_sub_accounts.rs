use egui::{Align2, Id, Sense, Ui, pos2, vec2};

use crate::components::{
    Button, Field, SheetFooterOpts, Space, Spacer, Theme, TypeRole, control_height, footnote,
    form_group, form_row, form_row_detail, form_value, section_label, sheet_band_centered,
    sheet_footer, shortcut_return, ui_width,
};
use crate::shell::ShellApp;
use crate::shell::action::{Action, Action as A};
use crate::shell::prefs::{AccountPanel, format_bytes};

use super::apply_sub_accounts::new_label_focus_key;
use super::sub_accounts::{ABOUT, name_example};

pub(crate) fn account_row(ui: &mut Ui, t: &Theme, queue: &mut Vec<Action>) {
    form_row_detail(ui, t, "Sub-accounts", "Separate keys that reach only what you share.", |ui| {
        if Button::quiet(t, "Manage").show(ui).clicked() {
            queue.push(A::OpenSubAccounts);
        }
    });
}

pub(crate) fn page(
    app: &mut ShellApp, ui: &mut Ui, t: &Theme, queue: &mut Vec<Action>, premium: bool,
) -> bool {
    match &app.account_panel {
        AccountPanel::SubAccounts => page_list(app, ui, t, queue, premium),
        AccountPanel::NewSubAccount { .. } => page_new(app, ui, t, queue),
        AccountPanel::SubAccount { username } => {
            let username = username.clone();
            page_detail(app, ui, t, queue, &username);
        }
        AccountPanel::SubAccountKeys { .. } => page_keys(app, ui, t, queue),
        AccountPanel::DeleteSubAccount { username } => {
            let username = username.clone();
            page_delete(app, ui, t, queue, &username);
        }
        _ => return false,
    }
    true
}

fn note(ui: &mut Ui, t: &Theme, message: &str) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui_width(ui), control_height()), Sense::hover());
    ui.painter().text(
        pos2(rect.left() + Space::Md.pts(), rect.center().y),
        Align2::LEFT_CENTER,
        message,
        TypeRole::Body.font_id(),
        t.neutral_fg_secondary(),
    );
}

fn error(ui: &mut Ui, t: &Theme, message: Option<&str>) {
    if let Some(message) = message {
        ui.add(Spacer::new(Space::Sm));
        ui.label(TypeRole::Body.rich(message).color(t.danger()));
    }
}

fn page_list(app: &ShellApp, ui: &mut Ui, t: &Theme, queue: &mut Vec<Action>, premium: bool) {
    let subs = &app.sub_accounts;

    section_label(ui, t, "Sub-accounts");
    form_group(ui, t, |ui| {
        for sub in &subs.list {
            form_row(ui, t, &sub.username, |ui| {
                if Button::quiet(t, "Open").show(ui).clicked() {
                    queue.push(A::OpenSubAccount(sub.username.clone()));
                }
                ui.add(Spacer::new(Space::Sm).fill_cross(control_height()));
                ui.label(
                    TypeRole::Body
                        .rich(format_bytes(sub.usage))
                        .color(t.neutral_fg_secondary()),
                );
            });
        }
        if !subs.loaded {
            match &subs.load_error {
                Some(e) => note(ui, t, e),
                None => sheet_band_centered(ui, control_height(), |ui| {
                    ui.spinner();
                }),
            }
        } else if subs.list.is_empty() {
            note(ui, t, "No sub-accounts");
        }
    });
    footnote(ui, t, ABOUT);

    ui.add(Spacer::new(Space::Lg));
    form_group(ui, t, |ui| {
        form_row(ui, t, "New sub-account", |ui| {
            if Button::quiet(t, "Create…")
                .enabled(premium && !subs.working)
                .show(ui)
                .clicked()
            {
                queue.push(A::OpenNewSubAccount);
            }
        });
    });
    if !premium {
        footnote(ui, t, "Creating a sub-account needs a premium subscription.");
    }

    ui.add(Spacer::new(Space::Md));
    let foot = sheet_footer(ui, t, "", SheetFooterOpts::default().divider(false).back_only());
    if foot.cancel {
        queue.push(A::HideAccountKey);
    }
}

fn page_new(app: &mut ShellApp, ui: &mut Ui, t: &Theme, queue: &mut Vec<Action>) {
    let parent = app
        .session
        .ready()
        .map(|r| r.workspace.account.username.clone())
        .unwrap_or_default();
    let working = app.sub_accounts.working;
    let failed = app.sub_accounts.error.clone();

    section_label(ui, t, "New sub-account");
    ui.label(
        TypeRole::Body
            .rich(format!(
                "It will be named {}. A name can’t be used again after its sub-account is deleted.",
                name_example(&parent)
            ))
            .color(t.neutral_fg()),
    );
    ui.add(Spacer::new(Space::Md));

    let field = Id::unique("shell_new_sub_account_field");
    let need_focus = ui
        .ctx()
        .data(|d| d.get_temp::<bool>(new_label_focus_key()).unwrap_or(false));
    let (can_create, edited) = {
        let AccountPanel::NewSubAccount { label } = &mut app.account_panel else {
            return;
        };
        let before = label.clone();
        let _ = Field::new(t, label).hint("Label").id(field).show(ui);
        if need_focus {
            let edit = field.with("edit");
            ui.memory_mut(|m| m.request_focus(edit));
            if ui.memory(|m| m.has_focus(edit)) {
                ui.ctx()
                    .data_mut(|d| d.insert_temp(new_label_focus_key(), false));
            }
        }
        (!label.trim().is_empty() && !working, *label != before)
    };
    if edited {
        app.sub_accounts.error = None;
    }
    error(ui, t, failed.as_deref().filter(|_| !edited));

    ui.add(Spacer::new(Space::Md));
    let foot = sheet_footer(
        ui,
        t,
        "Create",
        SheetFooterOpts::default()
            .divider(false)
            .primary_enabled(can_create)
            .primary_shortcut(shortcut_return()),
    );
    if foot.cancel {
        queue.push(A::SubAccountsBack);
    }
    if foot.primary {
        queue.push(A::ConfirmNewSubAccount);
    }
}

fn page_detail(app: &ShellApp, ui: &mut Ui, t: &Theme, queue: &mut Vec<Action>, username: &str) {
    let subs = &app.sub_accounts;
    let usage = subs
        .usage(username)
        .map(format_bytes)
        .unwrap_or_else(|| "—".into());

    section_label(ui, t, "Sub-account");
    form_group(ui, t, |ui| {
        form_value(ui, t, "Username", username);
        form_value(ui, t, "Storage", &usage);
    });

    ui.add(Spacer::new(Space::Lg));
    form_group(ui, t, |ui| {
        form_row_detail(
            ui,
            t,
            "Keys",
            "Sign in as this sub-account with its keys. Anyone who has them can reach what you’ve shared with it.",
            |ui| {
                if Button::quiet(t, "Reveal…")
                    .enabled(!subs.working)
                    .show(ui)
                    .clicked()
                {
                    queue.push(A::RevealSubAccountKeys);
                }
            },
        );
        form_row(ui, t, "Delete sub-account", |ui| {
            if Button::quiet(t, "Delete…")
                .danger()
                .enabled(!subs.working)
                .show(ui)
                .clicked()
            {
                queue.push(A::OpenDeleteSubAccount);
            }
        });
    });
    error(ui, t, subs.error.as_deref());

    ui.add(Spacer::new(Space::Md));
    let foot = sheet_footer(ui, t, "", SheetFooterOpts::default().divider(false).back_only());
    if foot.cancel {
        queue.push(A::SubAccountsBack);
    }
}

fn page_keys(app: &ShellApp, ui: &mut Ui, t: &Theme, queue: &mut Vec<Action>) {
    let Some(keys) = &app.sub_accounts.keys else {
        queue.push(A::SubAccountsBack);
        return;
    };

    ui.label(
        TypeRole::Body
            .rich(format!(
                "Enter these 24 words in Lockbook on another device to sign in as {}.",
                keys.username
            ))
            .color(t.neutral_fg()),
    );
    ui.add(Spacer::new(Space::Md));
    super::settings_account::phrase_plate(ui, t, &keys.phrase);

    ui.add(Spacer::new(Space::Md));
    form_group(ui, t, |ui| {
        form_row_detail(ui, t, "Compact key", "The same key as one line, for a script.", |ui| {
            if Button::quiet(t, "Copy")
                .copy_feedback(Id::unique("shell_copy_sub_account_key"))
                .show(ui)
                .clicked()
            {
                queue.push(A::CopySubAccountKey);
            }
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
            .copy_feedback("shell_copy_sub_account_phrase"),
    );
    if foot.cancel {
        queue.push(A::SubAccountsBack);
    }
    if foot.primary {
        queue.push(A::CopySubAccountPhrase);
    }
}

fn page_delete(app: &ShellApp, ui: &mut Ui, t: &Theme, queue: &mut Vec<Action>, username: &str) {
    use workspace_rs::widgets::GlyphonLabel;

    ui.add(
        GlyphonLabel::new_rich(
            vec![("Delete ", false), (username, true), ("?", false)],
            t.neutral_fg(),
        )
        .font_size(TypeRole::Body.size())
        .line_height(TypeRole::Body.line_height())
        .max_width(ui_width(ui).max(1.0)),
    );
    ui.add(Spacer::new(Space::Xs));
    ui.label(
        TypeRole::Body
            .rich("The files it owns are deleted, and its name can’t be used again.")
            .color(t.neutral_fg()),
    );
    ui.add(Spacer::new(Space::Md));
    let foot = sheet_footer(
        ui,
        t,
        "Delete",
        SheetFooterOpts::default()
            .danger(true)
            .divider(false)
            .primary_enabled(!app.sub_accounts.working)
            .primary_shortcut(shortcut_return()),
    );
    if foot.cancel {
        queue.push(A::SubAccountsBack);
    }
    if foot.primary {
        queue.push(A::ConfirmDeleteSubAccount);
    }
}
