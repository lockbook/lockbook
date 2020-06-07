use crate::index_db::generate_version::generate_version;
use crate::index_db::generate_version::Error as GenerateVersionError;
use tokio_postgres::error::Error as PostgresError;
use tokio_postgres::Transaction;

#[derive(Debug)]
pub enum Error {
    Uninterpreted(PostgresError),
    VersionGeneration(GenerateVersionError),
    FileDoesNotExist,
    IncorrectOldVersion(u64),
    FileDeleted,
}

pub async fn update_file_metadata_version(
    transaction: &Transaction<'_>,
    file_id: &String,
    old_metadata_version: u64,
) -> Result<u64, Error> {
    let new_metadata_version = generate_version(transaction)
        .await
        .map_err(Error::VersionGeneration)?;
    let num_affected = transaction.execute(
        "UPDATE files SET file_metadata_version = $1 WHERE file_id = $2 AND file_metadata_version = $3;",
        &[&(new_metadata_version as i64), &file_id, &(old_metadata_version as i64)]
    ).await.map_err(Error::Uninterpreted)?;
    let row_vec = transaction
        .query(
            "SELECT file_metadata_version, deleted FROM files WHERE file_id = $1;",
            &[&file_id],
        )
        .await
        .map_err(Error::Uninterpreted)?;

    match row_vec.len() {
        0 => Err(Error::FileDoesNotExist),
        _ => {
            let current_version: i64 = row_vec[0].try_get(0).map_err(Error::Uninterpreted)?;
            let deleted = row_vec[0].try_get(1).map_err(Error::Uninterpreted)?;

            match (num_affected, deleted) {
                (0, false) => Err(Error::IncorrectOldVersion(current_version as u64)),
                (_, false) => Ok(new_metadata_version),
                (_, true) => Err(Error::FileDeleted),
            }
        }
    }
}
