use crate::messages::{Messenger, Msg};
use std::cell::{Ref, RefCell};
use std::rc::Rc;
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use crate::backend::LbCore;
use lockbook_models::work_unit::WorkUnit;

pub struct BackgroundWork {
    pub messenger: Messenger,
    pub core: Arc<LbCore>,
    pub auto_save_state: Rc<RefCell<AutoSaveState>>,
    pub auto_sync_state: Rc<RefCell<AutoSyncState>>,
}

impl BackgroundWork {
    pub fn default(m: &Messenger) -> Self {
        Self {
            messenger: m.clone(),
            auto_save_state: Rc::new(RefCell::new(AutoSaveState::default())),
            auto_sync_state: Rc::new(RefCell::new(AutoSyncState::default())),
        }
    }

    pub fn init_background_work(state: Arc<Mutex<Self>>) {
        loop {
            thread::sleep(Duration::from_secs(1));
            let bks = state.lock().unwrap();

            if bks.auto_save_state.is_active {
                AutoSaveState::auto_save(bks.auto_save_state.borrow(), &bks.messenger)
            }

            if bks.auto_sync_state.is_active {}
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
        self.last_change = Self::current_time();
    }

    fn current_time() -> u128 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_millis()
    }

    fn auto_save(auto_save_state: Ref<Self>, m: &Messenger) {
        let current_time = Self::current_time();

        let time_between_edits = current_time - auto_save_state.last_change;

        // Required check to prevent overflow
        if auto_save_state.last_change > auto_save_state.last_save && time_between_edits > 1000 {
            // There are changes since we last saved
            auto_save_state.last_save = current_time;
            m.send(Msg::SaveFile);
        }
    }
}

pub struct AutoSyncState {
    pub is_active: bool,
}

impl AutoSyncState {
    pub fn default() -> Self {
        Self { is_active: false }
    }

    pub fn auto_sync(auto_sync_state: Ref<Self>, core: Arc<LbCore>) {
        match core.calculate_work() {
            Ok(work) => {
                if work.work_units.iter().all(|w_u| match w_u {
                    WorkUnit::LocalChange { .. } => false,
                    WorkUnit::ServerChange { .. } => true,
                }) {
                    auto_sync_state.messenger.send(Msg::PerformSync)
                }
            }
            Err(err) => auto_sync_state.messenger.send_err("calculating work", err),
        }
    }
}
