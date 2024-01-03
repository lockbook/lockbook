use crate::output::{DirtynessMsg, WsOutput};
use crate::workspace::{Workspace, WsMsg};
use lb_rs::{CoreError, LbError, SyncProgress, SyncStatus};
use std::sync::atomic::Ordering;
use std::thread;

impl Workspace {
    // todo should anyone outside workspace ever call this? Or should they call something more
    // general that would allow workspace to determine if a sync is needed
    pub fn perform_sync(&self) {
        if self.syncing.load(Ordering::SeqCst) {
            return;
        }

        let syncing = self.syncing.clone();
        let core = self.core.clone();
        let update_tx = self.updates_tx.clone();
        let ctx = self.ctx.clone();

        thread::spawn(move || {
            syncing.store(true, Ordering::SeqCst);
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
            syncing.store(false, Ordering::SeqCst);
            update_tx.send(WsMsg::SyncDone(result)).unwrap();

            ctx.request_repaint();
        });
    }

    pub fn sync_message(&self, prog: SyncProgress, out: &mut WsOutput) {
        out.status.sync_progress = prog.progress as f32 / prog.total as f32;
        out.status.sync_message = Some(prog.msg);
    }

    pub fn sync_done(&self, outcome: Result<SyncStatus, LbError>, out: &mut WsOutput) {
        match outcome {
            Ok(_) => {}
            Err(err) => match err.kind {
                CoreError::ServerUnreachable => out.status.offline = true,
                CoreError::ClientUpdateRequired => out.status.update_req = true,
                CoreError::UsageIsOverDataCap => out.status.out_of_space = true,
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

            let dirty = DirtynessMsg { last_synced, dirty_files };

            update_tx.send(WsMsg::Dirtyness(dirty)).unwrap();
            ctx.request_repaint();
        });
    }

    pub fn dirty_msg(&self, dirt: DirtynessMsg, out: &mut WsOutput) {
        out.status.dirtyness = dirt;
    }
}
