use lb_rs::Uuid;
use lb_rs::model::file::File;

// todo: dirty docs
#[derive(Debug, Default, Clone)]
pub struct Response {
    /// What file the workspace is currently showing
    pub selected_file: Option<Uuid>,
    pub file_created: Option<Result<File, String>>,
    pub file_renamed: Option<(Uuid, String)>,
    pub file_moved: Option<(Uuid, Uuid)>,
    pub file_deleted: Option<Uuid>,
    pub new_folder_clicked: bool,

    pub selected_folder_changed: bool,
    pub tab_title_clicked: bool,

    // acknowledging the need for a better pattern here, but there are some editor-specific outputs that need
    // to make their way across FFI and it's cleaner to put them in this transient data structure than to maintain them
    // as persistent editor state
    pub markdown_editor_text_updated: bool,
    pub markdown_editor_selection_updated: bool,
    pub markdown_editor_scroll_updated: bool,
    pub markdown_editor_find_widget_height: f32,

    pub tabs_changed: bool,

    pub failure_messages: Vec<String>, // shown as toasts in egui client

    pub open_camera: bool,

    pub file_cache_updated: bool,
}
