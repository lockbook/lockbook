use libsecp256k1::PublicKey;

use lockbook_models::crypto::{UserAccessInfo, UserAccessMode};
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

use uuid::Uuid;

use lockbook_crypto::{pubkey, symkey};
use lockbook_models::account::Account;
use lockbook_models::file_metadata::{DecryptedFileMetadata, DecryptedFiles, FileType, Owner};
use lockbook_models::tree::{FileMetaMapExt, FileMetaVecExt, FileMetadata};

use crate::model::errors::core_err_unexpected;
use crate::model::filename::NameComponents;
use crate::service::file_encryption_service;
use crate::{model::repo::RepoState, CoreError};

pub fn create(
    file_type: FileType, parent: Uuid, name: &str, owner: &PublicKey,
) -> DecryptedFileMetadata {
    DecryptedFileMetadata {
        id: Uuid::new_v4(),
        file_type,
        parent,
        decrypted_name: String::from(name),
        owner: Owner(*owner),
        shares: Vec::new(),
        metadata_version: 0,
        content_version: 0,
        deleted: false,
        decrypted_access_key: symkey::generate_key(),
        folder_access_key: None,
    }
}

pub fn create_root(account: &Account) -> Result<DecryptedFileMetadata, CoreError> {
    let id = Uuid::new_v4();
    let decrypted_access_key = symkey::generate_key();
    let public_key = account.public_key();
    let share_key =
        pubkey::get_aes_key(&account.private_key, &public_key).map_err(core_err_unexpected)?;
    let shares = vec![UserAccessInfo {
        mode: UserAccessMode::Owner,
        encrypted_by_username: account.username.clone(),
        encrypted_by_public_key: public_key,
        encrypted_for_username: account.username.clone(),
        encrypted_for_public_key: public_key,
        access_key: file_encryption_service::encrypt_user_access_key(
            &decrypted_access_key,
            &account.private_key,
            &public_key,
        )?,
        file_name: file_encryption_service::encrypt_file_name(&account.username, &share_key)?,
        marked_for_deletion: false,
    }];
    Ok(DecryptedFileMetadata {
        id,
        file_type: FileType::Folder,
        parent: id,
        decrypted_name: account.username.clone(),
        owner: Owner::from(account),
        shares,
        metadata_version: 0,
        content_version: 0,
        deleted: false,
        decrypted_access_key,
        folder_access_key: None,
    })
}

/// Validates a create operation for a file in the context of all files and returns a version of
/// the file with the operation applied. This is a pure function.
pub fn apply_create(
    user: &Owner, files: &DecryptedFiles, file_type: FileType, parent: Uuid, name: &str,
) -> Result<DecryptedFileMetadata, CoreError> {
    let file = create(file_type, parent, name, &user.0);
    validate_not_root(&file)?;
    validate_file_name(name)?;
    let parent = files
        .maybe_find(parent)
        .ok_or(CoreError::FileParentNonexistent)?;
    validate_is_folder(&parent)?;

    if let FileType::Link { linked_file } = file_type {
        match files.maybe_find_ref(linked_file) {
            Some(link_target) => {
                if &link_target.owner == user {
                    return Err(CoreError::LinkTargetIsOwned);
                }
            }
            None => {
                return Err(CoreError::LinkTargetNonexistent);
            }
        }
    }

    let staged_changes = HashMap::with(file.clone());
    if !files.get_path_conflicts(&staged_changes)?.is_empty() {
        return Err(CoreError::PathTaken);
    }
    if !files.get_shared_links(user, &staged_changes)?.is_empty() {
        return Err(CoreError::LinkInSharedFolder);
    }
    if !files.get_duplicate_links(&staged_changes)?.is_empty() {
        return Err(CoreError::MultipleLinksToSameFile);
    }
    if files.get_access_level(user, parent.id)? < UserAccessMode::Write {
        return Err(CoreError::InsufficientPermission);
    }

    Ok(file)
}

/// Validates a rename operation for a file in the context of all files and returns a version of the file with the operation applied. This is a pure function.
pub fn apply_rename(
    user: &Owner, files: &DecryptedFiles, target_id: Uuid, new_name: &str,
) -> Result<DecryptedFileMetadata, CoreError> {
    let mut file = files.find(target_id)?;
    validate_not_root(&file)?;
    validate_file_name(new_name)?;

    if files.get_access_level(user, target_id)? < UserAccessMode::Write
        || file.is_shared_with_user(user)
    {
        return Err(CoreError::InsufficientPermission);
    }

    file.decrypted_name = String::from(new_name);
    if !files
        .get_path_conflicts(&[file.clone()].to_map())?
        .is_empty()
    {
        return Err(CoreError::PathTaken);
    }

    Ok(file)
}

/// Validates a move operation for a file in the context of all files and returns a version of the file with the operation applied. This is a pure function.
pub fn apply_move(
    user: &Owner, files: &DecryptedFiles, target_id: Uuid, new_parent: Uuid,
) -> Result<DecryptedFileMetadata, CoreError> {
    let mut file = files.find(target_id)?;
    let parent = files
        .maybe_find(new_parent)
        .ok_or(CoreError::FileParentNonexistent)?;
    validate_not_root(&file)?;
    validate_is_folder(&parent)?;

    if files.get_access_level(user, target_id)? < UserAccessMode::Write
        || file.is_shared_with_user(user)
    {
        return Err(CoreError::InsufficientPermission);
    }

    file.parent = new_parent;
    let staged_changes = HashMap::with(file.clone());
    if !files.get_invalid_cycles(user, &staged_changes)?.is_empty() {
        return Err(CoreError::FolderMovedIntoSelf);
    }
    if !files.get_path_conflicts(&staged_changes)?.is_empty() {
        return Err(CoreError::PathTaken);
    }
    if !files.get_shared_links(user, &staged_changes)?.is_empty() {
        return Err(CoreError::LinkInSharedFolder);
    }

    Ok(file)
}

/// Validates a delete operation for a file in the context of all files and returns a version of the
/// file with the operation applied. This is a pure function.
pub fn apply_delete(
    user: &Owner, files: &DecryptedFiles, target_id: Uuid,
) -> Result<DecryptedFileMetadata, CoreError> {
    let mut file = files.find(target_id)?;
    validate_not_root(&file)?;

    if files.get_access_level(user, target_id)? < UserAccessMode::Write
        || file.is_shared_with_user(user)
    {
        return Err(CoreError::InsufficientPermission);
    }

    file.deleted = true;
    Ok(file)
}

pub fn validate_not_root(file: &DecryptedFileMetadata) -> Result<(), CoreError> {
    if file.id != file.parent {
        Ok(())
    } else {
        Err(CoreError::RootModificationInvalid)
    }
}

pub fn validate_is_folder(file: &DecryptedFileMetadata) -> Result<(), CoreError> {
    if file.is_folder() {
        Ok(())
    } else {
        Err(CoreError::FileNotFolder)
    }
}

pub fn validate_file_name(name: &str) -> Result<(), CoreError> {
    if name.is_empty() {
        return Err(CoreError::FileNameEmpty);
    }
    if name.contains('/') {
        return Err(CoreError::FileNameContainsSlash);
    }
    Ok(())
}

pub fn suggest_non_conflicting_filename(
    id: Uuid, files: &DecryptedFiles, staged_changes: &DecryptedFiles,
) -> Result<String, CoreError> {
    let files: DecryptedFiles = files
        .stage_with_source(staged_changes)
        .into_iter()
        .map(|(id, (f, _))| (id, f))
        .collect::<DecryptedFiles>();

    let file = files.find(id)?;
    let sibblings = files.find_children(file.parent);

    let mut new_name = NameComponents::from(&file.decrypted_name).generate_next();
    loop {
        if !sibblings
            .values()
            .any(|f| f.decrypted_name == new_name.to_name())
        {
            return Ok(new_name.to_name());
        } else {
            new_name = new_name.generate_next();
        }
    }
}

// TODO this is not a pure function and should be relocated
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

pub fn maybe_find_state<Fm: FileMetadata>(
    files: &[RepoState<Fm>], target_id: Uuid,
) -> Option<RepoState<Fm>> {
    files.iter().find(|f| match f {
        RepoState::New(l) => l.id(),
        RepoState::Modified { local: l, base: _ } => l.id(),
        RepoState::Unmodified(b) => b.id(),
    } == target_id).cloned()
}
