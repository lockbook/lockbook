use std::{sync::Arc, time::Duration};

use tokio::{
    sync::{Mutex, RwLock},
    time::Instant,
};
use uuid::Uuid;

use crate::{
    model::errors::{LbErrKind, LbResult, Unexpected},
    service::{events::Event, sync::SyncIncrement, usage::UsageMetrics},
    Lb,
};

#[derive(Clone, Default)]
pub struct StatusUpdater {
    current_status: Arc<RwLock<Status>>,
    space_updated: Arc<Mutex<SpaceUpdater>>,
}

pub struct SpaceUpdater {
    spawned: bool,
    last_computed: Instant,
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

    /// a sync is in progress
    pub syncing: bool,

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
    /// callers should be prepared to handle ids they don't know
    /// about yet.
    pub pulling_files: Vec<Uuid>,

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

    pub async fn set_initial_state(&self) -> LbResult<()> {
        if self.keychain.get_account().is_ok() {
            let mut current = self.status.current_status.write().await;
            current.dirty_locally = self.local_changes().await;
            if current.dirty_locally.is_empty() {
                current.sync_status = self.get_last_synced_human().await.log_and_ignore();
            }
            current.pending_shares = !self.get_pending_shares().await?.is_empty();
        }

        Ok(())
    }

    pub async fn setup_status(&self) -> LbResult<()> {
        self.set_initial_state().await?;
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

        Ok(())
    }

    async fn process_event(&self, e: Event) -> LbResult<()> {
        let mut current = self.status.current_status.read().await.clone();
        match e {
            Event::MetadataChanged | Event::DocumentWritten(_) => {
                self.compute_dirty_locally(&mut current).await?;
            }
            Event::Sync(s) => self.update_sync(s, &mut current).await?,
            _ => {}
        }
        Ok(())
    }

    async fn compute_dirty_locally(&self, status: &mut Status) -> LbResult<()> {
        let new = self.local_changes().await;
        if new != status.dirty_locally {
            status.dirty_locally = self.local_changes().await;
            self.events.status_updated();
        }
        Ok(())
    }

    async fn compute_usage(&self) {
        let mut lock = self.status.space_updated.lock().await;
        if lock.spawned {
            return;
        }
        lock.spawned = true;
        let computed = lock.last_computed;
        drop(lock);

        let bg = self.clone();
        tokio::spawn(async move {
            if computed.elapsed() < Duration::from_secs(60) {
                tokio::time::sleep(Duration::from_secs(60) - computed.elapsed()).await;
            }
            let usage = bg.get_usage().await.log_and_ignore();
            let mut lock = bg.status.space_updated.lock().await;
            lock.spawned = false;
            lock.last_computed = Instant::now();
            drop(lock);

            bg.status.current_status.write().await.space_used = usage;
            bg.events.status_updated();
        });
    }

    async fn update_sync(&self, s: SyncIncrement, status: &mut Status) -> LbResult<()> {
        match s {
            SyncIncrement::SyncStarted => {
                self.reset_sync(status);
                status.syncing = true;
            }
            SyncIncrement::PullingDocument(id, in_progress) => {
                if in_progress {
                    status.pulling_files.push(id);
                } else {
                    status.pulling_files.retain(|fid| id != *fid);
                }
            }
            SyncIncrement::PushingDocument(id, in_progress) => {
                if in_progress {
                    status.pushing_files.push(id);
                } else {
                    status.pushing_files.retain(|fid| id != *fid);
                }
            }
            SyncIncrement::SyncFinished(maybe_problem) => {
                self.reset_sync(status);
                self.compute_usage().await;
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
                    None => {
                        status.dirty_locally = self.local_changes().await;
                        if status.dirty_locally.is_empty() {
                            status.sync_status = self.get_last_synced_human().await.ok();
                        }
                        status.pending_shares = !self.get_pending_shares().await?.is_empty();
                    }
                    _ => {
                        error!("unexpected sync problem found {maybe_problem:?}");
                    }
                }
            }
        }

        self.events.status_updated();

        Ok(())
    }

    fn reset_sync(&self, status: &mut Status) {
        status.syncing = false;
        status.pulling_files.clear();
        status.pushing_files.clear();
        status.offline = false;
        status.update_required = false;
        status.out_of_space = false;
        status.sync_status = None;
    }
}

impl Default for SpaceUpdater {
    fn default() -> Self {
        Self { spawned: false, last_computed: Instant::now() }
    }
}
