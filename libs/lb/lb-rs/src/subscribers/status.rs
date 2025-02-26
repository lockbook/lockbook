use std::sync::Arc;

use tokio::sync::RwLock;
use uuid::Uuid;

use crate::{
    service::{
        events::Event,
        usage::{UsageItemMetric, UsageMetrics},
    },
    Lb,
};

#[derive(Clone, Default)]
pub struct StatusUpdater {
    current_status: Arc<RwLock<Status>>,
}

#[derive(Default)]
pub struct Status {
    pub offline: bool,
    pub out_of_space: bool,
    pub pending_shares: bool,

    pub local_status: Option<LocalStatus>,
    pub sync_status: Option<SyncStatus>,
    pub space_used: Option<SpaceStatus>,
}

pub struct LocalStatus {
    pub dirty_locally: Vec<Uuid>,
    pub updates_available: Vec<Uuid>,
}

pub enum SyncStatus {
    FetchingMetadata,
    PushingMetadata,
    SyncingDocuments {
        pushes_queued: Vec<Uuid>,
        pushes_progress: Vec<Uuid>,
        pushes_completed: Vec<Uuid>,

        pulls_queued: Vec<Uuid>,
        pulls_progress: Vec<Uuid>,
        pulls_completed: Vec<Uuid>,
    },
    CleaningUp,
    LastSynced {
        ts: i64,
        desc: String,
    },
}

pub struct SpaceStatus {
    // todo: should this move over to usage?
    pub unsynced_changes: UsageItemMetric,
    pub server_usage: UsageItemMetric,
    pub data_cap: UsageItemMetric,
}

impl Lb {
    fn status(&self) -> Status {
        todo!()
    }

    pub fn setup_status(&self) {
        let mut rx = self.subscribe();

        tokio::spawn(async move {
            loop {
                let evt = match rx.recv().await {
                    Ok(evt) => evt,
                    Err(err) => {
                        error!("failed to receive from a channel {err}");
                        return;
                    }
                };

                match evt {
                    Event::MetadataChanged(_) => todo!(),
                    Event::DocumentWritten(_) => todo!(),
                }
            }
        });
    }
}

// this is going to be cheap to ask for
// we will eagerly compute this and have it ready
// we will broadcast changes to these fields
// we will consume other status updates and keep these fields up to date
// some of these fields can invalidate one another
// offline for example can invalidate the other statuses, and it's nice to
// centrally manage that data dependency here
//
// we should now be able to communicate status excellently
//
