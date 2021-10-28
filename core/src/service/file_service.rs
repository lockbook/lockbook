use crate::utils::StageSource;
use crate::{utils, CoreError};
use lockbook_crypto::symkey;
use lockbook_models::file_metadata::{DecryptedFileMetadata, FileMetadata, FileType};
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use uuid::Uuid;

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
    let parent = utils::find(files, parent).map_err(|e| match e {
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
    let mut file = utils::find(files, target_id)?;
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
    let mut file = utils::find(files, target_id)?;
    let parent = utils::find_parent(files, target_id)?;
    validate_not_root(&file)?;
    validate_is_folder(&parent)?;

    file.parent = new_parent;
    if !get_path_conflicts(files, &[file.clone()])?.is_empty() {
        return Err(CoreError::PathTaken);
    }
    if !get_invalid_cycles(files, &[file.clone()])?.is_empty() {
        return Err(CoreError::FolderMovedIntoSelf);
    }

    Ok(file)
}

/// Validates a delete operation for a file in the context of all files and returns a version of the
/// file with the operation applied. This is a pure function.
pub fn apply_delete(
    files: &[DecryptedFileMetadata],
    target_id: Uuid,
) -> Result<DecryptedFileMetadata, CoreError> {
    let mut file = utils::find(files, target_id)?;
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

pub fn get_invalid_cycles_encrypted(
    files: &[FileMetadata],
    staged_changes: &[FileMetadata],
) -> Result<Vec<Uuid>, CoreError> {
    let maybe_root = utils::maybe_find_root_encrypted(files);
    let files_with_sources = utils::stage_encrypted(files, staged_changes);
    let files = &files_with_sources
        .iter()
        .map(|(f, _)| f.clone())
        .collect::<Vec<FileMetadata>>();
    let mut result = Vec::new();
    let mut found_root = maybe_root.is_some();

    for file in files {
        let mut ancestor_single = utils::find_parent_encrypted(files, file.id)?;
        let mut ancestor_double = utils::find_parent_encrypted(files, ancestor_single.id)?;
        while ancestor_single.id != ancestor_double.id {
            ancestor_single = utils::find_parent_encrypted(files, ancestor_single.id)?;
            ancestor_double = utils::find_parent_encrypted(
                files,
                utils::find_parent_encrypted(files, ancestor_double.id)?.id,
            )?;
        }
        if ancestor_single.id == file.id {
            // root in files -> non-root cycles invalid
            // no root in files -> accept first root from staged_changes
            if let Some(ref root) = maybe_root {
                if file.id != root.id {
                    result.push(file.id);
                }
            } else if !found_root {
                found_root = true;
            } else {
                result.push(file.id);
            }
        }
    }

    Ok(result)
}

pub fn get_invalid_cycles(
    files: &[DecryptedFileMetadata],
    staged_changes: &[DecryptedFileMetadata],
) -> Result<Vec<Uuid>, CoreError> {
    let maybe_root = utils::maybe_find_root(files);
    let files_with_sources = utils::stage(files, staged_changes);
    let files = &files_with_sources
        .iter()
        .map(|(f, _)| f.clone())
        .collect::<Vec<DecryptedFileMetadata>>();
    let mut result = Vec::new();
    let mut found_root = maybe_root.is_some();

    for file in files {
        let mut ancestor_single = utils::find_parent(files, file.id)?;
        let mut ancestor_double = utils::find_parent(files, ancestor_single.id)?;
        while ancestor_single.id != ancestor_double.id {
            ancestor_single = utils::find_parent(files, ancestor_single.id)?;
            ancestor_double =
                utils::find_parent(files, utils::find_parent(files, ancestor_double.id)?.id)?;
        }
        if ancestor_single.id == file.id {
            // root in files -> non-root cycles invalid
            // no root in files -> accept first root from staged_changes
            if let Some(ref root) = maybe_root {
                if file.id != root.id {
                    result.push(file.id);
                }
            } else if !found_root {
                found_root = true;
            } else {
                result.push(file.id);
            }
        }
    }

    Ok(result)
}

pub struct PathConflict {
    pub existing: Uuid,
    pub staged: Uuid,
}

pub fn get_path_conflicts(
    files: &[DecryptedFileMetadata],
    staged_changes: &[DecryptedFileMetadata],
) -> Result<Vec<PathConflict>, CoreError> {
    let files_with_sources = utils::stage(files, staged_changes);
    let files = &files_with_sources
        .iter()
        .map(|(f, _)| f.clone())
        .collect::<Vec<DecryptedFileMetadata>>();
    let mut result = Vec::new();

    for file in files {
        let children = utils::find_children(files, file.id);
        let mut child_ids_by_name: HashMap<String, Uuid> = HashMap::new();
        for child in children {
            if let Some(conflicting_child_id) = child_ids_by_name.get(&child.decrypted_name) {
                let (_, child_source) = files_with_sources
                    .iter()
                    .find(|(f, _)| f.id == child.id)
                    .ok_or_else(|| {
                    CoreError::Unexpected(String::from(
                        "get_path_conflicts: could not find child by id",
                    ))
                })?;
                match child_source {
                    StageSource::Base => result.push(PathConflict {
                        existing: child.id,
                        staged: conflicting_child_id.to_owned(),
                    }),
                    StageSource::Staged => result.push(PathConflict {
                        existing: conflicting_child_id.to_owned(),
                        staged: child.id,
                    }),
                }
            }
            child_ids_by_name.insert(child.decrypted_name, child.id);
        }
    }

    Ok(result)
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

#[cfg(test)]
mod unit_tests {
    use lockbook_models::file_metadata::FileType;

    use crate::{
        service::{file_service, test_utils},
        CoreError,
    };

    #[test]
    fn apply_rename() {
        let account = test_utils::generate_account();
        let root = file_service::create_root(&account.username);
        let folder = file_service::create(FileType::Folder, root.id, "folder", &account.username);
        let document =
            file_service::create(FileType::Document, root.id, "document", &account.username);

        let document_id = document.id;
        file_service::apply_rename(&[root, folder, document], document_id, "document2").unwrap();
    }

    #[test]
    fn apply_rename_not_found() {
        let account = test_utils::generate_account();
        let root = file_service::create_root(&account.username);
        let folder = file_service::create(FileType::Folder, root.id, "folder", &account.username);
        let document =
            file_service::create(FileType::Document, root.id, "document", &account.username);

        let result = file_service::apply_rename(&[root, folder], document.id, "document2");
        assert_eq!(result, Err(CoreError::FileNonexistent));
    }

    #[test]
    fn apply_rename_root() {
        let account = test_utils::generate_account();
        let root = file_service::create_root(&account.username);
        let folder = file_service::create(FileType::Folder, root.id, "folder", &account.username);
        let document =
            file_service::create(FileType::Document, root.id, "document", &account.username);

        let root_id = root.id;
        let result = file_service::apply_rename(&[root, folder, document], root_id, "root2");
        assert_eq!(result, Err(CoreError::RootModificationInvalid));
    }

    #[test]
    fn apply_rename_invalid_name() {
        let account = test_utils::generate_account();
        let root = file_service::create_root(&account.username);
        let folder = file_service::create(FileType::Folder, root.id, "folder", &account.username);
        let document =
            file_service::create(FileType::Document, root.id, "document", &account.username);

        let document_id = document.id;
        let result =
            file_service::apply_rename(&[root, folder, document], document_id, "invalid/name");
        assert_eq!(result, Err(CoreError::FileNameContainsSlash));
    }

    #[test]
    fn apply_rename_path_conflict() {
        let account = test_utils::generate_account();
        let root = file_service::create_root(&account.username);
        let folder = file_service::create(FileType::Folder, root.id, "folder", &account.username);
        let document1 =
            file_service::create(FileType::Document, root.id, "document1", &account.username);
        let document2 =
            file_service::create(FileType::Document, root.id, "document2", &account.username);

        let document1_id = document1.id;
        let result = file_service::apply_rename(
            &[root, folder, document1, document2],
            document1_id,
            "document2",
        );
        assert_eq!(result, Err(CoreError::PathTaken));
    }

    #[test]
    fn apply_move() {
        let account = test_utils::generate_account();
        let root = file_service::create_root(&account.username);
        let folder = file_service::create(FileType::Folder, root.id, "folder", &account.username);
        let document =
            file_service::create(FileType::Document, root.id, "document", &account.username);

        let folder_id = folder.id;
        let document_id = document.id;
        file_service::apply_move(&[root, folder, document], document_id, folder_id).unwrap();
    }

    #[test]
    fn apply_move_not_found() {
        let account = test_utils::generate_account();
        let root = file_service::create_root(&account.username);
        let folder = file_service::create(FileType::Folder, root.id, "folder", &account.username);
        let document =
            file_service::create(FileType::Document, root.id, "document", &account.username);

        let folder_id = folder.id;
        let document_id = document.id;
        let result = file_service::apply_move(&[root, folder], document_id, folder_id);
        assert_eq!(result, Err(CoreError::FileNonexistent));
    }

    #[test]
    fn apply_move_parent_not_found() {
        let account = test_utils::generate_account();
        let root = file_service::create_root(&account.username);
        let folder = file_service::create(FileType::Folder, root.id, "folder", &account.username);
        let document =
            file_service::create(FileType::Document, root.id, "document", &account.username);

        let folder_id = folder.id;
        let document_id = document.id;
        let result = file_service::apply_move(&[root, document], document_id, folder_id);
        assert_eq!(result, Err(CoreError::FileParentNonexistent));
    }

    #[test]
    fn apply_move_parent_document() {
        let account = test_utils::generate_account();
        let root = file_service::create_root(&account.username);
        let document1 =
            file_service::create(FileType::Document, root.id, "document1", &account.username);
        let document2 =
            file_service::create(FileType::Document, root.id, "document2", &account.username);

        let document1_id = document1.id;
        let document2_id = document2.id;
        let result =
            file_service::apply_move(&[root, document1, document2], document2_id, document1_id);
        assert_eq!(result, Err(CoreError::FileNotFolder));
    }

    #[test]
    fn apply_move_root() {
        let account = test_utils::generate_account();
        let root = file_service::create_root(&account.username);
        let folder = file_service::create(FileType::Folder, root.id, "folder", &account.username);
        let document =
            file_service::create(FileType::Document, root.id, "document", &account.username);

        let folder_id = folder.id;
        let root_id = root.id;
        let result = file_service::apply_move(&[root, folder, document], root_id, folder_id);
        assert_eq!(result, Err(CoreError::RootModificationInvalid));
    }

    #[test]
    fn apply_move_path_conflict() {
        let account = test_utils::generate_account();
        let root = file_service::create_root(&account.username);
        let folder = file_service::create(FileType::Folder, root.id, "folder", &account.username);
        let document1 =
            file_service::create(FileType::Document, root.id, "document", &account.username);
        let document2 =
            file_service::create(FileType::Document, folder.id, "document", &account.username);

        let folder_id = folder.id;
        let document1_id = document1.id;
        let result = file_service::apply_move(
            &[root, folder, document1, document2],
            document1_id,
            folder_id,
        );
        assert_eq!(result, Err(CoreError::PathTaken));
    }

    #[test]
    fn apply_move_2cycle() {
        let account = test_utils::generate_account();
        let root = file_service::create_root(&account.username);
        let folder1 = file_service::create(FileType::Folder, root.id, "folder1", &account.username);
        let folder2 =
            file_service::create(FileType::Folder, folder1.id, "folder2", &account.username);

        let folder1_id = folder1.id;
        let folder2_id = folder2.id;
        let result = file_service::apply_move(&[root, folder1, folder2], folder1_id, folder2_id);
        assert_eq!(result, Err(CoreError::FolderMovedIntoSelf));
    }

    #[test]
    fn apply_move_1cycle() {
        let account = test_utils::generate_account();
        let root = file_service::create_root(&account.username);
        let folder = file_service::create(FileType::Folder, root.id, "folder1", &account.username);

        let folder1_id = folder.id;
        let result = file_service::apply_move(&[root, folder], folder1_id, folder1_id);
        assert_eq!(result, Err(CoreError::FolderMovedIntoSelf));
    }

    #[test]
    fn apply_delete() {
        let account = test_utils::generate_account();
        let root = file_service::create_root(&account.username);
        let folder = file_service::create(FileType::Folder, root.id, "folder", &account.username);
        let document =
            file_service::create(FileType::Document, root.id, "document", &account.username);

        let document_id = document.id;
        file_service::apply_delete(&[root, folder, document], document_id).unwrap();
    }

    #[test]
    fn apply_delete_root() {
        let account = test_utils::generate_account();
        let root = file_service::create_root(&account.username);
        let folder = file_service::create(FileType::Folder, root.id, "folder", &account.username);
        let document =
            file_service::create(FileType::Document, root.id, "document", &account.username);

        let root_id = root.id;
        let result = file_service::apply_delete(&[root, folder, document], root_id);
        assert_eq!(result, Err(CoreError::RootModificationInvalid));
    }
}
