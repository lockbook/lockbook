use crate::workspace::{Workspace, WsMsg, WsOutput};
use lb::{CoreError, LbError, SyncProgress, SyncStatus};
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
        out.sync_progress = prog.progress as f32 / prog.total as f32;
        out.message = Some(prog.msg);
    }

    pub fn sync_done(&self, outcome: Result<SyncStatus, LbError>, out: &mut WsOutput) {
        match outcome {
            Ok(_) => {}
            Err(err) => match err.kind {
                CoreError::ServerUnreachable => out.offline = true,
                CoreError::ClientUpdateRequired => out.update_req = true,
                CoreError::UsageIsOverDataCap => out.out_of_space = true,
                CoreError::Unexpected(msg) => out.error = Some(msg),
                _ => {}
            },
        }
    }
}
