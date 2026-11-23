//! The backup nudge: a card in the sidebar footer asking someone to back up
//! their secret key immediately upon arriving in the app after signup.
//! Runs only on the device that created the account. `nudges.json` holds when
//! this device created the account and whether backup was completed.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use super::action::Action as A;
use super::session::Ready;

/// Schema version for `nudges.json`. Version `0` (legacy `SignInNudge`
/// experiment files without a `version` field) is migrated to `done = true`
/// on load so existing installs don't suddenly receive the new checklist card.
const CURRENT_VERSION: u8 = 2;

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct NudgeState {
    version: u8,
    /// When this device created the account, in ms since the epoch.
    created_at: Option<u64>,
    /// Key backup was completed inside `Modal::BackupKey`; the card never comes back.
    done: bool,
}

pub struct Nudges {
    state: NudgeState,
    path: PathBuf,
}

impl Nudges {
    pub fn load(ready: &Ready) -> Self {
        let path = ready.workspace.core.get_config().writeable_path.clone();
        Self::load_from_path(PathBuf::from(path).join("nudges.json"))
    }

    fn load_from_path(path: PathBuf) -> Self {
        let mut state: NudgeState = std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        let migrated = if state.version < CURRENT_VERSION && state.created_at.is_some() {
            state.done = true;
            state.version = CURRENT_VERSION;
            true
        } else {
            false
        };
        let nudges = Self { state, path };
        if migrated {
            nudges.save();
        }
        nudges
    }

    /// This device just created the account — show the backup card immediately.
    pub fn created(&mut self) {
        self.state.version = CURRENT_VERSION;
        self.state.created_at = Some(now());
        self.state.done = false;
        self.save();
    }

    /// The card is on screen immediately for newly created accounts until backup completes.
    pub fn showing(&self) -> bool {
        !self.state.done && self.state.created_at.is_some()
    }

    /// Clicking the card opens the unified backup sheet without retiring the
    /// nudge — closing the sheet with `Later` or `×` keeps the card visible.
    pub fn accept(&mut self) -> A {
        A::OpenBackupKey
    }

    /// Backup completed inside `Modal::BackupKey` — permanently retire the card.
    pub fn complete(&mut self) {
        self.state.done = true;
        self.save();
    }

    fn save(&self) {
        if let Ok(json) = serde_json::to_vec(&self.state) {
            let _ = std::fs::write(&self.path, json);
        }
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_nudges_json_is_migrated_to_done_on_load() {
        let dir = std::env::temp_dir().join(format!("lb_nudge_mig_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("nudges.json");
        // Legacy format before checklist rewrite: no `version`, created_at set, done = false.
        std::fs::write(&path, r#"{"created_at":1700000000000,"done":false}"#).unwrap();

        let nudges = Nudges::load_from_path(path.clone());
        assert!(!nudges.showing(), "pre-existing installs must not suddenly show the checklist");
        assert!(nudges.state.done);
        assert_eq!(nudges.state.version, CURRENT_VERSION);

        // Reloading stays retired.
        let reloaded = Nudges::load_from_path(path.clone());
        assert!(!reloaded.showing());

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }
}
