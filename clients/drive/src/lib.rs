use event::DriveEvent;
use lb::Core;
use local_sync::WatcherState;
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};
use test_utils::{test_core, test_core_with_account};

pub mod event;
mod import;
mod local_sync;

#[derive(Clone)]
pub struct Drive {
    pub c: Core,
    pub watcher_state: Arc<Mutex<WatcherState>>,
    pub pending_events: Arc<Mutex<VecDeque<DriveEvent>>>,
}

impl Drive {
    pub fn test_drive() -> Self {
        let c = test_core_with_account();
        let watcher_state = Default::default();
        let pending_events = Default::default();

        Self { c, watcher_state, pending_events }
    }
}
