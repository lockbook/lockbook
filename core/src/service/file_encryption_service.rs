use crate::{core_err_unexpected, CoreError};
use lockbook_crypto::{pubkey, symkey};
use lockbook_models::account::Account;
use lockbook_models::crypto::*;
use lockbook_models::file_metadata::{DecryptedFileMetadata, FileMetadata};
use std::collections::HashMap;
use uuid::Uuid;

/// Converts a DecryptedFileMetadata to a FileMetadata using its decrypted parent key. Sharing is not supported; user access keys are encrypted for the provided account. This is a pure function.
pub fn encrypt_metadatum(
    account: &Account,
    parent_key: &AESKey,
    target: &DecryptedFileMetadata,
) -> Result<FileMetadata, CoreError> {
    let user_access_keys = if target.id == target.parent {
        encrypt_user_access_keys(account, &target.decrypted_access_key)?
    } else {
        Default::default()
    };
    Ok(FileMetadata {
        id: target.id,
        file_type: target.file_type,
        parent: target.parent,
        name: encrypt_file_name(&target.decrypted_name, &parent_key)?,
        owner: target.owner.clone(),
        metadata_version: target.metadata_version,
        content_version: target.content_version,
        deleted: target.deleted,
        user_access_keys,
        folder_access_keys: encrypt_folder_access_keys(&target.decrypted_access_key, &parent_key)?,
    })
}

/// Converts a set of DecryptedFileMetadata's to FileMetadata's. All parents of files must be included in files. Sharing is not supported; user access keys are encrypted for the provided account. This is a pure function.
pub fn encrypt_metadata(
    account: &Account,
    files: &[DecryptedFileMetadata],
) -> Result<Vec<FileMetadata>, CoreError> {
    let mut result = Vec::new();
    for target in files {
        let parent_key = files
            .iter()
            .find(|m| m.id == target.parent)
            .ok_or(CoreError::Unexpected(String::from(
                "parent metadata missing during call to file_encrpytion_service::encrypt_metadata",
            )))?
            .decrypted_access_key;
        result.push(encrypt_metadatum(account, &parent_key, target)?);
    }
    Ok(result)
}

fn encrypt_file_name(
    decrypted_name: &str,
    parent_key: &AESKey,
) -> Result<SecretFileName, CoreError> {
    symkey::encrypt_and_hmac(parent_key, decrypted_name).map_err(core_err_unexpected)
}

fn encrypt_user_access_keys(
    account: &Account,
    decrypted_file_key: &AESKey,
) -> Result<HashMap<String, UserAccessInfo>, CoreError> {
    let user_key = pubkey::get_aes_key(&account.private_key, &account.public_key())
        .map_err(core_err_unexpected)?;
    let encrypted_file_key =
        symkey::encrypt(&user_key, decrypted_file_key).map_err(core_err_unexpected)?;
    let mut result = HashMap::new();
    result.insert(
        account.username.clone(),
        UserAccessInfo {
            username: account.username.clone(),
            encrypted_by: account.public_key(),
            access_key: encrypted_file_key,
        },
    );
    Ok(result)
}

fn encrypt_folder_access_keys(
    target_key: &AESKey,
    parent_key: &AESKey,
) -> Result<EncryptedFolderAccessKey, CoreError> {
    symkey::encrypt(parent_key, target_key).map_err(core_err_unexpected)
}

/// Converts a FileMetadata to a DecryptedFileMetadata using its decrypted parent key. Sharing is not supported; user access keys not for the provided account are ignored. This is a pure function.
pub fn decrypt_metadatum(
    parent_key: &AESKey,
    target: &FileMetadata,
) -> Result<DecryptedFileMetadata, CoreError> {
    Ok(DecryptedFileMetadata {
        id: target.id,
        file_type: target.file_type,
        parent: target.parent,
        decrypted_name: decrypt_file_name(&target.name, &parent_key)?,
        owner: target.owner.clone(),
        metadata_version: target.metadata_version,
        content_version: target.content_version,
        deleted: target.deleted,
        decrypted_access_key: decrypt_folder_access_keys(&target.folder_access_keys, &parent_key)?,
    })
}

/// Converts a set of FileMetadata's to DecryptedFileMetadata's. All parents of files must be included in files. Sharing is not supported; user access keys not for the provided account are ignored. This is a pure function.
/// CPU optimization opportunity: this function decrypts all ancestors for each file provided, which duplicates a lot of decryption.
pub fn decrypt_metadata(
    account: &Account,
    files: &[FileMetadata],
) -> Result<Vec<DecryptedFileMetadata>, CoreError> {
    let mut result = Vec::new();
    for target in files {
        let parent_key = decrypt_file_key(account, target.parent, files)?;
        result.push(decrypt_metadatum(&parent_key, target)?);
    }
    Ok(result)
}

/// Decrypts the file key given a target and its ancestors. All ancestors of target, as well as target itself, must be included in target_with_ancestors.
fn decrypt_file_key(
    account: &Account,
    target_id: Uuid,
    target_with_ancestors: &[FileMetadata],
) -> Result<AESKey, CoreError> {
    let target = target_with_ancestors
        .iter()
        .find(|m| m.id == target_id)
        .ok_or(CoreError::Unexpected(String::from(
            "target or ancestor missing during call to file_encryption_service::decrypt_file_key",
        )))?;
    match target.user_access_keys.get(&account.username) {
        Some(user_access) => {
            let user_access_key =
                pubkey::get_aes_key(&account.private_key, &user_access.encrypted_by)
                    .map_err(core_err_unexpected)?;
            let key = symkey::decrypt(&user_access_key, &user_access.access_key)
                .map_err(core_err_unexpected)?;
            Ok(key)
        }
        None => {
            let parent_key = decrypt_file_key(&account, target.parent, target_with_ancestors)?;
            let key = symkey::decrypt(&parent_key, &target.folder_access_keys)
                .map_err(core_err_unexpected)?;
            Ok(key)
        }
    }
}

fn decrypt_file_name(
    encrypted_name: &SecretFileName,
    parent_key: &AESKey,
) -> Result<String, CoreError> {
    symkey::decrypt_and_verify(parent_key, encrypted_name).map_err(core_err_unexpected)
}

fn decrypt_folder_access_keys(
    encrypted_keys: &EncryptedFolderAccessKey,
    parent_key: &AESKey,
) -> Result<AESKey, CoreError> {
    symkey::decrypt(&parent_key, &encrypted_keys).map_err(core_err_unexpected)
}

pub fn encrypt_document(
    document: &[u8],
    metadata: &DecryptedFileMetadata,
) -> Result<EncryptedDocument, CoreError> {
    symkey::encrypt(&metadata.decrypted_access_key, &document.to_vec()).map_err(core_err_unexpected)
}

pub fn decrypt_document(
    document: &EncryptedDocument,
    metadata: &DecryptedFileMetadata,
) -> Result<DecryptedDocument, CoreError> {
    symkey::decrypt(&metadata.decrypted_access_key, document).map_err(core_err_unexpected)
}

#[cfg(test)]
mod unit_tests {
    use lockbook_crypto::symkey;
    use lockbook_models::file_metadata::FileType;
    use uuid::Uuid;

    use crate::{service::{file_encryption_service, file_service, test_utils}, utils};

    #[test]
    fn encrypt_decrypt_metadatum() {
        let account = test_utils::generate_account();
        let key = symkey::generate_key();
        let file = file_service::create(FileType::Folder, Uuid::new_v4(), "folder", "owner");

        let encrypted_file = file_encryption_service::encrypt_metadatum(&account, &key, &file).unwrap();
        let decrypted_file = file_encryption_service::decrypt_metadatum(&key, &encrypted_file).unwrap();

        assert_eq!(file, decrypted_file);
    }

    #[test]
    fn encrypt_decrypt_metadata() {
        let account = test_utils::generate_account();
        let root = file_service::create_root(&account.username);
        let folder = file_service::create(FileType::Folder, root.id, "folder", &account.username);
        let document = file_service::create(FileType::Folder, folder.id, "document", &account.username);
        let files = [root.clone(), folder.clone(), document.clone()];

        let encrypted_files = file_encryption_service::encrypt_metadata(&account, &files).unwrap();
        let decrypted_files = file_encryption_service::decrypt_metadata(&account, &encrypted_files).unwrap();

        assert_eq!(
            utils::find(&files, root.id).unwrap(),
            utils::find(&decrypted_files, root.id).unwrap(),
        );
        assert_eq!(
            utils::find(&files, folder.id).unwrap(),
            utils::find(&decrypted_files, folder.id).unwrap(),
        );
        assert_eq!(
            utils::find(&files, document.id).unwrap(),
            utils::find(&decrypted_files, document.id).unwrap(),
        );
    }
}