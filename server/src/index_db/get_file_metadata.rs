use lockbook_core::model::api::FileMetadata;
use tokio_postgres::error::Error as PostgresError;
use tokio_postgres::Transaction;

#[derive(Debug)]
pub enum Error {
    Postgres(PostgresError),
    FileDoesNotExist(()),
}

pub fn to_file_metadata(row: &tokio_postgres::row::Row) -> FileMetadata {
    FileMetadata {
        file_id: row.get("file_id"),
        file_name: row.get("file_name"),
        file_parent: row.get("file_path"),
        file_content_version: row.get::<&str, i64>("file_content_version") as u64,
        file_metadata_version: row.get::<&str, i64>("file_metadata_version") as u64,
        deleted: row.get("deleted"),
    }
}

pub async fn get_file_metadata(
    transaction: &Transaction<'_>,
    username: &str,
    file_id: &str,
) -> Result<FileMetadata, Error> {
    match transaction.query_one(
        "SELECT file_id, file_name, file_path, file_content_version, file_metadata_version, deleted
    FROM files WHERE username = $1 AND file_id = $2;",
        &[&username, &file_id],
    ).await {
        Ok(row) => Ok(to_file_metadata(&row)),
        Err(err) => Err(Error::Postgres(err)),
    }
}
