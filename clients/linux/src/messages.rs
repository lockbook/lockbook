use uuid::Uuid;

use crate::filetree::FileTreeCol;

pub type MsgReceiver = glib::Receiver<Msg>;
pub type MsgFn = fn() -> Msg;

pub enum Msg {
    CreateAccount(String),
    ImportAccount(String),
    ExportAccount,
    PerformSync,
    Quit,

    NewFile(String),
    OpenFile(Option<Uuid>),
    SaveFile,
    CloseFile,
    DeleteFiles,

    SearchFieldFocus,
    SearchFieldBlur,
    SearchFieldUpdate,
    SearchFieldUpdateIcon,
    SearchFieldExec(Option<String>),

    ToggleTreeCol(FileTreeCol),

    ShowDialogNew,
    ShowDialogPreferences,
    ShowDialogUsage,
    ShowDialogAbout,

    UnexpectedErr(String, String),
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
}
