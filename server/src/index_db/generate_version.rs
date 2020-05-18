use tokio_postgres;
use tokio_postgres::error::Error as PostgresError;
use tokio_postgres::Client as PostgresClient;

#[derive(Debug)]
pub enum Error {
    Uninterpreted(PostgresError),
}

impl From<PostgresError> for Error {
    fn from(e: PostgresError) -> Error {
        Error::Uninterpreted(e)
    }
}

pub async fn generate_version(client: &mut PostgresClient) -> Result<i64, Error> {
    let version = client
        .query_one(
            "SELECT CAST(EXTRACT(EPOCH FROM NOW()) * 1000000 AS BIGINT);",
            &[],
        )
        .await?
        .try_get(0)?;
    Ok(version)
}
