use tokio_postgres::error::Error as PostgresError;
use tokio_postgres::Transaction;

#[derive(Debug)]
pub enum Error {
    Uninterpreted(PostgresError),
}

impl From<PostgresError> for Error {
    fn from(e: PostgresError) -> Error {
        Error::Uninterpreted(e)
    }
}

pub async fn generate_version(transaction: &Transaction<'_>) -> Result<i64, Error> {
    let version = transaction
        .query_one(
            "SELECT CAST(EXTRACT(EPOCH FROM NOW()) * 1000000 AS BIGINT);",
            &[],
        )
        .await?
        .try_get(0)?;
    Ok(version)
}
