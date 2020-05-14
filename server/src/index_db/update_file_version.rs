use crate::index_db::generate_version::generate_version;
use crate::index_db::generate_version::Error as VersionGenerationError;
use tokio_postgres;
use tokio_postgres::error::Error as PostgresError;
use tokio_postgres::Client as PostgresClient;

#[derive(Debug)]
pub enum Error {
    Uninterpreted(PostgresError),
    VersionGeneration(VersionGenerationError),
    FileDoesNotExist,
    IncorrectOldVersion(i64),
    FileDeleted,
}

impl From<PostgresError> for Error {
    fn from(e: PostgresError) -> Error {
        Error::Uninterpreted(e)
    }
}

impl From<VersionGenerationError> for Error {
    fn from(e: VersionGenerationError) -> Error {
        Error::VersionGeneration(e)
    }
}

pub async fn update_file_version(
    client: &mut PostgresClient,
    file_id: &String,
    old_version: &i64,
) -> Result<i64, Error> {
    let new_version = generate_version(client).await?;

    let mut transaction = client.transaction().await?;
    let num_affected = transaction.execute(
        "UPDATE files SET file_content_version = $1 WHERE file_id = $2 AND file_content_version = $3;",
        &[&new_version, &file_id, &old_version]
    ).await?;
    let row_vec = transaction
        .query(
            "SELECT file_content_version, deleted FROM files WHERE file_id = $1;",
            &[&file_id],
        )
        .await?;
    transaction.commit().await?;

    match row_vec.len() {
        0 => Err(Error::FileDoesNotExist),
        _ => {
            let current_version = row_vec[0].try_get(0)?;
            let deleted = row_vec[0].try_get(1)?;

            match (num_affected, deleted) {
                (0, false) => Err(Error::IncorrectOldVersion(current_version)),
                (_, false) => Ok(new_version),
                (_, true) => Err(Error::FileDeleted),
            }
        }
    }
}
