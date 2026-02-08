use std::sync::Arc;
use web_time::Duration;

use tokio::sync::{Mutex, RwLock};
use uuid::Uuid;
use web_time::Instant;

use crate::model::errors::{LbErrKind, LbResult, Unexpected};
use crate::service::events::Event;
use crate::service::sync::SyncIncrement;
use crate::service::usage::UsageMetrics;
use crate::{Lb, tokio_spawn};

#[derive(Clone, Default)]
pub struct StatusUpdater {
    current_status: Arc<RwLock<Status>>,
    space_updated: Arc<Mutex<SpaceUpdater>>,
}

/// rate limit get_usage calls to once every 60 seconds or so
pub struct SpaceUpdater {
    initialized: bool,
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
#[derive(Default, Clone, Debug)]
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

    pub unexpected_sync_problem: Option<String>,
}

impl Status {
    pub fn msg(&self) -> Option<String> {
        if self.syncing {
            return Some("Syncing...".to_string());
        }

        if self.offline {
            if !self.dirty_locally.is_empty() {
                let len = self.dirty_locally.len();
                return Some(format!(
                    "Offline, {} change{} unsynced.",
                    len,
                    if len > 1 { "s" } else { "" }
                ));
            }

            if let Some(last_synced) = &self.sync_status {
                return Some(format!("Offline, last synced: {last_synced}"));
            }

            return Some("Offline.".to_string());
        }

        if self.out_of_space {
            return Some("You're out of space!".to_string());
        }

        if self.update_required {
            return Some("An update is required to continue.".to_string());
        }

        if let Some(err) = &self.unexpected_sync_problem {
            return Some(err.to_string());
        }

        if !self.dirty_locally.is_empty() {
            let dirty_locally = self.dirty_locally.len();
            return Some(format!("{dirty_locally} changes unsynced"));
        }

        if let Some(last_synced) = &self.sync_status {
            return Some(format!("Last synced: {last_synced}"));
        }

        None
    }
}

impl Lb {
    pub async fn status(&self) -> Status {
        self.status.current_status.read().await.clone()
    }

    pub async fn set_initial_state(&self) -> LbResult<()> {
        if self.keychain.get_account().is_ok() {
            self.spawn_compute_usage().await;
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

        tokio_spawn!(async move {
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
        let current = self.status.current_status.read().await.clone();
        match e {
            Event::MetadataChanged | Event::DocumentWritten(_, _) => {
                self.compute_dirty_locally(current).await?;
            }
            Event::Sync(s) => self.update_sync(s, current).await?,
            _ => {}
        }
        Ok(())
    }

    async fn set_status(&self, status: Status) -> LbResult<()> {
        *self.status.current_status.write().await = status;
        self.events.status_updated();
        Ok(())
    }

    async fn compute_dirty_locally(&self, mut status: Status) -> LbResult<()> {
        let new = self.local_changes().await;
        if new != status.dirty_locally {
            status.dirty_locally = self.local_changes().await;
            self.set_status(status).await?;
        }
        Ok(())
    }

    async fn spawn_compute_usage(&self) {
        let mut lock = self.status.space_updated.lock().await;
        if lock.spawned {
            return;
        }
        let initialized = lock.initialized;
        lock.spawned = true;
        lock.initialized = true;
        let computed = lock.last_computed;
        drop(lock);

        let bg = self.clone();
        tokio_spawn!(async move {
            if initialized && computed.elapsed() < Duration::from_secs(60) {
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

    async fn update_sync(&self, s: SyncIncrement, mut status: Status) -> LbResult<()> {
        match s {
            SyncIncrement::SyncStarted => {
                self.reset_in_flight_sync(&mut status);
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
                // Clear prior sync outcomes here (not at start) so errors persist until a new completed sync
                self.reset_sync_outcome(&mut status);

                self.spawn_compute_usage().await;
                status.dirty_locally = self.local_changes().await;
                if status.dirty_locally.is_empty() {
                    status.sync_status = self.get_last_synced_human().await.ok();
                }
                // @smailbarkouch has requested that this be a Vec<Uuid> instead of a bool
                // we also could consume the PendingSharesChanged event
                status.pending_shares = !self.get_pending_shares().await?.is_empty();
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
                    None => {}
                    Some(e) => {
                        status.unexpected_sync_problem =
                            Some(format!("unexpected error {e:?}: {e}"));
                        error!("unexpected error {e:?}: {e}");
                    }
                }
            }
        }

        self.set_status(status).await?;

        Ok(())
    }

    fn reset_in_flight_sync(&self, status: &mut Status) {
        status.syncing = false;
        status.pulling_files.clear();
        status.pushing_files.clear();
        status.sync_status = None;
        status.unexpected_sync_problem = None;
    }

    fn reset_sync_outcome(&self, status: &mut Status) {
        status.offline = false;
        status.update_required = false;
        status.out_of_space = false;
    }
}

impl Default for SpaceUpdater {
    fn default() -> Self {
        Self { spawned: false, last_computed: Instant::now(), initialized: false }
    }
}
