use crate::index_db::update_file_metadata_version::update_file_metadata_version;
use crate::index_db::update_file_metadata_version::Error as UpdateFileMetadataVersionError;
use tokio_postgres::error::Error as PostgresError;
use tokio_postgres::Transaction;

#[derive(Debug)]
pub enum Error {
    Uninterpreted(PostgresError),
    MetadataVersionUpdate(UpdateFileMetadataVersionError),
    FileDoesNotExist,
    FileDeleted,
}

pub async fn rename_file(
    transaction: &Transaction<'_>,
    file_id: &str,
    old_metadata_version: u64,
    new_file_name: &str,
) -> Result<u64, Error> {
    let new_version = update_file_metadata_version(transaction, file_id, old_metadata_version)
        .await
        .map_err(Error::MetadataVersionUpdate)?;
    let num_affected = transaction
        .execute(
            "UPDATE files SET file_name = $1 WHERE file_id = $2;",
            &[&new_file_name, &file_id],
        )
        .await
        .map_err(Error::Uninterpreted)?;
    let row_vec = transaction
        .query("SELECT deleted FROM files WHERE file_id = $1;", &[&file_id])
        .await
        .map_err(Error::Uninterpreted)?;

    match num_affected {
        0 => Err(Error::FileDoesNotExist),
        _ => {
            let deleted = row_vec[0].try_get(0).map_err(Error::Uninterpreted)?;

            if deleted {
                Err(Error::FileDeleted)
            } else {
                Ok(new_version)
            }
        }
    }
}
