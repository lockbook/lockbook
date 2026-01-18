use lb_rs::Uuid;
use lb_rs::model::file::File;
use lb_rs::service::sync::SyncStatus;

// todo: dirty docs
#[derive(Debug, Default, Clone)]
pub struct Response {
    /// What file the workspace is currently showing
    pub selected_file: Option<Uuid>,

    pub file_created: Option<Result<File, String>>,
    pub file_renamed: Option<(Uuid, String)>,
    pub file_moved: Option<(Uuid, Uuid)>,

    pub selected_folder_changed: bool,

    pub new_folder_clicked: bool,
    pub tab_title_clicked: bool,

    pub settings_updated: bool,

    pub sync_done: Option<SyncStatus>,
    pub status_updated: bool,

    // acknowledging the need for a better pattern here, but there are some editor-specific outputs that need
    // to make their way across FFI and it's cleaner to put them in this transient data structure than to maintain them
    // as persistent editor state
    pub markdown_editor_text_updated: bool,
    pub markdown_editor_selection_updated: bool,
    pub markdown_editor_scroll_updated: bool,

    pub tabs_changed: bool,

    pub failure_messages: Vec<String>, // shown as toasts in egui client

    pub open_camera: bool,
}

#[derive(Default, Clone)]
pub struct WsStatus {
    pub sync_error: Option<String>,
    pub sync_status_update_error: Option<String>,
    pub offline: bool,
    pub update_req: bool,
    pub out_of_space: bool,
    pub dirtyness: DirtynessMsg,
    pub sync_message: Option<String>,

    /// summary of the booleans above
    pub message: String,
}

#[derive(Clone, Debug)]
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
