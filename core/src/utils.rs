use lockbook_crypto::symkey;
use lockbook_models::file_metadata::{DecryptedFileMetadata, FileMetadata, FileType};
use std::collections::HashMap;
use uuid::Uuid;

pub fn metadata_vec_to_map(metadata: Vec<FileMetadata>) -> HashMap<Uuid, FileMetadata> {
    metadata.into_iter().map(|m| (m.id, m)).collect()
}

// https://stackoverflow.com/a/58175659/4638697
pub fn slices_equal<T: PartialEq>(a: &[T], b: &[T]) -> bool {
    let matching = a.iter().zip(b.iter()).filter(|&(a, b)| a == b).count();
    matching == a.len() && matching == b.len()
}

pub fn new_decrypted_metadata(
    file_type: FileType,
    parent: Uuid,
    name: &str,
    owner: &str,
) -> DecryptedFileMetadata {
    DecryptedFileMetadata {
        id: Uuid::new_v4(),
        file_type,
        parent,
        decrypted_name: String::from(name),
        owner: String::from(owner),
        metadata_version: 0,
        content_version: 0,
        deleted: false,
        decrypted_user_access_key: None,
        decrypted_folder_access_keys: symkey::generate_key(),
    }
}

pub fn new_decrypted_root_metadata(username: &str) -> DecryptedFileMetadata {
    let id = Uuid::new_v4();
    let key = symkey::generate_key();
    DecryptedFileMetadata {
        id,
        file_type: FileType::Folder,
        parent: id,
        decrypted_name: String::from(username),
        owner: String::from(username),
        metadata_version: 0,
        content_version: 0,
        deleted: false,
        decrypted_user_access_key: Some(key),
        decrypted_folder_access_keys: key,
    }
}
