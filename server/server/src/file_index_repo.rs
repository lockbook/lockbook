use crate::config::IndexDbConfig;
use base64;
use libsecp256k1::PublicKey;
use lockbook_models::api::FileUsage;
use lockbook_models::crypto::{
    EncryptedFolderAccessKey, EncryptedUserAccessKey, SecretFileName, UserAccessInfo,
};
use lockbook_models::file_metadata::{FileMetadata, FileMetadataDiff, FileType};
use log::debug;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{ConnectOptions, PgPool, Postgres, Transaction};
use std::array::IntoIter;
use std::collections::HashMap;
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
    debug!("Connecting to index_db...");
    let mut pool_options = PgConnectOptions::new()
        .username(&config.user)
        .host(&config.host)
        .password(&config.pass)
        .port(config.port)
        .database(&config.db)
        .application_name("lockbook-server");
    pool_options.disable_statement_logging();

    if config.cert.as_str() != "" {
        pool_options = pool_options.ssl_root_cert_from_pem(config.cert.clone().into_bytes());
    }

    let pool = PgPoolOptions::new()
        .max_connections(config.pool_size)
        .connect_with(pool_options)
        .await
        .map_err(ConnectError::Postgres);
    debug!("Connected to index_db");

    pool
}

#[derive(Debug)]
pub enum UpsertFileMetadataError {
    Postgres(sqlx::Error),
    Serialize(serde_json::Error),
    FailedPreconditions,
}

pub async fn upsert_file_metadata(
    transaction: &mut Transaction<'_, Postgres>,
    public_key: &PublicKey,
    upsert: &FileMetadataDiff,
) -> Result<u64, UpsertFileMetadataError> {
    let old_parent_param = upsert
        .old_parent_and_name
        .clone()
        .map(|(parent, _)| format!("{}", parent))
        .unwrap_or_default();
    let old_name_param = match &upsert
        .old_parent_and_name
        .clone()
        .map(|(_, name)| name.hmac)
    {
        Some(name_hmac) => base64::encode(name_hmac),
        None => String::from(""),
    };

    sqlx::query!(
        r#"
WITH
    preconditions AS (
        SELECT
            -- both args empty and file does not exist...
            ($9 = '' AND $10 = '' AND NOT EXISTS(SELECT * FROM files WHERE id = $1)) OR
            -- ...or neither arg empty and matching file exists
            ($9 != '' AND $10 != '' AND EXISTS(SELECT * FROM files WHERE id = $1 AND is_folder = $4 AND parent = $9 AND name_hmac = $10)) AS met
            UNION ALL
        -- new parent must be a folder if it exists already
        SELECT NOT EXISTS(SELECT * FROM files WHERE id = $2 AND NOT is_folder) AS met
            UNION ALL
        -- cannot have children if document
        SELECT $4 OR NOT EXISTS(SELECT * FROM files WHERE parent = $1) AS met
    ),
    insert AS (
        INSERT INTO files (
            id,
            parent,
            parent_access_key,
            is_folder,
            name_encrypted,
            name_hmac,
            owner,
            deleted,
            metadata_version,
            content_version,
            document_size
        )
        SELECT
            $1,
            $2,
            $3,
            $4,
            $5,
            $6,
            $7,
            $8,
            CAST(EXTRACT(EPOCH FROM NOW()) * 1000 AS BIGINT),
            0,
            0
        ON CONFLICT (id) DO UPDATE SET
            metadata_version =
                (CASE WHEN (SELECT BOOL_AND(met) FROM preconditions)
                THEN CAST(EXTRACT(EPOCH FROM NOW()) * 1000 AS BIGINT)
                ELSE files.metadata_version END),
            parent =
                (CASE WHEN (SELECT BOOL_AND(met) FROM preconditions)
                THEN $2
                ELSE files.parent END),
            parent_access_key =
                (CASE WHEN (SELECT BOOL_AND(met) FROM preconditions)
                THEN $3
                ELSE files.parent_access_key END),
            name_encrypted =
                (CASE WHEN (SELECT BOOL_AND(met) FROM preconditions)
                THEN $5
                ELSE files.name_encrypted END),
            name_hmac =
                (CASE WHEN (SELECT BOOL_AND(met) FROM preconditions)
                THEN $6
                ELSE files.name_hmac END),
            deleted =
                (CASE WHEN (SELECT BOOL_AND(met) FROM preconditions)
                THEN excluded.deleted OR $8
                ELSE files.deleted END)
    )
SELECT
    BOOL_AND(met) AS "preconditions_met!",
    CAST(EXTRACT(EPOCH FROM NOW()) * 1000 AS BIGINT) AS "version!"
FROM preconditions;
        "#,
        &format!("{}", upsert.id),
        &format!("{}", upsert.new_parent),
        &serde_json::to_string(&upsert.new_folder_access_keys)
            .map_err(UpsertFileMetadataError::Serialize)?,
        &(upsert.file_type == FileType::Folder),
        &serde_json::to_string(&upsert.new_name.encrypted_value)
            .map_err(UpsertFileMetadataError::Serialize)?,
        &base64::encode(upsert.new_name.hmac),
        &serde_json::to_string(public_key).map_err(UpsertFileMetadataError::Serialize)?,
        &upsert.new_deleted,
        &old_parent_param,
        &old_name_param,
    )
    .fetch_one(transaction)
    .await
    .map(|row| {
        if !row.preconditions_met {
            Err(UpsertFileMetadataError::FailedPreconditions)
        } else {
            Ok(row.version as u64)
        }
    })
    .map_err(UpsertFileMetadataError::Postgres)?
}

#[derive(Debug)]
pub enum CheckCyclesError {
    Postgres(sqlx::Error),
    Serialize(serde_json::Error),
    CyclesDetected,
}

pub async fn check_cycles(
    transaction: &mut Transaction<'_, Postgres>,
    public_key: &PublicKey,
) -> Result<(), CheckCyclesError> {
    sqlx::query!(
        r#"
        WITH RECURSIVE not_sure AS (
            SELECT
                FALSE AS i,
                id AS id,
                parent AS parent,
                (SELECT parent_files.parent FROM files AS parent_files WHERE parent_files.id = files.parent) AS grandparent
            FROM files
            WHERE owner = $1
                UNION DISTINCT
            SELECT
                TRUE AS i,
                id AS id,
                (SELECT parent_files.parent FROM files AS parent_files WHERE parent_files.id = not_sure.parent) AS parent,
                (SELECT grandparent_files.parent FROM files AS grandparent_files WHERE grandparent_files.id = (SELECT parent_files.parent FROM files AS parent_files WHERE id = (SELECT files.parent FROM files WHERE id = not_sure.parent))) AS grandparent
            FROM not_sure
            WHERE parent != grandparent
        )
        SELECT COUNT(*) != 0 AS "has_cycles!" FROM not_sure WHERE id = parent AND i;
        "#,
        &serde_json::to_string(public_key).map_err(CheckCyclesError::Serialize)?,
    )
    .fetch_one(transaction)
    .await
    .map(|row| if row.has_cycles { Err(CheckCyclesError::CyclesDetected) } else { Ok(()) })
    .map_err(CheckCyclesError::Postgres)?
}

#[derive(Debug)]
pub enum ApplyRecursiveDeletionsError {
    Postgres(sqlx::Error),
    Serialize(serde_json::Error),
    UuidDeserialize(uuid::Error),
}

pub async fn apply_recursive_deletions(
    transaction: &mut Transaction<'_, Postgres>,
    public_key: &PublicKey,
) -> Result<Vec<Uuid>, ApplyRecursiveDeletionsError> {
    sqlx::query!(
        r#"
        WITH RECURSIVE effective_deleted AS (
            SELECT
                id,
                parent,
                deleted
            FROM files
            WHERE owner = $1
                UNION DISTINCT
            SELECT
                effective_deleted.id,
                files.parent,
                effective_deleted.deleted OR files.deleted
            FROM effective_deleted
            JOIN files ON effective_deleted.parent = files.id
        ),
        deletions AS (
            SELECT
                effective_deleted.id
            FROM effective_deleted
            JOIN files ON effective_deleted.id = files.id
            WHERE effective_deleted.deleted AND NOT files.deleted
        )
        UPDATE files SET
            deleted = true,
            metadata_version = CAST(EXTRACT(EPOCH FROM NOW()) * 1000 AS BIGINT)
        WHERE id IN (SELECT id FROM deletions)
        RETURNING id AS "id!";
        "#,
        &serde_json::to_string(public_key).map_err(ApplyRecursiveDeletionsError::Serialize)?,
    )
    .fetch_all(transaction)
    .await
    .map_err(ApplyRecursiveDeletionsError::Postgres)?
    .iter()
    .map(|row| Uuid::parse_str(&row.id).map_err(ApplyRecursiveDeletionsError::UuidDeserialize))
    .collect()
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
        &format!("{}", id),
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
    AncestorDeleted,
}

pub async fn create_file(
    transaction: &mut Transaction<'_, Postgres>,
    id: Uuid,
    parent: Uuid,
    file_type: FileType,
    name: &SecretFileName,
    public_key: &PublicKey,
    access_key: &EncryptedFolderAccessKey,
    maybe_document_bytes: Option<u64>,
) -> Result<u64, CreateFileError> {
    match sqlx::query!(
        r#"
WITH RECURSIVE file_ancestors AS (
    SELECT * FROM files AS new_file_parent
    WHERE new_file_parent.id = $2
        UNION DISTINCT
    SELECT ancestors.* FROM files AS ancestors
    JOIN file_ancestors ON file_ancestors.parent = ancestors.id
),
insert_cte AS (
    INSERT INTO files (
        id,
        parent,
        parent_access_key,
        is_folder,
        name_encrypted,
        name_hmac,
        owner,
        deleted,
        metadata_version,
        content_version,
        document_size
    )
    SELECT
        $1,
        $2,
        $3,
        $4,
        $5,
        $6,
        $7,
        FALSE,
        CAST(EXTRACT(EPOCH FROM NOW()) * 1000 AS BIGINT),
        CAST(EXTRACT(EPOCH FROM NOW()) * 1000 AS BIGINT),
        $8
    WHERE NOT EXISTS(SELECT * FROM file_ancestors WHERE deleted)
    RETURNING NULL
)
SELECT
    CAST(EXTRACT(EPOCH FROM NOW()) * 1000 AS BIGINT) AS "metadata_version!",
    EXISTS(SELECT * FROM file_ancestors WHERE deleted) AS "ancestor_deleted!";
        "#,
        &format!("{}", id),
        &format!("{}", parent),
        &serde_json::to_string(&access_key).map_err(CreateFileError::Serialize)?,
        &(file_type == FileType::Folder),
        &serde_json::to_string(&name.encrypted_value).map_err(CreateFileError::Serialize)?,
        &base64::encode(name.hmac),
        &serde_json::to_string(public_key).map_err(CreateFileError::Serialize)?,
        (maybe_document_bytes.map(|bytes_u64| bytes_u64 as i64))
    )
    .fetch_one(transaction)
    .await
    {
        Ok(row) => {
            if !row.ancestor_deleted {
                Ok(row.metadata_version as u64)
            } else {
                Err(CreateFileError::AncestorDeleted)
            }
        }
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
pub enum PublicKeyError {
    Postgres(sqlx::Error),
    Deserialization(serde_json::Error),
    UserNotFound,
}

pub async fn get_public_key(
    transaction: &mut Transaction<'_, Postgres>,
    username: &str,
) -> Result<PublicKey, PublicKeyError> {
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
    HmacDeserialize(DeserializeHmacError),
}

pub async fn get_files(
    transaction: &mut Transaction<'_, Postgres>,
    public_key: &PublicKey,
) -> Result<Vec<FileMetadata>, GetFilesError> {
    sqlx::query!(
        r#"
SELECT
    files.*,
    user_access_keys.encrypted_key AS "encrypted_key?",
    accounts.public_key,
    accounts.name AS username
FROM files
JOIN accounts ON files.owner = accounts.public_key
LEFT JOIN user_access_keys ON files.id = user_access_keys.file_id AND accounts.public_key = user_access_keys.sharee
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
        name: SecretFileName{
            encrypted_value: serde_json::from_str(&row.name_encrypted).map_err(GetFilesError::Deserialize)?,
            hmac: deserialize_hmac(&row.name_hmac).map_err(GetFilesError::HmacDeserialize)?,
        },
        owner: row.owner.clone(),
        metadata_version: row.metadata_version as u64,
        content_version: row.content_version as u64,
        deleted: row.deleted,
        user_access_keys: {
            if let Some(encrypted_key) = &row.encrypted_key {
                    IntoIter::new(
                        [
                            (
                                row.username.clone(),
                                UserAccessInfo {
                                    username: row.username.clone(),
                                    encrypted_by: serde_json::from_str(&row.public_key)
                                        .map_err(GetFilesError::Deserialize)?,
                                    access_key: serde_json::from_str(encrypted_key)
                                        .map_err(GetFilesError::Deserialize)?,
                                }
                            )
                        ]
                    )
                        .collect()
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
    HmacDeserialize(DeserializeHmacError),
}

pub async fn get_updates(
    transaction: &mut Transaction<'_, Postgres>,
    public_key: &PublicKey,
    metadata_version: u64,
) -> Result<Vec<FileMetadata>, GetUpdatesError> {
    sqlx::query!(
        r#"
SELECT
    files.*,
    user_access_keys.encrypted_key AS "encrypted_key?",
    accounts.public_key,
    accounts.name AS username
FROM files
JOIN accounts ON files.owner = accounts.public_key
LEFT JOIN user_access_keys ON files.id = user_access_keys.file_id AND accounts.public_key = user_access_keys.sharee
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
        name: SecretFileName{
            encrypted_value: serde_json::from_str(&row.name_encrypted).map_err(GetUpdatesError::Deserialize)?,
            hmac: deserialize_hmac(&row.name_hmac).map_err(GetUpdatesError::HmacDeserialize)?,
        },
        owner: row.username.clone(),
        metadata_version: row.metadata_version as u64,
        content_version: row.content_version as u64,
        deleted: row.deleted,
        user_access_keys: {
            if let Some(encrypted_key) = &row.encrypted_key {
                    IntoIter::new(
                        [
                            (
                                row.username.clone(),
                                UserAccessInfo {
                                    username: row.username.clone(),
                                    encrypted_by: serde_json::from_str(&row.public_key)
                                        .map_err(GetUpdatesError::Deserialize)?,
                                    access_key: serde_json::from_str(encrypted_key)
                                        .map_err(GetUpdatesError::Deserialize)?,
                                }
                            )
                        ]
                    ).collect()
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
pub enum NewAccountError {
    Postgres(sqlx::Error),
    Serialization(serde_json::Error),
    UsernameTaken,
    PublicKeyTaken,
}

pub async fn new_account(
    transaction: &mut Transaction<'_, Postgres>,
    username: &str,
    public_key: &PublicKey,
) -> Result<(), NewAccountError> {
    match sqlx::query!(
        r#"
WITH i1 AS (
    INSERT INTO account_tiers (bytes_cap) VALUES (1000000) RETURNING id
)
INSERT INTO accounts (name, public_key, account_tier) VALUES ($1, $2, (SELECT id FROM i1))
        "#,
        &(username.to_uppercase()),
        &serde_json::to_string(&public_key).map_err(NewAccountError::Serialization)?,
    )
    .execute(transaction)
    .await
    {
        Ok(_) => Ok(()),
        Err(sqlx::Error::Database(db_err)) => match db_err.constraint() {
            Some("accounts_pkey") => Err(NewAccountError::PublicKeyTaken),
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
    public_key: &PublicKey,
    folder_id: Uuid,
    user_access_key: &EncryptedUserAccessKey,
) -> Result<(), CreateUserAccessKeyError> {
    sqlx::query!(
        r#"
INSERT INTO user_access_keys (file_id, sharee, encrypted_key) VALUES ($1, $2, $3);
        "#,
        &format!("{}", folder_id),
        &serde_json::to_string(&public_key).map_err(CreateUserAccessKeyError::Serialization)?,
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
    Serialize(serde_json::Error),
}

pub async fn delete_account_access_keys(
    transaction: &mut Transaction<'_, Postgres>,
    pk: &PublicKey,
) -> Result<(), DeleteAccountAccessKeysError> {
    sqlx::query!(
        r#"
DELETE FROM user_access_keys where sharee = $1
        "#,
        &serde_json::to_string(pk).map_err(DeleteAccountAccessKeysError::Serialize)?,
    )
    .execute(transaction)
    .await
    .map_err(DeleteAccountAccessKeysError::Postgres)?;
    Ok(())
}

#[derive(Debug)]
pub enum DeleteAllFilesOfAccountError {
    DoesNotExist,
    Postgres(sqlx::Error),
    Serialize(serde_json::Error),
    UuidDeserialize(uuid::Error),
}

pub async fn delete_all_files_of_account(
    transaction: &mut Transaction<'_, Postgres>,
    pk: &PublicKey,
) -> Result<Vec<FileDeleteResponse>, DeleteAllFilesOfAccountError> {
    match sqlx::query!(
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
        &serde_json::to_string(pk).map_err(DeleteAllFilesOfAccountError::Serialize)?
    )
    .fetch_all(transaction)
    .await
    .map_err(DeleteAllFilesOfAccountError::Postgres)?
    .as_slice()
    {
        [] => Err(DeleteAllFilesOfAccountError::DoesNotExist),
        rows => rows
            .iter()
            .map(|row| {
                Ok(FileDeleteResponse {
                    id: Uuid::parse_str(&row.id)
                        .map_err(DeleteAllFilesOfAccountError::UuidDeserialize)?,
                    old_content_version: row.old_content_version as u64,
                    new_metadata_version: row.new_metadata_version as u64,
                    is_folder: row.is_folder,
                })
            })
            .collect(),
    }
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
    public_key: &PublicKey,
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
    public_key: &PublicKey,
) -> Result<Vec<FileUsage>, GetFileUsageError> {
    sqlx::query!(
        r#"
    WITH RECURSIVE file_ancestors AS (
        SELECT id, id AS ancestor FROM files
            UNION DISTINCT
        SELECT file_ancestors.id, files.parent AS ancestor FROM files
        JOIN file_ancestors ON files.id = file_ancestors.ancestor
    ),
    files_with_deleted_ancestors AS (
        SELECT id FROM files WHERE EXISTS(
            SELECT * FROM file_ancestors
            JOIN files AS ancestor_files ON file_ancestors.ancestor = ancestor_files.id
            WHERE files.id = file_ancestors.id AND ancestor_files.deleted
        )
    )
    SELECT
        files.id,
        CASE WHEN EXISTS(SELECT * FROM files_with_deleted_ancestors WHERE files_with_deleted_ancestors.id = files.id) THEN 0 ELSE files.document_size END AS "document_size!"
    FROM files
    WHERE
        files.owner = $1 AND
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

#[derive(Debug)]
pub enum DeserializeHmacError {
    Base64Decode(base64::DecodeError),
    WrongLength(usize),
}

fn deserialize_hmac(s: &str) -> Result<[u8; 32], DeserializeHmacError> {
    let v = base64::decode(s).map_err(DeserializeHmacError::Base64Decode)?;
    if v.len() != 32 {
        Err(DeserializeHmacError::WrongLength(v.len()))
    } else {
        let mut result = [0; 32];
        result.clone_from_slice(&v);
        Ok(result)
    }
}
