use std::collections::HashMap;
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};
use std::{mem, thread};

use egui::Context;
use lb_rs::blocking::Lb;
use lb_rs::logic::crypto::DecryptedDocument;
use lb_rs::model::errors::LbResult;
use lb_rs::model::file_metadata::DocumentHmac;
use lb_rs::service::sync::{SyncProgress, SyncStatus};
use lb_rs::Uuid;
use tracing::{debug, error, info, warn};

use crate::tab::{Tab, TabContent};

#[derive(Default)]
pub struct Tasks {
    // queued tasks launch when ready with no follow-up required
    queued_loads: Vec<QueuedLoad>,
    queued_saves: Vec<QueuedSave>,
    queued_syncs: Vec<QueuedSync>,

    // launched tasks tracked here until complete
    pub in_progress_loads: Vec<InProgressLoad>,
    pub in_progress_saves: Vec<InProgressSave>,
    pub in_progress_sync: Option<InProgressSync>,

    // completions stashed here then returned in the response on the next frame
    completed_loads: Vec<CompletedLoad>,
    completed_saves: Vec<CompletedSave>,
    completed_sync: Option<CompletedSync>,
}

impl Tasks {
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
}

// Timing
#[derive(Clone, Copy)]
pub struct QueuedTiming {
    pub queued_at: Instant,
}

impl QueuedTiming {
    fn new() -> Self {
        Self { queued_at: Instant::now() }
    }
}

#[derive(Clone, Copy)]
pub struct InProgressTiming {
    pub queued_at: Instant,
    pub started_at: Instant,
}

impl InProgressTiming {
    fn new(queued: QueuedTiming) -> Self {
        Self { queued_at: queued.queued_at, started_at: Instant::now() }
    }
}

#[derive(Clone, Copy)]
pub struct CompletedTiming {
    pub queued_at: Instant,
    pub started_at: Instant,
    pub completed_at: Instant,
}

impl CompletedTiming {
    fn new(in_progress: InProgressTiming) -> Self {
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

    timing: QueuedTiming,
}

#[derive(Clone)]
struct QueuedSave {
    request: SaveRequest,

    timing: QueuedTiming,
}

#[derive(Clone)]
struct QueuedSync {
    timing: QueuedTiming,
}

pub struct InProgressLoad {
    pub request: LoadRequest,

    pub timing: InProgressTiming,
}

impl InProgressLoad {
    fn new(queued: QueuedLoad) -> Self {
        Self { request: queued.request, timing: InProgressTiming::new(queued.timing) }
    }
}

pub struct InProgressSave {
    pub request: SaveRequest,

    pub timing: InProgressTiming,
}

impl InProgressSave {
    fn new(queued: QueuedSave) -> Self {
        Self { request: queued.request, timing: InProgressTiming::new(queued.timing) }
    }
}

pub struct InProgressSync {
    pub progress: mpsc::Receiver<SyncProgress>,

    pub timing: InProgressTiming,
}

impl InProgressSync {
    fn new(queued: QueuedSync, progress: mpsc::Receiver<SyncProgress>) -> Self {
        Self { progress, timing: InProgressTiming::new(queued.timing) }
    }
}

pub struct CompletedLoad {
    pub request: LoadRequest,
    pub content_result: LbResult<(Option<DocumentHmac>, DecryptedDocument)>,

    pub timing: CompletedTiming,
}

pub struct CompletedSave {
    pub request: SaveRequest,
    pub seq: usize,
    pub content: String,
    pub new_hmac_result: LbResult<DocumentHmac>,

    pub timing: CompletedTiming,
}

pub struct CompletedSync {
    pub status_result: LbResult<SyncStatus>,

    pub timing: CompletedTiming,
}

#[derive(Clone)]
pub struct TaskManager {
    pub tasks: Arc<Mutex<Tasks>>,
    core: Lb,
    ctx: Context,
}

impl TaskManager {
    pub fn new(core: Lb, ctx: Context) -> Self {
        Self { tasks: Default::default(), core, ctx }
    }

    pub fn queue_load(&mut self, request: LoadRequest) {
        debug!("queued load of file {}", request.id);
        self.tasks
            .lock()
            .unwrap()
            .queued_loads
            .push(QueuedLoad { request, timing: QueuedTiming::new() });
    }

    pub fn queue_save(&mut self, request: SaveRequest) {
        debug!("queued save of file {}", request.id);
        self.tasks
            .lock()
            .unwrap()
            .queued_saves
            .push(QueuedSave { request, timing: QueuedTiming::new() });
    }

    pub fn queue_sync(&mut self) {
        debug!("queued sync");
        self.tasks
            .lock()
            .unwrap()
            .queued_syncs
            .push(QueuedSync { timing: QueuedTiming::new() });
    }

    pub fn load_or_save_queued(&self, id: Uuid) -> bool {
        self.tasks.lock().unwrap().load_or_save_queued(id)
    }

    pub fn load_or_save_in_progress(&self, id: Uuid) -> bool {
        self.tasks.lock().unwrap().load_or_save_in_progress(id)
    }

    pub fn save_queued_at(&self, id: Uuid) -> Option<Instant> {
        let tasks = self.tasks.lock().unwrap();
        if let Some(queued_save) = tasks
            .queued_saves
            .iter()
            .find(|queued_save| queued_save.request.id == id)
        {
            Some(queued_save.timing.queued_at)
        } else if let Some(in_progress_save) = tasks
            .in_progress_saves
            .iter()
            .find(|in_progress_save| in_progress_save.request.id == id)
        {
            Some(in_progress_save.timing.queued_at)
        } else if let Some(completed_save) = tasks
            .completed_saves
            .iter()
            .find(|completed_save| completed_save.request.id == id)
        {
            Some(completed_save.timing.queued_at)
        } else {
            None
        }
    }

    /// Launches whichever queued tasks are ready to be launched, moving their status from queued to in-progress.
    /// In-progress tasks have status moved to completed by background threads. This fn called whenever a task is
    /// queued or explicitly - background threads will not call it and will instead only call request_repaint() when
    /// done - so it's the UI's responsibility to check in on it from time-to-time. This is necessary so that the UI
    /// can interject between tasks that are queued and tasks that they are queued behind i.e. to provide the latest
    /// hmac and file content so that a save succeeds.
    pub fn check_launch(&self, tabs: &[Tab]) {
        let mut tasks = self.tasks.lock().unwrap();

        // Prioritize loads over saves because when they are both queued, it's likely because a sync pulled updates to
        // a file that was open and modified by the user. The save will fail via the safe_write mechanism until the new
        // sync-pulled version is merged into the user-modified version. The other order would be safe but inefficient.
        let mut ids_to_load = Vec::new();
        for queued_load in &tasks.queued_loads {
            let id = queued_load.request.id;
            if tasks.load_or_save_in_progress(id) {
                continue;
            }
            ids_to_load.push(id);
        }

        let mut ids_to_save = Vec::new();
        for queued_save in &tasks.queued_saves {
            let id = queued_save.request.id;
            if tasks.load_or_save_in_progress(id) {
                continue;
            }
            if tasks
                .completed_saves
                .iter()
                .any(|completed_save| completed_save.request.id == id)
            {
                continue; // result of completed save must be processed before another save to the same file
            }
            ids_to_save.push(id);
        }

        // Syncs don't need to be prioritized because they don't conflict with each other or with loads or saves. For
        // efficiency, we wait for all saves to complete before we launch a sync. A save always queues a sync upon
        // completion.
        let should_sync = !tasks.queued_syncs.is_empty()
            && tasks.in_progress_sync.is_none()
            && !tasks.any_load_or_save_queued_or_in_progress();

        // Get launched things from the queue and remove duplicates (avoiding clones)
        let mut loads_to_launch = HashMap::new();
        let mut saves_to_launch = HashMap::new();

        for load in mem::take(&mut tasks.queued_loads) {
            if ids_to_load.contains(&load.request.id) {
                loads_to_launch.insert(load.request.id, load); // use latest of duplicate ids
            } else {
                tasks.queued_loads.push(load); // put back the ones we're not launching
            }
        }
        for save in mem::take(&mut tasks.queued_saves) {
            if ids_to_save.contains(&save.request.id) {
                saves_to_launch.insert(save.request.id, save); // use latest of duplicate ids
            } else {
                tasks.queued_saves.push(save); // put back the ones we're not launching
            }
        }

        let sync_to_launch =
            if should_sync { mem::take(&mut tasks.queued_syncs).into_iter().next() } else { None };

        let any_to_launch =
            !loads_to_launch.is_empty() || !saves_to_launch.is_empty() || sync_to_launch.is_some();
        if any_to_launch {
            info!(
                "launching {} loads, {} saves, and {} syncs; {} loads, {} saves, and {} syncs remain queued",
                loads_to_launch.len(),
                saves_to_launch.len(),
                sync_to_launch.is_some() as usize,
                tasks.queued_loads.len(),
                tasks.queued_saves.len(),
                tasks.queued_syncs.len()
            );
        }

        // Launch the things
        for queued_load in loads_to_launch.into_values() {
            let request = queued_load.request.clone();
            let in_progress_load = InProgressLoad::new(queued_load);
            let queue_time = in_progress_load
                .timing
                .started_at
                .duration_since(in_progress_load.timing.queued_at);
            if queue_time > Duration::from_secs(1) {
                warn!(
                    "load of file {} spent {}ms in the task queue",
                    request.id,
                    queue_time.as_millis()
                );
            }
            tasks.in_progress_loads.push(in_progress_load);

            let self_clone = self.clone();
            thread::spawn(move || {
                let id = request.id;
                let content_result = self_clone.core.read_document_with_hmac(id);

                {
                    let mut tasks = self_clone.tasks.lock().unwrap();

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
                        .expect("failed to find in-progress entry for load that just completed");
                    // ^ above error may indicate concurrent loads to the same file, which would cause problems

                    let completed_load = CompletedLoad {
                        request: in_progress_load.request,
                        content_result,
                        timing: CompletedTiming::new(in_progress_load.timing),
                    };
                    let in_progress_time = completed_load
                        .timing
                        .completed_at
                        .duration_since(completed_load.timing.started_at);
                    if in_progress_time > Duration::from_secs(1) {
                        warn!("loaded file {} ({}ms)", request.id, in_progress_time.as_millis());
                    } else {
                        info!("loaded file {} ({}ms)", request.id, in_progress_time.as_millis());
                    }
                    tasks.completed_loads.push(completed_load);
                }

                self_clone.ctx.request_repaint();
            });
        }

        for queued_save in saves_to_launch.into_values() {
            let request = queued_save.request.clone();
            let in_progress_save = InProgressSave::new(queued_save);
            let content = {
                let mut result = None;
                for tab in tabs {
                    if tab.id == request.id {
                        let start = Instant::now();
                        result = tab.content.clone();
                        let time = Instant::now().duration_since(start);
                        if time > Duration::from_millis(100) {
                            warn!(
                                "spent {}ms on UI thread cloning content for file {} for background thread to save it",
                                request.id,
                                time.as_millis()
                            );
                        }
                        break;
                    }
                }
                if let Some(result) = result {
                    result
                } else {
                    error!(
                        "could not launch save for file {} because its tab does not exist",
                        request.id
                    );
                    continue;
                }
            };
            let queue_time = in_progress_save
                .timing
                .started_at
                .duration_since(in_progress_save.timing.queued_at);
            if queue_time > Duration::from_secs(1) {
                warn!(
                    "save of file {} spent {}ms in the task queue",
                    request.id,
                    queue_time.as_millis()
                );
            }
            tasks.in_progress_saves.push(in_progress_save);

            let self_clone = self.clone();
            thread::spawn(move || {
                let id = request.id;
                let (old_hmac, seq, content) = match content {
                    TabContent::Markdown(editor) => {
                        (editor.hmac, editor.buffer.current.seq, editor.buffer.current.text)
                    }
                    TabContent::Svg(svg) => (svg.buffer.open_file_hmac, 0, svg.buffer.serialize()),
                    TabContent::Image(_) => unimplemented!("images aren't saveable"),
                    TabContent::Pdf(_) => unimplemented!("pdfs aren't saveable"),
                };
                let new_hmac_result =
                    self_clone
                        .core
                        .safe_write(request.id, old_hmac, content.clone().into()); // todo: unnecessary clone

                {
                    let mut tasks = self_clone.tasks.lock().unwrap();

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
                        .expect("failed to find in-progress entry for save that just completed");
                    // ^ above error may indicate concurrent saves to the same file, which would cause problems

                    let completed_save = CompletedSave {
                        request: in_progress_save.request,
                        seq,
                        content,
                        new_hmac_result,
                        timing: CompletedTiming::new(in_progress_save.timing),
                    };
                    let in_progress_time = completed_save
                        .timing
                        .completed_at
                        .duration_since(completed_save.timing.started_at);
                    if in_progress_time > Duration::from_secs(1) {
                        warn!("saved file {} ({}ms)", request.id, in_progress_time.as_millis());
                    } else {
                        info!("saved file {} ({}ms)", request.id, in_progress_time.as_millis());
                    }
                    tasks.completed_saves.push(completed_save);
                }

                self_clone.ctx.request_repaint();
            });
        }

        if let Some(sync) = sync_to_launch {
            let (sender, receiver) = mpsc::channel();
            let in_progress_sync = InProgressSync::new(sync, receiver);
            let queue_time = in_progress_sync
                .timing
                .started_at
                .duration_since(in_progress_sync.timing.queued_at);
            if queue_time > Duration::from_secs(1) {
                warn!("sync spent {}ms in the task queue", queue_time.as_millis());
            }
            tasks.in_progress_sync = Some(in_progress_sync);

            let self_clone = self.clone();
            thread::spawn(move || {
                let status_result = {
                    let ctx = self_clone.ctx.clone();
                    let progress_closure = move |p| {
                        sender.send(p).unwrap();
                        ctx.request_repaint();
                    };
                    self_clone.core.sync(Some(Box::new(progress_closure)))
                };

                {
                    let mut tasks = self_clone.tasks.lock().unwrap();
                    let in_progress_sync = tasks
                        .in_progress_sync
                        .take()
                        .expect("failed to find in-progress entry for sync that just completed");
                    // ^ above error may indicate concurrent syncs, which would cause problems

                    let completed_sync = CompletedSync {
                        status_result,
                        timing: CompletedTiming::new(in_progress_sync.timing),
                    };
                    let in_progress_time = completed_sync
                        .timing
                        .completed_at
                        .duration_since(completed_sync.timing.started_at);
                    if in_progress_time > Duration::from_secs(1) {
                        warn!("synced ({}ms)", in_progress_time.as_millis());
                    } else {
                        info!("synced ({}ms)", in_progress_time.as_millis());
                    }
                    tasks.completed_sync = Some(completed_sync);
                }

                self_clone.ctx.request_repaint();
            });
        }

        if any_to_launch {
            self.ctx.request_repaint();
        }
    }

    pub fn update(&mut self) -> Response {
        let mut tasks = self.tasks.lock().unwrap();
        Response {
            completed_loads: mem::take(&mut tasks.completed_loads),
            completed_saves: mem::take(&mut tasks.completed_saves),
            completed_sync: mem::take(&mut tasks.completed_sync),
        }
    }
}
