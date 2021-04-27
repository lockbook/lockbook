use crate::messages::{Messenger, Msg};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub struct BackgroundWork {
    pub messenger: Messenger,
    pub auto_save_state: AutoSaveState,
    pub auto_sync_state: AutoSyncState,
}

impl BackgroundWork {
    pub fn default(m: &Messenger) -> Self {
        Self {
            messenger: m.clone(),
            auto_save_state: AutoSaveState::default(),
            auto_sync_state: AutoSyncState::default(),
        }
    }

    pub fn current_time() -> u128 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_millis()
    }

    pub fn init_background_work(state: Arc<Mutex<Self>>) {
        loop {
            thread::sleep(Duration::from_secs(1));
            let mut bgs = state.lock().unwrap();

            if bgs.auto_save_state.is_active {
                bgs.auto_save();
            }

            if bgs.auto_sync_state.is_active {
                bgs.auto_sync();
            }
        }
    }

    pub fn auto_sync(&mut self) {
        let current_time = Self::current_time();

        if self.auto_save_state.last_save < self.auto_save_state.last_change {
            return;
        }

        let millis_since_edit = current_time - self.auto_save_state.last_change;
        let millis_since_sync = current_time - self.auto_sync_state.last_sync;

        let unsynced_changes = self.auto_sync_state.last_sync < self.auto_save_state.last_save;

        if (unsynced_changes && millis_since_edit > Self::SYNC_AFTER_EDIT_DELAY)
            || millis_since_sync > Self::SYNC_AFTER_SYNC_DELAY
        {
            self.auto_sync_state.last_sync = current_time;
            self.messenger.send(Msg::PerformSync);
        }
    }

    pub fn auto_save(&mut self) {
        let current_time = BackgroundWork::current_time();
        let millis_since_edit = current_time - self.auto_save_state.last_change;

        // Required check to prevent overflow
        if self.auto_save_state.last_change > self.auto_save_state.last_save
            && millis_since_edit > Self::SAVE_AFTER_EDIT_DELAY
        {
            // There are changes since we last saved
            self.auto_save_state.last_save = current_time;
            self.messenger.send(Msg::SaveFile);
        }
    }

    // TODO: make a setting to adjust these durations
    pub const SYNC_AFTER_SYNC_DELAY: u128 = 1800000;
    pub const SYNC_AFTER_EDIT_DELAY: u128 = 90000;
    pub const SAVE_AFTER_EDIT_DELAY: u128 = 1000;
}

pub struct AutoSaveState {
    pub last_change: u128,
    pub last_save: u128,
    pub is_active: bool,
}

impl AutoSaveState {
    pub fn default() -> Self {
        Self {
            last_change: 0,
            last_save: 0,
            is_active: false,
        }
    }

    pub fn file_changed(&mut self) {
        self.last_change = BackgroundWork::current_time();
    }
}

pub struct AutoSyncState {
    pub is_active: bool,
    pub last_sync: u128,
}

impl AutoSyncState {
    pub fn default() -> Self {
        Self {
            is_active: false,
            last_sync: 0,
        }
    }
}
