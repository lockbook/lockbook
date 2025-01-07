use lb_rs::model::errors::LbErrKind;
use lb_rs::model::work_unit::WorkUnit;
use lb_rs::service::sync::SyncStatus;
use tracing::error;

use crate::output::DirtynessMsg;
use crate::task_manager::CompletedSync;
use crate::workspace::Workspace;
use std::time::Instant;

impl Workspace {
    // todo should anyone outside workspace ever call this? Or should they call something more
    // general that would allow workspace to determine if a sync is needed
    pub fn perform_sync(&mut self) {
        if self.status.sync_started.is_some() {
            return;
        }

        self.status.error = None;
        self.out.status_updated = true;
        self.status.sync_started = Some(Instant::now());

        self.tasks.queue_sync();
    }

    pub fn sync_done(&mut self, outcome: CompletedSync) {
        let CompletedSync { status_result, timing: _ } = outcome;

        self.out.status_updated = true;
        self.status.sync_started = None;
        self.last_sync = Some(Instant::now());
        match status_result {
            Ok(done) => {
                self.status.error = None;
                self.status.offline = false;
                self.refresh_sync_status();
                self.refresh_files(&done);
                self.out.sync_done = Some(done)
            }
            Err(err) => match err.kind {
                LbErrKind::ServerUnreachable => self.status.offline = true,
                LbErrKind::ClientUpdateRequired => self.status.update_req = true,
                LbErrKind::UsageIsOverDataCap => self.status.out_of_space = true,
                LbErrKind::Unexpected(msg) => self.out.error = Some(msg),
                _ => {
                    error!("Unhandled sync error: {:?}", err);
                    self.out.error = format!("{:?}", err).into();
                }
            },
        }
    }

    pub fn refresh_sync_status(&mut self) {
        let last_synced = self.core.get_last_synced_human_string().unwrap();
        let dirty_files = self.core.get_local_changes().unwrap();
        let pending_shares = self.core.get_pending_shares().unwrap();

        let dirty = DirtynessMsg { last_synced, dirty_files, pending_shares };

        self.out.status_updated = true;
        self.status.dirtyness = dirty;

        self.last_sync_status_refresh = Some(Instant::now());
    }

    pub fn refresh_files(&mut self, work: &SyncStatus) {
        let server_ids = work.work_units.iter().filter_map(|wu| match wu {
            WorkUnit::LocalChange { .. } => None,
            WorkUnit::ServerChange(id) => Some(*id),
        });

        for id in server_ids {
            for i in 0..self.tabs.len() {
                if self.tabs[i].id == id && !self.tabs[i].is_closing {
                    tracing::info!("Reloading file after sync: {}", id);
                    self.open_file(id, false, false);
                }
            }
        }
    }
}
