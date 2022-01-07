use uuid::Uuid;
use crate::file_metadata::FileMetadata;

// https://stackoverflow.com/a/58175659/4638697
pub fn slices_equal<T: PartialEq>(a: &[T], b: &[T]) -> bool {
    let matching = a.iter().zip(b.iter()).filter(|&(a, b)| a == b).count();
    matching == a.len() && matching == b.len()
}

pub fn maybe_find<Fm: FileMetadata>(files: &[Fm], target_id: Uuid) -> Option<Fm> {
    files.iter().find(|f| f.id() == target_id).cloned()
}
