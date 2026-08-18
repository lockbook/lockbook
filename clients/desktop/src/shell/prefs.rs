//! Shell helpers that are not desktop prefs.
//!
//! Desktop persistence is [`crate::settings::Settings`] (`egui/settings.json`).
//! Workspace prefs live on [`workspace_rs::workspace::WsPersistentStore`].
//! Session UI (account subpanels, debug reveal) lives on [`super::ShellApp`].

use super::action::UpgradeStage;

/// In-content Account subview (phrase / QR / manage / upgrade). Esc / Back → closed.
/// Not persisted — session UI only. Experiences launched from Settings stay here
/// (not separate sheets).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum AccountPanel {
    #[default]
    Closed,
    Phrase,
    Qr,
    /// Log out — ack required before primary.
    Logout {
        acked: bool,
    },
    /// Delete account — type username to enable primary.
    DeleteAccount {
        typed: String,
    },
    CancelSub,
    /// Stripe upgrade — in-Settings page (not a sheet).
    Upgrade {
        stage: UpgradeStage,
        /// Display buffer (spaces grouped while typing).
        number: String,
        /// Display buffer `MM/YY` (slash auto-inserted).
        exp: String,
        cvc: String,
        error: Option<String>,
        /// Set when Paying finishes (`Ok` or error string).
        done: Option<Result<(), String>>,
    },
}

/// SI byte label (1000-based), matching lb `bytes_to_human` so caps read as 30 GB.
pub fn format_bytes(n: u64) -> String {
    lb::model::usage::bytes_to_human(n)
}

/// Section title for a recents row (Today / Yesterday / …).
///
/// `last_modified` is **milliseconds** since epoch (lb `File::last_modified`).
/// Using raw seconds here collapses every file into "Today".
pub fn recents_bucket(modified_ms: i64) -> &'static str {
    use chrono::Local;

    let Some(utc) = chrono::DateTime::from_timestamp_millis(modified_ms) else {
        return "Earlier";
    };
    let file_day = utc.with_timezone(&Local).date_naive();
    let today = Local::now().date_naive();
    let days = (today - file_day).num_days();

    // Calendar days (start-of-day), not rolling 24h windows.
    if days <= 0 {
        "Today"
    } else if days == 1 {
        "Yesterday"
    } else if days < 7 {
        "Previous 7 Days"
    } else if days < 30 {
        "Previous 30 Days"
    } else {
        "Earlier"
    }
}
