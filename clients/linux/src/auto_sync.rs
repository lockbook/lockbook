use crate::backend::LbCore;
use crate::error::LbError;
use crate::messages::{Messenger, Msg};
use lockbook_core::service::sync_service::WorkCalculated;
use lockbook_models::work_unit::WorkUnit;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub struct AutoSyncState {
    pub messenger: Messenger,
    pub is_active: bool,
}

impl AutoSyncState {
    pub fn default(m: &Messenger) -> Self {
        Self {
            messenger: m.clone(),
            is_active: false,
        }
    }

    pub fn auto_sync_loop(state: Arc<Mutex<Self>>, core: Arc<LbCore>) {
        loop {
            thread::sleep(Duration::from_secs(30));

            let auto_sync_state = state.lock().unwrap();

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

            if !auto_sync_state.is_active {
                break;
            }
        }
    }
}
