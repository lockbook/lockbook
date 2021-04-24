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
            let mut bks = state.lock().unwrap();
            let m = bks.messenger.clone();

            if bks.auto_save_state.is_active {
                bks.auto_save_state.auto_save(&m);
            }

            if bks.auto_sync_state.is_active {
                let last_change = bks.auto_save_state.last_change;
                let last_save = bks.auto_save_state.last_save;

                bks.auto_sync_state.auto_sync(&m, last_change, last_save)
            }
        }
    }
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

    pub fn auto_save(&mut self, m: &Messenger) {
        let current_time = BackgroundWork::current_time();
        let time_between_edits = current_time - self.last_change;

        // Required check to prevent overflow
        if self.last_change > self.last_save && time_between_edits > 1000 {
            // There are changes since we last saved
            self.last_save = current_time;
            m.send(Msg::SaveFile);
        }
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

    pub fn auto_sync(&mut self, m: &Messenger, last_change: u128, last_save: u128) {
        let current_time = BackgroundWork::current_time();

        let time_between_edit = current_time - last_change;
        let time_between_syncs = current_time - self.last_sync;
        let is_file_clean = last_save as i128 - last_change as i128 > 0;

        if time_between_syncs > 60000 && (time_between_edit > 90000 && is_file_clean) {
            self.last_sync = current_time;
            m.send(Msg::PerformSync);
        }
    }
}
