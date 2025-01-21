use lb_rs::model::errors::LbErrKind;
use lb_rs::model::work_unit::WorkUnit;
use lb_rs::service::sync::SyncStatus;
use tracing::{debug, error};

use crate::task_manager::{CompletedSync, CompletedSyncStatusUpdate};
use crate::workspace::Workspace;

impl Workspace {
    pub fn sync_done(&mut self, outcome: CompletedSync) {
        let CompletedSync { status_result, timing } = outcome;

        self.out.status_updated = true;
        self.last_sync_completed = Some(timing.completed_at);
        match status_result {
            Ok(done) => {
                self.status.sync_error = None;
                self.status.sync_message = None;

                self.tasks.queue_sync_status_update();
                self.tasks.queue_file_cache_refresh();
                self.refresh_files(&done);
                self.out.sync_done = Some(done);
            }
            Err(err) => match err.kind {
                LbErrKind::ServerUnreachable => self.status.offline = true,
                LbErrKind::ClientUpdateRequired => self.status.update_req = true,
                LbErrKind::UsageIsOverDataCap => self.status.out_of_space = true,
                LbErrKind::Unexpected(msg) => self.status.sync_error = Some(msg),
                _ => {
                    error!("Unhandled sync error: {:?}", err);
                    self.status.sync_error = format!("{:?}", err).into();
                }
            },
        }
    }

    fn refresh_files(&mut self, work: &SyncStatus) {
        let server_ids = work.work_units.iter().filter_map(|wu| match wu {
            WorkUnit::LocalChange { .. } => None,
            WorkUnit::ServerChange(id) => Some(*id),
        });

        for id in server_ids {
            for i in 0..self.tabs.len() {
                if self.tabs[i].id == id && !self.tabs[i].is_closing {
                    debug!("Reloading file after sync: {}", id);
                    self.open_file(id, false, false);
                }
            }
        }
    }

    pub fn sync_status_update_done(&mut self, outcome: CompletedSyncStatusUpdate) {
        let CompletedSyncStatusUpdate { status_result, timing } = outcome;

        self.out.status_updated = true;
        self.last_sync_status_refresh_completed = Some(timing.completed_at);
        match status_result {
            Ok(dirtyness) => {
                self.status.dirtyness = dirtyness;
                self.status.sync_status_update_error = None;
            }
            Err(err) => {
                error!("Unhandled sync status update error: {:?}", err);
                self.status.sync_status_update_error = format!("{:?}", err).into();
            }
        }
    }
}
