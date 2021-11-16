use lockbook_models::file_metadata::DecryptedFileMetadata;

pub enum EditMode {
    Folder {
        path: String,
        meta: DecryptedFileMetadata,
        n_children: usize,
    },

    PlainText {
        path: String,
        meta: DecryptedFileMetadata,
        content: String,
    },

    None,
}
