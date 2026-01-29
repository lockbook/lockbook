use std::ffi::{CString, c_char};

use lb_c::Uuid;

use super::super::response::*;
use super::cursor_icon::CCursorIcon;

#[repr(C)]
pub struct MacOSResponse {
    // platform response
    pub redraw_in: u64,
    pub copied_text: *mut c_char,
    pub url_opened: *mut c_char,
    pub cursor: CCursorIcon,
    pub request_paste: bool,

    // widget response
    pub selected_file: CUuid,
    pub refresh_files: bool,
    pub doc_created: CUuid,
    pub new_folder_btn_pressed: bool,
    pub status_updated: bool,
    pub tabs_changed: bool,
    pub selected_folder_changed: bool,
}

impl From<crate::Response> for MacOSResponse {
    fn from(value: crate::Response) -> Self {
        let crate::Response {
            workspace:
                workspace_rs::Response {
                    selected_file,
                    file_renamed,
                    file_moved: _,
                    new_folder_clicked,
                    tab_title_clicked: _,
                    file_created,
                    settings_updated: _,
                    sync_done,
                    status_updated,
                    markdown_editor_text_updated: _,
                    markdown_editor_selection_updated: _,
                    markdown_editor_scroll_updated: _,
                    tabs_changed,
                    failure_messages: _,
                    selected_folder_changed,
                    open_camera: _,
                },
            redraw_in,
            copied_text,
            url_opened,
            cursor,
            virtual_keyboard_shown: _,
            window_title: _,
            request_paste,
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
            cursor: cursor.into(),
            request_paste,
            selected_folder_changed,
        }
    }
}
