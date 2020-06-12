use crate::index_db::generate_version::generate_version;
use crate::index_db::generate_version::Error as GenerateVersionError;
use tokio_postgres::error::Error as PostgresError;
use tokio_postgres::error::SqlState;
use tokio_postgres::Transaction;

#[derive(Debug)]
pub enum Error {
    FileIdTaken,
    FilePathTaken,
    Uninterpreted(PostgresError),
    VersionGeneration(GenerateVersionError),
}

impl From<PostgresError> for Error {
    fn from(e: PostgresError) -> Error {
        match (e.code(), e.to_string()) {
            (Some(error_code), error_string)
                if error_code == &SqlState::UNIQUE_VIOLATION
                    && error_string.contains("pk_files") =>
            {
                Error::FileIdTaken
            }
            (Some(error_code), error_string)
                if error_code == &SqlState::UNIQUE_VIOLATION
                    && error_string.contains("unique_file_path") =>
            {
                Error::FilePathTaken
            }
            _ => Error::Uninterpreted(e),
        }
    }
}

pub async fn create_file(
    transaction: &Transaction<'_>,
    file_id: &str,
    username: &str,
    file_name: &str,
    file_path: &str,
) -> Result<u64, Error> {
    let version = generate_version(transaction)
        .await
        .map_err(Error::VersionGeneration)?;

    transaction.execute("
INSERT INTO files (file_id, file_name, file_path, username, file_content_version, file_metadata_version, deleted)
VALUES ($1, $2, $3, $4, $5, $6, $7);
", &[file_id, file_name, file_path, username, &(version as i64), &(version as i64), &false]).await?;

    Ok(version)
}
