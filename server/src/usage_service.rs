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
            VALUES ($1, $2, $3, now());",
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
            "with a as (
                            select file_id, generate_series(cast(date_trunc('month', current_date) as date), current_date, interval '1 day') dt
                            from usage_ledger
                            where owner = $1
                            group by file_id
                        ),
                        intervaled as (
                            select file_id,
                                   (select t1.bytes
                                    from usage_ledger t1
                                    where t1.file_id = a.file_id
                                      and t1.timestamp::date <= a.dt
                                    order by timestamp desc
                                    limit 1),
                                   dt
                            from a
                            order by dt desc
                        ),
                        with_latest as (
                            select *, last_value(bytes) over (ORDER BY file_id DESC) AS most_recent
                            from intervaled
                            where bytes is not null
                        )
                        select file_id, avg(bytes)::bigint AS usage_mtd_avg, max(most_recent) AS usage_latest
                        from with_latest
                        where dt > cast(date_trunc('month', current_date) as date)
                        group by file_id;",
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
    pub usage_mtd_avg: i64,
    pub usage_latest: i64,
}

fn row_to_usage(row: &tokio_postgres::row::Row) -> Result<FileUsage, UsageCalculateError> {
    Ok(FileUsage {
        file_id: row
            .try_get("file_id")
            .map_err(UsageCalculateError::Postgres)?,
        usage_mtd_avg: row
            .try_get("usage_mtd_avg")
            .map_err(UsageCalculateError::Postgres)?,
        usage_latest: row
            .try_get("usage_latest")
            .map_err(UsageCalculateError::Postgres)?,
    })
}
