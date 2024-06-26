use crate::cursor_icon::CCursorIcon;
use egui_editor::input::canonical::{Location, Region};
use lb_external_interface::lb_rs::Uuid;
use std::ffi::{c_char, CString};
use workspace_rs::output::WsOutput;

#[repr(C)]
#[derive(Debug)]
pub struct IntegrationOutput {
    pub workspace_resp: FfiWorkspaceResp,
    pub redraw_in: u64,
    pub copied_text: *mut c_char,
    pub url_opened: *mut c_char,
    pub cursor: CCursorIcon,
}

impl Default for IntegrationOutput {
    fn default() -> Self {
        Self {
            redraw_in: Default::default(),
            workspace_resp: Default::default(),
            copied_text: std::ptr::null_mut(),
            url_opened: std::ptr::null_mut(),
            cursor: Default::default(),
        }
    }
}

#[derive(Debug)]
#[repr(C)]
pub struct FfiWorkspaceResp {
    selected_file: CUuid,
    doc_created: CUuid,

    msg: *mut c_char,
    syncing: bool,
    refresh_files: bool,

    new_folder_btn_pressed: bool,

    #[cfg(target_os = "ios")]
    pub hide_virtual_keyboard: bool,

    #[cfg(target_os = "ios")]
    pub text_updated: bool,
    #[cfg(target_os = "ios")]
    pub selection_updated: bool,

    #[cfg(target_os = "ios")]
    pub tab_title_clicked: bool,
}

impl Default for FfiWorkspaceResp {
    fn default() -> Self {
        Self {
            selected_file: Default::default(),
            doc_created: Default::default(),
            msg: std::ptr::null_mut(),
            syncing: Default::default(),
            refresh_files: Default::default(),
            new_folder_btn_pressed: Default::default(),
            #[cfg(target_os = "ios")]
            hide_virtual_keyboard: false,
            #[cfg(target_os = "ios")]
            text_updated: Default::default(),
            #[cfg(target_os = "ios")]
            selection_updated: Default::default(),
            #[cfg(target_os = "ios")]
            tab_title_clicked: false,
        }
    }
}

impl From<WsOutput> for FfiWorkspaceResp {
    fn from(value: WsOutput) -> Self {
        Self {
            selected_file: value.selected_file.unwrap_or_default().into(),
            msg: CString::new(value.status.message).unwrap().into_raw(),
            syncing: value.status.syncing,
            refresh_files: value.sync_done.is_some()
                || value.file_renamed.is_some()
                || value.file_created.is_some(),
            doc_created: match value.file_created {
                Some(Ok(f)) => {
                    if f.is_document() {
                        f.id.into()
                    } else {
                        Uuid::nil().into()
                    }
                }
                _ => Uuid::nil().into(),
            },
            new_folder_btn_pressed: value.new_folder_clicked,

            #[cfg(target_os = "ios")]
            hide_virtual_keyboard: value.hide_virtual_keyboard,
            #[cfg(target_os = "ios")]
            text_updated: false,
            #[cfg(target_os = "ios")]
            selection_updated: false,
            #[cfg(target_os = "ios")]
            tab_title_clicked: value.tab_title_clicked,
        }
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct UITextSelectionRects {
    pub size: i32,
    pub rects: *const CRect,
}

#[cfg(target_vendor = "apple")]
impl Default for UITextSelectionRects {
    fn default() -> Self {
        UITextSelectionRects { size: 0, rects: std::ptr::null() }
    }
}

/// https://developer.apple.com/documentation/uikit/uitextrange
#[repr(C)]
#[derive(Debug, Default)]
pub struct CTextRange {
    /// used to represent a non-existent state of this struct
    pub none: bool,
    pub start: CTextPosition,
    pub end: CTextPosition,
}

#[repr(C)]
#[derive(Debug, Default)]
pub struct CTextPosition {
    /// used to represent a non-existent state of this struct
    pub none: bool,
    pub pos: usize, // represents a grapheme index
}

#[repr(C)]
#[derive(Debug)]
pub enum CTextLayoutDirection {
    Right = 2,
    Left = 3,
    Up = 4,
    Down = 5,
}

#[repr(C)]
#[derive(Debug)]
pub struct CPoint {
    pub x: f64,
    pub y: f64,
}

#[repr(C)]
#[derive(Debug)]
pub enum CTextGranularity {
    Character = 0,
    Word = 1,
    Sentence = 2,
    Paragraph = 3,
    Line = 4,
    Document = 5,
}

#[repr(C)]
#[derive(Debug, Default)]
pub struct CRect {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

impl From<CTextRange> for Region {
    fn from(value: CTextRange) -> Self {
        Region::BetweenLocations { start: value.start.into(), end: value.end.into() }
    }
}

impl From<CTextPosition> for Location {
    fn from(value: CTextPosition) -> Self {
        Location::DocCharOffset(value.pos.into())
    }
}

#[repr(C)]
#[derive(Debug, Default)]
pub struct CUuid([u8; 16]);

impl From<Uuid> for CUuid {
    fn from(value: Uuid) -> Self {
        Self(value.into_bytes())
    }
}

impl From<CUuid> for Uuid {
    fn from(value: CUuid) -> Self {
        Uuid::from_bytes(value.0)
    }
}
