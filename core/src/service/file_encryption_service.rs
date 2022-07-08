use std::collections::HashMap;

use libsecp256k1::{PublicKey, SecretKey};
use uuid::Uuid;

use lockbook_crypto::{pubkey, symkey};
use lockbook_models::account::Account;
use lockbook_models::crypto::*;
use lockbook_models::file_metadata::{
    DecryptedFileMetadata, DecryptedFiles, EncryptedFileMetadata, EncryptedFiles,
};
use lockbook_models::tree::FileMetaMapExt;

use crate::model::errors::{core_err_unexpected, CoreError};

/// Converts a DecryptedFileMetadata to a FileMetadata using its decrypted parent key. Sharing is
/// not supported; user access keys are encrypted for the provided account. This is a pure function.
pub fn encrypt_metadatum(
    parent_key: &AESKey, target: &DecryptedFileMetadata,
) -> Result<EncryptedFileMetadata, CoreError> {
    Ok(EncryptedFileMetadata {
        id: target.id,
        file_type: target.file_type,
        parent: target.parent,
        name: encrypt_file_name(&target.decrypted_name, parent_key)?,
        owner: target.owner.clone(),
        metadata_version: target.metadata_version,
        content_version: target.content_version,
        deleted: target.deleted,
        user_access_keys: target.shares.clone(),
        folder_access_key: encrypt_folder_access_keys(&target.decrypted_access_key, parent_key)?,
    })
}

pub fn encrypt_metadata(
    account: &Account, files: &DecryptedFiles,
) -> Result<EncryptedFiles, CoreError> {
    let mut result = HashMap::new();
    for target in files.values() {
        if let Some(user_access) = target
            .shares
            .iter()
            .find(|k| k.encrypted_for_username == account.username)
        {
            let share_key =
                pubkey::get_aes_key(&account.private_key, &user_access.encrypted_by_public_key)
                    .map_err(core_err_unexpected)?;
            let folder_access_key = if target.id == target.parent {
                encrypt_folder_access_keys(
                    &target.decrypted_access_key,
                    &target.decrypted_access_key,
                )?
            } else {
                target.folder_access_key.clone().ok_or_else(|| {
                    core_err_unexpected("unshared decrypted metadata with no folder key")
                })?
            };
            result.push(EncryptedFileMetadata {
                id: target.id,
                file_type: target.file_type,
                parent: target.parent,
                name: encrypt_file_name(&target.decrypted_name, &share_key)?,
                owner: target.owner.clone(),
                metadata_version: target.metadata_version,
                content_version: target.content_version,
                deleted: target.deleted,
                // shares are marked for deletion so that they live long enough to re-encrypt the metadata
                user_access_keys: target
                    .shares
                    .iter()
                    .filter(|s| !s.marked_for_deletion)
                    .cloned()
                    .collect(),
                folder_access_key, // todo(sharing): do something better?
            });
        } else {
            let parent_key = files
                .iter()
                .find(|(_, m)| m.id == target.parent)
                .ok_or_else(|| {
                    CoreError::Unexpected(String::from(
                        "parent metadata missing during call to file_encrpytion_service::encrypt_metadata",
                    ))
                })?
                .1.decrypted_access_key;
            result.push(encrypt_metadatum(&parent_key, target)?);
        }
    }
    Ok(result)
}

pub fn encrypt_file_name(
    decrypted_name: &str, parent_key: &AESKey,
) -> Result<SecretFileName, CoreError> {
    symkey::encrypt_and_hmac(parent_key, decrypted_name).map_err(core_err_unexpected)
}

pub fn encrypt_user_access_key(
    decrypted_file_key: &AESKey, sharer_secret_key: &SecretKey, sharee_public_key: &PublicKey,
) -> Result<AESEncrypted<[u8; 32]>, CoreError> {
    let user_key =
        pubkey::get_aes_key(sharer_secret_key, sharee_public_key).map_err(core_err_unexpected)?;
    let encrypted_file_key =
        symkey::encrypt(&user_key, decrypted_file_key).map_err(core_err_unexpected)?;
    Ok(encrypted_file_key)
}

fn encrypt_folder_access_keys(
    target_key: &AESKey, parent_key: &AESKey,
) -> Result<EncryptedFolderAccessKey, CoreError> {
    symkey::encrypt(parent_key, target_key).map_err(core_err_unexpected)
}

/// Converts a FileMetadata to a DecryptedFileMetadata using its decrypted parent key. Sharing is
/// not supported; user access keys not for the provided account are ignored. This is a pure function.
pub fn decrypt_metadatum(
    parent_key: &AESKey, target: &EncryptedFileMetadata,
) -> Result<DecryptedFileMetadata, CoreError> {
    Ok(DecryptedFileMetadata {
        id: target.id,
        file_type: target.file_type,
        parent: target.parent,
        decrypted_name: decrypt_file_name(&target.name, parent_key)?,
        owner: target.owner.clone(),
        shares: target.user_access_keys.clone(),
        metadata_version: target.metadata_version,
        content_version: target.content_version,
        deleted: target.deleted,
        decrypted_access_key: decrypt_folder_access_keys(&target.folder_access_key, parent_key)?,
        folder_access_key: Some(target.folder_access_key.clone()),
    })
}

/// Converts a set of FileMetadata's to DecryptedFileMetadata's. All parents of files must be
/// included in files, unless they are shared to this account. This is a pure function.
pub fn decrypt_metadata(
    account: &Account, files: &EncryptedFiles, key_cache: &mut HashMap<Uuid, AESKey>,
) -> Result<DecryptedFiles, CoreError> {
    let mut result = HashMap::new();

    for target in files.values() {
        // todo(sharing): weird code duplication with decrypt_file_key
        if let Some(user_access) = target
            .user_access_keys
            .iter()
            .find(|k| k.encrypted_for_username == account.username)
        {
            let user_access_key =
                pubkey::get_aes_key(&account.private_key, &user_access.encrypted_by_public_key)
                    .map_err(core_err_unexpected)?;
            let file_key = symkey::decrypt(&user_access_key, &user_access.access_key)
                .map_err(core_err_unexpected)?;

            result.push(DecryptedFileMetadata {
                id: target.id,
                file_type: target.file_type,
                parent: target.parent,
                decrypted_name: decrypt_file_name(&user_access.file_name, &user_access_key)?,
                owner: target.owner.clone(),
                shares: target.user_access_keys.clone(),
                metadata_version: target.metadata_version,
                content_version: target.content_version,
                deleted: target.deleted,
                decrypted_access_key: file_key,
                folder_access_key: Some(target.folder_access_key.clone()),
            });
        } else {
            if files.maybe_find_ref(target.parent).is_none()
                && target.owner.0 != account.public_key()
            {
                // file was shared, then the share was deleted, and now it's not decryptable
                continue;
            } else {
                let maybe_parent_key = decrypt_file_key(account, target.parent, files, key_cache)?;
                if let Some(parent_key) = maybe_parent_key {
                    result.push(decrypt_metadatum(&parent_key, target)?);
                }
            }
        }
    }
    Ok(result)
}

/// Decrypts the file key given a target and its ancestors. All ancestors of target, as well as
/// target itself, must be included in target_with_ancestors.
fn decrypt_file_key(
    account: &Account, target_id: Uuid, target_with_ancestors: &EncryptedFiles,
    key_cache: &mut HashMap<Uuid, AESKey>,
) -> Result<Option<AESKey>, CoreError> {
    if let Some(key) = key_cache.get(&target_id) {
        return Ok(Some(*key));
    }

    let target = target_with_ancestors.maybe_find(target_id).ok_or_else(|| {
        CoreError::Unexpected(String::from(
            "target or ancestor missing during call to file_encryption_service::decrypt_file_key",
        ))
    })?;

    let maybe_key = match target
        .user_access_keys
        .iter()
        .find(|k| k.encrypted_for_username == account.username)
    {
        Some(user_access) => {
            let user_access_key =
                pubkey::get_aes_key(&account.private_key, &user_access.encrypted_by_public_key)
                    .map_err(core_err_unexpected)?;
            Some(
                symkey::decrypt(&user_access_key, &user_access.access_key)
                    .map_err(core_err_unexpected)?,
            )
        }
        None => {
            let maybe_parent_key =
                decrypt_file_key(account, target.parent, target_with_ancestors, key_cache)?;
            if let Some(parent_key) = maybe_parent_key {
                Some(
                    symkey::decrypt(&parent_key, &target.folder_access_key)
                        .map_err(core_err_unexpected)?,
                )
            } else {
                None
            }
        }
    };

    if let Some(key) = maybe_key {
        key_cache.insert(target_id, key);
    }

    Ok(maybe_key)
}

fn decrypt_file_name(
    encrypted_name: &SecretFileName, parent_key: &AESKey,
) -> Result<String, CoreError> {
    symkey::decrypt_and_verify(parent_key, encrypted_name).map_err(core_err_unexpected)
}

fn decrypt_folder_access_keys(
    encrypted_keys: &EncryptedFolderAccessKey, parent_key: &AESKey,
) -> Result<AESKey, CoreError> {
    symkey::decrypt(parent_key, encrypted_keys).map_err(core_err_unexpected)
}

pub fn encrypt_document(
    document: &[u8], metadata: &DecryptedFileMetadata,
) -> Result<EncryptedDocument, CoreError> {
    symkey::encrypt(&metadata.decrypted_access_key, &document.to_vec()).map_err(core_err_unexpected)
}

pub fn decrypt_document(
    document: &EncryptedDocument, metadata: &DecryptedFileMetadata,
) -> Result<DecryptedDocument, CoreError> {
    symkey::decrypt(&metadata.decrypted_access_key, document).map_err(core_err_unexpected)
}
