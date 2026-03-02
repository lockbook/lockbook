use std::sync::Arc;

use tokio::sync::Mutex;

use crate::Lb;

pub type Syncer = Arc<Mutex<SyncState>>;

#[derive(Default)]
pub struct SyncState {}

// we are gonna have a fetch metadata fn which will get the docs that it needs to get, the ones
// that match should_fetch
//
// should_fetch is going to be a tree fn that will return true if:
//     is md or svg that descends from 

impl Lb {
    pub(crate) fn setup_syncer(&self) {
        let bg_lb = self.clone();

        if self.config.background_work {
            let bg_lb = self.clone();
            tokio::spawn(async move {
                let events = bg_lb.subscribe();
                loop {
                    events.recv().await.unwrap()
                }
            });
        }
    }
}
