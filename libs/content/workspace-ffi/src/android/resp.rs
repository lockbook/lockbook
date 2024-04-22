use egui_editor::{
    input::canonical::{Location, Region},
    offset_types::{DocCharOffset, RelCharOffset},
};
use lb_external_interface::lb_rs::Uuid;
use serde::Serialize;
use workspace_rs::output::WsOutput;

#[derive(Serialize, Default)]
pub struct IntegrationOutput {
    pub workspace_resp: FfiWorkspaceResp,
    pub redraw_in: u128,
    pub copied_text: String,
    pub url_opened: String,
}

#[derive(Serialize, Default)]
pub struct FfiWorkspaceResp {
    selected_file: Uuid,
    doc_created: Uuid,

    pub msg: String,
    syncing: bool,
    refresh_files: bool,

    new_folder_btn_pressed: bool,
    pub tab_title_clicked: bool,
}

impl From<WsOutput> for FfiWorkspaceResp {
    fn from(value: WsOutput) -> Self {
        Self {
            selected_file: value.selected_file.unwrap_or_default(),
            msg: value.status.message,
            syncing: value.status.syncing,
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
        }
    }
}

#[derive(Serialize, Default)]
pub struct JTextRange {
    pub none: bool,
    pub start: usize,
    pub end: usize,
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
