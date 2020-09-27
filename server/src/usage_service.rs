use lockbook_core::model::api::FileUsage;
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
    let result = transaction
        .query(
            "
with months as (
    select generate_series($1::text::date, $2::text::date, interval '1 hour') as timestamp
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
                          where ul.timestamp < wm.timestamp
                            and ul.file_id = wm.file_id
                          limit 1), 0) bytes
         from with_months wm
         union
         select *
         from usage_ledger
         where timestamp > $1::text::date
         and timestamp < $2::text::date
         order by file_id, timestamp desc
     ),
     lagged as (
         select file_id,
                timestamp          as start_date,
                coalesce(lag(timestamp) OVER (PARTITION BY file_id ORDER BY timestamp desc), $2::text::date) as end_date,
                bytes,
                owner
         from with_months_and_usage
         where with_months_and_usage.owner = $3
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
select * from integrated_by_month;
",
            &[
                &"2020-01-01",
                &"2020-10-01",
                username,
            ],
        )
        .await
        .map_err(UsageCalculateError::Postgres)?;

    debug!("Results {}", result.len());

    result.iter().map(row_to_usage).collect()
}

fn row_to_usage(row: &tokio_postgres::row::Row) -> Result<FileUsage, UsageCalculateError> {
    debug!("Row {:#?}", row);
    Ok(FileUsage {
        file_id: row
            .try_get("file_id")
            .map_err(UsageCalculateError::Postgres)?,
        byte_secs: row
            .try_get("byte_secs")
            .map_err(UsageCalculateError::Postgres)?,
        secs: row.try_get("secs").map_err(UsageCalculateError::Postgres)?,
    })
}
