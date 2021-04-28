use lockbook_models::api::FileUsage;
use rsa::RSAPublicKey;
use tokio_postgres::error::Error as PostgresError;
use tokio_postgres::Transaction;
use uuid::Uuid;

#[derive(Debug)]
pub enum UsageTrackError {
    Serialize(serde_json::Error),
    Postgres(PostgresError),
}

pub async fn track_content_change(
    transaction: &Transaction<'_>,
    file_id: &Uuid,
    public_key: &RSAPublicKey,
    file_content_len: i64,
) -> Result<(), UsageTrackError> {
    let _ = transaction
        .execute(
            "INSERT INTO usage_ledger (file_id, owner, bytes, timestamp)
            VALUES ($1, (SELECT name FROM accounts WHERE public_key = $2), $3, now());",
            &[
                &serde_json::to_string(file_id).map_err(UsageTrackError::Serialize)?,
                &serde_json::to_string(public_key).map_err(UsageTrackError::Serialize)?,
                &file_content_len,
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
    public_key: &RSAPublicKey,
    start_date: chrono::NaiveDateTime,
    end_date: chrono::NaiveDateTime,
) -> Result<Vec<FileUsage>, UsageCalculateError> {
    debug!("Calculating usage from {} to {}", start_date, end_date);
    let result = transaction
        .query(
            "
with months as (
    select unnest(array[$1::timestamp, $2::timestamp]) as timestamp
),
     with_months as (
         select distinct m.timestamp, ul.file_id, ul.owner
         from months m
                  cross join usage_ledger ul
     ),
     with_months_and_usage as (
         select file_id,
                timestamp,
                owner,
                coalesce((select first_value(bytes) OVER (ORDER BY timestamp DESC)
                          from usage_ledger ul
                          where ul.timestamp <= wm.timestamp
                            and ul.file_id = wm.file_id
                          limit 1), 0) bytes
         from with_months wm
         union
         select *
         from usage_ledger
         where timestamp >= $1
         and timestamp <= $2
         order by file_id, timestamp desc
     ),
     lagged as (
         select file_id,
                timestamp          as start_date,
                coalesce(lag(timestamp) OVER (PARTITION BY file_id ORDER BY timestamp desc), $2::timestamp) as end_date,
                bytes,
                owner
         from with_months_and_usage
         where with_months_and_usage.owner = (SELECT name FROM accounts WHERE public_key = $3)
         order by file_id, start_date desc
     ),
     lagged_with_area as (
         select *, (extract(epoch from (end_date - start_date)) * bytes)::bigint byte_secs
         from lagged
     ),
     integrated_by_month as (
         select file_id, sum(byte_secs)::bigint byte_secs, min(start_date), max(end_date), extract(epoch from (max(end_date) - min(start_date)))::bigint secs
         from lagged_with_area
         group by file_id
     )
select * from integrated_by_month
;",
            &[
                &start_date,
                &end_date,
                &serde_json::to_string(public_key).map_err(UsageCalculateError::Serialize)?,
            ],
        )
        .await
        .map_err(UsageCalculateError::Postgres)?;

    trace!("Usage query results {}", result.len());

    result.iter().map(row_to_usage).collect()
}

fn row_to_usage(row: &tokio_postgres::row::Row) -> Result<FileUsage, UsageCalculateError> {
    trace!("Parsing usage row {:?}", row);
    Ok(FileUsage {
        file_id: serde_json::from_str(
            row.try_get("file_id")
                .map_err(UsageCalculateError::Postgres)?,
        )
        .map_err(UsageCalculateError::Serialize)?,
        byte_secs: row
            .try_get::<&str, i64>("byte_secs")
            .map_err(UsageCalculateError::Postgres)? as u64,
        secs: row
            .try_get::<&str, i64>("secs")
            .map_err(UsageCalculateError::Postgres)? as u64,
    })
}
