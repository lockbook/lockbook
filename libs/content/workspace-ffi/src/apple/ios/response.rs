use lb_c::Uuid;
use lb_c::model::text::offset_types::{DocCharOffset, RangeExt as _};
use std::ffi::{CString, c_char};
use workspace_rs::tab::markdown_editor::input::{Bound, Location, Region};

use super::super::response::*;

#[repr(C)]
pub struct IOSResponse {
    // platform response
    pub redraw_in: u64,
    pub copied_text: *mut c_char,
    pub url_opened: *mut c_char,
    pub open_camera: bool,
    pub has_virtual_keyboard_shown: bool,
    pub virtual_keyboard_shown: bool,

    // widget response
    pub selected_file: CUuid,
    pub refresh_files: bool,
    pub doc_created: CUuid,
    pub new_folder_btn_pressed: bool,
    pub status_updated: bool,
    pub tabs_changed: bool,

    pub text_updated: bool,
    pub selection_updated: bool,
    pub scroll_updated: bool,
    pub tab_title_clicked: bool,
    pub selected_folder_changed: bool,
}

impl From<crate::Response> for IOSResponse {
    fn from(value: crate::Response) -> Self {
        let crate::Response {
            workspace:
                workspace_rs::Response {
                    selected_file,
                    file_renamed,
                    file_moved: _,
                    new_folder_clicked,
                    tab_title_clicked,
                    file_created,
                    settings_updated: _,
                    sync_done,
                    status_updated,
                    markdown_editor_text_updated,
                    markdown_editor_selection_updated,
                    markdown_editor_scroll_updated,
                    tabs_changed,
                    failure_messages: _,
                    selected_folder_changed,
                    open_camera,
                },
            redraw_in,
            copied_text,
            url_opened,
            cursor: _,
            virtual_keyboard_shown,
            window_title: _,
            request_paste: _,
            context_menu: _,
        } = value;

        let doc_created = match file_created {
            Some(Ok(ref f)) if f.is_document() => f.id.into(),
            _ => Uuid::nil().into(),
        };
        let url_opened = url_opened
            .map(|u| CString::new(u).unwrap().into_raw())
            .unwrap_or(std::ptr::null_mut());
        Self {
            selected_file: selected_file.unwrap_or_default().into(),
            refresh_files: sync_done.is_some() || file_renamed.is_some() || file_created.is_some(),
            doc_created,
            new_folder_btn_pressed: new_folder_clicked,
            status_updated,
            tabs_changed,
            redraw_in: redraw_in.unwrap_or(u64::MAX),
            copied_text: CString::new(copied_text).unwrap().into_raw(),
            url_opened,
            open_camera,
            text_updated: markdown_editor_text_updated,
            selection_updated: markdown_editor_selection_updated,
            scroll_updated: markdown_editor_scroll_updated,
            tab_title_clicked,
            has_virtual_keyboard_shown: virtual_keyboard_shown.is_some(),
            virtual_keyboard_shown: virtual_keyboard_shown.unwrap_or_default(),
            selected_folder_changed,
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

impl Default for UITextSelectionRects {
    fn default() -> Self {
        UITextSelectionRects { size: 0, rects: std::ptr::null() }
    }
}

/// https://developer.apple.com/documentation/uikit/uitextrange
#[repr(C)]
#[derive(Debug, Clone)]
pub struct CTextRange {
    /// used to represent a non-existent state of this struct
    pub none: bool,
    pub start: CTextPosition,
    pub end: CTextPosition,
}

impl Default for CTextRange {
    fn default() -> Self {
        CTextRange { none: true, start: CTextPosition::default(), end: CTextPosition::default() }
    }
}

impl From<(DocCharOffset, DocCharOffset)> for CTextRange {
    fn from(value: (DocCharOffset, DocCharOffset)) -> Self {
        if value.is_empty() {
            CTextRange::default()
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
#[derive(Debug, Clone)]
pub struct CTextPosition {
    /// used to represent a non-existent state of this struct
    pub none: bool,
    pub pos: usize, // represents a grapheme index
}

impl Default for CTextPosition {
    fn default() -> Self {
        CTextPosition { none: true, pos: 0 }
    }
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
        if value.none { None } else { Some(Location::DocCharOffset(value.pos.into())) }
    }
}

impl From<CTextPosition> for Option<DocCharOffset> {
    fn from(value: CTextPosition) -> Self {
        if value.none { None } else { Some(value.pos.into()) }
    }
}
