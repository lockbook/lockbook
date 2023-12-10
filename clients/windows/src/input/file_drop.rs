use std::path::PathBuf;

use egui::DroppedFile;
use lbeguiapp::WgpuLockbook;
use windows::{
    core::*,
    Win32::{
        Foundation::*,
        System::{Com::*, Ole::*, SystemServices::*},
    },
};

#[derive(Clone, Debug)]
pub enum Message {
    DragEnter { object: Option<IDataObject>, state: MODIFIERKEYS_FLAGS, point: POINTL },
    DragOver { state: MODIFIERKEYS_FLAGS, point: POINTL },
    DragLeave,
    Drop { object: Option<IDataObject>, state: MODIFIERKEYS_FLAGS, point: POINTL },
}

#[implement(IDropTarget)]
pub struct FileDropHandler {
    pub handler: Box<dyn Fn(Message)>,
}

impl IDropTarget_Impl for FileDropHandler {
    fn DragEnter(
        &self, object: Option<&IDataObject>, state: MODIFIERKEYS_FLAGS, point: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> Result<()> {
        // indicates to the drop source that they don't need to delete the source data (unlike DROPEFFECT_MOVE)
        // "If DoDragDrop returns DROPEFFECT_MOVE, delete the source data from the source document immediately. No other return value from DoDragDrop has any effect on a drop source."
        // https://learn.microsoft.com/en-us/cpp/mfc/drag-and-drop-ole?view=msvc-170
        unsafe { *effect = DROPEFFECT_COPY };

        (self.handler)(Message::DragEnter { object: object.cloned(), state, point: *point });
        Ok(())
    }

    fn DragOver(
        &self, state: MODIFIERKEYS_FLAGS, point: &POINTL, _: *mut DROPEFFECT,
    ) -> Result<()> {
        (self.handler)(Message::DragOver { state, point: *point });
        Ok(())
    }

    fn DragLeave(&self) -> Result<()> {
        (self.handler)(Message::DragLeave);
        Ok(())
    }

    fn Drop(
        &self, object: Option<&IDataObject>, state: MODIFIERKEYS_FLAGS, point: &POINTL,
        _: *mut DROPEFFECT,
    ) -> Result<()> {
        (self.handler)(Message::Drop { object: object.cloned(), state, point: *point });
        Ok(())
    }
}

pub fn handle(app: &mut WgpuLockbook, path: PathBuf) {
    let path = Some(path);
    app.raw_input
        .dropped_files
        .push(DroppedFile { path, ..Default::default() });
}
