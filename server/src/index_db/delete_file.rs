use crate::index_db::generate_version::generate_version;
use crate::index_db::generate_version::Error as VersionGenerationError;
use postgres::Client as PostgresClient;
use tokio_postgres;
use tokio_postgres::error::Error as PostgresError;

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

pub fn delete_file(client: &mut PostgresClient, file_id: &String) -> Result<i64, Error> {
    let new_version = generate_version(client)?;

    let mut transaction = client.transaction()?;
    let row_vec =
        transaction.query("SELECT deleted FROM files WHERE file_id = $1;", &[&file_id])?;
    let num_affected = transaction.execute(
        "UPDATE files SET deleted = TRUE WHERE file_id = $1;",
        &[&file_id],
    )?;
    transaction.commit()?;

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
