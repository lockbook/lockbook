use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use crate::workspace::WsMsg;

pub enum BwIncomingMsg {
    Tick,
    Shutdown,
}

pub enum Signal {
    MaybeSync,
    SaveAll,
    UpdateStatus,
    BwDone,
}

#[derive(Clone)]
pub struct BackgroundWorker {
    ctx: egui::Context,
    updates: mpsc::Sender<WsMsg>,

    worker_state: WorkerState,
}

#[derive(Clone)]
struct WorkerState {
    last_auto_save: Instant,
    last_auto_sync: Instant,
    last_sync_stat: Instant,
}

impl Default for WorkerState {
    fn default() -> Self {
        Self {
            last_auto_save: Instant::now(),
            last_auto_sync: Instant::now(),
            last_sync_stat: Instant::now(),
        }
    }
}

impl BackgroundWorker {
    pub fn new(ctx: &egui::Context, updates: &mpsc::Sender<WsMsg>) -> Self {
        let ws = Default::default();

        Self { ctx: ctx.clone(), updates: updates.clone(), worker_state: ws }
    }

    pub fn spawn_worker(&self) -> mpsc::Sender<BwIncomingMsg> {
        let (back_tx, back_rx) = mpsc::channel();

        let timer = back_tx.clone();
        thread::spawn(move || loop {
            thread::sleep(Duration::from_secs(1));
            if timer.send(BwIncomingMsg::Tick).is_err() {
                // probably we shut down
                return;
            }
        });

        let thread_self = self.clone();
        thread::spawn(move || thread_self.event_loop(back_rx));

        back_tx
    }

    fn tick(&mut self) {
        let now = Instant::now();

        if now.duration_since(self.worker_state.last_auto_sync) > Duration::from_secs(1) {
            self.worker_state.last_auto_sync = now;
            self.updates
                .send(WsMsg::BgSignal(Signal::MaybeSync))
                .unwrap();
            self.ctx.request_repaint();
        }

        // note: saving all files is a no-op for files that aren't dirty so this is cheap to do often
        // note: saving a dirty file triggers a sync, so this controls the latency for pushing local changes
        if now.duration_since(self.worker_state.last_auto_save) > Duration::from_secs(1) {
            self.worker_state.last_auto_save = now;
            self.updates.send(WsMsg::BgSignal(Signal::SaveAll)).unwrap();
            self.ctx.request_repaint();
        }

        if now.duration_since(self.worker_state.last_sync_stat) > Duration::from_secs(1) {
            self.worker_state.last_sync_stat = now;
            self.updates
                .send(WsMsg::BgSignal(Signal::UpdateStatus))
                .unwrap();
            self.ctx.request_repaint();
        }
    }

    fn event_loop(mut self, rx: mpsc::Receiver<BwIncomingMsg>) {
        while let Ok(req) = rx.recv() {
            match req {
                BwIncomingMsg::Tick => self.tick(),
                BwIncomingMsg::Shutdown => {
                    self.updates.send(WsMsg::BgSignal(Signal::BwDone)).unwrap();
                    self.ctx.request_repaint();
                    return;
                }
            }
        }
    }
}
