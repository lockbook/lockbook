use crate::index_db::generate_version::generate_version;
use crate::index_db::generate_version::Error as VersionGenerationError;
use tokio_postgres;
use tokio_postgres::error::Error as PostgresError;
use tokio_postgres::error::SqlState;
use tokio_postgres::Client as PostgresClient;

#[derive(Debug)]
pub enum Error {
    Uninterpreted(PostgresError),
    VersionGeneration(VersionGenerationError),
    FileDoesNotExist,
    FileDeleted,
    FilePathTaken,
}

impl From<PostgresError> for Error {
    fn from(e: PostgresError) -> Error {
        match (e.code(), e.to_string()) {
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

impl From<VersionGenerationError> for Error {
    fn from(e: VersionGenerationError) -> Error {
        Error::VersionGeneration(e)
    }
}

pub async fn move_file(
    client: &mut PostgresClient,
    file_id: &String,
    new_file_path: &String,
) -> Result<i64, Error> {
    let new_version = generate_version(client).await?;

    let transaction = client.transaction().await?;
    let num_affected = transaction
        .execute(
            "UPDATE files SET file_path = $1 WHERE file_id = $2;",
            &[&new_file_path, &file_id],
        )
        .await?;
    let row_vec = transaction
        .query("SELECT deleted FROM files WHERE file_id = $1;", &[&file_id])
        .await?;
    transaction.commit().await?;

    match num_affected {
        0 => Err(Error::FileDoesNotExist),
        _ => {
            let deleted = row_vec[0].try_get(0)?;

            match deleted {
                false => Ok(new_version),
                true => Err(Error::FileDeleted),
            }
        }
    }
}
