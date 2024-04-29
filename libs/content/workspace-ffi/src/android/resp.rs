use egui_editor::{
    input::canonical::{Location, Region},
    offset_types::{DocCharOffset, RelCharOffset},
};
use lb_external_interface::lb_rs::Uuid;
use serde::Serialize;
use utf16string::WString;
use workspace_rs::output::WsOutput;

#[derive(Serialize, Default)]
pub struct IntegrationOutput {
    pub workspace_resp: FfiWorkspaceResp,
    pub redraw_in: u64,
    pub has_copied_text: bool,
    pub copied_text: String,
    pub url_opened: String,
}

#[derive(Serialize, Default)]
pub struct FfiWorkspaceResp {
    selected_file: Uuid,
    doc_created: Uuid,

    pub status_updated: bool,
    refresh_files: bool,

    new_folder_btn_pressed: bool,
    pub tab_title_clicked: bool,

    pub show_edit_menu: bool,
    pub edit_menu_x: f32,
    pub edit_menu_y: f32,

    pub selection_updated: bool,
    pub text_updated: bool,
}

impl From<WsOutput> for FfiWorkspaceResp {
    fn from(value: WsOutput) -> Self {
        Self {
            selected_file: value.selected_file.unwrap_or_default(),
            status_updated: value.status_updated,
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
            tab_title_clicked: value.tab_title_clicked,
            show_edit_menu: value.markdown_editor_show_edit_menu,
            edit_menu_x: value.markdown_editor_edit_menu_x,
            edit_menu_y: value.markdown_editor_edit_menu_y,

            selection_updated: value.markdown_editor_selection_updated,
            text_updated: value.markdown_editor_text_updated,
        }
    }
}

// uses utf16 encoding
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
