use lb_external_interface::lb_rs::Uuid;
use serde::Serialize;
use workspace_rs::tab::markdown_editor::{
    input::canonical::{Location, Region},
    offset_types::{DocCharOffset, RelCharOffset},
};

#[derive(Serialize)]
pub struct Response {
    // widget response
    selected_file: Uuid,
    doc_created: Uuid,

    status_updated: bool,
    refresh_files: bool,

    new_folder_btn_pressed: bool,
    tab_title_clicked: bool,

    show_edit_menu: bool,
    edit_menu_x: f32,
    edit_menu_y: f32,

    selection_updated: bool,
    text_updated: bool,

    // platform response
    redraw_in: u64,
    has_copied_text: bool,
    copied_text: String,
    has_url_opened: bool,
    url_opened: String,
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
            window_title,
            context_menu,
        } = value;

        let doc_created = match file_created {
            Some(Ok(f)) if f.is_document() => f.id.into(),
            _ => Uuid::nil().into(),
        };
        Self {
            selected_file: selected_file.unwrap_or_default().into(),
            doc_created,
            status_updated,
            refresh_files: sync_done.is_some() || file_renamed.is_some() || file_created.is_some(),
            new_folder_btn_pressed: new_folder_clicked,
            tab_title_clicked,
            redraw_in,
            copied_text,
            url_opened,
            text_updated: markdown_editor_text_updated,
            selection_updated: markdown_editor_selection_updated,
            show_edit_menu: context_menu.is_some(),
            edit_menu_x: context_menu.unwrap_or_default().x,
            edit_menu_y: context_menu.unwrap_or_default().y,
            has_copied_text: todo!(),
            has_url_opened: todo!(),
        }
    }
}

#[derive(Serialize, Default)]
pub struct JTextPosition {
    pub none: bool,
    pub position: usize,
}

#[derive(Serialize, Default)]
pub struct JTextRange {
    pub none: bool,
    pub start: usize,
    pub end: usize,
}

#[derive(Serialize, Default)]
pub struct JRect {
    pub min_x: f32,
    pub min_y: f32,
    pub max_x: f32,
    pub max_y: f32,
}

impl From<JTextRange> for (DocCharOffset, DocCharOffset) {
    fn from(value: JTextRange) -> Self {
        (value.start.into(), value.end.into())
    }
}

impl From<JTextRange> for (RelCharOffset, RelCharOffset) {
    fn from(value: JTextRange) -> Self {
        (value.start.into(), value.end.into())
    }
}

impl From<JTextRange> for Region {
    fn from(value: JTextRange) -> Self {
        Region::BetweenLocations {
            start: Location::DocCharOffset(value.start.into()),
            end: Location::DocCharOffset(value.start.into()),
        }
    }
}
