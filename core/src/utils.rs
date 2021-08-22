use lockbook_models::file_metadata::{DecryptedFileMetadata, FileMetadata, FileType};
use uuid::Uuid;

use crate::CoreError;

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

pub fn find(
    files: &[DecryptedFileMetadata],
    target_id: Uuid,
) -> Result<DecryptedFileMetadata, CoreError> {
    maybe_find(files, target_id).ok_or(CoreError::FileNonexistent)
}

pub fn maybe_find(
    files: &[DecryptedFileMetadata],
    target_id: Uuid,
) -> Option<DecryptedFileMetadata> {
    files.iter().find(|f| f.id == target_id).map(|f| f.clone())
}

pub fn find_mut(
    files: &mut [DecryptedFileMetadata],
    target_id: Uuid,
) -> Result<&mut DecryptedFileMetadata, CoreError> {
    maybe_find_mut(files, target_id).ok_or(CoreError::FileNonexistent)
}

pub fn maybe_find_mut(
    files: &mut [DecryptedFileMetadata],
    target_id: Uuid,
) -> Option<&mut DecryptedFileMetadata> {
    files.iter_mut().find(|f| f.id == target_id)
}

pub fn maybe_find_encrypted(files: &[FileMetadata], target_id: Uuid) -> Option<FileMetadata> {
    files.iter().find(|f| f.id == target_id).map(|f| f.clone())
}

pub fn find_parent(
    files: &[DecryptedFileMetadata],
    target_id: Uuid,
) -> Result<DecryptedFileMetadata, CoreError> {
    maybe_find_parent(files, target_id).ok_or(CoreError::FileParentNonexistent)
}

pub fn maybe_find_parent(
    files: &[DecryptedFileMetadata],
    target_id: Uuid,
) -> Option<DecryptedFileMetadata> {
    let file = maybe_find(files, target_id)?;
    maybe_find(files, file.parent)
}

pub fn find_children(
    files: &[DecryptedFileMetadata],
    target_id: Uuid,
) -> Vec<DecryptedFileMetadata> {
    files
        .iter()
        .filter(|f| f.parent == target_id)
        .map(|f| f.clone())
        .collect()
}

pub fn find_with_descendants(
    files: &[DecryptedFileMetadata],
    target_id: Uuid,
) -> Result<Vec<DecryptedFileMetadata>, CoreError> {
    let mut result = Vec::new();
    result.push(find(files, target_id)?);
    let mut i = 0;
    while i < result.len() {
        let target = result.get(i).ok_or(CoreError::Unexpected(String::from(
            "filter_deleted: missing target",
        )))?;
        let children = find_children(files, target.id);
        for child in children {
            result.push(child);
        }
        i += 1;
    }
    Ok(result)
}

pub fn find_root(files: &[DecryptedFileMetadata]) -> Result<DecryptedFileMetadata, CoreError> {
    maybe_find_root(files).ok_or(CoreError::RootNonexistent)
}

pub fn maybe_find_root(files: &[DecryptedFileMetadata]) -> Option<DecryptedFileMetadata> {
    files.iter().find(|f| f.id == f.parent).map(|f| f.clone())
}

/// Returns the files which are not deleted and have no deleted ancestors.
pub fn filter_not_deleted(
    files: &[DecryptedFileMetadata],
) -> Result<Vec<DecryptedFileMetadata>, CoreError> {
    let mut result = Vec::new();
    result.push(find_root(files)?);
    let mut i = 0;
    while i < result.len() {
        let target = result.get(i).ok_or(CoreError::Unexpected(String::from(
            "filter_deleted: missing target",
        )))?;
        let children = find_children(files, target.id);
        for child in children {
            if !child.deleted {
                result.push(child);
            }
        }
        i += 1;
    }
    Ok(result)
}

/// Returns the files which are deleted or have deleted ancestors.
pub fn filter_deleted(
    files: &[DecryptedFileMetadata],
) -> Result<Vec<DecryptedFileMetadata>, CoreError> {
    let not_deleted = filter_not_deleted(&files)?;
    Ok(files
        .iter()
        .filter(|f| !not_deleted.iter().any(|nd| nd.id == f.id))
        .map(|f| f.clone())
        .collect())
}

/// Returns the files which are documents.
pub fn filter_documents(files: &[DecryptedFileMetadata]) -> Vec<DecryptedFileMetadata> {
    files
        .iter()
        .filter(|f| f.file_type == FileType::Document)
        .map(|f| f.clone())
        .collect()
}

pub enum StageSource {
    Base,
    Staged,
}

pub fn stage(
    files: &[DecryptedFileMetadata],
    staged_changes: &[DecryptedFileMetadata],
) -> Vec<(DecryptedFileMetadata, StageSource)> {
    let mut result = Vec::new();
    for file in files {
        if let Some(ref staged) = maybe_find(staged_changes, file.id) {
            result.push((staged.clone(), StageSource::Staged));
        } else {
            result.push((file.clone(), StageSource::Base));
        }
    }
    for staged in staged_changes {
        if maybe_find(files, staged.id).is_none() {
            result.push((staged.clone(), StageSource::Staged));
        }
    }
    result
}

pub fn stage_encrypted(
    files: &[FileMetadata],
    staged_changes: &[FileMetadata],
) -> Vec<(FileMetadata, StageSource)> {
    let mut result = Vec::new();
    for file in files {
        if let Some(ref staged) = maybe_find_encrypted(staged_changes, file.id) {
            result.push((staged.clone(), StageSource::Staged));
        } else {
            result.push((file.clone(), StageSource::Base));
        }
    }
    for staged in staged_changes {
        if maybe_find_encrypted(files, staged.id).is_none() {
            result.push((staged.clone(), StageSource::Staged));
        }
    }
    result
}
