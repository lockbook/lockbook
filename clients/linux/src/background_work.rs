use crate::messages::{Messenger, Msg};
use std::cell::{Ref, RefCell};
use std::rc::Rc;
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use crate::backend::LbCore;
use lockbook_models::work_unit::WorkUnit;
use std::sync::atomic::{AtomicBool, Ordering};

pub struct BackgroundWork {
    pub messenger: Messenger,
    pub core: Arc<LbCore>,
    pub auto_save_state: Rc<RefCell<AutoSaveState>>,
    pub auto_sync_state: Rc<RefCell<AutoSyncState>>,
}

impl BackgroundWork {
    pub fn default(m: &Messenger, c: Arc<LbCore>) -> Self {
        Self {
            messenger: m.clone(),
            core: c,
            auto_save_state: Rc::new(RefCell::new(AutoSaveState::default())),
            auto_sync_state: Rc::new(RefCell::new(AutoSyncState::default())),
        }
    }

    fn current_time() -> u128 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_millis()
    }

    pub fn init_background_work(state: Arc<Mutex<Self>>, open_file_dirty: Arc<AtomicBool>) {
        loop {
            thread::sleep(Duration::from_secs(1));
            let bks = state.lock().unwrap();
            let current_time = Self::current_time();

            if bks.auto_save_state.is_active {
                AutoSaveState::auto_save(bks.auto_save_state.borrow(), &bks.messenger, current_time)
            }

            if bks.auto_sync_state.is_active {
                if !bks.auto_save_state.borrow().is_active && open_file_dirty.load(Ordering::SeqCst) {
                    break
                }

                AutoSyncState::auto_sync(bks.auto_sync_state.borrow(), &bks.messenger, bks.core.clone(), open_file_dirty.clone(), current_time)
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
        self.last_change = Self::current_time();
    }

    fn auto_save(auto_save_state: Ref<Self>, m: &Messenger, current_time: u128) {
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

    pub fn auto_sync(auto_sync_state: Ref<Self>, m: &Messenger, c: Arc<LbCore>, open_file_dirty: Arc<AtomicBool>, current_time: u128) {
        if open_file_dirty.load(Ordering::Relaxed) {
            m.send(Msg::SaveFile);
            m.send(Msg::OpenFile(None));
        }

        c.sync()

        m.send(Msg::Sync)
    }
}
