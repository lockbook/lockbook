use crate::config::IndexDbConfig;
use lockbook_core::model::api::FileMetadata;
use openssl::error::ErrorStack as OpenSslError;
use openssl::ssl::{SslConnector, SslMethod};
use postgres_openssl::MakeTlsConnector;
use rsa::RSAPublicKey;
use tokio_postgres::error::Error as PostgresError;
use tokio_postgres::error::SqlState;
use tokio_postgres::Client as PostgresClient;
use tokio_postgres::Config as PostgresConfig;
use tokio_postgres::NoTls;
use tokio_postgres::Transaction;
use uuid::Uuid;

#[derive(Debug)]
pub enum ConnectError {
    OpenSsl(OpenSslError),
    Postgres(PostgresError),
}

#[derive(Debug)]
pub enum AccountError {
    Postgres(PostgresError),
    Serialization(serde_json::Error),
    UsernameTaken,
}

impl From<PostgresError> for AccountError {
    fn from(e: PostgresError) -> AccountError {
        match e.code() {
            Some(x) if x == &SqlState::UNIQUE_VIOLATION => AccountError::UsernameTaken,
            _ => AccountError::Postgres(e),
        }
    }
}

#[derive(Debug)]
pub enum PublicKeyError {
    Deserialization(serde_json::Error),
    Postgres(PostgresError),
}

#[derive(Debug)]
pub enum FileError {
    Deleted,
    Deserialize(serde_json::Error),
    DoesNotExist,
    IdTaken,
    IncorrectOldVersion(u64),
    PathTaken,
    Postgres(PostgresError),
    Serialize(serde_json::Error),
    Unknown(String),
}

impl From<PostgresError> for FileError {
    fn from(e: PostgresError) -> FileError {
        match (e.code(), e.to_string()) {
            (Some(error_code), error_string)
                if error_code == &SqlState::UNIQUE_VIOLATION
                    && error_string.contains("pk_files") =>
            {
                FileError::IdTaken
            }
            (Some(error_code), error_string)
                if error_code == &SqlState::UNIQUE_VIOLATION
                    && error_string.contains("unique_file_path") =>
            {
                FileError::PathTaken
            }
            _ => FileError::Postgres(e),
        }
    }
}

pub async fn connect(config: &IndexDbConfig) -> Result<PostgresClient, ConnectError> {
    let mut postgres_config = PostgresConfig::new();
    postgres_config
        .user(&config.user)
        .host(&config.host)
        .password(&config.pass)
        .port(config.port)
        .dbname(&config.db);

    match config.cert.as_str() {
        "" => connect_no_tls(&postgres_config).await,
        cert => connect_with_tls(&postgres_config, &cert).await,
    }
}

async fn connect_no_tls(postgres_config: &PostgresConfig) -> Result<PostgresClient, ConnectError> {
    match postgres_config.connect(NoTls).await {
        Ok((client, connection)) => {
            tokio::spawn(async move {
                if let Err(e) = connection.await {
                    panic!("connection error: {}", e);
                }
            });
            Ok(client)
        }
        Err(err) => Err(ConnectError::Postgres(err)),
    }
}

async fn connect_with_tls(
    postgres_config: &PostgresConfig,
    cert: &str,
) -> Result<PostgresClient, ConnectError> {
    let mut builder = match SslConnector::builder(SslMethod::tls()) {
        Ok(builder) => builder,
        Err(err) => return Err(ConnectError::OpenSsl(err)),
    };
    builder.set_ca_file(cert).map_err(ConnectError::OpenSsl)?;
    match postgres_config
        .connect(MakeTlsConnector::new(builder.build()))
        .await
    {
        Ok((client, connection)) => {
            tokio::spawn(async move {
                if let Err(e) = connection.await {
                    panic!("connection error: {}", e);
                }
            });
            Ok(client)
        }
        Err(err) => Err(ConnectError::Postgres(err)),
    }
}

// todo return old content version
pub async fn change_document_content_version(
    transaction: &Transaction<'_>,
    file_id: &str,
    old_metadata_version: u64,
) -> Result<(u64, u64), FileError> {
    let rows = transaction
        .query(
            "WITH old AS (SELECT * FROM documents WHERE id = $1 FOR UPDATE)
            UPDATE documents new
            SET
                metadata_version = 
                    (CASE WHEN NOT old.deleted AND old.metadata_version = $2
                    THEN CAST(EXTRACT(EPOCH FROM NOW()) * 1000 AS BIGINT)
                    ELSE old.metadata_version END),
                content_version = 
                    (CASE WHEN NOT old.deleted AND old.metadata_version = $2
                    THEN CAST(EXTRACT(EPOCH FROM NOW()) * 1000 AS BIGINT)
                    ELSE old.content_version END)
            FROM old WHERE old.id = new.id
            RETURNING old.deleted AS old_deleted, old.metadata_version AS old_metadata_version, new.*;",
            &[&file_id, &(old_metadata_version as i64)],
        )
        .await
        .map_err(FileError::Postgres)?;
    let metadata = rows_to_metadata(&rows, old_metadata_version)?;
    Ok((metadata.old_content_version, metadata.new.metadata_version))
}

pub async fn create_document(
    transaction: &Transaction<'_>,
    id: Uuid,
    parent: Uuid,
    name: &str,
    owner: &str,
    signature: &str,
) -> Result<u64, FileError> {
    let row = transaction.query_one(
        "INSERT INTO documents (id, parent, name, owner, signature, metadata_version, content_version, deleted)
        VALUES ($1, $2, $3, $4, $5, CAST(EXTRACT(EPOCH FROM NOW()) * 1000 AS BIGINT), CAST(EXTRACT(EPOCH FROM NOW()) * 1000 AS BIGINT), false)
        RETURNING *;",
        &[&(serde_json::to_string(&id).map_err(FileError::Serialize)?), &(serde_json::to_string(&parent).map_err(FileError::Serialize)?), &name, &owner, &signature]).await.map_err(FileError::Postgres)?;
    Ok(row
        .try_get::<&str, i64>("metadata_version")
        .map_err(FileError::Postgres)? as u64)
}

pub async fn delete_document(
    transaction: &Transaction<'_>,
    file_id: &str,
    old_metadata_version: u64,
) -> Result<u64, FileError> {
    let rows = transaction
        .query(
            "WITH old AS (SELECT * FROM documents WHERE id = $1 FOR UPDATE)
            UPDATE documents new
            SET
                deleted = true,
                metadata_version = 
                    (CASE
                    WHEN NOT old.deleted AND old.metadata_version = $2
                    THEN CAST(EXTRACT(EPOCH FROM NOW()) * 1000 AS BIGINT)
                    ELSE old.metadata_version
                    END)
            FROM old WHERE old.id = new.id
            RETURNING old.deleted AS old_deleted, old.metadata_version AS old_metadata_version, new.*;",
            &[&file_id, &(old_metadata_version as i64)],
        )
        .await
        .map_err(FileError::Postgres)?;
    Ok(rows_to_metadata(&rows, old_metadata_version)?.new.metadata_version)
}

pub async fn move_document(
    transaction: &Transaction<'_>,
    id: Uuid,
    old_metadata_version: u64,
    parent: Uuid,
) -> Result<u64, FileError> {
    let rows = transaction
        .query(
            "WITH old AS (SELECT * FROM documents WHERE id = $1 FOR UPDATE)
            UPDATE documents new
            SET
                parent = $3,
                metadata_version = 
                    (CASE
                    WHEN NOT old.deleted AND old.metadata_version = $2
                    THEN CAST(EXTRACT(EPOCH FROM NOW()) * 1000 AS BIGINT)
                    ELSE old.metadata_version
                    END)
            FROM old WHERE old.id = new.id
            RETURNING old.deleted AS old_deleted, old.metadata_version AS old_metadata_version, new.*;",
            &[&serde_json::to_string(&id).map_err(FileError::Serialize)?, &(old_metadata_version as i64), &serde_json::to_string(&parent).map_err(FileError::Serialize)?],
        )
        .await
        .map_err(FileError::Postgres)?;
    Ok(rows_to_metadata(&rows, old_metadata_version)?.new.metadata_version)
}

pub async fn rename_document(
    transaction: &Transaction<'_>,
    id: Uuid,
    old_metadata_version: u64,
    name: &str,
) -> Result<u64, FileError> {
    let rows = transaction
        .query(
            "WITH old AS (SELECT * FROM documents WHERE id = $1 FOR UPDATE)
            UPDATE documents new
            SET
                name = $3,
                metadata_version = 
                    (CASE
                    WHEN NOT old.deleted AND old.metadata_version = $2
                    THEN CAST(EXTRACT(EPOCH FROM NOW()) * 1000 AS BIGINT)
                    ELSE old.metadata_version
                    END)
            FROM old WHERE old.id = new.id
            RETURNING old.deleted AS old_deleted, old.metadata_version AS old_metadata_version, new.*;",
            &[&serde_json::to_string(&id).map_err(FileError::Serialize)?, &(old_metadata_version as i64), &name],
        )
        .await
        .map_err(FileError::Postgres)?;
    Ok(rows_to_metadata(&rows, old_metadata_version)?.new.metadata_version)
}

pub async fn create_folder(
    transaction: &Transaction<'_>,
    id: Uuid,
    parent: Uuid,
    name: &str,
    owner: &str,
    signature: &str,
) -> Result<u64, FileError> {
    let row = transaction.query_one(
        "INSERT INTO folders (id, parent, name, owner, signature, metadata_version, content_version, deleted)
        VALUES ($1, $2, $3, $4, $5, CAST(EXTRACT(EPOCH FROM NOW()) * 1000 AS BIGINT), CAST(EXTRACT(EPOCH FROM NOW()) * 1000 AS BIGINT), false)
        RETURNING *;",
        &[&(serde_json::to_string(&id).map_err(FileError::Serialize)?), &(serde_json::to_string(&parent).map_err(FileError::Serialize)?), &name, &owner, &signature]).await.map_err(FileError::Postgres)?;
    Ok(row
        .try_get::<&str, i64>("metadata_version")
        .map_err(FileError::Postgres)? as u64)
}

pub async fn delete_folder(
    transaction: &Transaction<'_>,
    file_id: &str,
    old_metadata_version: u64,
) -> Result<u64, FileError> {
    let rows = transaction
        .query(
            "WITH old AS (SELECT * FROM folders WHERE id = $1 FOR UPDATE)
            UPDATE folders new
            SET
                deleted = true,
                metadata_version = 
                    (CASE
                    WHEN NOT old.deleted AND old.metadata_version = $2
                    THEN CAST(EXTRACT(EPOCH FROM NOW()) * 1000 AS BIGINT)
                    ELSE old.metadata_version
                    END)
            FROM old WHERE old.id = new.id
            RETURNING old.deleted AS old_deleted, old.metadata_version AS old_metadata_version, new.*;",
            &[&file_id, &(old_metadata_version as i64)],
        )
        .await
        .map_err(FileError::Postgres)?;
    Ok(rows_to_metadata(&rows, old_metadata_version)?.new.metadata_version)
}

pub async fn move_folder(
    transaction: &Transaction<'_>,
    id: Uuid,
    old_metadata_version: u64,
    parent: Uuid,
) -> Result<u64, FileError> {
    let rows = transaction
        .query(
            "WITH old AS (SELECT * FROM folders WHERE id = $1 FOR UPDATE)
            UPDATE folders new
            SET
                parent = $3,
                metadata_version = 
                    (CASE
                    WHEN NOT old.deleted AND old.metadata_version = $2
                    THEN CAST(EXTRACT(EPOCH FROM NOW()) * 1000 AS BIGINT)
                    ELSE old.metadata_version
                    END)
            FROM old WHERE old.id = new.id
            RETURNING old.deleted AS old_deleted, old.metadata_version AS old_metadata_version, new.*;",
            &[&serde_json::to_string(&id).map_err(FileError::Serialize)?, &(old_metadata_version as i64), &serde_json::to_string(&parent).map_err(FileError::Serialize)?],
        )
        .await
        .map_err(FileError::Postgres)?;
    Ok(rows_to_metadata(&rows, old_metadata_version)?.new.metadata_version)
}

pub async fn rename_folder(
    transaction: &Transaction<'_>,
    id: Uuid,
    old_metadata_version: u64,
    name: &str,
) -> Result<u64, FileError> {
    let rows = transaction
        .query(
            "WITH old AS (SELECT * FROM folders WHERE id = $1 FOR UPDATE)
            UPDATE folders new
            SET
                name = $3,
                metadata_version = 
                    (CASE
                    WHEN NOT old.deleted AND old.metadata_version = $2
                    THEN CAST(EXTRACT(EPOCH FROM NOW()) * 1000 AS BIGINT)
                    ELSE old.metadata_version
                    END)
            FROM old WHERE old.id = new.id
            RETURNING old.deleted AS old_deleted, old.metadata_version AS old_metadata_version, new.*;",
            &[&serde_json::to_string(&id).map_err(FileError::Serialize)?, &(old_metadata_version as i64), &name],
        )
        .await
        .map_err(FileError::Postgres)?;
    Ok(rows_to_metadata(&rows, old_metadata_version)?.new.metadata_version)
}

pub async fn get_public_key(
    transaction: &Transaction<'_>,
    username: &str,
) -> Result<RSAPublicKey, PublicKeyError> {
    match transaction
        .query_one(
            "SELECT public_key FROM users WHERE username = $1;",
            &[&username],
        )
        .await
    {
        Ok(row) => {
            Ok(serde_json::from_str(row.get("public_key"))
                .map_err(PublicKeyError::Deserialization)?)
        }
        Err(e) => Err(PublicKeyError::Postgres(e)),
    }
}

struct FileUpdateMetadata {
    old_deleted: bool,
    old_metadata_version: u64,
    old_content_version: u64,
    new: FileMetadata,
}

pub fn rows_to_metadata(
    rows: &Vec<tokio_postgres::row::Row>,
    old_metadata_version: u64,
) -> Result<FileUpdateMetadata, FileError> {
    match rows.as_slice() {
        [] => Err(FileError::DoesNotExist),
        [row] => {
            let metadata = row_to_metadata(row)?;
            if metadata.old_deleted {
                return Err(FileError::Deleted);
            }
            if metadata.old_metadata_version != old_metadata_version {
                return Err(FileError::IncorrectOldVersion(metadata.old_metadata_version));
            }
            Ok(metadata)
        }
        _ => Err(FileError::Unknown(String::from(
            "unexpected multiple postgres rows",
        ))),
    }
}

pub fn row_to_metadata(row: &tokio_postgres::row::Row) -> Result<FileUpdateMetadata, FileError> {
    Ok(FileUpdateMetadata {
        old_deleted: row.try_get("old_deleted").map_err(FileError::Postgres)?,
        old_metadata_version: row
            .try_get::<&str, i64>("old_metadata_version")
            .map_err(FileError::Postgres)? as u64,
        old_content_version: match row
            .try_get::<&str, i64>("old_content_version") {
                Ok(v) => v as u64,
                Err(_) => 0
            },
        new: FileMetadata {
            id: serde_json::from_str(
                row.try_get::<&str, &str>("id")
                    .map_err(FileError::Postgres)?,
            )
            .map_err(FileError::Deserialize)?,
            parent: serde_json::from_str(
                row.try_get::<&str, &str>("parent")
                    .map_err(FileError::Postgres)?,
            )
            .map_err(FileError::Deserialize)?,
            name: row.try_get("name").map_err(FileError::Postgres)?,
            signature: serde_json::from_str(
                row.try_get::<&str, &str>("signature")
                    .map_err(FileError::Postgres)?,
            )
            .map_err(FileError::Deserialize)?,
            metadata_version: row
                .try_get::<&str, i64>("metadata_version")
                .map_err(FileError::Postgres)? as u64,
            content_version: row
                .try_get::<&str, i64>("content_version")
                .map_err(FileError::Postgres)? as u64,
            deleted: row.try_get("deleted").map_err(FileError::Postgres)?,
            user_access_keys: Default::default(),   // todo
            folder_access_keys: Default::default(), // todo
        },
    })
}

pub async fn get_updates(
    transaction: &Transaction<'_>,
    username: &str,
    metadata_version: u64,
) -> Result<Vec<FileMetadata>, FileError> {
    transaction.query(
        "SELECT file_id, file_name, file_path, file_content_version, file_metadata_version, deleted
    FROM files WHERE username = $1 AND file_metadata_version > $2",
        &[&username, &(metadata_version as i64)],
    ).await.map_err(FileError::Postgres)?.iter().map(|row| Ok(row_to_metadata(row)?.new)).collect()
}

pub async fn new_account(
    transaction: &Transaction<'_>,
    username: &str,
    public_key: &str,
) -> Result<(), AccountError> {
    transaction
        .execute(
            "INSERT INTO users (username, public_key) VALUES ($1, $2);",
            &[&username, &public_key],
        )
        .await?;
    Ok(())
}
