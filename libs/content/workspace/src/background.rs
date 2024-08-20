use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use crate::workspace::WsMsg;

const AUTO_SAVE_INTERVAL: Duration = Duration::from_secs(1);
const SYNC_STATUS_INTERVAL: Duration = Duration::from_secs(1);

pub enum BwIncomingMsg {
    EguiUpdate,
    Tick,
    Shutdown,
}

pub enum Signal {
    Sync,
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

    user_last_seen: Instant,
}

impl Default for WorkerState {
    fn default() -> Self {
        Self {
            last_auto_save: Instant::now(),
            last_auto_sync: Instant::now(),
            user_last_seen: Instant::now(),
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

    fn target_sync_frequency(&self, now: &Instant) -> Duration {
        match now
            .duration_since(self.worker_state.user_last_seen)
            .as_secs()
        {
            // todo: revisit
            0..=59 => Duration::from_secs(10),
            60..=3600 => Duration::from_secs(60),
            _ => Duration::from_secs(3600),
        }
    }

    fn tick(&mut self) {
        let now = Instant::now();

        if now.duration_since(self.worker_state.last_auto_sync) > self.target_sync_frequency(&now) {
            self.worker_state.last_auto_sync = now;
            self.updates.send(WsMsg::BgSignal(Signal::Sync)).unwrap();
            self.ctx.request_repaint();
        }

        if now.duration_since(self.worker_state.last_auto_save) > AUTO_SAVE_INTERVAL {
            self.worker_state.last_auto_save = now;
            self.updates.send(WsMsg::BgSignal(Signal::SaveAll)).unwrap();
            self.ctx.request_repaint();
        }

        if now.duration_since(self.worker_state.last_sync_stat) > SYNC_STATUS_INTERVAL {
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
                BwIncomingMsg::EguiUpdate => {
                    // todo: is this wrong?
                    // todo: yes, as the events are loaded and processed on another thread in pretty fast succession
                    // unlikely they'll hang around long enough for you to notice them
                    if !self.ctx.input(|inp| inp.raw.events.is_empty()) {
                        self.worker_state.user_last_seen = Instant::now();
                    }
                }
            }
        }
    }
}
