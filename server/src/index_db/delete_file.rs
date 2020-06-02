use crate::index_db::generate_version::generate_version;
use crate::index_db::generate_version::Error as VersionGenerationError;
use tokio_postgres::error::Error as PostgresError;
use tokio_postgres::Transaction;

#[derive(Debug)]
pub enum Error {
    Uninterpreted(PostgresError),
    VersionGeneration(VersionGenerationError),
    FileDoesNotExist,
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

pub async fn delete_file(transaction: &Transaction<'_>, file_id: &String) -> Result<i64, Error> {
    let new_version = generate_version(transaction).await?;
    let row_vec = transaction
        .query("SELECT deleted FROM files WHERE file_id = $1;", &[&file_id])
        .await?;
    let num_affected = transaction
        .execute(
            "UPDATE files SET deleted = TRUE WHERE file_id = $1;",
            &[&file_id],
        )
        .await?;

    match num_affected {
        0 => Err(Error::FileDoesNotExist),
        _ => {
            let deleted = row_vec[0].try_get(0)?;

            if deleted {
                Err(Error::FileDeleted)
            } else {
                Ok(new_version)
            }
        }
    }
}
