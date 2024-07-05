use crate::cursor_icon::CCursorIcon;
use lb_external_interface::lb_rs::Uuid;
use std::ffi::c_char;
use workspace_rs::output::WsOutput;
use workspace_rs::tab::markdown_editor::input::canonical::{Bound, Location, Region};
use workspace_rs::tab::markdown_editor::offset_types::{DocCharOffset, RangeExt as _};

#[repr(C)]
#[derive(Debug)]
pub struct Output {
    // widget response
    pub workspace: Response,

    // platform response
    pub redraw_in: u64,
    pub copied_text: *mut c_char,
    pub url_opened: *mut c_char,
    pub cursor: CCursorIcon,
    pub virtual_keyboard_shown_set: bool,
    pub virtual_keyboard_shown_val: bool,
}

impl Default for Output {
    fn default() -> Self {
        Self {
            redraw_in: Default::default(),
            workspace: Default::default(),
            copied_text: std::ptr::null_mut(),
            url_opened: std::ptr::null_mut(),
            cursor: Default::default(),
            virtual_keyboard_shown_set: Default::default(),
            virtual_keyboard_shown_val: Default::default(),
        }
    }
}

#[derive(Debug, Default)]
#[repr(C)]
pub struct Response {
    selected_file: CUuid,
    doc_created: CUuid,

    status_updated: bool,
    refresh_files: bool,

    new_folder_btn_pressed: bool,

    tabs_changed: bool,

    #[cfg(target_os = "ios")]
    pub text_updated: bool,
    #[cfg(target_os = "ios")]
    pub selection_updated: bool,
    #[cfg(target_os = "ios")]
    pub scroll_updated: bool,

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
            text_updated: value.markdown_editor_text_updated,
            #[cfg(target_os = "ios")]
            selection_updated: value.markdown_editor_selection_updated,
            #[cfg(target_os = "ios")]
            scroll_updated: value.markdown_editor_scroll_updated,
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
#[derive(Debug, Default, Clone)]
pub struct CTextRange {
    /// used to represent a non-existent state of this struct
    pub none: bool,
    pub start: CTextPosition,
    pub end: CTextPosition,
}

impl From<(DocCharOffset, DocCharOffset)> for CTextRange {
    fn from(value: (DocCharOffset, DocCharOffset)) -> Self {
        if value.is_empty() {
            CTextRange { none: true, ..Default::default() }
        } else {
            // note: preserve range order, even if it's backwards  (unlike opposite conversion)
            CTextRange { none: false, start: value.0.into(), end: value.1.into() }
        }
    }
}

impl From<Option<(DocCharOffset, DocCharOffset)>> for CTextRange {
    fn from(value: Option<(DocCharOffset, DocCharOffset)>) -> Self {
        match value {
            Some(range) => range.into(),
            None => CTextRange { none: true, start: Default::default(), end: Default::default() },
        }
    }
}

#[repr(C)]
#[derive(Debug, Default, Clone)]
pub struct CTextPosition {
    /// used to represent a non-existent state of this struct
    pub none: bool,
    pub pos: usize, // represents a grapheme index
}

impl From<DocCharOffset> for CTextPosition {
    fn from(value: DocCharOffset) -> Self {
        CTextPosition { none: false, pos: value.0 }
    }
}

impl From<Option<DocCharOffset>> for CTextPosition {
    fn from(value: Option<DocCharOffset>) -> Self {
        match value {
            Some(offset) => offset.into(),
            None => CTextPosition { none: true, pos: 0 },
        }
    }
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

impl From<CTextGranularity> for Bound {
    fn from(val: CTextGranularity) -> Bound {
        match val {
            CTextGranularity::Character => Bound::Char,
            CTextGranularity::Word => Bound::Word,
            CTextGranularity::Sentence => Bound::Paragraph, // note: sentence handled as paragraph
            CTextGranularity::Paragraph => Bound::Paragraph,
            CTextGranularity::Line => Bound::Line,
            CTextGranularity::Document => Bound::Doc,
        }
    }
}

#[repr(C)]
#[derive(Debug, Default, Clone)]
pub struct CRect {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

impl From<CTextRange> for Option<(DocCharOffset, DocCharOffset)> {
    fn from(value: CTextRange) -> Self {
        if value.none {
            None
        } else {
            // note: re-order values if necessary (unlike opposite conversion)
            let result = (value.start.pos.into(), value.end.pos.into());
            Some((result.start(), result.end()))
        }
    }
}

impl From<CTextRange> for Option<Region> {
    fn from(value: CTextRange) -> Self {
        if value.none {
            None
        } else {
            let result: (DocCharOffset, DocCharOffset) =
                (value.start.pos.into(), value.end.pos.into());
            Some(Region::BetweenLocations {
                start: Location::DocCharOffset(result.start()),
                end: Location::DocCharOffset(result.end()),
            })
        }
    }
}

impl From<CTextPosition> for Option<Location> {
    fn from(value: CTextPosition) -> Self {
        if value.none {
            None
        } else {
            Some(Location::DocCharOffset(value.pos.into()))
        }
    }
}

impl From<CTextPosition> for Option<DocCharOffset> {
    fn from(value: CTextPosition) -> Self {
        if value.none {
            None
        } else {
            Some(value.pos.into())
        }
    }
}

#[repr(C)]
#[derive(Debug, Default, Clone)]
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
