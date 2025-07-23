use egui::DroppedFile;
use lbeguiapp::WgpuLockbook;
use windows::Win32::Foundation::*;
use windows::Win32::System::Com::*;
use windows::Win32::System::Memory::*;
use windows::Win32::System::Ole::*;
use windows::Win32::System::SystemServices::*;
use windows::Win32::UI::Shell::*;
use windows::core::*;

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

pub fn handle(app: &mut WgpuLockbook, object: Option<IDataObject>) -> bool {
    if let Some(object) = object {
        let format_enumerator: IEnumFORMATETC = unsafe {
            object
                .EnumFormatEtc(DATADIR_GET.0 as _)
                .expect("enumerate drop formats")
        };
        let mut rgelt = [FORMATETC::default(); 1];
        loop {
            let mut fetched: u32 = 0;
            if unsafe { format_enumerator.Next(&mut rgelt, Some(&mut fetched as _)) }.is_err() {
                break;
            }
            if fetched == 0 {
                break;
            }

            // todo: support additional formats (including custome/registerd formats)
            let format = CLIPBOARD_FORMAT(rgelt[0].cfFormat);
            let is_predefined_format = format_str(format).is_some();
            // use windows::Win32::System::DataExchange::*,
            // let mut format_name = [0u16; 1000];
            // let is_registered_format =
            // unsafe { GetClipboardFormatNameW(format.0 as _, &mut format_name) != 0 };
            if !is_predefined_format {
                continue;
            }

            let stgm = unsafe { object.GetData(&rgelt[0]) }.expect("get drop data");

            let tymed = TYMED(stgm.tymed as _);
            if tymed_str(tymed).is_none() {
                continue;
            }

            if tymed == TYMED_HGLOBAL {
                let hglobal = unsafe { stgm.u.hGlobal };

                // for unknown reasons, if I don't cast the HGLOBAL to an HDROP and query the file count, the next call to object.GetData fails
                // (this applies even if the format isn't CF_HDROP)
                let hdrop = HDROP(unsafe {
                    std::mem::transmute::<windows::Win32::Foundation::HGLOBAL, isize>(hglobal)
                });

                let file_count = unsafe { DragQueryFileW(hdrop, 0xFFFFFFFF, None) };
                if format == CF_HDROP {
                    for i in 0..file_count {
                        let mut file_path_bytes = [0u16; MAX_PATH as _];
                        unsafe { DragQueryFileW(hdrop, i, Some(&mut file_path_bytes)) };
                        let path = Some(
                            String::from_utf16_lossy(&file_path_bytes)
                                .trim_matches(char::from(0))
                                .into(),
                        );
                        app.raw_input
                            .dropped_files
                            .push(DroppedFile { path, ..Default::default() });
                    }
                } else {
                    let size = unsafe { GlobalSize(hglobal) };
                    let mut bytes = vec![0u8; size as _];
                    unsafe {
                        std::ptr::copy_nonoverlapping(
                            GlobalLock(hglobal),
                            bytes.as_mut_ptr() as _,
                            size as _,
                        );
                        let _ = GlobalUnlock(hglobal);
                    }
                    let _global_bytes = String::from_utf8_lossy(&bytes).len();

                    // todo: do something with dropped bytes (e.g. support dropped text blocks)
                }
            }
        }
    }
    true
}

fn tymed_str(tymed: TYMED) -> Option<&'static str> {
    match tymed {
        TYMED_HGLOBAL => Some("TYMED_HGLOBAL"),
        TYMED_FILE => Some("TYMED_FILE"),
        TYMED_ISTREAM => Some("TYMED_ISTREAM"),
        TYMED_ISTORAGE => Some("TYMED_ISTORAGE"),
        TYMED_GDI => Some("TYMED_GDI"),
        TYMED_MFPICT => Some("TYMED_MFPICT"),
        TYMED_ENHMF => Some("TYMED_ENHMF"),
        TYMED_NULL => Some("TYMED_NULL"),
        _ => None,
    }
}

fn format_str(format: CLIPBOARD_FORMAT) -> Option<&'static str> {
    match format {
        CF_TEXT => Some("CF_TEXT"),
        CF_BITMAP => Some("CF_BITMAP"),
        CF_METAFILEPICT => Some("CF_METAFILEPICT"),
        CF_SYLK => Some("CF_SYLK"),
        CF_DIF => Some("CF_DIF"),
        CF_TIFF => Some("CF_TIFF"),
        CF_OEMTEXT => Some("CF_OEMTEXT"),
        CF_DIB => Some("CF_DIB"),
        CF_PALETTE => Some("CF_PALETTE"),
        CF_PENDATA => Some("CF_PENDATA"),
        CF_RIFF => Some("CF_RIFF"),
        CF_WAVE => Some("CF_WAVE"),
        CF_UNICODETEXT => Some("CF_UNICODETEXT"),
        CF_ENHMETAFILE => Some("CF_ENHMETAFILE"),
        CF_HDROP => Some("CF_HDROP"),
        CF_LOCALE => Some("CF_LOCALE"),
        CF_DIBV5 => Some("CF_DIBV5"),
        CF_OWNERDISPLAY => Some("CF_OWNERDISPLAY"),
        CF_DSPTEXT => Some("CF_DSPTEXT"),
        CF_DSPBITMAP => Some("CF_DSPBITMAP"),
        CF_DSPMETAFILEPICT => Some("CF_DSPMETAFILEPICT"),
        CF_DSPENHMETAFILE => Some("CF_DSPENHMETAFILE"),
        _ => None,
    }
}
