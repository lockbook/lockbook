use lb_c::Uuid;
use lb_c::model::text::offset_types::{Grapheme, Graphemes};
use serde::Serialize;
use workspace_rs::tab::markdown_editor::input::{Location, Region};

#[derive(Serialize)]
pub struct AndroidResponse {
    // platform response
    pub redraw_in: u64,
    pub copied_text: String,
    pub has_url_opened: bool,
    pub url_opened: String,
    pub virtual_keyboard_shown: Option<bool>,

    // widget response
    pub selected_file: Uuid,
    pub doc_created: Uuid,

    pub tabs_changed: bool,

    pub has_edit_menu: bool,
    pub edit_menu_x: f32,
    pub edit_menu_y: f32,

    pub selection_updated: bool,
    pub text_updated: bool,
}

impl From<crate::Response> for AndroidResponse {
    fn from(value: crate::Response) -> Self {
        let crate::Response {
            workspace:
                workspace_rs::Response {
                    selected_file,
                    file_renamed: _,
                    file_moved: _,
                    file_deleted: _,
                    new_folder_clicked: _,
                    tab_title_clicked: _,
                    file_created,
                    markdown_editor_text_updated,
                    markdown_editor_selection_updated,
                    markdown_editor_scroll_updated: _,
                    markdown_editor_find_widget_height: _,
                    tabs_changed,
                    failure_messages: _,
                    selected_folder_changed: _,
                    open_camera: _,
                    file_cache_updated: _,
                },
            redraw_in,
            copied_text,
            urls_opened,
            cursor: _,
            virtual_keyboard_shown,
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
            tabs_changed,
            redraw_in: redraw_in.unwrap_or(u64::MAX),
            copied_text,
            has_url_opened: !urls_opened.is_empty(),
            url_opened: urls_opened.into_iter().next().unwrap_or_default(),
            text_updated: markdown_editor_text_updated,
            selection_updated: markdown_editor_selection_updated,
            has_edit_menu: context_menu.is_some(),
            edit_menu_x: context_menu.unwrap_or_default().x,
            edit_menu_y: context_menu.unwrap_or_default().y,
            virtual_keyboard_shown,
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

impl From<JTextRange> for (Grapheme, Grapheme) {
    fn from(value: JTextRange) -> Self {
        (value.start.into(), value.end.into())
    }
}

impl From<JTextRange> for (Graphemes, Graphemes) {
    fn from(value: JTextRange) -> Self {
        (value.start.into(), value.end.into())
    }
}

impl From<JTextRange> for Region {
    fn from(value: JTextRange) -> Self {
        Region::BetweenLocations {
            start: Location::Grapheme(value.start.into()),
            end: Location::Grapheme(value.start.into()),
        }
    }
}
