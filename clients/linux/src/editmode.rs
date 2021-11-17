use lockbook_models::file_metadata::DecryptedFileMetadata;

pub enum EditMode {
    Folder {
        meta: DecryptedFileMetadata,
        n_children: usize,
    },

    PlainText {
        meta: DecryptedFileMetadata,
        content: String,
    },

    None,
}
