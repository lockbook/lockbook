//! The nudge: a card in the sidebar footer asking someone to sign in on
//! another device, five minutes after signup once they've made a note. It
//! runs only on the device that created the account and only for the
//! [`Cohort::On`] arm. `nudges.json` holds only whether this device created
//! the account and whether the card is done.

use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use lb::Uuid;
use serde::{Deserialize, Serialize};
use workspace_rs::file_cache::FilesExt;

use super::action::Action as A;
use super::session::Ready;

/// This experiment's arm.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cohort {
    Holdout,
    On,
}

/// The arm for `username`. The experiment's name is hashed with it so arms
/// are independent of other experiments' with the same number of arms.
pub fn cohort(username: &str) -> Cohort {
    let mut hasher = DefaultHasher::new();
    ("nudges", username).hash(&mut hasher);
    match hasher.finish() % 2 {
        0 => Cohort::Holdout,
        _ => Cohort::On,
    }
}

/// How long after signup the card shows: where retention jumps in
/// first-session spans (growth-analysis/journeys.md, Nov 2026).
const FIRST_SESSION_MS: u64 = 5 * 60 * 1000;

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct NudgeState {
    /// This device created the account.
    created_here: bool,
    /// The files the account started with (the welcome docs), never counted.
    starter: Vec<Uuid>,
    /// The card was acted on or closed, and never comes back.
    done: bool,
}

pub struct Nudges {
    state: NudgeState,
    path: PathBuf,
    /// This device created the account and the person is in [`Cohort::On`].
    running: bool,
    /// When the card is due: [`FIRST_SESSION_MS`] after signup, once the
    /// person has a note of their own.
    due_at: Option<u64>,
    /// The files epoch `due_at` was read at.
    epoch: u64,
}

impl Nudges {
    pub fn load(ready: &Ready) -> Self {
        let path = ready.workspace.core.get_config().writeable_path.clone();
        let path = PathBuf::from(path).join("nudges.json");
        let state = std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        let mut nudges = Self { state, path, running: false, due_at: None, epoch: 0 };
        nudges.set_running(ready);
        nudges
    }

    fn set_running(&mut self, ready: &Ready) {
        let on = cohort(&ready.workspace.account.username) == Cohort::On;
        self.running = self.state.created_here && on;
    }

    /// This device just created the account; what's in it now came with it.
    pub fn created(&mut self, ready: &Ready) {
        self.state.created_here = true;
        let files = ready.workspace.files.read().unwrap();
        self.state.starter = files
            .iter_files()
            .filter(|f| !f.is_root())
            .map(|f| f.id)
            .collect();
        drop(files);
        self.set_running(ready);
        self.save();
    }

    /// Once a frame: rereads the files when they change, and repaints when
    /// the card comes due.
    pub fn tick(&mut self, ready: &Ready, ctx: &egui::Context) {
        if !self.running || self.state.done {
            return;
        }
        if self.epoch != ready.files_epoch {
            self.epoch = ready.files_epoch;
            let files = ready.workspace.files.read().unwrap();
            let root = files.root();
            let own = files.iter_files().any(|f| {
                f.is_document() && f.owner == root.owner && !self.state.starter.contains(&f.id)
            });
            self.due_at = own.then_some(root.last_modified + FIRST_SESSION_MS);
        }
        if let Some(wait) = self.due_at.and_then(|at| at.checked_sub(now())) {
            ctx.request_repaint_after(Duration::from_millis(wait));
        }
    }

    /// The card is on screen.
    pub fn showing(&self) -> bool {
        let due = self.due_at.is_some_and(|at| now() >= at);
        self.running && due && !self.state.done
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Arms are recomputed from usernames when retention is measured, so the
    /// hash must never change between releases.
    #[test]
    fn assignments_are_stable() {
        let arms: String = (0..32)
            .map(|i| (cohort(&format!("user{i}")) as usize).to_string())
            .collect();
        assert_eq!(arms, "10101111110101100000110000101100");
    }
}
