use std::collections::HashSet;

#[derive(Default)]
pub struct NodeResponse {
    pub open_requests: HashSet<lb::Uuid>,
    pub new_doc_modal: Option<lb::File>,
    pub new_folder_modal: Option<lb::File>,
    pub rename_request: Option<(lb::Uuid, String)>,
    pub delete_request: bool,
    pub dropped_on: Option<lb::Uuid>,
}

impl NodeResponse {
    pub fn union(self, other: Self) -> Self {
        let mut this = self;
        this.new_doc_modal = this.new_doc_modal.or(other.new_doc_modal);
        this.new_folder_modal = this.new_folder_modal.or(other.new_folder_modal);
        this.open_requests.extend(other.open_requests);
        this.rename_request = this.rename_request.or(other.rename_request);
        this.delete_request = this.delete_request || other.delete_request;
        this.dropped_on = this.dropped_on.or(other.dropped_on);
        this
    }
}
