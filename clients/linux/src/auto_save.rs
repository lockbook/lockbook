use crate::messages::{Messenger, Msg, ThreadState};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub struct AutoSaveState {
    pub messenger: Messenger,
    pub last_change: u128,
    pub last_save: u128,
    pub is_active: bool,
}

impl AutoSaveState {
    pub fn default(m: &Messenger) -> Self {
        Self {
            messenger: m.clone(),
            last_change: 0,
            last_save: 0,
            is_active: false
        }
    }

    pub fn file_changed(&mut self) {
        self.last_change = Self::current_time();
    }

    fn current_time() -> u128 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_millis()
    }

    pub fn auto_save_loop(state: Arc<Mutex<Self>>) {
        loop {
            thread::sleep(Duration::from_secs(1));
            let current_time = Self::current_time();

            let mut auto_save_state = state.lock().unwrap();

            let time_between_edits = current_time - auto_save_state.last_change;

            // Required check to prevent overflow
            if auto_save_state.last_change > auto_save_state.last_save && time_between_edits > 1000
            {
                // There are changes since we last saved
                auto_save_state.last_save = current_time;
                auto_save_state.messenger.send(Msg::SaveFile);
            }

            if !state.lock().unwrap().is_active {
                break
            }
        }
    }
}
