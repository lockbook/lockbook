use crate::index_db::get_file_metadata::to_file_metadata;
use lockbook_core::model::api::FileMetadata;
use tokio_postgres::error::Error as PostgresError;
use tokio_postgres::Client as PostgresClient;

#[derive(Debug)]
pub enum Error {
    Postgres(PostgresError),
}

pub async fn get_updates(
    client: &mut PostgresClient,
    username: &String,
    metadata_version: i64,
) -> Result<Vec<FileMetadata>, Error> {
    match client.query(
        "SELECT file_id, file_name, file_path, file_content_version, file_metadata_version, deleted
    FROM files WHERE username = $1 AND file_metadata_version > $2",
        &[&username, &metadata_version],
    ).await {
        Ok(rows) => Ok(rows.iter().map(to_file_metadata).collect()),
        Err(err) => Err(Error::Postgres(err)),
    }
}
