use std::collections::HashMap;
use std::sync::{mpsc, Arc, Mutex};
use std::time::Instant;
use std::{mem, thread};

use egui::Context;
use lb_rs::blocking::Lb;
use lb_rs::logic::crypto::DecryptedDocument;
use lb_rs::model::errors::LbResult;
use lb_rs::model::file_metadata::DocumentHmac;
use lb_rs::service::sync::{SyncProgress, SyncStatus};
use lb_rs::Uuid;

#[derive(Default)]
pub struct TaskManagerInner {
    // queue tasks then call `update` to launch them
    queued_loads: Vec<QueuedLoad>,
    queued_saves: Vec<QueuedSave>,
    queued_syncs: Vec<QueuedSync>,

    // launched tasks stored here until complete
    pub in_progress_loads: Vec<InProgressLoad>,
    pub in_progress_saves: Vec<InProgressSave>,
    pub in_progress_sync: Option<InProgressSync>,

    // completions stored here then returned in the response on the next frame
    completed_loads: Vec<CompletedLoad>,
    completed_saves: Vec<CompletedSave>,
    completed_sync: Option<CompletedSync>,
}

struct InnerResponse {
    loads_to_launch: Vec<QueuedLoad>,
    saves_to_launch: Vec<QueuedSave>,
    sync_to_launch: Option<QueuedSync>,

    completed_loads: Vec<CompletedLoad>,
    completed_saves: Vec<CompletedSave>,
    completed_sync: Option<CompletedSync>,
}

pub struct Response {
    pub completed_loads: Vec<CompletedLoad>,
    pub completed_saves: Vec<CompletedSave>,
    pub completed_sync: Option<CompletedSync>,
}

// Requests
#[derive(Clone)]
pub struct LoadRequest {
    pub id: Uuid,
    pub is_new_file: bool,
    pub tab_created: bool,
}

#[derive(Clone)]
pub struct SaveRequest {
    pub id: Uuid,
    pub old_hmac: Option<DocumentHmac>,
    pub seq: usize,
    pub content: String,
}

// Telemetry
#[derive(Clone, Copy)]
pub struct QueuedTelemetry {
    pub queued_at: Instant,
}

impl QueuedTelemetry {
    fn new() -> Self {
        Self { queued_at: Instant::now() }
    }
}

#[derive(Clone, Copy)]
pub struct InProgressTelemetry {
    pub queued_at: Instant,
    pub started_at: Instant,
}

impl InProgressTelemetry {
    fn new(queued: QueuedTelemetry) -> Self {
        Self { queued_at: queued.queued_at, started_at: Instant::now() }
    }
}

#[derive(Clone, Copy)]
pub struct CompletedTelemetry {
    pub queued_at: Instant,
    pub started_at: Instant,
    pub completed_at: Instant,
}

impl CompletedTelemetry {
    fn new(in_progress: InProgressTelemetry) -> Self {
        Self {
            queued_at: in_progress.queued_at,
            started_at: in_progress.started_at,
            completed_at: Instant::now(),
        }
    }
}

// Queued, in-progress, and completed tasks
#[derive(Clone)]
struct QueuedLoad {
    request: LoadRequest,

    telemetry: QueuedTelemetry,
}

#[derive(Clone)]
struct QueuedSave {
    request: SaveRequest,

    telemetry: QueuedTelemetry,
}

#[derive(Clone)]
struct QueuedSync {
    telemetry: QueuedTelemetry,
}

pub struct InProgressLoad {
    pub request: LoadRequest,

    pub telemetry: InProgressTelemetry,
}

impl InProgressLoad {
    fn new(queued: QueuedLoad) -> Self {
        Self { request: queued.request, telemetry: InProgressTelemetry::new(queued.telemetry) }
    }
}

pub struct InProgressSave {
    pub request: SaveRequest,

    pub telemetry: InProgressTelemetry,
}

impl InProgressSave {
    fn new(queued: QueuedSave) -> Self {
        Self { request: queued.request, telemetry: InProgressTelemetry::new(queued.telemetry) }
    }
}

pub struct InProgressSync {
    pub progress: mpsc::Receiver<SyncProgress>,

    pub telemetry: InProgressTelemetry,
}

impl InProgressSync {
    fn new(queued: QueuedSync, progress: mpsc::Receiver<SyncProgress>) -> Self {
        Self { progress, telemetry: InProgressTelemetry::new(queued.telemetry) }
    }
}

pub struct CompletedLoad {
    pub request: LoadRequest,
    pub content_result: LbResult<(Option<DocumentHmac>, DecryptedDocument)>,

    pub telemetry: CompletedTelemetry,
}

pub struct CompletedSave {
    pub request: SaveRequest,
    pub new_hmac_result: LbResult<DocumentHmac>,

    pub telemetry: CompletedTelemetry,
}

pub struct CompletedSync {
    pub status_result: LbResult<SyncStatus>,

    pub telemetry: CompletedTelemetry,
}

impl TaskManagerInner {
    /// Queues a load for the given file. A call to [`update`] will launch the task when it is ready and a later
    /// call to [`update`] will return the result of the task when it completes. Queued loads of the same file are
    /// coalesced into a single load task.
    fn queue_load(&mut self, request: LoadRequest) {
        self.queued_loads
            .push(QueuedLoad { request, telemetry: QueuedTelemetry::new() });
    }

    /// Queues a save for the given file. A call to [`update`] will launch the task when it is ready and a later
    /// call to [`update`] will return the result of the task when it completes. Queued saves of the same file are
    /// coalesced into a single save task.
    fn queue_save(&mut self, request: SaveRequest) {
        self.queued_saves
            .push(QueuedSave { request, telemetry: QueuedTelemetry::new() });
    }

    /// Queues a sync task. A call to [`update`] will launch the task when it is ready and a later call to
    /// [`update`] will return the result of the task when it completes. Queued syncs are coalesced into a single
    /// sync task.
    fn queue_sync(&mut self) {
        self.queued_syncs
            .push(QueuedSync { telemetry: QueuedTelemetry::new() });
    }

    /// Launches whichever queued tasks are ready to be launched, moving their status from queued to in-progress.
    /// In-progress tasks have status moved to completed by background threads. Returns the results of tasks
    /// that have completed since the last call to `update`.
    fn update(&mut self) -> InnerResponse {
        // Prioritize loads over saves because when they are both queued, it's likely because a sync pulled updates to
        // a file that was open and modified by the user. The save will fail via the safe_write mechanism until the new
        // sync-pulled version is merged into the user-modified version. The other order would be safe but inefficient.
        let mut ids_to_load = Vec::new();
        for queued_load in &self.queued_loads {
            let id = queued_load.request.id;
            if self.load_or_save_in_progress(id) {
                continue;
            }
            ids_to_load.push(id);
        }

        let mut ids_to_save = Vec::new();
        for queued_save in &self.queued_saves {
            let id = queued_save.request.id;
            if self.load_or_save_in_progress(id) {
                continue;
            }
            ids_to_save.push(id);
        }

        // Syncs don't need to be prioritized because they don't conflict with each other or with loads or saves. For
        // efficiency, we wait for all saves to complete before we launch a sync. A save always queues a sync upon
        // completion.
        let should_sync = !self.queued_syncs.is_empty()
            && self.in_progress_sync.is_none()
            && !self.any_load_or_save_queued_or_in_progress();

        // Get launched things from the queue and remove duplicates (avoiding clones)
        let mut loads_to_launch = HashMap::new();
        let mut saves_to_launch = HashMap::new();

        for load in mem::take(&mut self.queued_loads) {
            if ids_to_load.contains(&load.request.id) {
                loads_to_launch.insert(load.request.id, load); // use latest of duplicate ids
            } else {
                self.queued_loads.push(load); // put back the ones we're not launching
            }
        }
        for save in mem::take(&mut self.queued_saves) {
            if ids_to_save.contains(&save.request.id) {
                saves_to_launch.insert(save.request.id, save); // use latest of duplicate ids
            } else {
                self.queued_saves.push(save); // put back the ones we're not launching
            }
        }

        let sync_to_launch =
            if should_sync { mem::take(&mut self.queued_syncs).into_iter().next() } else { None };

        // Actual launching happens in `impl TaskManagerExt for Arc<Mutex<TaskManager>>` block because it needs cloned Arc
        InnerResponse {
            loads_to_launch: loads_to_launch.into_values().collect(),
            saves_to_launch: saves_to_launch.into_values().collect(),
            sync_to_launch,
            completed_loads: mem::take(&mut self.completed_loads),
            completed_saves: mem::take(&mut self.completed_saves),
            completed_sync: mem::take(&mut self.completed_sync),
        }
    }

    fn load_or_save_queued(&self, id: Uuid) -> bool {
        let load_queued = self
            .queued_loads
            .iter()
            .any(|queued_load| queued_load.request.id == id);
        let save_queued = self
            .queued_saves
            .iter()
            .any(|queued_save| queued_save.request.id == id);
        load_queued || save_queued
    }

    fn load_or_save_in_progress(&self, id: Uuid) -> bool {
        let save_in_progress = self
            .in_progress_saves
            .iter()
            .any(|in_progress_save| in_progress_save.request.id == id);
        let load_in_progress = self
            .in_progress_loads
            .iter()
            .any(|in_progress_load| in_progress_load.request.id == id);
        save_in_progress || load_in_progress
    }

    fn any_load_or_save_queued_or_in_progress(&self) -> bool {
        !self.queued_loads.is_empty()
            || !self.queued_saves.is_empty()
            || !self.in_progress_loads.is_empty()
            || !self.in_progress_saves.is_empty()
    }
}

#[derive(Clone)]
pub struct TaskManager(pub Arc<Mutex<TaskManagerInner>>);

impl TaskManager {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(TaskManagerInner::default())))
    }

    pub fn queue_load(&mut self, request: LoadRequest) {
        let mut tasks = self.0.lock().unwrap();
        tasks.queue_load(request);
    }

    pub fn queue_save(&mut self, request: SaveRequest) {
        let mut tasks = self.0.lock().unwrap();
        tasks.queue_save(request);
    }

    pub fn queue_sync(&mut self) {
        let mut tasks = self.0.lock().unwrap();
        tasks.queue_sync();
    }

    pub fn load_or_save_queued(&self, id: Uuid) -> bool {
        let tasks = self.0.lock().unwrap();
        tasks.load_or_save_queued(id)
    }

    pub fn load_or_save_in_progress(&self, id: Uuid) -> bool {
        let tasks = self.0.lock().unwrap();
        tasks.load_or_save_in_progress(id)
    }

    pub fn update(&mut self, ctx: &Context, core: &Lb) -> Response {
        let mut tasks = self.0.lock().unwrap();
        let InnerResponse {
            loads_to_launch,
            saves_to_launch,
            sync_to_launch,
            completed_loads,
            completed_saves,
            completed_sync,
        } = tasks.update();

        for queued_load in loads_to_launch {
            let request = queued_load.request.clone();
            let in_progress_load = InProgressLoad::new(queued_load);
            tasks.in_progress_loads.push(in_progress_load);

            let self_clone = self.clone();
            let core = core.clone();
            let ctx = ctx.clone();
            thread::spawn(move || {
                let id = request.id;
                let content_result = core.read_document_with_hmac(id);

                let mut tasks = self_clone.0.lock().unwrap();

                let mut in_progress_load = None;
                for load in mem::take(&mut tasks.in_progress_loads) {
                    if load.request.id == id {
                        in_progress_load = Some(load); // use latest of duplicate ids
                        break;
                    } else {
                        tasks.in_progress_loads.push(load); // put back the ones we're not completing
                    }
                }
                let in_progress_load = in_progress_load
                    .expect("Failed to find in-progress entry for load that just completed");
                // ^ above error may indicate concurrent loads to the same file, which would cause problems

                let completed_load = CompletedLoad {
                    request: in_progress_load.request,
                    content_result,
                    telemetry: CompletedTelemetry::new(in_progress_load.telemetry),
                };
                tasks.completed_loads.push(completed_load);

                ctx.request_repaint();
            });
        }

        for queued_save in saves_to_launch {
            // content cloned; one copy sent to disk and other retained in UI to represent on-disk version for merge
            // first step to alleviate: https://github.com/lockbook/lockbook/issues/3241
            let request = queued_save.request.clone();
            let in_progress_save = InProgressSave::new(queued_save);
            tasks.in_progress_saves.push(in_progress_save);

            let self_clone = self.clone();
            let core = core.clone();
            let ctx = ctx.clone();
            thread::spawn(move || {
                let id = request.id;
                let new_hmac_result =
                    core.safe_write(request.id, request.old_hmac, request.content.into());

                let mut tasks = self_clone.0.lock().unwrap();

                let mut in_progress_save = None;
                for save in mem::take(&mut tasks.in_progress_saves) {
                    if save.request.id == id {
                        in_progress_save = Some(save); // use latest of duplicate ids
                        break;
                    } else {
                        tasks.in_progress_saves.push(save); // put back the ones we're not completing
                    }
                }
                let in_progress_save = in_progress_save
                    .expect("Failed to find in-progress entry for save that just completed");
                // ^ above error may indicate concurrent saves to the same file, which would cause problems

                let completed_save = CompletedSave {
                    request: in_progress_save.request,
                    new_hmac_result,
                    telemetry: CompletedTelemetry::new(in_progress_save.telemetry),
                };
                tasks.completed_saves.push(completed_save);

                ctx.request_repaint();
            });
        }

        if let Some(sync) = sync_to_launch {
            let (sender, receiver) = mpsc::channel();
            let in_progress_sync = InProgressSync::new(sync, receiver);
            tasks.in_progress_sync = Some(in_progress_sync);

            let self_clone = self.clone();
            let core = core.clone();
            let ctx = ctx.clone();
            thread::spawn(move || {
                let status_result = {
                    let ctx = ctx.clone();
                    let progress_closure = move |p| {
                        sender.send(p).unwrap();
                        ctx.request_repaint();
                    };
                    core.sync(Some(Box::new(progress_closure)))
                };

                let mut tasks = self_clone.0.lock().unwrap();
                let in_progress_sync = tasks
                    .in_progress_sync
                    .take()
                    .expect("Failed to find in-progress entry for sync that just completed");
                // ^ above error may indicate concurrent syncs, which would cause problems

                let completed_sync = CompletedSync {
                    status_result,
                    telemetry: CompletedTelemetry::new(in_progress_sync.telemetry),
                };
                tasks.completed_sync = Some(completed_sync);

                ctx.request_repaint();
            });
        }

        Response { completed_loads, completed_saves, completed_sync }
    }
}
