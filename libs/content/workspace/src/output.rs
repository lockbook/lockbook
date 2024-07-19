use lb_rs::{File, SyncStatus, Uuid};

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

    pub hide_virtual_keyboard: bool,

    pub file_created: Option<Result<File, String>>,

    pub error: Option<String>,

    pub settings_updated: bool,

    pub sync_done: Option<SyncStatus>,

    // todo see below comment about this anti-patern I've created. Only one place should be updating this, we'll refactor
    // this more thoroughly in 0.8.6
    pub status: PersistentWsStatus,

    // first of all, love the above commitment to refactor something in 0.8.6 (we're now on 0.9.4). it do be like that.
    // next up, acknowledging the need for a better pattern here, but there are some editor-specific outputs that need
    // to make their way across FFI and it's cleaner to put them in this transient data structure than to maintain them
    // as persistent editor state
    pub markdown_editor_text_updated: bool,
    pub markdown_editor_selection_updated: bool,

    pub markdown_editor_show_edit_menu: bool,
    pub markdown_editor_edit_menu_x: f32,
    pub markdown_editor_edit_menu_y: f32,
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
    pub pending_shares: Vec<File>,
}

impl Default for DirtynessMsg {
    fn default() -> Self {
        Self {
            last_synced: "calculating...".to_string(),
            dirty_files: vec![],
            pending_shares: vec![],
        }
    }
}
