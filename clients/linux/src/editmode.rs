use lockbook_core::model::client_conversion::ClientFileMetadata;

pub enum EditMode {
    Folder {
        path: String,
        meta: ClientFileMetadata,
        n_children: usize,
    },

    PlainText {
        path: String,
        meta: ClientFileMetadata,
        content: String,
    },

    None,
}
