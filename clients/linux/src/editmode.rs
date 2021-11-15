use lockbook_core::model::client_conversion::ClientFileMetadata;

pub enum EditMode {
    Folder {
        meta: ClientFileMetadata,
        n_children: usize,
    },

    PlainText {
        meta: ClientFileMetadata,
        content: String,
    },

    None,
}
