use lockbook_core::model::api::FileMetadata;
use postgres::Client as PostgresClient;
use tokio_postgres;
use tokio_postgres::error::Error as PostgresError;

#[derive(Debug)]
pub enum Error {
    Postgres(PostgresError),
    FileDoesNotExist(()),
}

pub fn to_file_metadata(row: &tokio_postgres::row::Row) -> FileMetadata {
    FileMetadata {
        file_id: row.get("file_id"),
        file_name: row.get("file_name"),
        file_path: row.get("file_path"),
        file_content_version: row.get::<&str, i64>("file_content_version") as u64,
        file_metadata_version: row.get::<&str, i64>("file_metadata_version") as u64,
        deleted: row.get("deleted"),
    }
}

pub fn get_file_metadata(
    client: &mut PostgresClient,
    username: &String,
    file_id: &String,
) -> Result<FileMetadata, Error> {
    match client.query_one(
        "SELECT file_id, file_name, file_path, file_content_version, file_metadata_version, deleted
    FROM files WHERE username = $1 AND file_id = $2;",
        &[&username, &file_id],
    ) {
        Ok(row) => Ok(to_file_metadata(&row)),
        Err(err) => Err(Error::Postgres(err)),
    }
}
