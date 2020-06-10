use tokio_postgres::error::Error as PostgresError;
use tokio_postgres::Transaction;

#[derive(Debug)]
pub enum Error {
    Uninterpreted(PostgresError),
}

pub async fn generate_version(transaction: &Transaction<'_>) -> Result<u64, Error> {
    let version: i64 = transaction
        .query_one(
            "SELECT CAST(EXTRACT(EPOCH FROM NOW()) * 1000 AS BIGINT);",
            &[],
        )
        .await
        .map_err(Error::Uninterpreted)?
        .try_get(0)
        .map_err(Error::Uninterpreted)?;
    Ok(version as u64)
}
