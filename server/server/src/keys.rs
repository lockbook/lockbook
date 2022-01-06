use lockbook_models::file_metadata::FileMetadata;
use uuid::Uuid;

pub fn public_key(username: &str) -> String {
    format!("account:{}:public_key", username)
}

pub fn owned_files(username: &str) -> String {
    format!("account:{}:owned_files", username)
}

pub fn file(id: Uuid) -> String {
    format!("file:{}", id)
}

pub fn meta<File: FileMetadata>(meta: &File) -> String {
    file(meta.id())
}
