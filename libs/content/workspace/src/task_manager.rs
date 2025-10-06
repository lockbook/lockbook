use std::collections::HashMap;
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};
use std::{mem, thread};

use egui::Context;
use lb_rs::Uuid;
use lb_rs::blocking::Lb;
use lb_rs::model::crypto::DecryptedDocument;
use lb_rs::model::errors::LbResult;
use lb_rs::model::file_metadata::DocumentHmac;
use lb_rs::service::sync::{SyncProgress, SyncStatus};
use tracing::{Level, debug, error, instrument, span, trace, warn};

use crate::tab::{Tab, TabSaveContent, TabsExt as _};

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
    fn load_queued(&self, id: Uuid) -> bool {
        self.queued_loads
            .iter()
            .any(|queued_load| queued_load.request.id == id)
    }

    fn save_queued(&self, id: Uuid) -> bool {
        self.queued_saves
            .iter()
            .any(|queued_save| queued_save.request.id == id)
    }

    fn load_or_save_queued(&self, id: Uuid) -> bool {
        self.load_queued(id) || self.save_queued(id)
    }

    fn load_in_progress(&self, id: Uuid) -> bool {
        self.in_progress_loads
            .iter()
            .any(|in_progress_load| in_progress_load.request.id == id)
    }

    fn save_in_progress(&self, id: Uuid) -> bool {
        self.in_progress_saves
            .iter()
            .any(|in_progress_save| in_progress_save.request.id == id)
    }

    fn load_or_save_in_progress(&self, id: Uuid) -> bool {
        self.load_in_progress(id) || self.save_in_progress(id)
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
#[derive(Clone, Debug)]
pub struct LoadRequest {
    pub id: Uuid,
    pub is_new_file: bool,

    // indicates whether the completed load should replace a tab or be merged
    // into one; unclear if we can just check for existing tabs when complete
    pub tab_created: bool,

    // indicates whether the completed load should make the tab current; this is
    // what focuses the tab and the tab must be shown every frame to hold focus
    // todo: hold focus for loading tabs and all the rest in one proper place
    pub make_current: bool,
}

#[derive(Clone, Debug)]
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
    pub content: TabSaveContent,
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
        trace!("queued load of file {}", request.id);
        self.tasks
            .lock()
            .unwrap()
            .queued_loads
            .push(QueuedLoad { request, timing: QueuedTiming::new() });
    }

    pub fn queue_save(&mut self, request: SaveRequest) {
        trace!("queued save of file {}", request.id);
        self.tasks
            .lock()
            .unwrap()
            .queued_saves
            .push(QueuedSave { request, timing: QueuedTiming::new() });
    }

    pub fn queue_sync(&mut self) {
        trace!("queued sync");
        self.tasks
            .lock()
            .unwrap()
            .queued_syncs
            .push(QueuedSync { timing: QueuedTiming::new() });
    }

    pub fn load_queued(&self, id: Uuid) -> bool {
        self.tasks.lock().unwrap().load_queued(id)
    }

    pub fn save_queued(&self, id: Uuid) -> bool {
        self.tasks.lock().unwrap().save_queued(id)
    }

    pub fn load_or_save_queued(&self, id: Uuid) -> bool {
        self.tasks.lock().unwrap().load_or_save_queued(id)
    }

    pub fn load_in_progress(&self, id: Uuid) -> bool {
        self.tasks.lock().unwrap().load_in_progress(id)
    }

    pub fn save_in_progress(&self, id: Uuid) -> bool {
        self.tasks.lock().unwrap().save_in_progress(id)
    }

    pub fn load_or_save_in_progress(&self, id: Uuid) -> bool {
        self.tasks.lock().unwrap().load_or_save_in_progress(id)
    }

    #[allow(clippy::manual_map)] // manual map clarifies overall fn structure
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

    #[allow(clippy::manual_map)] // manual map clarifies overall fn structure
    pub fn sync_queued_at(&self) -> Option<Instant> {
        let tasks = self.tasks.lock().unwrap();
        if let Some(queued_sync) = tasks.queued_syncs.last() {
            Some(queued_sync.timing.queued_at)
        } else if let Some(in_progress_sync) = tasks.in_progress_sync.as_ref() {
            Some(in_progress_sync.timing.queued_at)
        } else if let Some(completed_sync) = tasks.completed_sync.as_ref() {
            Some(completed_sync.timing.queued_at)
        } else {
            None
        }
    }

    #[allow(clippy::manual_map)] // manual map clarifies overall fn structure
    pub fn sync_started_at(&self) -> Option<Instant> {
        let tasks = self.tasks.lock().unwrap();
        if let Some(in_progress_sync) = tasks.in_progress_sync.as_ref() {
            Some(in_progress_sync.timing.started_at)
        } else if let Some(completed_sync) = tasks.completed_sync.as_ref() {
            Some(completed_sync.timing.started_at)
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
                // result of completed save must be processed before another save to the same file; this guarantees
                // that the hmac from the completed save is used for the next, preventing a ReReadRequired error
                continue;
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
                // use earliest of duplicate ids
                // this supports opening the same file twice in the same frame,
                // which requires the `tab_created` value from the first request
                loads_to_launch.entry(load.request.id).or_insert(load);
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

        // Launch the things
        for queued_load in loads_to_launch.into_values() {
            let span = span!(Level::TRACE, "load_launch", id = queued_load.request.id.to_string());
            let _enter = span.enter();

            let request = queued_load.request.clone();
            let in_progress_load = InProgressLoad::new(queued_load);
            let queue_time = in_progress_load
                .timing
                .started_at
                .duration_since(in_progress_load.timing.queued_at);
            if queue_time > Duration::from_secs(1) {
                warn!("load spent {queue_time:?} in the task queue");
            }
            tasks.in_progress_loads.push(in_progress_load);

            let self_clone = self.clone();
            thread::spawn(move || self_clone.background_load(request));
        }

        for queued_save in saves_to_launch.into_values() {
            let span = span!(Level::TRACE, "save_launch", id = queued_save.request.id.to_string());
            let _enter = span.enter();

            let request = queued_save.request.clone();
            let in_progress_save = InProgressSave::new(queued_save);
            let (old_hmac, seq, content) = {
                let Some(tab) = tabs.get_by_id(request.id) else {
                    error!("could not launch save because its tab does not exist");
                    continue;
                };

                let start = Instant::now();

                let old_hmac = tab.hmac();
                let seq = tab.seq();
                let Some(content) = tab.clone_content() else {
                    break;
                };

                let time = Instant::now().duration_since(start);
                if time > Duration::from_millis(100) {
                    warn!("spent {time:?} on UI thread cloning content");
                }

                (old_hmac, seq, content)
            };
            let queue_time = in_progress_save
                .timing
                .started_at
                .duration_since(in_progress_save.timing.queued_at);
            if queue_time > Duration::from_secs(1) {
                warn!("save spent {queue_time:?} in the task queue");
            }
            tasks.in_progress_saves.push(in_progress_save);

            let self_clone = self.clone();
            thread::spawn(move || self_clone.background_save(request, old_hmac, seq, content));
        }

        if let Some(sync) = sync_to_launch {
            let span = span!(Level::TRACE, "sync_launch");
            let _enter = span.enter();

            let (sender, receiver) = mpsc::channel();
            let in_progress_sync = InProgressSync::new(sync, receiver);
            let queue_time = in_progress_sync
                .timing
                .started_at
                .duration_since(in_progress_sync.timing.queued_at);
            if queue_time > Duration::from_secs(1) {
                warn!("sync spent {:?} in the task queue", queue_time);
            }
            tasks.in_progress_sync = Some(in_progress_sync);

            let self_clone = self.clone();
            thread::spawn(move || self_clone.background_sync(sender));
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

    /// Move a request to in-progress, then call this from a background thread
    #[instrument(level = "warn", skip(self), fields(thread = format!("{:?}", thread::current().id())))]
    fn background_load(&self, request: LoadRequest) {
        let id = request.id;
        let content_result = self.core.read_document_with_hmac(id, true);

        {
            let mut tasks = self.tasks.lock().unwrap();

            let mut in_progress_load = None;
            for load in mem::take(&mut tasks.in_progress_loads) {
                if load.request.id == id {
                    in_progress_load = Some(load); // use latest of duplicate ids
                } else {
                    tasks.in_progress_loads.push(load); // put back the ones we're not completing
                }
            }
            let in_progress_load = in_progress_load
                .expect("failed to find in-progress entry for load that just completed");
            // ^ above error may indicate concurrent loads to the same file, which would cause problems

            let timing = CompletedTiming::new(in_progress_load.timing);
            let in_progress_time = timing.completed_at.duration_since(timing.started_at);
            match &content_result {
                Ok((hmac, _)) if in_progress_time > Duration::from_secs(1) => {
                    warn!(?hmac, "loaded ({:?})", in_progress_time);
                }
                Ok((hmac, _)) => {
                    debug!(?hmac, "loaded ({:?})", in_progress_time);
                }
                Err(err) => {
                    error!("load failed ({:?}): {:?}", in_progress_time, err);
                }
            }

            let completed_load =
                CompletedLoad { request: in_progress_load.request, content_result, timing };
            tasks.completed_loads.push(completed_load);
        }

        self.ctx.request_repaint();
    }

    /// Move a request to in-progress, then call this from a background thread
    #[instrument(level = "debug", skip(self, content), fields(thread = format!("{:?}", thread::current().id())))]
    fn background_save(
        &self, request: SaveRequest, old_hmac: Option<DocumentHmac>, seq: usize,
        content: TabSaveContent,
    ) {
        let id = request.id;
        let new_hmac_result =
            self.core
                .safe_write(request.id, old_hmac, content.clone().into_bytes()); // todo: unnecessary clone

        {
            let mut tasks = self.tasks.lock().unwrap();

            let mut in_progress_save = None;
            for save in mem::take(&mut tasks.in_progress_saves) {
                if save.request.id == id {
                    in_progress_save = Some(save); // use latest of duplicate ids
                } else {
                    tasks.in_progress_saves.push(save); // put back the ones we're not completing
                }
            }
            let in_progress_save = in_progress_save
                .expect("failed to find in-progress entry for save that just completed");
            // ^ above error may indicate concurrent saves to the same file, which would cause problems

            let timing = CompletedTiming::new(in_progress_save.timing);
            let in_progress_time = timing.completed_at.duration_since(timing.started_at);
            match &new_hmac_result {
                Ok(new_hmac) if in_progress_time > Duration::from_secs(1) => {
                    warn!(?new_hmac, "saved ({:?})", in_progress_time);
                }
                Ok(new_hmac) => {
                    debug!(?new_hmac, "saved ({:?})", in_progress_time);
                }
                Err(err) => {
                    error!("save failed ({:?}): {:?}", in_progress_time, err);
                }
            }

            let completed_save = CompletedSave {
                request: in_progress_save.request,
                seq,
                content,
                new_hmac_result,
                timing,
            };
            tasks.completed_saves.push(completed_save);
        }

        self.ctx.request_repaint();
    }

    /// Move a request to in-progress, then call this from a background thread
    #[instrument(level = "debug", skip(self, sender), fields(thread = format!("{:?}", thread::current().id())))]
    fn background_sync(&self, sender: mpsc::Sender<SyncProgress>) {
        let status_result = {
            let ctx = self.ctx.clone();
            let progress_closure = move |p| {
                sender.send(p).unwrap();
                ctx.request_repaint();
            };
            self.core.sync(Some(Box::new(progress_closure)))
        };

        {
            let mut tasks = self.tasks.lock().unwrap();
            let in_progress_sync = tasks
                .in_progress_sync
                .take()
                .expect("failed to find in-progress entry for sync that just completed");
            // ^ above error may indicate concurrent syncs, which would cause problems

            let timing = CompletedTiming::new(in_progress_sync.timing);
            let in_progress_time = timing.completed_at.duration_since(timing.started_at);
            if let Err(err) = &status_result {
                error!("sync failed ({:?}): {:?}", in_progress_time, err);
            } else if in_progress_time > Duration::from_secs(5) {
                warn!(?status_result, "synced ({:?})", in_progress_time);
            } else {
                debug!("synced ({:?})", in_progress_time);
            }

            let completed_sync = CompletedSync { status_result, timing };
            tasks.completed_sync = Some(completed_sync);
        }

        self.ctx.request_repaint();
    }

    pub fn task_status(&self, id: Uuid) -> TaskStatus {
        if self.load_in_progress(id) {
            TaskStatus::LoadInProgress
        } else if self.save_in_progress(id) {
            TaskStatus::SaveInProgress
        } else if self.load_queued(id) {
            TaskStatus::LoadQueued
        } else if self.save_queued(id) {
            TaskStatus::SaveQueued
        } else {
            TaskStatus::Clean
        }
    }
}

pub enum TaskStatus {
    LoadInProgress,
    SaveInProgress,
    LoadQueued,
    SaveQueued,
    Clean,
}
