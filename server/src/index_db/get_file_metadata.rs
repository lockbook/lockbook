use postgres::Client as PostgresClient;
use serde::{Deserialize, Serialize};
use tokio_postgres;
use tokio_postgres::error::Error as PostgresError;

#[derive(Debug)]
pub enum Error {
    Postgres(PostgresError),
    FileDoesNotExist(()),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileMetadata {
    pub file_id: String,
    pub file_name: String,
    pub file_path: String,
    pub file_content_version: i64,
    pub file_metadata_version: i64,
    pub deleted: bool,
}

impl From<&tokio_postgres::row::Row> for FileMetadata {
    fn from(row: &tokio_postgres::row::Row) -> FileMetadata {
        FileMetadata {
            file_id: row.get("file_id"),
            file_name: row.get("file_name"),
            file_path: row.get("file_path"),
            file_content_version: row.get("file_content_version"),
            file_metadata_version: row.get("file_metadata_version"),
            deleted: row.get("deleted"),
        }
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
        Ok(row) => Ok(FileMetadata::from(&row)),
        Err(err) => Err(Error::Postgres(err)),
    }
}
