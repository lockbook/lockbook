use crate::output::{DirtynessMsg, WsOutput};
use crate::workspace::{Workspace, WsMsg};
use lb_rs::{CoreError, LbError, SyncProgress, SyncStatus};
use std::thread;

impl Workspace {
    // todo should anyone outside workspace ever call this? Or should they call something more
    // general that would allow workspace to determine if a sync is needed
    pub fn perform_sync(&mut self) {
        if self.pers_status.syncing {
            return;
        }

        self.pers_status.syncing = true;

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

            println!("sync called");
            let result = core.sync(Some(Box::new(closure)));
            update_tx.send(WsMsg::SyncDone(result)).unwrap();

            println!("sync done");

            ctx.request_repaint();
        });
    }

    pub fn sync_message(&mut self, prog: SyncProgress) {
        self.pers_status.sync_progress = prog.progress as f32 / prog.total as f32;
        self.pers_status.sync_message = Some(prog.msg);
    }

    pub fn sync_done(&mut self, outcome: Result<SyncStatus, LbError>, out: &mut WsOutput) {
        self.pers_status.syncing = false;
        match outcome {
            Ok(done) => {
                self.refresh_sync_status();
                out.sync_done = Some(done)
            }
            Err(err) => match err.kind {
                CoreError::ServerUnreachable => self.pers_status.offline = true,
                CoreError::ClientUpdateRequired => self.pers_status.update_req = true,
                CoreError::UsageIsOverDataCap => self.pers_status.out_of_space = true,
                CoreError::Unexpected(msg) => out.error = Some(msg),
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

    pub fn dirty_msg(&mut self, dirt: DirtynessMsg) {
        self.pers_status.dirtyness = dirt;
    }
}
