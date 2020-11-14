use uuid::Uuid;

use crate::filetree::FileTreeCol;

pub type MsgReceiver = glib::Receiver<Msg>;

pub enum Msg {
    CreateAccount(String),
    ImportAccount(String),
    ExportAccount,
    PerformSync,
    Quit,

    NewFile(String),
    OpenFile(Uuid),
    SaveFile,
    CloseFile,

    ToggleTreeCol(FileTreeCol),

    ShowDialogNew,
    ShowDialogOpen,
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
