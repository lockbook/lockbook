use uuid::Uuid;

use crate::error::LbError;
use crate::filetree::FileTreeCol;
use lockbook_models::file_metadata::FileType;

pub type MsgFn = fn() -> Msg;

pub enum Msg {
    CreateAccount(String),
    ImportAccount(String),
    ExportAccount,
    PerformSync,
    RefreshSyncStatus,
    RefreshUsageStatus,
    Quit,

    NewFile(FileType),
    OpenFile(Option<Uuid>),
    FileEdited,
    SaveFile,
    CloseFile,
    DeleteFiles,
    RenameFile,

    SearchFieldFocus,
    SearchFieldBlur(bool),
    SearchFieldUpdate,
    SearchFieldUpdateIcon,
    SearchFieldExec(Option<String>),

    ToggleTreeCol(FileTreeCol),
    RefreshTree,

    AccountScreenShown,
    ShowDialogSyncDetails,
    ShowDialogPreferences,
    ShowDialogUsage,
    ShowDialogAbout,
    ShowDialogImportFile(Vec<String>),

    ToggleAutoSave(bool),
    ToggleAutoSync(bool),

    ErrorDialog(String, LbError),
    SetStatus(String, Option<String>),
}

#[derive(Clone)]
pub struct Messenger {
    s: glib::Sender<Msg>,
}

impl Messenger {
    pub fn new(s: glib::Sender<Msg>) -> Self {
        Self { s }
    }

    pub fn send(&self, m: Msg) {
        self.s.send(m).unwrap();
    }

    pub fn send_err_dialog(&self, title: &str, err: LbError) {
        self.send(Msg::ErrorDialog(title.to_string(), err))
    }

    pub fn send_err_status_panel(&self, msg: &str) {
        self.send(Msg::SetStatus(msg.to_string(), None))
    }
}
