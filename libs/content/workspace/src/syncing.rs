use crate::{
    output::DirtynessMsg,
    workspace::{Workspace, WsMsg},
};
use lb_rs::{CoreError, LbError, SyncProgress, SyncStatus};
use std::thread;

impl Workspace {
    // todo should anyone outside workspace ever call this? Or should they call something more
    // general that would allow workspace to determine if a sync is needed
    pub fn perform_sync(&mut self) {
        // todo: save all dirty tabs

        if self.status.syncing {
            return;
        }

        self.status.error = None;
        self.out.status_updated = true;
        self.status.syncing = true;

        let core = self.core.clone();
        let update_tx = self.updates_tx.clone();
        let ctx = self.ctx.clone();

        thread::spawn(move || {
            ctx.request_repaint();

            let closure = {
                let update_tx = update_tx.clone();
                let ctx = ctx.clone();

                move |p: SyncProgress| {
                    update_tx.send(WsMsg::SyncMsg(p)).unwrap();
                    ctx.request_repaint();
                }
            };

            let result = core.sync(Some(Box::new(closure)));
            update_tx.send(WsMsg::SyncDone(result)).unwrap();

            ctx.request_repaint();
        });
    }

    pub fn sync_message(&mut self, prog: SyncProgress) {
        self.out.status_updated = true;
        self.status.sync_progress = prog.progress as f32 / prog.total as f32;
        self.status.sync_message = Some(prog.msg);
    }

    pub fn sync_done(&mut self, outcome: Result<SyncStatus, LbError>) {
        self.out.status_updated = true;
        self.status.syncing = false;
        match outcome {
            Ok(done) => {
                self.status.error = None;
                self.status.offline = false;
                self.refresh_sync_status();
                self.refresh_files(&done);
                self.out.sync_done = Some(done)
            }
            Err(err) => match err.kind {
                CoreError::ServerUnreachable => self.status.offline = true,
                CoreError::ClientUpdateRequired => self.status.update_req = true,
                CoreError::UsageIsOverDataCap => self.status.out_of_space = true,
                CoreError::Unexpected(msg) => self.out.error = Some(msg),
                _ => {}
            },
        }
    }

    pub fn refresh_sync_status(&self) {
        let core = self.core.clone();
        let update_tx = self.updates_tx.clone();
        let ctx = self.ctx.clone();

        thread::spawn(move || {
            let last_synced = core.get_last_synced_human_string().unwrap();
            let dirty_files = core.get_local_changes().unwrap();
            let pending_shares = core.get_pending_shares().unwrap();

            let dirty = DirtynessMsg { last_synced, dirty_files, pending_shares };

            update_tx.send(WsMsg::Dirtyness(dirty)).unwrap();
            ctx.request_repaint();
        });
    }

    pub fn refresh_files(&mut self, work: &SyncStatus) {
        let server_ids = work.work_units.iter().filter_map(|wu| match wu {
            lb_rs::WorkUnit::LocalChange { .. } => None,
            lb_rs::WorkUnit::ServerChange(id) => Some(*id),
        });

        for id in server_ids {
            if self.tabs.iter().any(|t| t.id == id) {
                self.open_file(id, false, false);
            }
        }
    }

    pub fn dirty_msg(&mut self, dirt: DirtynessMsg) {
        self.out.status_updated = true;
        self.status.dirtyness = dirt;
    }
}
