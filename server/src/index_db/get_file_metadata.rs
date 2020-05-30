use lockbook_core::model::api::FileMetadata;
use tokio_postgres::error::Error as PostgresError;
use tokio_postgres::Client as PostgresClient;

#[derive(Debug)]
pub enum Error {
    Postgres(PostgresError),
    FileDoesNotExist(()),
    InvalidUsername,
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

pub async fn get_file_metadata(
    client: &mut PostgresClient,
    username: &String,
    file_id: &String,
) -> Result<FileMetadata, Error> {

    if !username.chars().all(char::is_alphanumeric) {
        return Err(Error::InvalidUsername);
    }

    match client.query_one(
        "SELECT file_id, file_name, file_path, file_content_version, file_metadata_version, deleted
    FROM files WHERE username = $1 AND file_id = $2;",
        &[&username.to_lowercase(), &file_id],
    ).await {
        Ok(row) => Ok(to_file_metadata(&row)),
        Err(err) => Err(Error::Postgres(err)),
    }
}
