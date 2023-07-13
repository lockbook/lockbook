use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use eframe::egui;

use super::AccountUpdate;

const AUTO_SAVE_INTERVAL: Duration = Duration::from_secs(2);
const SYNC_STATUS_INTERVAL: Duration = Duration::from_secs(60);

pub enum BackgroundEvent {
    EguiUpdate,
    Tick,
    Shutdown,
}

#[derive(Clone)]
pub struct BackgroundWorker {
    ctx: egui::Context,
    updates: mpsc::Sender<AccountUpdate>,

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
    pub fn new(ctx: &egui::Context, updates: &mpsc::Sender<AccountUpdate>) -> Self {
        let ws = Default::default();

        Self { ctx: ctx.clone(), updates: updates.clone(), worker_state: ws }
    }

    pub fn spawn_worker(&self) -> mpsc::Sender<BackgroundEvent> {
        let (back_tx, back_rx) = mpsc::channel();

        let timer = back_tx.clone();
        thread::spawn(move || loop {
            thread::sleep(Duration::from_secs(1));
            if timer.send(BackgroundEvent::Tick).is_err() {
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
            0..=59 => Duration::from_secs(5),
            60..=3600 => Duration::from_secs(240),
            _ => Duration::from_secs(3600),
        }
    }

    fn tick(&mut self) {
        let now = Instant::now();

        if now.duration_since(self.worker_state.last_auto_sync) > self.target_sync_frequency(&now) {
            self.worker_state.last_auto_sync = now;
            self.updates.send(AccountUpdate::AutoSyncSignal).unwrap();
            self.ctx.request_repaint();
        }

        if now.duration_since(self.worker_state.last_auto_save) > AUTO_SAVE_INTERVAL {
            self.worker_state.last_auto_save = now;
            self.updates.send(AccountUpdate::AutoSaveSignal).unwrap();
            self.ctx.request_repaint();
        }

        if now.duration_since(self.worker_state.last_sync_stat) > SYNC_STATUS_INTERVAL {
            self.worker_state.last_sync_stat = now;
            self.updates.send(AccountUpdate::SyncStatusSignal).unwrap();
            self.ctx.request_repaint();
        }
    }

    fn event_loop(mut self, rx: mpsc::Receiver<BackgroundEvent>) {
        while let Ok(req) = rx.recv() {
            match req {
                BackgroundEvent::Tick => self.tick(),
                BackgroundEvent::Shutdown => {
                    self.updates
                        .send(AccountUpdate::BackgroundWorkerDone)
                        .unwrap();
                    self.ctx.request_repaint();
                    return;
                }
                BackgroundEvent::EguiUpdate => {
                    if !self.ctx.input(|inp| inp.raw.events.is_empty()) {
                        self.worker_state.user_last_seen = Instant::now();
                    }
                }
            }
        }
    }
}
