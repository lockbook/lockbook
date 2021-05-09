use crate::config::IndexDbConfig;
use lockbook_models::api::FileUsage;
use lockbook_models::crypto::{EncryptedUserAccessKey, FolderAccessInfo, UserAccessInfo};
use lockbook_models::file_metadata::FileMetadata;
use lockbook_models::file_metadata::FileType;
use rsa::RSAPublicKey;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{PgPool, Postgres, Transaction};
use std::array::IntoIter;
use std::collections::HashMap;
use std::iter::FromIterator;
use uuid::Uuid;

// TODO:
// * check ownership
// * signatures
// * better serialization

#[derive(Debug)]
pub enum ConnectError {
    Postgres(sqlx::Error),
}

pub async fn connect(config: &IndexDbConfig) -> Result<PgPool, ConnectError> {
    let mut pool_options = PgConnectOptions::new()
        .username(&config.user)
        .host(&config.host)
        .password(&config.pass)
        .port(config.port)
        .database(&config.db);

    if config.cert.as_str() != "" {
        pool_options = pool_options.ssl_root_cert_from_pem(config.cert.clone().into_bytes());
    }

    PgPoolOptions::new()
        .max_connections(20)
        .connect_with(pool_options)
        .await
        .map_err(ConnectError::Postgres)
}

#[derive(Debug)]
pub enum ChangeDocumentVersionAndSizeError {
    Postgres(sqlx::Error),
    Deserialize(serde_json::Error),
    DoesNotExist,
    Deleted,
    IncorrectOldVersion,
}

pub async fn change_document_version_and_size(
    transaction: &mut Transaction<'_, Postgres>,
    id: Uuid,
    document_size_bytes: u64,
    old_metadata_version: u64,
) -> Result<(u64, u64), ChangeDocumentVersionAndSizeError> {
    match sqlx::query!(
        r#"
WITH old AS (SELECT * FROM files WHERE id = $1 FOR UPDATE)
UPDATE files new
SET
    metadata_version =
        (CASE WHEN NOT old.deleted AND old.metadata_version = $2 AND NOT old.is_folder
        THEN CAST(EXTRACT(EPOCH FROM NOW()) * 1000 AS BIGINT)
        ELSE old.metadata_version END),
    content_version =
        (CASE WHEN NOT old.deleted AND old.metadata_version = $2 AND NOT old.is_folder
        THEN CAST(EXTRACT(EPOCH FROM NOW()) * 1000 AS BIGINT)
        ELSE old.content_version END),
    document_size = 
        (CASE WHEN NOT old.deleted AND old.metadata_version = $2 AND NOT old.is_folder
        THEN $3
        ELSE old.document_size END)
FROM old
WHERE old.id = new.id
RETURNING
    old.deleted AS old_deleted,
    old.metadata_version AS old_metadata_version,
    old.content_version AS old_content_version,
    old.parent AS parent_id,
    new.metadata_version AS new_metadata_version,
    old.is_folder AS is_folder;
        "#,
        &id.to_simple()
            .encode_lower(&mut Uuid::encode_buffer())
            .to_owned(),
        &(old_metadata_version as i64),
        &(document_size_bytes as i64)
    )
    .fetch_optional(transaction)
    .await
    .map_err(ChangeDocumentVersionAndSizeError::Postgres)?
    {
        Some(row) => {
            if row.old_deleted {
                Err(ChangeDocumentVersionAndSizeError::Deleted)
            } else if row.old_metadata_version as u64 != old_metadata_version {
                Err(ChangeDocumentVersionAndSizeError::IncorrectOldVersion)
            } else {
                Ok((
                    row.old_content_version as u64,
                    row.new_metadata_version as u64,
                ))
            }
        }
        None => Err(ChangeDocumentVersionAndSizeError::DoesNotExist),
    }
}

#[derive(Debug)]
pub enum CreateFileError {
    Postgres(sqlx::Error),
    Serialize(serde_json::Error),
    IdTaken,
    PathTaken,
    OwnerDoesNotExist,
    ParentDoesNotExist,
}

pub async fn create_file(
    transaction: &mut Transaction<'_, Postgres>,
    id: Uuid,
    parent: Uuid,
    file_type: FileType,
    name: &str,
    public_key: &RSAPublicKey,
    access_key: &FolderAccessInfo,
    maybe_document_bytes: Option<u64>,
) -> Result<u64, CreateFileError> {
    match sqlx::query!(
        r#"
INSERT INTO files (
    id,
    parent,
    parent_access_key,
    is_folder,
    name,
    owner,
    signature,
    deleted,
    metadata_version,
    content_version,
    document_size
)
VALUES (
    $1,
    $2,
    $3,
    $4,
    $5,
    (
        SELECT name
        FROM accounts
        WHERE public_key = $6
    ),
    $7,
    FALSE,
    CAST(EXTRACT(EPOCH FROM NOW()) * 1000 AS BIGINT),
    CAST(EXTRACT(EPOCH FROM NOW()) * 1000 AS BIGINT),
    $8
)
RETURNING metadata_version;
        "#,
        &id.to_simple()
            .encode_lower(&mut Uuid::encode_buffer())
            .to_owned(),
        &parent
            .to_simple()
            .encode_lower(&mut Uuid::encode_buffer())
            .to_owned(),
        &serde_json::to_string(&access_key).map_err(CreateFileError::Serialize)?,
        &(file_type == FileType::Folder),
        &name,
        &serde_json::to_string(public_key).map_err(CreateFileError::Serialize)?,
        &serde_json::to_string("signature goes here").map_err(CreateFileError::Serialize)?,
        (maybe_document_bytes.map(|bytes_u64| bytes_u64 as i64))
    )
    .fetch_one(transaction)
    .await
    {
        Ok(row) => Ok(row.metadata_version as u64),
        Err(sqlx::Error::Database(db_err)) => match db_err.constraint() {
            Some("pk_files") => Err(CreateFileError::IdTaken),
            Some("uk_files_name_parent") => Err(CreateFileError::PathTaken),
            Some("fk_files_parent_files_id") => Err(CreateFileError::ParentDoesNotExist),
            Some("fk_files_owner_accounts_name") => Err(CreateFileError::OwnerDoesNotExist),
            _ => Err(CreateFileError::Postgres(sqlx::Error::Database(db_err))),
        },
        Err(db_err) => Err(CreateFileError::Postgres(db_err)),
    }
}

#[derive(Debug)]
pub struct FileDeleteResponse {
    pub id: Uuid,
    pub old_content_version: u64,
    pub new_metadata_version: u64,
    pub is_folder: bool,
}

#[derive(Debug)]
pub enum DeleteFileError {
    Postgres(sqlx::Error),
    Serialize(serde_json::Error),
    Deserialize(serde_json::Error),
    UuidDeserialize(uuid::Error),
    DoesNotExist,
    Deleted,
    IllegalRootChange,
}

pub async fn delete_file(
    transaction: &mut Transaction<'_, Postgres>,
    id: Uuid,
) -> Result<Vec<FileDeleteResponse>, DeleteFileError> {
    match sqlx::query!(
        r#"
WITH RECURSIVE file_descendants AS (
        SELECT * FROM files AS parent
        WHERE parent.id = $1
            UNION
        SELECT children.* FROM files AS children
        JOIN file_descendants ON file_descendants.id = children.parent
    ),
    old AS (SELECT * FROM files WHERE id IN (SELECT id FROM file_descendants) FOR UPDATE)
UPDATE files new
SET
    document_size =
        (CASE WHEN
            NOT old.deleted AND
            old.id != old.parent
        THEN
            (CASE WHEN
                old.is_folder
            THEN NULL
            ELSE 0 END)
        ELSE old.document_size END),
    deleted =
        (CASE WHEN
            NOT old.deleted AND
            old.id != old.parent
        THEN TRUE
        ELSE old.deleted END),
    metadata_version =
        (CASE WHEN
            NOT old.deleted AND
            old.id != old.parent
        THEN CAST(EXTRACT(EPOCH FROM NOW()) * 1000 AS BIGINT)
        ELSE old.metadata_version END)
FROM old
WHERE old.id = new.id
RETURNING
    old.id AS id,
    old.deleted AS old_deleted,
    old.parent AS parent_id,
    old.content_version AS old_content_version,
    new.metadata_version AS new_metadata_version,
    old.is_folder AS is_folder;
        "#,
        &id.to_simple()
            .encode_lower(&mut Uuid::encode_buffer())
            .to_owned()
    )
    .fetch_all(transaction)
    .await
    .map_err(DeleteFileError::Postgres)?
    .as_slice()
    {
        [] => Err(DeleteFileError::DoesNotExist),
        rows => rows
            .into_iter()
            .map(|row| {
                if row.parent_id == row.id {
                    Err(DeleteFileError::IllegalRootChange)
                } else if row.id
                    == id
                        .to_simple()
                        .encode_lower(&mut Uuid::encode_buffer())
                        .to_owned()
                    && row.old_deleted
                {
                    Err(DeleteFileError::Deleted)
                } else {
                    Ok(FileDeleteResponse {
                        id: Uuid::parse_str(&row.id).map_err(DeleteFileError::UuidDeserialize)?,
                        old_content_version: row.old_content_version as u64,
                        new_metadata_version: row.new_metadata_version as u64,
                        is_folder: row.is_folder,
                    })
                }
            })
            .collect(),
    }
}

#[derive(Debug)]
pub enum MoveFileError {
    Postgres(sqlx::Error),
    Serialize(serde_json::Error),
    Deleted,
    DoesNotExist,
    IncorrectOldVersion,
    ParentDeleted,
    FolderMovedIntoDescendants,
    IllegalRootChange,
    PathTaken,
    ParentDoesNotExist,
}

pub async fn move_file(
    transaction: &mut Transaction<'_, Postgres>,
    id: Uuid,
    old_metadata_version: u64,
    parent: Uuid,
    access_key: FolderAccessInfo,
) -> Result<u64, MoveFileError> {
    match sqlx::query!(
        r#"
WITH RECURSIVE file_descendants AS (
        SELECT * FROM files AS parent
        WHERE parent.id = $1
            UNION
        SELECT children.* FROM files AS children
        JOIN file_descendants ON file_descendants.id = children.parent
    ),
    old AS (SELECT * FROM files WHERE id = $1 FOR UPDATE),
    parent AS (
        SELECT * FROM files WHERE id = $4
    )
UPDATE files new
SET
    parent =
        (CASE WHEN
            NOT old.deleted
            AND old.id != old.parent
            AND old.metadata_version = $2
            AND NOT EXISTS(SELECT * FROM file_descendants WHERE id = $3)
            AND EXISTS(SELECT * FROM parent WHERE NOT deleted)
        THEN $3
        ELSE old.parent END),
    metadata_version =
        (CASE WHEN
            NOT old.deleted
            AND old.id != old.parent
            AND old.metadata_version = $2
            AND NOT EXISTS(SELECT * FROM file_descendants WHERE id = $3)
            AND EXISTS(SELECT * FROM parent WHERE NOT deleted)
        THEN CAST(EXTRACT(EPOCH FROM NOW()) * 1000 AS BIGINT)
        ELSE old.metadata_version END),
    parent_access_key =
        (CASE WHEN
            NOT old.deleted
            AND old.id != old.parent
            AND old.metadata_version = $2
            AND NOT EXISTS(SELECT * FROM file_descendants WHERE id = $3)
            AND EXISTS(SELECT * FROM parent WHERE NOT deleted)
        THEN $4
        ELSE old.parent_access_key END)
FROM old
LEFT JOIN parent ON TRUE
WHERE old.id = new.id
RETURNING
    old.deleted AS old_deleted,
    parent.deleted AS parent_deleted,
    old.parent AS parent_id,
    COALESCE(EXISTS(SELECT * FROM file_descendants WHERE id = $3), FALSE) AS "moved_into_descendant!",
    EXISTS(SELECT * FROM parent) AS "parent_exists!",
    old.metadata_version AS old_metadata_version,
    new.metadata_version AS new_metadata_version,
    old.is_folder AS is_folder;
        "#,
        &id.to_simple().encode_lower(&mut Uuid::encode_buffer()).to_owned(),
        &(old_metadata_version as i64),
        &parent.to_simple().encode_lower(&mut Uuid::encode_buffer()).to_owned(),
        &serde_json::to_string(&access_key).map_err(MoveFileError::Serialize)?,
    )
        .fetch_optional(transaction)
        .await
    {
        Ok(Some(row)) => {
            if row.old_deleted {
                Err(MoveFileError::Deleted)
            } else if row.old_metadata_version as u64 != old_metadata_version {
                Err(MoveFileError::IncorrectOldVersion)
            } else if row.parent_deleted {
                Err(MoveFileError::ParentDeleted)
            } else if row.moved_into_descendant {
                Err(MoveFileError::FolderMovedIntoDescendants)
            } else if !row.parent_exists {
                Err(MoveFileError::ParentDoesNotExist)
            } else if &row.parent_id == id.to_simple().encode_lower(&mut Uuid::encode_buffer()) {
                Err(MoveFileError::IllegalRootChange)
            } else {
                Ok(row.new_metadata_version as u64)
            }
        }
        Ok(None) => Err(MoveFileError::DoesNotExist),
        Err(sqlx::Error::Database(db_err)) => match db_err.constraint() {
            Some("uk_files_name_parent") => Err(MoveFileError::PathTaken),
            _ => Err(MoveFileError::Postgres(sqlx::Error::Database(db_err))),
        },
        Err(db_err) => Err(MoveFileError::Postgres(db_err)),
    }
}

#[derive(Debug)]
pub enum RenameFileError {
    Postgres(sqlx::Error),
    Serialize(serde_json::Error),
    Deleted,
    DoesNotExist,
    IncorrectOldVersion,
    IllegalRootChange,
    PathTaken,
}

pub async fn rename_file(
    transaction: &mut Transaction<'_, Postgres>,
    id: Uuid,
    old_metadata_version: u64,
    file_type: FileType,
    name: &str,
) -> Result<u64, RenameFileError> {
    match sqlx::query!(
        r#"
WITH old AS (SELECT * FROM files WHERE id = $1 FOR UPDATE)
UPDATE files new
SET
    name =
        (CASE WHEN NOT old.deleted
        AND old.metadata_version = $2
        AND old.is_folder = $3
        AND old.id != old.parent
        THEN $4
        ELSE old.name END),
    metadata_version =
        (CASE WHEN NOT old.deleted
        AND old.metadata_version = $2
        AND old.is_folder = $3
        AND old.id != old.parent
        THEN CAST(EXTRACT(EPOCH FROM NOW()) * 1000 AS BIGINT)
        ELSE old.metadata_version END)
FROM old
WHERE old.id = new.id
RETURNING
    old.deleted AS old_deleted,
    old.metadata_version AS old_metadata_version,
    old.content_version AS old_content_version,
    old.parent AS parent_id,
    new.metadata_version AS new_metadata_version,
    old.is_folder AS is_folder;
        "#,
        &id.to_simple()
            .encode_lower(&mut Uuid::encode_buffer())
            .to_owned(),
        &(old_metadata_version as i64),
        &(file_type == FileType::Folder),
        &name,
    )
    .fetch_optional(transaction)
    .await
    {
        Ok(Some(row)) => {
            if row.old_deleted {
                Err(RenameFileError::Deleted)
            } else if row.old_metadata_version as u64 != old_metadata_version {
                Err(RenameFileError::IncorrectOldVersion)
            } else if &row.parent_id == id.to_simple().encode_lower(&mut Uuid::encode_buffer()) {
                Err(RenameFileError::IllegalRootChange)
            } else {
                Ok(row.new_metadata_version as u64)
            }
        }
        Ok(None) => Err(RenameFileError::DoesNotExist),
        Err(sqlx::Error::Database(db_err)) => match db_err.constraint() {
            Some("uk_files_name_parent") => Err(RenameFileError::PathTaken),
            _ => Err(RenameFileError::Postgres(sqlx::Error::Database(db_err))),
        },
        Err(db_err) => Err(RenameFileError::Postgres(db_err)),
    }
}

#[derive(Debug)]
pub enum PublicKeyError {
    Postgres(sqlx::Error),
    Deserialization(serde_json::Error),
    UserNotFound,
}

pub async fn get_public_key(
    transaction: &mut Transaction<'_, Postgres>,
    username: &str,
) -> Result<RSAPublicKey, PublicKeyError> {
    match sqlx::query!(
        r#"
SELECT public_key FROM accounts WHERE name = $1;
        "#,
        &username
    )
    .fetch_optional(transaction)
    .await
    .map_err(PublicKeyError::Postgres)?
    {
        Some(row) => {
            Ok(serde_json::from_str(&row.public_key).map_err(PublicKeyError::Deserialization)?)
        }
        None => Err(PublicKeyError::UserNotFound),
    }
}

#[derive(Debug)]
pub enum GetFilesError {
    Postgres(sqlx::Error),
    Serialize(serde_json::Error),
    Deserialize(serde_json::Error),
    UuidDeserialize(uuid::Error),
}

pub async fn get_files(
    transaction: &mut Transaction<'_, Postgres>,
    public_key: &RSAPublicKey,
) -> Result<Vec<FileMetadata>, GetFilesError> {
    sqlx::query!(
        r#"
SELECT
    files.*,
    user_access_keys.encrypted_key AS "encrypted_key?",
    accounts.public_key
FROM files
JOIN accounts ON files.owner = accounts.name
LEFT JOIN user_access_keys ON files.id = user_access_keys.file_id AND files.owner = user_access_keys.sharee_id
WHERE
    accounts.public_key = $1;
        "#,
        &serde_json::to_string(public_key).map_err(GetFilesError::Serialize)?,
    )
    .fetch_all(transaction)
    .await
    .map_err(GetFilesError::Postgres)?
    .iter()
    .map(|row| Ok(FileMetadata {
        id: Uuid::parse_str(&row.id).map_err(GetFilesError::UuidDeserialize)?,
        file_type: if row.is_folder { FileType::Folder } else { FileType::Document },
        parent: Uuid::parse_str(&row.parent).map_err(GetFilesError::UuidDeserialize)?,
        name: row.name.clone(),
        owner: row.owner.clone(),
        metadata_version: row.metadata_version as u64,
        content_version: row.content_version as u64,
        deleted: row.deleted,
        user_access_keys: {
            if let Some(encrypted_key) = &row.encrypted_key {
                HashMap::<_, _>::from_iter(
                    IntoIter::new(
                        [
                            (
                                row.name.clone(),
                                UserAccessInfo {
                                    username: row.name.clone(),
                                    public_key: serde_json::from_str(&row.public_key)
                                        .map_err(GetFilesError::Deserialize)?,
                                    access_key: serde_json::from_str(&encrypted_key)
                                        .map_err(GetFilesError::Deserialize)?,
                                }
                            )
                        ]
                    )
                )
            } else {
                HashMap::new()
            }
        },
        folder_access_keys: serde_json::from_str(&row.parent_access_key)
        .map_err(GetFilesError::Deserialize)?,
    }))
    .collect()
}

#[derive(Debug)]
pub enum GetUpdatesError {
    Postgres(sqlx::Error),
    Serialize(serde_json::Error),
    Deserialize(serde_json::Error),
    UuidDeserialize(uuid::Error),
}

pub async fn get_updates(
    transaction: &mut Transaction<'_, Postgres>,
    public_key: &RSAPublicKey,
    metadata_version: u64,
) -> Result<Vec<FileMetadata>, GetUpdatesError> {
    sqlx::query!(
        r#"
SELECT
    files.*,
    user_access_keys.encrypted_key AS "encrypted_key?",
    accounts.public_key
FROM files
JOIN accounts ON files.owner = accounts.name
LEFT JOIN user_access_keys ON files.id = user_access_keys.file_id AND files.owner = user_access_keys.sharee_id
WHERE
    accounts.public_key = $1 AND
    metadata_version > $2;
        "#,
        &serde_json::to_string(public_key).map_err(GetUpdatesError::Serialize)?,
        &(metadata_version as i64),
    )
    .fetch_all(transaction)
    .await
    .map_err(GetUpdatesError::Postgres)?
    .iter()
    .map(|row| Ok(FileMetadata {
        id: Uuid::parse_str(&row.id).map_err(GetUpdatesError::UuidDeserialize)?,
        file_type: if row.is_folder { FileType::Folder } else { FileType::Document },
        parent: Uuid::parse_str(&row.parent).map_err(GetUpdatesError::UuidDeserialize)?,
        name: row.name.clone(),
        owner: row.owner.clone(),
        metadata_version: row.metadata_version as u64,
        content_version: row.content_version as u64,
        deleted: row.deleted,
        user_access_keys: {
            if let Some(encrypted_key) = &row.encrypted_key {
                HashMap::<_, _>::from_iter(
                    IntoIter::new(
                        [
                            (
                                row.name.clone(),
                                UserAccessInfo {
                                    username: row.name.clone(),
                                    public_key: serde_json::from_str(&row.public_key)
                                        .map_err(GetUpdatesError::Deserialize)?,
                                    access_key: serde_json::from_str(&encrypted_key)
                                        .map_err(GetUpdatesError::Deserialize)?,
                                }
                            )
                        ]
                    )
                )
            } else {
                HashMap::new()
            }
        },
        folder_access_keys: serde_json::from_str(&row.parent_access_key)
        .map_err(GetUpdatesError::Deserialize)?,
    }))
    .collect()
}

#[derive(Debug)]
pub enum GetRootError {
    Postgres(sqlx::Error),
    Serialize(serde_json::Error),
    Deserialize(serde_json::Error),
    UuidDeserialize(uuid::Error),
}

pub async fn get_root(
    transaction: &mut Transaction<'_, Postgres>,
    public_key: &RSAPublicKey,
) -> Result<FileMetadata, GetRootError> {
    let row = sqlx::query!(
        r#"
SELECT
    files.*,
    user_access_keys.encrypted_key AS "encrypted_key?",
    accounts.public_key
FROM files
JOIN accounts ON files.owner = accounts.name
LEFT JOIN user_access_keys ON files.id = user_access_keys.file_id AND files.owner = user_access_keys.sharee_id
WHERE
    accounts.public_key = $1 AND
    id = parent;
        "#,
        &serde_json::to_string(public_key).map_err(GetRootError::Serialize)?
    )
    .fetch_one(transaction)
    .await
    .map_err(GetRootError::Postgres)?;

    Ok(FileMetadata {
        id: Uuid::parse_str(&row.id).map_err(GetRootError::UuidDeserialize)?,
        file_type: if row.is_folder {
            FileType::Folder
        } else {
            FileType::Document
        },
        parent: Uuid::parse_str(&row.parent).map_err(GetRootError::UuidDeserialize)?,
        name: row.name.clone(),
        owner: row.owner.clone(),
        metadata_version: row.metadata_version as u64,
        content_version: row.content_version as u64,
        deleted: row.deleted,
        user_access_keys: {
            if let Some(encrypted_key) = &row.encrypted_key {
                HashMap::<_, _>::from_iter(IntoIter::new([(
                    row.name.clone(),
                    UserAccessInfo {
                        username: row.name.clone(),
                        public_key: serde_json::from_str(&row.public_key)
                            .map_err(GetRootError::Deserialize)?,
                        access_key: serde_json::from_str(&encrypted_key)
                            .map_err(GetRootError::Deserialize)?,
                    },
                )]))
            } else {
                HashMap::new()
            }
        },
        folder_access_keys: serde_json::from_str(&row.parent_access_key)
            .map_err(GetRootError::Deserialize)?,
    })
}

#[derive(Debug)]
pub enum NewAccountError {
    Postgres(sqlx::Error),
    Serialization(serde_json::Error),
    UsernameTaken,
}

pub async fn new_account(
    transaction: &mut Transaction<'_, Postgres>,
    username: &str,
    public_key: &RSAPublicKey,
) -> Result<(), NewAccountError> {
    match sqlx::query!(
        r#"
WITH i1 AS (
    INSERT INTO account_tiers (bytes_cap) VALUES (1000000) RETURNING id
)
INSERT INTO accounts (name, public_key, account_tier) VALUES ($1, $2, (SELECT id FROM i1))
        "#,
        &username,
        &serde_json::to_string(&public_key).map_err(NewAccountError::Serialization)?,
    )
    .execute(transaction)
    .await
    {
        Ok(_) => Ok(()),
        Err(sqlx::Error::Database(db_err)) => match db_err.constraint() {
            Some("pk_accounts") => Err(NewAccountError::UsernameTaken),
            _ => Err(NewAccountError::Postgres(sqlx::Error::Database(db_err))),
        },
        Err(db_err) => Err(NewAccountError::Postgres(db_err)),
    }
}

#[derive(Debug)]
pub enum CreateUserAccessKeyError {
    Postgres(sqlx::Error),
    Serialization(serde_json::Error),
}

pub async fn create_user_access_key(
    transaction: &mut Transaction<'_, Postgres>,
    username: &str,
    folder_id: Uuid,
    user_access_key: &EncryptedUserAccessKey,
) -> Result<(), CreateUserAccessKeyError> {
    sqlx::query!(
        r#"
INSERT INTO user_access_keys (file_id, sharee_id, encrypted_key) VALUES ($1, $2, $3);
        "#,
        &folder_id
            .to_simple()
            .encode_lower(&mut Uuid::encode_buffer())
            .to_owned(),
        &username,
        &serde_json::to_string(&user_access_key)
            .map_err(CreateUserAccessKeyError::Serialization)?,
    )
    .execute(transaction)
    .await
    .map_err(CreateUserAccessKeyError::Postgres)?;
    Ok(())
}

#[derive(Debug)]
pub enum DeleteAccountAccessKeysError {
    Postgres(sqlx::Error),
}

pub async fn delete_account_access_keys(
    transaction: &mut Transaction<'_, Postgres>,
    username: &str,
) -> Result<(), DeleteAccountAccessKeysError> {
    sqlx::query!(
        r#"
DELETE FROM user_access_keys where sharee_id = $1
        "#,
        &username.to_string(),
    )
    .execute(transaction)
    .await
    .map_err(DeleteAccountAccessKeysError::Postgres)?;
    Ok(())
}

#[derive(Debug)]
pub enum DeleteAllFilesOfAccountError {
    Postgres(sqlx::Error),
}

pub async fn delete_all_files_of_account(
    transaction: &mut Transaction<'_, Postgres>,
    username: &str,
) -> Result<(), DeleteAllFilesOfAccountError> {
    sqlx::query!(
        r#"
DELETE FROM files
WHERE owner = $1
RETURNING
    id AS id,
    deleted AS old_deleted,
    parent AS parent_id,
    content_version AS old_content_version,
    metadata_version AS new_metadata_version,
    is_folder AS is_folder;
        "#,
        &username.to_string()
    )
    .fetch_one(transaction)
    .await
    .map_err(DeleteAllFilesOfAccountError::Postgres)?;
    Ok(())
}

#[derive(Debug)]
pub enum DeleteAccountError {
    Postgres(sqlx::Error),
}

pub async fn delete_account(
    transaction: &mut Transaction<'_, Postgres>,
    username: &str,
) -> Result<(), DeleteAccountError> {
    sqlx::query!(
        r#"
DELETE FROM accounts where name = $1
        "#,
        &username.to_string(),
    )
    .execute(transaction)
    .await
    .map_err(DeleteAccountError::Postgres)?;
    Ok(())
}

#[derive(Debug)]
pub enum GetDataCapError {
    TierNotFound,
    Serialize(serde_json::Error),
    Postgres(sqlx::Error),
    Unknown(String),
}

pub async fn get_account_data_cap(
    transaction: &mut Transaction<'_, Postgres>,
    public_key: &RSAPublicKey,
) -> Result<u64, GetDataCapError> {
    match sqlx::query!(
        r#"
SELECT bytes_cap
FROM account_tiers
WHERE id =
    (SELECT account_tier FROM accounts WHERE public_key = $1);
        "#,
        &serde_json::to_string(public_key).map_err(GetDataCapError::Serialize)?
    )
    .fetch_optional(transaction)
    .await
    .map_err(GetDataCapError::Postgres)?
    {
        Some(row) => Ok(row.bytes_cap as u64),
        None => Err(GetDataCapError::TierNotFound),
    }
}

#[derive(Debug)]
pub enum GetFileUsageError {
    Postgres(sqlx::Error),
    Serialize(serde_json::Error),
    UuidDeserialize(uuid::Error),
}

pub async fn get_file_usages(
    transaction: &mut Transaction<'_, Postgres>,
    public_key: &RSAPublicKey,
) -> Result<Vec<FileUsage>, GetFileUsageError> {
    sqlx::query!(
        r#"
    SELECT
        files.id,
        files.document_size AS "document_size!"
    FROM files
    JOIN accounts ON files.owner = accounts.name
    WHERE
        accounts.public_key = $1 AND
        NOT files.is_folder;
        "#,
        &serde_json::to_string(public_key).map_err(GetFileUsageError::Serialize)?
    )
    .fetch_all(transaction)
    .await
    .map_err(GetFileUsageError::Postgres)?
    .into_iter()
    .map(|row| {
        Ok(FileUsage {
            file_id: Uuid::parse_str(&row.id).map_err(GetFileUsageError::UuidDeserialize)?,
            size_bytes: row.document_size as u64,
        })
    })
    .collect()
}
