//! The nudge: a card in the sidebar footer asking someone to sign in on
//! another device, a delay after signup set by the [`SignInNudge`] arm. It
//! runs only on the device that created the account. `nudges.json` holds only
//! when this device created the account and whether the card is done.

use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use lb::experiments::{SignInNudge, cohort_in};
use serde::{Deserialize, Serialize};

use super::action::Action as A;
use super::session::Ready;

/// How long after signup the card shows for `arm`, if it does.
fn delay_ms(arm: SignInNudge) -> Option<u64> {
    match arm {
        SignInNudge::Holdout => None,
        SignInNudge::TwoMinutes => Some(2 * 60 * 1000),
        SignInNudge::FiveMinutes => Some(5 * 60 * 1000),
    }
}

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct NudgeState {
    /// When this device created the account, in ms since the epoch.
    created_at: Option<u64>,
    /// The card was acted on or closed, and never comes back.
    done: bool,
}

pub struct Nudges {
    state: NudgeState,
    path: PathBuf,
    /// When the card is due, if this device created the account and the
    /// person's arm shows it.
    due_at: Option<u64>,
}

impl Nudges {
    pub fn load(ready: &Ready) -> Self {
        let path = ready.workspace.core.get_config().writeable_path.clone();
        let path = PathBuf::from(path).join("nudges.json");
        let state = std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        let mut nudges = Self { state, path, due_at: None };
        nudges.set_due(ready);
        nudges
    }

    fn set_due(&mut self, ready: &Ready) {
        let username = &ready.workspace.account.username;
        let delay = delay_ms(cohort_in(SignInNudge::EXPERIMENT, username));
        self.due_at = self
            .state
            .created_at
            .zip(delay)
            .map(|(at, delay)| at + delay);
    }

    /// This device just created the account.
    pub fn created(&mut self, ready: &Ready) {
        self.state.created_at = Some(now());
        self.set_due(ready);
        self.save();
    }

    /// Once a frame: repaints when the card comes due.
    pub fn tick(&self, ctx: &egui::Context) {
        if self.state.done {
            return;
        }
        if let Some(wait) = self.due_at.and_then(|at| at.checked_sub(now())) {
            ctx.request_repaint_after(Duration::from_millis(wait));
        }
    }

    /// The card is on screen.
    pub fn showing(&self) -> bool {
        !self.state.done && self.due_at.is_some_and(|at| now() >= at)
    }

    /// The card's button: the app shows the sign-in QR code.
    pub fn accept(&mut self) -> A {
        self.dismiss();
        A::OpenAccountQr
    }

    pub fn dismiss(&mut self) {
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
