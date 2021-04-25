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

        let time_since_edit = current_time - self.auto_save_state.last_change;
        let time_since_sync = current_time - self.auto_sync_state.last_sync;
        let sync_on_last_edit =
            !self.auto_sync_state.synced_last_edit && time_since_edit > Self::SYNC_TIME_AFTER_EDIT;

        if sync_on_last_edit {
            self.auto_sync_state.synced_last_edit = true;
        }

        if self.auto_save_state.last_save > self.auto_save_state.last_change
            && (sync_on_last_edit
                || self.auto_sync_state.synced_last_edit
                    && time_since_sync > Self::TIME_BETWEEN_SYNCS)
        {
            // TODO: make a setting to adjust these durations
            self.auto_sync_state.last_sync = current_time;
            self.messenger.send(Msg::PerformSync);
        }
    }

    pub fn auto_save(&mut self) {
        let current_time = BackgroundWork::current_time();
        let time_since_edit = current_time - self.auto_save_state.last_change;

        // Required check to prevent overflow
        if self.auto_save_state.last_change > self.auto_save_state.last_save
            && time_since_edit > Self::SAVE_TIME_AFTER_EDIT
        {
            // There are changes since we last saved
            self.auto_sync_state.synced_last_edit = false;
            self.auto_save_state.last_save = current_time;
            self.messenger.send(Msg::SaveFile);
        }
    }

    pub const TIME_BETWEEN_SYNCS: u128 = 1800000;
    pub const SYNC_TIME_AFTER_EDIT: u128 = 90000;
    pub const SAVE_TIME_AFTER_EDIT: u128 = 1000;
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
    pub synced_last_edit: bool,
}

impl AutoSyncState {
    pub fn default() -> Self {
        Self {
            is_active: false,
            last_sync: 0,
            synced_last_edit: false,
        }
    }
}
