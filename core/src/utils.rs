use lockbook_models::file_metadata::FileMetadata;
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

pub fn single_or<T, E>(v: Vec<T>, e: E) -> Result<T, E> {
    let mut v = v;
    match &v[..] {
        [_v0] => Ok(v.remove(0)),
        _ => Err(e),
    }
}
