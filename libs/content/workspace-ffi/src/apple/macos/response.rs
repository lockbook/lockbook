#[repr(C)]
pub struct Response {
    // widget response
    pub selected_file: CUuid,
    pub refresh_files: bool,
    pub doc_created: CUuid,
    pub new_folder_btn_pressed: bool,
    pub tabs_changed: bool,

    // platform response
    pub redraw_in: u64,
    pub copied_text: *mut c_char,
    pub url_opened: *mut c_char,
    pub cursor: CCursorIcon,
}

impl From<crate::Response> for Response {
    fn from(value: crate::Response) -> Self {
        let crate::Response {
            workspace:
                workspace_rs::Response {
                    selected_file,
                    file_renamed,
                    new_folder_clicked,
                    tab_title_clicked,
                    file_created,
                    error,
                    settings_updated,
                    sync_done,
                    status_updated,
                    markdown_editor_text_updated,
                    markdown_editor_selection_updated,
                    markdown_editor_scroll_updated,
                    tabs_changed,
                },
            redraw_in,
            copied_text,
            url_opened,
            cursor,
            virtual_keyboard_shown,
        } = value;

        Self {
            selected_file: selected_file.unwrap_or_default().into(),
            refresh_files: sync_done.is_some() || file_renamed.is_some() || file_created.is_some(),
            doc_created: match file_created {
                Some(Ok(f)) => {
                    if f.is_document() {
                        f.id.into()
                    } else {
                        Uuid::nil().into()
                    }
                }
                _ => Uuid::nil().into(),
            },
            new_folder_btn_pressed: new_folder_clicked,
            tabs_changed,
            redraw_in,
            copied_text,
            url_opened,
            cursor,
        }
    }
}
