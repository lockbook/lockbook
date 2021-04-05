use lockbook_models::file_metadata::FileMetadata;

pub enum EditMode {
    Folder {
        path: String,
        meta: FileMetadata,
        n_children: usize,
    },

    PlainText {
        path: String,
        meta: FileMetadata,
        content: String,
    },

    None,
}
