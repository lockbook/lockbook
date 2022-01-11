use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

use uuid::Uuid;

use lockbook_crypto::symkey;
use lockbook_models::file_metadata::{DecryptedFileMetadata, FileMetadata, FileType};
use lockbook_models::utils::{maybe_find, maybe_find_mut};

use crate::model::filename::NameComponents;
use crate::{model::repo::RepoState, CoreError};

pub fn single_or<T, E>(v: Vec<T>, e: E) -> Result<T, E> {
    let mut v = v;
    match &v[..] {
        [_v0] => Ok(v.remove(0)),
        _ => Err(e),
    }
}

pub fn create(file_type: FileType, parent: Uuid, name: &str, owner: &str) -> DecryptedFileMetadata {
    DecryptedFileMetadata {
        id: Uuid::new_v4(),
        file_type,
        parent,
        decrypted_name: String::from(name),
        owner: String::from(owner),
        metadata_version: 0,
        content_version: 0,
        deleted: false,
        decrypted_access_key: symkey::generate_key(),
    }
}

pub fn create_root(username: &str) -> DecryptedFileMetadata {
    let id = Uuid::new_v4();
    DecryptedFileMetadata {
        id,
        file_type: FileType::Folder,
        parent: id,
        decrypted_name: String::from(username),
        owner: String::from(username),
        metadata_version: 0,
        content_version: 0,
        deleted: false,
        decrypted_access_key: symkey::generate_key(),
    }
}

/// Validates a create operation for a file in the context of all files and returns a version of
/// the file with the operation applied. This is a pure function.
pub fn apply_create(
    files: &[DecryptedFileMetadata],
    file_type: FileType,
    parent: Uuid,
    name: &str,
    owner: &str,
) -> Result<DecryptedFileMetadata, CoreError> {
    let file = create(file_type, parent, name, owner);
    validate_not_root(&file)?;
    validate_file_name(name)?;
    let parent = find(files, parent).map_err(|e| match e {
        CoreError::FileNonexistent => CoreError::FileParentNonexistent,
        e => e,
    })?;
    validate_is_folder(&parent)?;

    if !get_path_conflicts(files, &[file.clone()])?.is_empty() {
        return Err(CoreError::PathTaken);
    }
    if !get_invalid_cycles(files, &[file.clone()])?.is_empty() {
        return Err(CoreError::FolderMovedIntoSelf);
    }

    Ok(file)
}

/// Validates a rename operation for a file in the context of all files and returns a version of the file with the operation applied. This is a pure function.
pub fn apply_rename(
    files: &[DecryptedFileMetadata],
    target_id: Uuid,
    new_name: &str,
) -> Result<DecryptedFileMetadata, CoreError> {
    let mut file = find(files, target_id)?;
    validate_not_root(&file)?;
    validate_file_name(new_name)?;

    file.decrypted_name = String::from(new_name);
    if !get_path_conflicts(files, &[file.clone()])?.is_empty() {
        return Err(CoreError::PathTaken);
    }

    Ok(file)
}

/// Validates a move operation for a file in the context of all files and returns a version of the file with the operation applied. This is a pure function.
pub fn apply_move(
    files: &[DecryptedFileMetadata],
    target_id: Uuid,
    new_parent: Uuid,
) -> Result<DecryptedFileMetadata, CoreError> {
    let mut file = find(files, target_id)?;
    let parent = find(files, new_parent).map_err(|err| match err {
        CoreError::FileNonexistent => CoreError::FileParentNonexistent,
        e => e,
    })?;
    validate_not_root(&file)?;
    validate_is_folder(&parent)?;

    file.parent = new_parent;
    if !get_invalid_cycles(files, &[file.clone()])?.is_empty() {
        return Err(CoreError::FolderMovedIntoSelf);
    }
    if !get_path_conflicts(files, &[file.clone()])?.is_empty() {
        return Err(CoreError::PathTaken);
    }

    Ok(file)
}

/// Validates a delete operation for a file in the context of all files and returns a version of the
/// file with the operation applied. This is a pure function.
pub fn apply_delete(
    files: &[DecryptedFileMetadata],
    target_id: Uuid,
) -> Result<DecryptedFileMetadata, CoreError> {
    let mut file = find(files, target_id)?;
    validate_not_root(&file)?;

    file.deleted = true;

    Ok(file)
}

fn validate_not_root(file: &DecryptedFileMetadata) -> Result<(), CoreError> {
    if file.id != file.parent {
        Ok(())
    } else {
        Err(CoreError::RootModificationInvalid)
    }
}

fn validate_is_folder(file: &DecryptedFileMetadata) -> Result<(), CoreError> {
    if file.file_type == FileType::Folder {
        Ok(())
    } else {
        Err(CoreError::FileNotFolder)
    }
}

fn validate_file_name(name: &str) -> Result<(), CoreError> {
    if name.is_empty() {
        return Err(CoreError::FileNameEmpty);
    }
    if name.contains('/') {
        return Err(CoreError::FileNameContainsSlash);
    }
    Ok(())
}

pub fn get_invalid_cycles<Fm: FileMetadata>(
    files: &[Fm],
    staged_changes: &[Fm],
) -> Result<Vec<Uuid>, CoreError> {
    let maybe_root = maybe_find_root(files);
    let files_with_sources = stage(files, staged_changes);
    let files = &files_with_sources
        .iter()
        .map(|(f, _)| f.clone())
        .collect::<Vec<Fm>>();
    let mut result = Vec::new();
    let mut found_root = maybe_root.is_some();

    for file in files {
        let mut ancestor_single = find_parent(files, file.id())?;
        let mut ancestor_double = find_parent(files, ancestor_single.id())?;
        while ancestor_single.id() != ancestor_double.id() {
            ancestor_single = find_parent(files, ancestor_single.id())?;
            ancestor_double = find_parent(files, find_parent(files, ancestor_double.id())?.id())?;
        }
        if ancestor_single.id() == file.id() {
            // root in files -> non-root cycles invalid
            // no root in files -> accept first root from staged_changes
            if let Some(ref root) = maybe_root {
                if file.id() != root.id() {
                    result.push(file.id());
                }
            } else if !found_root {
                found_root = true;
            } else {
                result.push(file.id());
            }
        }
    }

    Ok(result)
}

#[derive(Debug, PartialEq)]
pub struct PathConflict {
    pub existing: Uuid,
    pub staged: Uuid,
}

pub fn get_path_conflicts<Fm: FileMetadata>(
    files: &[Fm],
    staged_changes: &[Fm],
) -> Result<Vec<PathConflict>, CoreError> {
    let files_with_sources = stage(files, staged_changes);
    let files = &files_with_sources
        .iter()
        .map(|(f, _)| f.clone())
        .collect::<Vec<Fm>>();
    let files = filter_not_deleted(files)?;
    let mut result = Vec::new();

    for file in &files {
        let children = find_children(&files, file.id());
        let mut child_ids_by_name: HashMap<Fm::Name, Uuid> = HashMap::new();
        for child in children {
            if let Some(conflicting_child_id) = child_ids_by_name.get(&child.name()) {
                let (_, child_source) = files_with_sources
                    .iter()
                    .find(|(f, _)| f.id() == child.id())
                    .ok_or_else(|| {
                        CoreError::Unexpected(String::from(
                            "get_path_conflicts: could not find child by id",
                        ))
                    })?;
                match child_source {
                    StageSource::Base => result.push(PathConflict {
                        existing: child.id(),
                        staged: conflicting_child_id.to_owned(),
                    }),
                    StageSource::Staged => result.push(PathConflict {
                        existing: conflicting_child_id.to_owned(),
                        staged: child.id(),
                    }),
                }
            }
            child_ids_by_name.insert(child.name(), child.id());
        }
    }

    Ok(result)
}

pub fn suggest_non_conflicting_filename(
    id: Uuid,
    files: &[DecryptedFileMetadata],
    staged_changes: &[DecryptedFileMetadata],
) -> Result<String, CoreError> {
    let files: Vec<DecryptedFileMetadata> = stage(files, staged_changes)
        .iter()
        .map(|(f, _)| f.clone())
        .collect();

    let file = find(&files, id)?;
    let sibblings = find_children(&files, file.parent);

    let mut new_name = NameComponents::from(&file.decrypted_name).generate_next();
    loop {
        if !sibblings
            .iter()
            .any(|f| f.decrypted_name == new_name.to_name())
        {
            return Ok(new_name.to_name());
        } else {
            new_name = new_name.generate_next();
        }
    }
}

pub fn save_document_to_disk(document: &[u8], location: String) -> Result<(), CoreError> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(Path::new(&location))
        .map_err(CoreError::from)?
        .write_all(document)
        .map_err(CoreError::from)?;
    Ok(())
}

pub fn find<Fm: FileMetadata>(files: &[Fm], target_id: Uuid) -> Result<Fm, CoreError> {
    maybe_find(files, target_id).ok_or(CoreError::FileNonexistent)
}

pub fn find_mut<Fm: FileMetadata>(files: &mut [Fm], target_id: Uuid) -> Result<&mut Fm, CoreError> {
    maybe_find_mut(files, target_id).ok_or(CoreError::FileNonexistent)
}

pub fn find_state<Fm: FileMetadata>(
    files: &[RepoState<Fm>],
    target_id: Uuid,
) -> Result<RepoState<Fm>, CoreError> {
    maybe_find_state(files, target_id).ok_or(CoreError::FileNonexistent)
}

pub fn maybe_find_state<Fm: FileMetadata>(
    files: &[RepoState<Fm>],
    target_id: Uuid,
) -> Option<RepoState<Fm>> {
    files.iter().find(|f| match f {
        RepoState::New(l) => l.id(),
        RepoState::Modified { local: l, base: _ } => l.id(),
        RepoState::Unmodified(b) => b.id(),
    } == target_id).cloned()
}

pub fn find_parent<Fm: FileMetadata>(files: &[Fm], target_id: Uuid) -> Result<Fm, CoreError> {
    maybe_find_parent(files, target_id).ok_or(CoreError::FileParentNonexistent)
}

pub fn maybe_find_parent<Fm: FileMetadata>(files: &[Fm], target_id: Uuid) -> Option<Fm> {
    let file = maybe_find(files, target_id)?;
    maybe_find(files, file.parent())
}

pub fn find_ancestors<Fm: FileMetadata>(files: &[Fm], target_id: Uuid) -> Vec<Fm> {
    let mut result = Vec::new();
    let mut current_target_id = target_id;
    while let Some(target) = maybe_find(files, current_target_id) {
        result.push(target.clone());
        if target.id() == target.parent() {
            break;
        }
        current_target_id = target.parent();
    }
    result
}

pub fn find_children<Fm: FileMetadata>(files: &[Fm], target_id: Uuid) -> Vec<Fm> {
    files
        .iter()
        .filter(|f| f.parent() == target_id && f.id() != f.parent())
        .cloned()
        .collect()
}

pub fn find_with_descendants<Fm: FileMetadata>(
    files: &[Fm],
    target_id: Uuid,
) -> Result<Vec<Fm>, CoreError> {
    let mut result = vec![find(files, target_id)?];
    let mut i = 0;
    while i < result.len() {
        let target = result.get(i).ok_or_else(|| {
            CoreError::Unexpected(String::from("find_with_descendants: missing target"))
        })?;
        let children = find_children(files, target.id());
        for child in children {
            if child.id() != target_id {
                result.push(child);
            }
        }
        i += 1;
    }
    Ok(result)
}

pub fn find_root<Fm: FileMetadata>(files: &[Fm]) -> Result<Fm, CoreError> {
    maybe_find_root(files).ok_or(CoreError::RootNonexistent)
}

pub fn maybe_find_root<Fm: FileMetadata>(files: &[Fm]) -> Option<Fm> {
    files.iter().find(|&f| f.id() == f.parent()).cloned()
}

pub fn is_deleted<Fm: FileMetadata>(files: &[Fm], target_id: Uuid) -> Result<bool, CoreError> {
    Ok(filter_deleted(files)?
        .into_iter()
        .any(|f| f.id() == target_id))
}

/// Returns the files which are not deleted and have no deleted ancestors. It is an error for the parent of a file argument not to also be included in the arguments.
pub fn filter_not_deleted<Fm: FileMetadata>(files: &[Fm]) -> Result<Vec<Fm>, CoreError> {
    let deleted = filter_deleted(files)?;
    Ok(files
        .iter()
        .filter(|f| !deleted.iter().any(|nd| nd.id() == f.id()))
        .cloned()
        .collect())
}

/// Returns the files which are deleted or have deleted ancestors. It is an error for the parent of a file argument not to also be included in the arguments.
pub fn filter_deleted<Fm: FileMetadata>(files: &[Fm]) -> Result<Vec<Fm>, CoreError> {
    let mut result = Vec::new();
    for file in files {
        let mut ancestor = file.clone();
        loop {
            if ancestor.deleted() {
                result.push(file.clone());
                break;
            }

            let parent = find(files, ancestor.parent())?;
            if ancestor.id() == parent.id() {
                break;
            }
            ancestor = parent;
            if ancestor.id() == file.id() {
                break; // this is a cycle but not our problem
            }
        }
    }
    Ok(result)
}

/// Returns the files which are documents.
pub fn filter_documents<Fm: FileMetadata>(files: &[Fm]) -> Vec<Fm> {
    files
        .iter()
        .filter(|f| f.file_type() == FileType::Document)
        .cloned()
        .collect()
}

pub enum StageSource {
    Base,
    Staged,
}

pub fn stage<Fm: FileMetadata>(files: &[Fm], staged_changes: &[Fm]) -> Vec<(Fm, StageSource)> {
    let mut result = Vec::new();
    for file in files {
        if let Some(ref staged) = maybe_find(staged_changes, file.id()) {
            result.push((staged.clone(), StageSource::Staged));
        } else {
            result.push((file.clone(), StageSource::Base));
        }
    }
    for staged in staged_changes {
        if maybe_find(files, staged.id()).is_none() {
            result.push((staged.clone(), StageSource::Staged));
        }
    }
    result
}

#[cfg(test)]
mod unit_tests {
    use lockbook_models::file_metadata::FileType;

    use crate::pure_functions::files::{self, PathConflict};
    use crate::{service::test_utils, CoreError};

    #[test]
    fn apply_rename() {
        let account = test_utils::generate_account();
        let root = files::create_root(&account.username);
        let folder = files::create(FileType::Folder, root.id, "folder", &account.username);
        let document = files::create(FileType::Document, root.id, "document", &account.username);

        let document_id = document.id;
        files::apply_rename(&[root, folder, document], document_id, "document2").unwrap();
    }

    #[test]
    fn apply_rename_not_found() {
        let account = test_utils::generate_account();
        let root = files::create_root(&account.username);
        let folder = files::create(FileType::Folder, root.id, "folder", &account.username);
        let document = files::create(FileType::Document, root.id, "document", &account.username);

        let result = files::apply_rename(&[root, folder], document.id, "document2");
        assert_eq!(result, Err(CoreError::FileNonexistent));
    }

    #[test]
    fn apply_rename_root() {
        let account = test_utils::generate_account();
        let root = files::create_root(&account.username);
        let folder = files::create(FileType::Folder, root.id, "folder", &account.username);
        let document = files::create(FileType::Document, root.id, "document", &account.username);

        let root_id = root.id;
        let result = files::apply_rename(&[root, folder, document], root_id, "root2");
        assert_eq!(result, Err(CoreError::RootModificationInvalid));
    }

    #[test]
    fn apply_rename_invalid_name() {
        let account = test_utils::generate_account();
        let root = files::create_root(&account.username);
        let folder = files::create(FileType::Folder, root.id, "folder", &account.username);
        let document = files::create(FileType::Document, root.id, "document", &account.username);

        let document_id = document.id;
        let result = files::apply_rename(&[root, folder, document], document_id, "invalid/name");
        assert_eq!(result, Err(CoreError::FileNameContainsSlash));
    }

    #[test]
    fn apply_rename_path_conflict() {
        let account = test_utils::generate_account();
        let root = files::create_root(&account.username);
        let folder = files::create(FileType::Folder, root.id, "folder", &account.username);
        let document1 = files::create(FileType::Document, root.id, "document1", &account.username);
        let document2 = files::create(FileType::Document, root.id, "document2", &account.username);

        let document1_id = document1.id;
        let result = files::apply_rename(
            &[root, folder, document1, document2],
            document1_id,
            "document2",
        );
        assert_eq!(result, Err(CoreError::PathTaken));
    }

    #[test]
    fn apply_move() {
        let account = test_utils::generate_account();
        let root = files::create_root(&account.username);
        let folder = files::create(FileType::Folder, root.id, "folder", &account.username);
        let document = files::create(FileType::Document, root.id, "document", &account.username);

        let folder_id = folder.id;
        let document_id = document.id;
        files::apply_move(&[root, folder, document], document_id, folder_id).unwrap();
    }

    #[test]
    fn apply_move_not_found() {
        let account = test_utils::generate_account();
        let root = files::create_root(&account.username);
        let folder = files::create(FileType::Folder, root.id, "folder", &account.username);
        let document = files::create(FileType::Document, root.id, "document", &account.username);

        let folder_id = folder.id;
        let document_id = document.id;
        let result = files::apply_move(&[root, folder], document_id, folder_id);
        assert_eq!(result, Err(CoreError::FileNonexistent));
    }

    #[test]
    fn apply_move_parent_not_found() {
        let account = test_utils::generate_account();
        let root = files::create_root(&account.username);
        let folder = files::create(FileType::Folder, root.id, "folder", &account.username);
        let document = files::create(FileType::Document, root.id, "document", &account.username);

        let folder_id = folder.id;
        let document_id = document.id;
        let result = files::apply_move(&[root, document], document_id, folder_id);
        assert_eq!(result, Err(CoreError::FileParentNonexistent));
    }

    #[test]
    fn apply_move_parent_document() {
        let account = test_utils::generate_account();
        let root = files::create_root(&account.username);
        let document1 = files::create(FileType::Document, root.id, "document1", &account.username);
        let document2 = files::create(FileType::Document, root.id, "document2", &account.username);

        let document1_id = document1.id;
        let document2_id = document2.id;
        let result = files::apply_move(&[root, document1, document2], document2_id, document1_id);
        assert_eq!(result, Err(CoreError::FileNotFolder));
    }

    #[test]
    fn apply_move_root() {
        let account = test_utils::generate_account();
        let root = files::create_root(&account.username);
        let folder = files::create(FileType::Folder, root.id, "folder", &account.username);
        let document = files::create(FileType::Document, root.id, "document", &account.username);

        let folder_id = folder.id;
        let root_id = root.id;
        let result = files::apply_move(&[root, folder, document], root_id, folder_id);
        assert_eq!(result, Err(CoreError::RootModificationInvalid));
    }

    #[test]
    fn apply_move_path_conflict() {
        let account = test_utils::generate_account();
        let root = files::create_root(&account.username);
        let folder = files::create(FileType::Folder, root.id, "folder", &account.username);
        let document1 = files::create(FileType::Document, root.id, "document", &account.username);
        let document2 = files::create(FileType::Document, folder.id, "document", &account.username);

        let folder_id = folder.id;
        let document1_id = document1.id;
        let result = files::apply_move(
            &[root, folder, document1, document2],
            document1_id,
            folder_id,
        );
        assert_eq!(result, Err(CoreError::PathTaken));
    }

    #[test]
    fn apply_move_2cycle() {
        let account = test_utils::generate_account();
        let root = files::create_root(&account.username);
        let folder1 = files::create(FileType::Folder, root.id, "folder1", &account.username);
        let folder2 = files::create(FileType::Folder, folder1.id, "folder2", &account.username);

        let folder1_id = folder1.id;
        let folder2_id = folder2.id;
        let result = files::apply_move(&[root, folder1, folder2], folder1_id, folder2_id);
        assert_eq!(result, Err(CoreError::FolderMovedIntoSelf));
    }

    #[test]
    fn apply_move_1cycle() {
        let account = test_utils::generate_account();
        let root = files::create_root(&account.username);
        let folder = files::create(FileType::Folder, root.id, "folder1", &account.username);

        let folder1_id = folder.id;
        let result = files::apply_move(&[root, folder], folder1_id, folder1_id);
        assert_eq!(result, Err(CoreError::FolderMovedIntoSelf));
    }

    #[test]
    fn apply_delete() {
        let account = test_utils::generate_account();
        let root = files::create_root(&account.username);
        let folder = files::create(FileType::Folder, root.id, "folder", &account.username);
        let document = files::create(FileType::Document, root.id, "document", &account.username);

        let document_id = document.id;
        files::apply_delete(&[root, folder, document], document_id).unwrap();
    }

    #[test]
    fn apply_delete_root() {
        let account = test_utils::generate_account();
        let root = files::create_root(&account.username);
        let folder = files::create(FileType::Folder, root.id, "folder", &account.username);
        let document = files::create(FileType::Document, root.id, "document", &account.username);

        let root_id = root.id;
        let result = files::apply_delete(&[root, folder, document], root_id);
        assert_eq!(result, Err(CoreError::RootModificationInvalid));
    }

    #[test]
    fn get_nonconflicting_filename() {
        let account = test_utils::generate_account();
        let root = files::create_root(&account.username);
        let folder = files::create(FileType::Folder, root.id, "folder", &account.username);
        assert_eq!(
            files::suggest_non_conflicting_filename(folder.id, &[root, folder], &[]).unwrap(),
            "folder-1"
        );
    }

    #[test]
    fn get_nonconflicting_filename2() {
        let account = test_utils::generate_account();
        let root = files::create_root(&account.username);
        let folder1 = files::create(FileType::Folder, root.id, "folder", &account.username);
        let folder2 = files::create(FileType::Folder, root.id, "folder-1", &account.username);
        assert_eq!(
            files::suggest_non_conflicting_filename(folder1.id, &[root, folder1, folder2], &[])
                .unwrap(),
            "folder-2"
        );
    }

    #[test]
    fn get_path_conflicts_no_conflicts() {
        let account = test_utils::generate_account();
        let root = files::create_root(&account.username);
        let folder1 = files::create(FileType::Folder, root.id, "folder", &account.username);
        let folder2 = files::create(FileType::Folder, root.id, "folder2", &account.username);

        let path_conflicts =
            files::get_path_conflicts(&[root, folder1.clone()], &[folder2.clone()]).unwrap();

        assert_eq!(path_conflicts.len(), 0);
    }

    #[test]
    fn get_path_conflicts_one_conflict() {
        let account = test_utils::generate_account();
        let root = files::create_root(&account.username);
        let folder1 = files::create(FileType::Folder, root.id, "folder", &account.username);
        let folder2 = files::create(FileType::Folder, root.id, "folder", &account.username);

        let path_conflicts =
            files::get_path_conflicts(&[root, folder1.clone()], &[folder2.clone()]).unwrap();

        assert_eq!(path_conflicts.len(), 1);
        assert_eq!(
            path_conflicts[0],
            PathConflict {
                existing: folder1.id,
                staged: folder2.id
            }
        );
    }
}
