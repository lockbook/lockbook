use glib::Receiver as GlibReceiver;
use uuid::Uuid;

use crate::filetree::FileTreeCol;

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
    SearchFieldBlur(bool),
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
    pub fn new_main_channel() -> (Self, GlibReceiver<Msg>) {
        let (s, r) = glib::MainContext::channel::<Msg>(glib::PRIORITY_DEFAULT);
        (Self { s }, r)
    }

    pub fn send(&self, m: Msg) {
        self.s.send(m).unwrap();
    }
}
