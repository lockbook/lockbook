use lb_c::Uuid;
use lb_c::model::text::offset_types::{DocCharOffset, RelCharOffset};
use serde::Serialize;
use workspace_rs::tab::markdown_editor::input::{Location, Region};

#[derive(Serialize)]
pub struct AndroidResponse {
    // platform response
    redraw_in: u64,
    copied_text: String,
    has_url_opened: bool,
    url_opened: String,

    // widget response
    selected_file: Uuid,
    doc_created: Uuid,

    status_updated: bool,
    refresh_files: bool,

    new_folder_btn_pressed: bool,
    tab_title_clicked: bool,

    has_edit_menu: bool,
    edit_menu_x: f32,
    edit_menu_y: f32,

    selection_updated: bool,
    text_updated: bool,
}

impl From<crate::Response> for AndroidResponse {
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
                    markdown_editor_scroll_updated: _,
                    tabs_changed: _,
                    failure_messages: _,
                    selected_folder_changed: _,
                },
            redraw_in,
            copied_text,
            url_opened,
            cursor: _,
            virtual_keyboard_shown: _,
            window_title: _,
            request_paste: _,
            context_menu,
        } = value;

        let doc_created = match file_created {
            Some(Ok(ref f)) if f.is_document() => f.id,
            _ => Uuid::nil(),
        };
        Self {
            selected_file: selected_file.unwrap_or_default(),
            doc_created,
            status_updated,
            refresh_files: sync_done.is_some() || file_renamed.is_some() || file_created.is_some(),
            new_folder_btn_pressed: new_folder_clicked,
            tab_title_clicked,
            redraw_in: redraw_in.unwrap_or(u64::MAX),
            copied_text,
            has_url_opened: url_opened.is_some(),
            url_opened: url_opened.unwrap_or_default(),
            text_updated: markdown_editor_text_updated,
            selection_updated: markdown_editor_selection_updated,
            has_edit_menu: context_menu.is_some(),
            edit_menu_x: context_menu.unwrap_or_default().x,
            edit_menu_y: context_menu.unwrap_or_default().y,
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
