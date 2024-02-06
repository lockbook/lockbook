use lb_rs::{File, Uuid};

// todo: dirty docs
#[derive(Default, Clone)]
pub struct WsOutput {
    /// What file the workspace is currently showing
    pub selected_file: Option<Uuid>,

    /// What the window title should be (based on filename generally)
    pub window_title: Option<String>,

    pub file_renamed: Option<(Uuid, String)>,

    pub new_folder_clicked: bool,
    pub tab_title_clicked: bool,

    pub file_created: Option<Result<File, String>>,

    pub error: Option<String>,

    pub settings_updated: bool,

    pub sync_done: bool,
    pub status: PersistentWsStatus,
}

// todo: this should probably not be included in output
// these things have ended up here because output is a major way state changes are communicated across FFI
// this is probably an incorrect way to model this. Output should only contain diffs, and then internal state
// should be easily communicateable, we can probably do this easily over FFI via fns. Probably would make output stack
// allocatable
#[derive(Default, Clone)]
pub struct PersistentWsStatus {
    pub syncing: bool,
    pub offline: bool,
    pub pending_share_found: bool,
    pub update_req: bool,
    pub out_of_space: bool,
    pub usage: f64,
    pub sync_progress: f32,
    pub dirtyness: DirtynessMsg,
    pub sync_message: Option<String>,

    /// summary of the booleans above
    pub message: String,
}

impl PersistentWsStatus {
    pub fn populate_message(&mut self) {
        if self.offline {
            self.message = "Offline".to_string();
            return;
        }

        if self.out_of_space {
            self.message = "You're out of space, buy more in settings!".to_string();
        }

        if self.syncing {
            if let Some(msg) = &self.sync_message {
                self.message = msg.to_string();
                return;
            }
        }

        if !self.dirtyness.dirty_files.is_empty() {
            let size = self.dirtyness.dirty_files.len();
            if size == 1 {
                self.message = format!("{size} file need to be synced");
            } else {
                self.message = format!("{size} files need to be synced");
            }
            return;
        }

        self.message = format!("Last synced: {}", self.dirtyness.last_synced);
    }
}

#[derive(Clone)]
pub struct DirtynessMsg {
    pub last_synced: String,
    pub dirty_files: Vec<Uuid>,
}

impl Default for DirtynessMsg {
    fn default() -> Self {
        Self { last_synced: "calculating...".to_string(), dirty_files: vec![] }
    }
}
