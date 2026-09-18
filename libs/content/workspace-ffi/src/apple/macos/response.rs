use std::ffi::{CString, c_char};

use lb_c::Uuid;

use super::super::response::*;
use super::cursor_icon::CCursorIcon;

#[repr(C)]
pub struct CUrls {
    pub size: i32,
    pub urls: *const *mut c_char,
}

#[repr(C)]
pub struct CBytes {
    pub size: i32,
    pub bytes: *mut u8,
}

impl From<Vec<u8>> for CBytes {
    fn from(value: Vec<u8>) -> Self {
        if value.is_empty() {
            return Self { size: 0, bytes: std::ptr::null_mut() };
        }
        let size = value.len() as i32;
        let bytes = Box::into_raw(value.into_boxed_slice()) as *mut u8;
        Self { size, bytes }
    }
}

#[repr(C)]
pub struct MacOSResponse {
    // platform response
    pub redraw_in: u64,
    pub copied_text: *mut c_char,
    pub copied_image: CBytes,
    pub urls_opened: CUrls,
    pub cursor: CCursorIcon,
    pub request_paste: bool,

    // widget response
    pub selected_file: CUuid,
    pub selected_tab: CUuid,
    pub doc_created: CUuid,
    pub tabs_changed: bool,
    pub selected_folder_changed: bool,
}

impl From<crate::Response> for MacOSResponse {
    fn from(value: crate::Response) -> Self {
        let crate::Response {
            workspace:
                workspace_rs::Response {
                    selected_file,
                    selected_session,
                    file_renamed: _,
                    file_moved: _,
                    file_deleted: _,
                    new_folder_clicked: _,
                    tab_title_clicked: _,
                    file_created,
                    markdown_editor_text_updated: _,
                    markdown_editor_selection_updated: _,
                    markdown_editor_scroll_updated: _,
                    text_interaction_rect: _,
                    mobile_toolbar_shown: _,
                    tabs_changed,
                    failure_messages: _,
                    selected_folder_changed,
                    open_camera: _,
                    file_cache_updated: _,
                },
            redraw_in,
            copied_text,
            copied_image,
            urls_opened,
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

        let url_ptrs: Vec<*mut c_char> = urls_opened
            .into_iter()
            .map(|u| CString::new(u).unwrap().into_raw())
            .collect();
        let urls_opened = CUrls {
            size: url_ptrs.len() as i32,
            urls: Box::into_raw(url_ptrs.into_boxed_slice()) as *const *mut c_char,
        };

        Self {
            selected_file: selected_file.unwrap_or_default().into(),
            selected_tab: selected_session
                .map(|id| id.as_uuid())
                .unwrap_or_default()
                .into(),
            doc_created,
            tabs_changed,
            redraw_in: redraw_in.unwrap_or(u64::MAX),
            copied_text: CString::new(copied_text).unwrap().into_raw(),
            copied_image: copied_image.into(),
            urls_opened,
            cursor: cursor.into(),
            request_paste,
            selected_folder_changed,
        }
    }
}

/// # Safety
/// Must be called with a CBytes returned from macos_frame or ios_frame.
#[no_mangle]
pub unsafe extern "C" fn free_bytes(bytes: CBytes) {
    if bytes.bytes.is_null() {
        return;
    }
    drop(Box::from_raw(std::slice::from_raw_parts_mut(bytes.bytes, bytes.size as usize)));
}

/// # Safety
/// Must be called with a CUrls returned from macos_frame. Each url string and
/// the urls array itself are freed.
#[no_mangle]
pub unsafe extern "C" fn free_urls(urls: CUrls) {
    let slice = std::slice::from_raw_parts_mut(urls.urls as *mut *mut c_char, urls.size as usize);
    for ptr in slice.iter() {
        drop(CString::from_raw(*ptr));
    }
    drop(Box::from_raw(slice as *mut [*mut c_char]));
}
