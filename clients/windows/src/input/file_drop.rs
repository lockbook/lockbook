use windows::{
    core::*,
    Win32::{
        Foundation::*,
        System::{Com::*, Ole::*, SystemServices::*},
    },
};

// todo: put some actual data here
#[derive(Clone, Debug)]
pub enum Message {
    DragEnter,
    DragOver,
    DragLeave,
    Drop,
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
        (self.handler)(Message::DragEnter);
        Ok(())
    }

    fn DragOver(&self, _: MODIFIERKEYS_FLAGS, _: &POINTL, _: *mut DROPEFFECT) -> Result<()> {
        (self.handler)(Message::DragOver);
        Ok(())
    }

    fn DragLeave(&self) -> Result<()> {
        (self.handler)(Message::DragLeave);
        Ok(())
    }

    fn Drop(
        &self, _: Option<&IDataObject>, _: MODIFIERKEYS_FLAGS, _: &POINTL, _: *mut DROPEFFECT,
    ) -> Result<()> {
        (self.handler)(Message::Drop);
        Ok(())
    }
}
