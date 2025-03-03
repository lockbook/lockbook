use std::sync::Arc;

use tokio::sync::RwLock;
use uuid::Uuid;

use crate::{
    model::{
        api::FileUsage,
        errors::{LbErr, LbErrKind, LbResult, Unexpected},
    },
    service::{
        events::Event,
        sync::SyncIncrement,
        usage::{UsageItemMetric, UsageMetrics},
    },
    Lb,
};

#[derive(Clone, Default)]
pub struct StatusUpdater {
    current_status: Arc<RwLock<Status>>,
}

/// lb-rs may be used by multiple disconnected components which may
/// not be able to seamlessly share state among one another. this struct
/// provides a snapshot into what overall state of data and tasks are
/// within lb-rs.
///
/// the fields are roughly in order of priority, if your UI has limited
/// space to represent information (phones?) earlier fields are more
/// important than later fields. Ideally anything with an ID is represented
/// in the file tree itself.
#[derive(Default, Clone)]
pub struct Status {
    /// some recent server interaction failed due to network conditions
    pub offline: bool,

    /// at-least one document cannot be pushed due to a data cap
    pub out_of_space: bool,

    /// there are pending shares
    pub pending_shares: bool,

    /// you must update to be able to sync, see update_available below
    pub update_required: bool,

    /// metadata or content for this id is being sent to the server
    pub pushing_files: Vec<Uuid>,

    /// following files need to be pushed
    pub dirty_locally: Vec<Uuid>,

    /// metadata or content for this id is being from the server
    pulling_files: Vec<Uuid>,

    /// a mix of human readable and precise data for
    /// used, and available space
    pub space_used: Option<UsageMetrics>,

    /// if there is no pending work this will have a human readable
    /// description of when we last synced successfully
    pub sync_status: Option<String>,
}

impl Lb {
    pub async fn status(&self) -> Status {
        self.status.current_status.read().await.clone()
    }

    pub fn setup_status(&self) {
        let mut rx = self.subscribe();
        let bg = self.clone();

        tokio::spawn(async move {
            loop {
                let evt = match rx.recv().await {
                    Ok(evt) => evt,
                    Err(err) => {
                        error!("failed to receive from a channel {err}");
                        return;
                    }
                };
                bg.process_event(evt).await.log_and_ignore();
            }
        });
    }

    async fn process_event(&self, e: Event) -> LbResult<()> {
        match e {
            Event::MetadataChanged(_) => todo!(),
            Event::DocumentWritten(_) => todo!(),
            Event::Sync(s) => todo!(),
            _ => {}
        }
        Ok(())
    }

    async fn compute_dirty_locally(&self, status: &mut Status) -> LbResult<()> {
        status.dirty_locally = self.local_changes().await;
        Ok(())
    }

    async fn compute_usage(&self, status: &mut Status) -> LbResult<()> {
        // this will need to be debounced, otherwise every keystroke is gonna trigger this
        match self.get_usage().await.map_err(LbErr::from) {
            Ok(usage) => {
                status.space_used = Some(usage);
            }
            Err(err) => match err.kind {
                LbErrKind::AccountNonexistent => todo!(),
                LbErrKind::ClientUpdateRequired => {
                    status.update_required = true;
                }
                LbErrKind::ServerUnreachable => {
                    status.offline = true;
                }
                _ => todo!(),
            },
        };

        Ok(())
    }

    async fn update_sync(&self, s: SyncIncrement, status: &mut Status) -> LbResult<()> {
        match s {
            SyncIncrement::SyncStarted => {
                self.reset_sync(status);
            }
            SyncIncrement::UpdatingMetadata => todo!(),
            SyncIncrement::PullingDocument(id) => {
                status.pulling_files.push(id);
            },
            SyncIncrement::PushingDocument(id) => {
                status.pushing_files.push(id);
            },
            SyncIncrement::SyncFinished(maybe_problem) => {
                self.reset_sync(status);
                match maybe_problem {
                    Some(LbErrKind::ClientUpdateRequired) => {
                        status.update_required = true;
                    }
                    Some(LbErrKind::ServerUnreachable) => {
                        status.offline = true;
                    }
                    Some(LbErrKind::UsageIsOverDataCap) => {
                        status.out_of_space = true;
                    }
                    _ => {
                        error!("unexpected sync problem found {maybe_problem:?}");
                    }
                }
            }
        }

        Ok(())
    }

    fn reset_sync(&self, status: &mut Status) {
        status.pulling_files.clear();
        status.pushing_files.clear();
        status.offline = false;
        status.update_required = true;
        status.out_of_space = false;
    }
}
