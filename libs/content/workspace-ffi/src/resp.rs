use crate::cursor_icon::CCursorIcon;
use egui_editor::input::canonical::{Location, Region};
use egui_editor::offset_types::{DocCharOffset, RelCharOffset};
use lb_external_interface::lb_rs::Uuid;
use std::ffi::c_char;
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

#[derive(Debug, Default)]
#[repr(C)]
pub struct FfiWorkspaceResp {
    selected_file: CUuid,
    doc_created: CUuid,

    status_updated: bool,
    refresh_files: bool,

    new_folder_btn_pressed: bool,

    tabs_changed: bool,

    #[cfg(target_os = "ios")]
    pub hide_virtual_keyboard: bool,

    #[cfg(target_os = "ios")]
    pub text_updated: bool,
    #[cfg(target_os = "ios")]
    pub selection_updated: bool,

    #[cfg(target_os = "ios")]
    pub tab_title_clicked: bool,
}

impl From<WsOutput> for FfiWorkspaceResp {
    fn from(value: WsOutput) -> Self {
        Self {
            selected_file: value.selected_file.unwrap_or_default().into(),
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
            tabs_changed: value.tabs_changed,

            #[cfg(target_os = "ios")]
            hide_virtual_keyboard: value.hide_virtual_keyboard,
            #[cfg(target_os = "ios")]
            text_updated: value.markdown_editor_text_updated,
            #[cfg(target_os = "ios")]
            selection_updated: value.markdown_editor_selection_updated,
            #[cfg(target_os = "ios")]
            tab_title_clicked: value.tab_title_clicked,
            status_updated: value.status_updated,
        }
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct UITextSelectionRects {
    pub size: i32,
    pub rects: *const CRect,
}

#[repr(C)]
#[derive(Debug)]
pub struct TabsIds {
    pub size: i32,
    pub ids: *const CUuid,
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

impl From<CTextRange> for (DocCharOffset, DocCharOffset) {
    fn from(value: CTextRange) -> Self {
        (value.start.pos.into(), value.end.pos.into())
    }
}

impl From<CTextRange> for (RelCharOffset, RelCharOffset) {
    fn from(value: CTextRange) -> Self {
        (value.start.pos.into(), value.end.pos.into())
    }
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
