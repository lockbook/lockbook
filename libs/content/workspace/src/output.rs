use lb_rs::Uuid;

// todo: dirty docs
#[derive(Default, Clone)]
pub struct WsOutput {
    /// What file the workspace is currently showing
    pub selected_file: Option<Uuid>,

    /// What the window title should be (based on filename generally)
    pub window_title: Option<String>,

    pub file_renamed: Option<(Uuid, String)>,

    pub new_folder_clicked: bool,
    pub new_document_clicked: bool,

    pub error: Option<String>,

    pub settings_updated: bool,

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

impl WsOutput {
    // todo incorporate local dirtyness & last synced
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
