use uuid::Uuid;

use crate::error::LbError;
use crate::filetree::FileTreeCol;

pub type MsgFn = fn() -> Msg;

pub enum Msg {
    CreateAccount(String),
    ImportAccount(String),
    ExportAccount,
    PerformSync,
    RefreshSyncStatus,
    Quit,

    NewFile(String),
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

    ShowDialogNew,
    ShowDialogSyncDetails,
    ShowDialogPreferences,
    ShowDialogUsage,
    ShowDialogAbout,

    ToggleAutoSave(bool),
    ToggleAutoSync(bool),

    Error(String, LbError),
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

    pub fn send_err(&self, title: &str, err: LbError) {
        self.send(Msg::Error(title.to_string(), err));
    }
}
