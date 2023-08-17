use std::{collections::HashSet, path::PathBuf};

use lb::LbError;

#[derive(Default)]
pub struct NodeResponse {
    pub open_requests: HashSet<lb::Uuid>,
    pub new_doc_modal: Option<lb::File>,
    pub export_file: Option<Result<(lb::File, PathBuf), LbError>>,
    pub new_folder_modal: Option<lb::File>,
    pub create_share_modal: Option<lb::File>,
    pub rename_request: Option<(lb::Uuid, String)>,
    pub delete_request: bool,
    pub dropped_on: Option<lb::Uuid>,
}

impl NodeResponse {
    pub fn union(self, other: Self) -> Self {
        let mut this = self;
        this.new_doc_modal = this.new_doc_modal.or(other.new_doc_modal);
        this.new_folder_modal = this.new_folder_modal.or(other.new_folder_modal);
        this.create_share_modal = this.create_share_modal.or(other.create_share_modal);
        this.export_file = this.export_file.or(other.export_file);
        this.open_requests.extend(other.open_requests);
        this.rename_request = this.rename_request.or(other.rename_request);
        this.delete_request = this.delete_request || other.delete_request;
        this.dropped_on = this.dropped_on.or(other.dropped_on);
        this
    }
}
