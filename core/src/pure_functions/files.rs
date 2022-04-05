use libsecp256k1::PublicKey;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

use uuid::Uuid;

use lockbook_crypto::symkey;
use lockbook_models::account::Account;
use lockbook_models::file_metadata::{DecryptedFileMetadata, FileType, Owner};
use lockbook_models::tree::FileMetaExt;
use lockbook_models::tree::FileMetadata;

use crate::model::filename::NameComponents;
use crate::{model::repo::RepoState, CoreError};

pub fn single_or<T, E>(v: Vec<T>, e: E) -> Result<T, E> {
    let mut v = v;
    match &v[..] {
        [_v0] => Ok(v.remove(0)),
        _ => Err(e),
    }
}

pub fn create(
    file_type: FileType, parent: Uuid, name: &str, owner: &PublicKey,
) -> DecryptedFileMetadata {
    DecryptedFileMetadata {
        id: Uuid::new_v4(),
        file_type,
        parent,
        decrypted_name: String::from(name),
        owner: Owner(*owner),
        metadata_version: 0,
        content_version: 0,
        deleted: false,
        decrypted_access_key: symkey::generate_key(),
    }
}

pub fn create_root(account: &Account) -> DecryptedFileMetadata {
    let id = Uuid::new_v4();
    DecryptedFileMetadata {
        id,
        file_type: FileType::Folder,
        parent: id,
        decrypted_name: account.username.clone(),
        owner: Owner::from(account),
        metadata_version: 0,
        content_version: 0,
        deleted: false,
        decrypted_access_key: symkey::generate_key(),
    }
}

/// Validates a create operation for a file in the context of all files and returns a version of
/// the file with the operation applied. This is a pure function.
pub fn apply_create(
    files: &[DecryptedFileMetadata], file_type: FileType, parent: Uuid, name: &str,
    owner: &PublicKey,
) -> Result<DecryptedFileMetadata, CoreError> {
    let file = create(file_type, parent, name, owner);
    validate_not_root(&file)?;
    validate_file_name(name)?;
    let parent = files
        .maybe_find(parent)
        .ok_or(CoreError::FileParentNonexistent)?;
    validate_is_folder(&parent)?;

    if !files.get_path_conflicts(&[file.clone()])?.is_empty() {
        return Err(CoreError::PathTaken);
    }

    Ok(file)
}

/// Validates a rename operation for a file in the context of all files and returns a version of the file with the operation applied. This is a pure function.
pub fn apply_rename(
    files: &[DecryptedFileMetadata], target_id: Uuid, new_name: &str,
) -> Result<DecryptedFileMetadata, CoreError> {
    let mut file = files.find(target_id)?;
    validate_not_root(&file)?;
    validate_file_name(new_name)?;

    file.decrypted_name = String::from(new_name);
    if !files.get_path_conflicts(&[file.clone()])?.is_empty() {
        return Err(CoreError::PathTaken);
    }

    Ok(file)
}

/// Validates a move operation for a file in the context of all files and returns a version of the file with the operation applied. This is a pure function.
pub fn apply_move(
    files: &[DecryptedFileMetadata], target_id: Uuid, new_parent: Uuid,
) -> Result<DecryptedFileMetadata, CoreError> {
    let mut file = files.find(target_id)?;
    let parent = files
        .maybe_find(new_parent)
        .ok_or(CoreError::FileParentNonexistent)?;
    validate_not_root(&file)?;
    validate_is_folder(&parent)?;

    file.parent = new_parent;
    if !files.get_invalid_cycles(&[file.clone()])?.is_empty() {
        return Err(CoreError::FolderMovedIntoSelf);
    }
    if !files.get_path_conflicts(&[file.clone()])?.is_empty() {
        return Err(CoreError::PathTaken);
    }

    Ok(file)
}

/// Validates a delete operation for a file in the context of all files and returns a version of the
/// file with the operation applied. This is a pure function.
pub fn apply_delete(
    files: &[DecryptedFileMetadata], target_id: Uuid,
) -> Result<DecryptedFileMetadata, CoreError> {
    let mut file = files.find(target_id)?;
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

pub fn suggest_non_conflicting_filename(
    id: Uuid, files: &[DecryptedFileMetadata], staged_changes: &[DecryptedFileMetadata],
) -> Result<String, CoreError> {
    let files: Vec<DecryptedFileMetadata> = files
        .stage(staged_changes)
        .iter()
        .map(|(f, _)| f.clone())
        .collect();

    let file = files.find(id)?;
    let sibblings = files.find_children(file.parent);

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

pub fn find_state<Fm: FileMetadata>(
    files: &[RepoState<Fm>], target_id: Uuid,
) -> Result<RepoState<Fm>, CoreError> {
    maybe_find_state(files, target_id).ok_or(CoreError::FileNonexistent)
}

pub fn maybe_find_state<Fm: FileMetadata>(
    files: &[RepoState<Fm>], target_id: Uuid,
) -> Option<RepoState<Fm>> {
    files.iter().find(|f| match f {
        RepoState::New(l) => l.id(),
        RepoState::Modified { local: l, base: _ } => l.id(),
        RepoState::Unmodified(b) => b.id(),
    } == target_id).cloned()
}

pub fn find_ancestors<Fm: FileMetadata>(files: &[Fm], target_id: Uuid) -> Vec<Fm> {
    let mut result = Vec::new();
    let mut current_target_id = target_id;
    while let Some(target) = files.maybe_find(current_target_id) {
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
    files: &[Fm], target_id: Uuid,
) -> Result<Vec<Fm>, CoreError> {
    let mut result = vec![files.find(target_id)?];
    let mut i = 0;
    while i < result.len() {
        let target = result.get(i).ok_or_else(|| {
            CoreError::Unexpected(String::from("find_with_descendants: missing target"))
        })?;
        let children = files.find_children(target.id());
        for child in children {
            if child.id() != target_id {
                result.push(child);
            }
        }
        i += 1;
    }
    Ok(result)
}

pub fn is_deleted<Fm: FileMetadata>(files: &[Fm], target_id: Uuid) -> Result<bool, CoreError> {
    Ok(files
        .filter_deleted()?
        .into_iter()
        .any(|f| f.id() == target_id))
}

#[cfg(test)]
mod unit_tests {
    use lockbook_models::file_metadata::FileType;
    use lockbook_models::tree::{FileMetaExt, PathConflict};

    use crate::pure_functions::files::{self};
    use crate::{service::test_utils, CoreError};

    #[test]
    fn apply_rename() {
        let account = test_utils::generate_account();
        let root = files::create_root(&account);
        let folder = files::create(FileType::Folder, root.id, "folder", &account.public_key());
        let document =
            files::create(FileType::Document, root.id, "document", &account.public_key());

        let document_id = document.id;
        files::apply_rename(&[root, folder, document], document_id, "document2").unwrap();
    }

    #[test]
    fn apply_rename_not_found() {
        let account = test_utils::generate_account();
        let root = files::create_root(&account);
        let folder = files::create(FileType::Folder, root.id, "folder", &account.public_key());
        let document =
            files::create(FileType::Document, root.id, "document", &account.public_key());

        let result = files::apply_rename(&[root, folder], document.id, "document2");
        assert_eq!(result, Err(CoreError::FileNonexistent));
    }

    #[test]
    fn apply_rename_root() {
        let account = test_utils::generate_account();
        let root = files::create_root(&account);
        let folder = files::create(FileType::Folder, root.id, "folder", &account.public_key());
        let document =
            files::create(FileType::Document, root.id, "document", &account.public_key());

        let root_id = root.id;
        let result = files::apply_rename(&[root, folder, document], root_id, "root2");
        assert_eq!(result, Err(CoreError::RootModificationInvalid));
    }

    #[test]
    fn apply_rename_invalid_name() {
        let account = test_utils::generate_account();
        let root = files::create_root(&account);
        let folder = files::create(FileType::Folder, root.id, "folder", &account.public_key());
        let document =
            files::create(FileType::Document, root.id, "document", &account.public_key());

        let document_id = document.id;
        let result = files::apply_rename(&[root, folder, document], document_id, "invalid/name");
        assert_eq!(result, Err(CoreError::FileNameContainsSlash));
    }

    #[test]
    fn apply_rename_path_conflict() {
        let account = test_utils::generate_account();
        let root = files::create_root(&account);
        let folder = files::create(FileType::Folder, root.id, "folder", &account.public_key());
        let document1 =
            files::create(FileType::Document, root.id, "document1", &account.public_key());
        let document2 =
            files::create(FileType::Document, root.id, "document2", &account.public_key());

        let document1_id = document1.id;
        let result =
            files::apply_rename(&[root, folder, document1, document2], document1_id, "document2");
        assert_eq!(result, Err(CoreError::PathTaken));
    }

    #[test]
    fn apply_move() {
        let account = test_utils::generate_account();
        let root = files::create_root(&account);
        let folder = files::create(FileType::Folder, root.id, "folder", &account.public_key());
        let document =
            files::create(FileType::Document, root.id, "document", &account.public_key());

        let folder_id = folder.id;
        let document_id = document.id;
        files::apply_move(&[root, folder, document], document_id, folder_id).unwrap();
    }

    #[test]
    fn apply_move_not_found() {
        let account = test_utils::generate_account();
        let root = files::create_root(&account);
        let folder = files::create(FileType::Folder, root.id, "folder", &account.public_key());
        let document =
            files::create(FileType::Document, root.id, "document", &account.public_key());

        let folder_id = folder.id;
        let document_id = document.id;
        let result = files::apply_move(&[root, folder], document_id, folder_id);
        assert_eq!(result, Err(CoreError::FileNonexistent));
    }

    #[test]
    fn apply_move_parent_not_found() {
        let account = test_utils::generate_account();
        let root = files::create_root(&account);
        let folder = files::create(FileType::Folder, root.id, "folder", &account.public_key());
        let document =
            files::create(FileType::Document, root.id, "document", &account.public_key());

        let folder_id = folder.id;
        let document_id = document.id;
        let result = files::apply_move(&[root, document], document_id, folder_id);
        assert_eq!(result, Err(CoreError::FileParentNonexistent));
    }

    #[test]
    fn apply_move_parent_document() {
        let account = test_utils::generate_account();
        let root = files::create_root(&account);
        let document1 =
            files::create(FileType::Document, root.id, "document1", &account.public_key());
        let document2 =
            files::create(FileType::Document, root.id, "document2", &account.public_key());

        let document1_id = document1.id;
        let document2_id = document2.id;
        let result = files::apply_move(&[root, document1, document2], document2_id, document1_id);
        assert_eq!(result, Err(CoreError::FileNotFolder));
    }

    #[test]
    fn apply_move_root() {
        let account = test_utils::generate_account();
        let root = files::create_root(&account);
        let folder = files::create(FileType::Folder, root.id, "folder", &account.public_key());
        let document =
            files::create(FileType::Document, root.id, "document", &account.public_key());

        let folder_id = folder.id;
        let root_id = root.id;
        let result = files::apply_move(&[root, folder, document], root_id, folder_id);
        assert_eq!(result, Err(CoreError::RootModificationInvalid));
    }

    #[test]
    fn apply_move_path_conflict() {
        let account = test_utils::generate_account();
        let root = files::create_root(&account);
        let folder = files::create(FileType::Folder, root.id, "folder", &account.public_key());
        let document1 =
            files::create(FileType::Document, root.id, "document", &account.public_key());
        let document2 =
            files::create(FileType::Document, folder.id, "document", &account.public_key());

        let folder_id = folder.id;
        let document1_id = document1.id;
        let result =
            files::apply_move(&[root, folder, document1, document2], document1_id, folder_id);
        assert_eq!(result, Err(CoreError::PathTaken));
    }

    #[test]
    fn apply_move_2cycle() {
        let account = test_utils::generate_account();
        let root = files::create_root(&account);
        let folder1 = files::create(FileType::Folder, root.id, "folder1", &account.public_key());
        let folder2 = files::create(FileType::Folder, folder1.id, "folder2", &account.public_key());

        let folder1_id = folder1.id;
        let folder2_id = folder2.id;
        let result = files::apply_move(&[root, folder1, folder2], folder1_id, folder2_id);
        assert_eq!(result, Err(CoreError::FolderMovedIntoSelf));
    }

    #[test]
    fn apply_move_1cycle() {
        let account = test_utils::generate_account();
        let root = files::create_root(&account);
        let folder = files::create(FileType::Folder, root.id, "folder1", &account.public_key());

        let folder1_id = folder.id;
        let result = files::apply_move(&[root, folder], folder1_id, folder1_id);
        assert_eq!(result, Err(CoreError::FolderMovedIntoSelf));
    }

    #[test]
    fn apply_delete() {
        let account = test_utils::generate_account();
        let root = files::create_root(&account);
        let folder = files::create(FileType::Folder, root.id, "folder", &account.public_key());
        let document =
            files::create(FileType::Document, root.id, "document", &account.public_key());

        let document_id = document.id;
        files::apply_delete(&[root, folder, document], document_id).unwrap();
    }

    #[test]
    fn apply_delete_root() {
        let account = test_utils::generate_account();
        let root = files::create_root(&account);
        let folder = files::create(FileType::Folder, root.id, "folder", &account.public_key());
        let document =
            files::create(FileType::Document, root.id, "document", &account.public_key());

        let root_id = root.id;
        let result = files::apply_delete(&[root, folder, document], root_id);
        assert_eq!(result, Err(CoreError::RootModificationInvalid));
    }

    #[test]
    fn get_nonconflicting_filename() {
        let account = test_utils::generate_account();
        let root = files::create_root(&account);
        let folder = files::create(FileType::Folder, root.id, "folder", &account.public_key());
        assert_eq!(
            files::suggest_non_conflicting_filename(folder.id, &[root, folder], &[]).unwrap(),
            "folder-1"
        );
    }

    #[test]
    fn get_nonconflicting_filename2() {
        let account = test_utils::generate_account();
        let root = files::create_root(&account);
        let folder1 = files::create(FileType::Folder, root.id, "folder", &account.public_key());
        let folder2 = files::create(FileType::Folder, root.id, "folder-1", &account.public_key());
        assert_eq!(
            files::suggest_non_conflicting_filename(folder1.id, &[root, folder1, folder2], &[])
                .unwrap(),
            "folder-2"
        );
    }

    #[test]
    fn get_path_conflicts_no_conflicts() {
        let account = test_utils::generate_account();
        let root = files::create_root(&account);
        let folder1 = files::create(FileType::Folder, root.id, "folder", &account.public_key());
        let folder2 = files::create(FileType::Folder, root.id, "folder2", &account.public_key());

        let path_conflicts = &[root, folder1].get_path_conflicts(&[folder2]).unwrap();

        assert_eq!(path_conflicts.len(), 0);
    }

    #[test]
    fn get_path_conflicts_one_conflict() {
        let account = test_utils::generate_account();
        let root = files::create_root(&account);
        let folder1 = files::create(FileType::Folder, root.id, "folder", &account.public_key());
        let folder2 = files::create(FileType::Folder, root.id, "folder", &account.public_key());

        let path_conflicts = &[root, folder1.clone()]
            .get_path_conflicts(&[folder2.clone()])
            .unwrap();

        assert_eq!(path_conflicts.len(), 1);
        assert_eq!(path_conflicts[0], PathConflict { existing: folder1.id, staged: folder2.id });
    }
}
