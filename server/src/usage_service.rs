use lockbook_core::model::crypto::EncryptedValueWithNonce;
use tokio_postgres::error::Error as PostgresError;
use tokio_postgres::Transaction;
use uuid::Uuid;

#[derive(Debug)]
pub enum UsageTrackError {
    Serialize(serde_json::Error),
    Postgres(PostgresError),
}

pub async fn track(
    transaction: &Transaction<'_>,
    file_id: &Uuid,
    username: &String,
    file_content: &EncryptedValueWithNonce,
) -> Result<(), UsageTrackError> {
    let _ = transaction
        .execute(
            "INSERT INTO usage_ledger (file_id, owner, bytes, timestamp)
            VALUES ($1, $2, $3, CAST(EXTRACT(EPOCH FROM NOW()) * 1000 AS BIGINT));",
            &[
                &serde_json::to_string(file_id).map_err(UsageTrackError::Serialize)?,
                username,
                &(file_content.garbage.as_bytes().len() as i64),
            ],
        )
        .await
        .map_err(UsageTrackError::Postgres)?;

    Ok(())
}

#[derive(Debug)]
pub enum UsageCalculateError {
    Serialize(serde_json::Error),
    Postgres(PostgresError),
}

pub async fn calculate(
    transaction: &Transaction<'_>,
    username: &String,
) -> Result<Vec<FileUsage>, UsageCalculateError> {
    let results = transaction
        .query(
            "WITH integrated_usage AS (
                    SELECT
                        file_id,
                        bytes,
                        timestamp,
                        least(
                            lag(timestamp) OVER (ORDER BY timestamp DESC) - timestamp,
                            CAST(EXTRACT(EPOCH FROM NOW()) * 1000 AS BIGINT) - timestamp
                        ) AS life
                    FROM usage_ledger
                    WHERE owner = $1
                )
                SELECT file_id, (sum(bytes*life)/(3600*24*30))::bigint usage
                FROM integrated_usage
                GROUP BY file_id;",
            &[username],
        )
        .await
        .map_err(UsageCalculateError::Postgres)?;

    debug!("Returned {} results!", results.len());

    results.iter().map(row_to_usage).collect()
}

#[derive(Debug)]
pub struct FileUsage {
    file_id: String,
    pub usage: i64,
}

fn row_to_usage(row: &tokio_postgres::row::Row) -> Result<FileUsage, UsageCalculateError> {
    Ok(FileUsage {
        file_id: row
            .try_get("file_id")
            .map_err(UsageCalculateError::Postgres)?,
        usage: row
            .try_get("usage")
            .map_err(UsageCalculateError::Postgres)?,
    })
}
