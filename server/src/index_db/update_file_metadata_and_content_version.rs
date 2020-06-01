use crate::index_db::update_file_metadata_version::update_file_metadata_version;
use crate::index_db::update_file_metadata_version::Error as UpdateFileMetadataVersionError;
use tokio_postgres::error::Error as PostgresError;
use tokio_postgres::Transaction;

#[derive(Debug)]
pub enum Error {
    Uninterpreted(PostgresError),
    MetadataVersionUpdate(UpdateFileMetadataVersionError),
}

pub async fn update_file_metadata_and_content_version(
    transaction: &Transaction<'_>,
    file_id: &String,
    old_metadata_version: u64,
) -> Result<u64, Error> {
    let new_version = update_file_metadata_version(transaction, file_id, old_metadata_version)
        .await
        .map_err(Error::MetadataVersionUpdate)?;
    transaction
        .execute(
            "UPDATE files SET file_content_version = file_metadata_version WHERE file_id = $2;",
            &[
                &(new_version as i64),
                &file_id,
                &(old_metadata_version as i64),
            ],
        )
        .await
        .map_err(Error::Uninterpreted)?;
    Ok(new_version)
}
