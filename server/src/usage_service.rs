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

pub async fn track_content_change(
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
                &(serde_json::to_vec(file_content)
                    .map_err(UsageTrackError::Serialize)?
                    .len() as i64),
            ],
        )
        .await
        .map_err(UsageTrackError::Postgres)?;

    Ok(())
}

pub async fn track_deletion(
    transaction: &Transaction<'_>,
    file_id: &Uuid,
    username: &String,
) -> Result<(), UsageTrackError> {
    let _ = transaction
        .execute(
            "INSERT INTO usage_ledger (file_id, owner, bytes, timestamp)
            VALUES ($1, $2, $3, now());",
            &[
                &serde_json::to_string(file_id).map_err(UsageTrackError::Serialize)?,
                username,
                &(0 as i64),
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
select * from integrated_by_month
;",
            &[
                &start_date,
                &end_date,
                username,
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

#[cfg(test)]
mod usage_service_tests {
    use std::str::FromStr;

    use lockbook_core::model::api::FileUsage;

    use crate::config::{config, IndexDbConfig};
    use crate::file_index_repo;
    use crate::usage_service::{calculate, UsageCalculateError};
    use uuid::Uuid;

    #[test]
    fn compute_usage() {
        let file_id = Uuid::new_v4();

        async fn do_stuff(
            config: &IndexDbConfig,
            file_id: Uuid,
        ) -> Result<Vec<FileUsage>, UsageCalculateError> {
            let mut pg_client = file_index_repo::connect(config).await.unwrap();

            let transaction = pg_client.transaction().await.unwrap();

            let date_start = chrono::NaiveDateTime::from_str("2000-10-01T00:00:00.000").unwrap();
            let date_end = chrono::NaiveDateTime::from_str("2000-10-31T00:00:00.000").unwrap();

            let _ = transaction
                .execute(
                    "INSERT INTO accounts (name, public_key) VALUES ('juicy', '');",
                    &[],
                )
                .await
                .unwrap();

            let _ = transaction.execute("INSERT INTO files (id, parent, parent_access_key, is_folder, name, owner, signature, deleted, metadata_version, content_version) VALUES ($1, $1, '', false, 'good_file.md', 'juicy', '', false, 0, 0);", &[&serde_json::to_string(&file_id).unwrap()]).await.unwrap();
            let _ = transaction.execute("INSERT INTO usage_ledger (file_id, timestamp, owner, bytes) VALUES ($1, '2000-09-15', 'juicy', 1000);", &[&serde_json::to_string(&file_id).unwrap()]).await.unwrap();
            let _ = transaction.execute("INSERT INTO usage_ledger (file_id, timestamp, owner, bytes) VALUES ($1, '2000-10-01', 'juicy', 10000);", &[&serde_json::to_string(&file_id).unwrap()]).await.unwrap();
            let _ = transaction.execute("INSERT INTO usage_ledger (file_id, timestamp, owner, bytes) VALUES ($1, '2000-10-15', 'juicy', 20000);", &[&serde_json::to_string(&file_id).unwrap()]).await.unwrap();
            let _ = transaction.execute("INSERT INTO usage_ledger (file_id, timestamp, owner, bytes) VALUES ($1, '2000-10-31', 'juicy', 30000);", &[&serde_json::to_string(&file_id).unwrap()]).await.unwrap();

            let res = calculate(&transaction, &"juicy".to_string(), date_start, date_end).await;

            res
        }

        let fake_config = config();

        let res = tokio_test::block_on(do_stuff(&fake_config.index_db, file_id)).unwrap();

        let top_usage = res.get(0).unwrap();
        assert_eq!(top_usage.file_id, file_id);
        assert_eq!(
            top_usage.byte_secs,
            ((10000 * 24 * 3600 * 16) + (20000 * 24 * 3600 * 15))
        );
    }
}
