#[cfg(test)]
mod usage_service_tests {
    use std::str::FromStr;

    use lockbook_models::api::FileUsage;

    use lockbook_crypto::clock_service::ClockImpl;
    use lockbook_crypto::crypto_service::{PubKeyCryptoService, RSAImpl};
    use lockbook_server_lib::config::{Config, IndexDbConfig};
    use lockbook_server_lib::file_index_repo;
    use lockbook_server_lib::usage_repo::{calculate, UsageCalculateError};
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

            let public_key = RSAImpl::<ClockImpl>::generate_key()
                .unwrap()
                .to_public_key();

            let _ = transaction
                .execute(
                    "INSERT INTO accounts (name, public_key) VALUES ('juicy', $1);",
                    &[&serde_json::to_string(&public_key).unwrap()],
                )
                .await
                .unwrap();

            let _ = transaction.execute("INSERT INTO files (id, parent, parent_access_key, is_folder, name, owner, signature, deleted, metadata_version, content_version) VALUES ($1, $1, '', false, 'good_file.md', 'juicy', '', false, 0, 0);", &[&serde_json::to_string(&file_id).unwrap()]).await.unwrap();
            let _ = transaction.execute("INSERT INTO usage_ledger (file_id, timestamp, owner, bytes) VALUES ($1, '2000-09-15', 'juicy', 1000);", &[&serde_json::to_string(&file_id).unwrap()]).await.unwrap();
            let _ = transaction.execute("INSERT INTO usage_ledger (file_id, timestamp, owner, bytes) VALUES ($1, '2000-10-01', 'juicy', 10000);", &[&serde_json::to_string(&file_id).unwrap()]).await.unwrap();
            let _ = transaction.execute("INSERT INTO usage_ledger (file_id, timestamp, owner, bytes) VALUES ($1, '2000-10-15', 'juicy', 20000);", &[&serde_json::to_string(&file_id).unwrap()]).await.unwrap();
            let _ = transaction.execute("INSERT INTO usage_ledger (file_id, timestamp, owner, bytes) VALUES ($1, '2000-10-31', 'juicy', 30000);", &[&serde_json::to_string(&file_id).unwrap()]).await.unwrap();

            let res = calculate(&transaction, &public_key, date_start, date_end).await;

            res
        }

        let fake_config = Config::from_env_vars();

        let res = tokio_test::block_on(do_stuff(&fake_config.index_db, file_id)).unwrap();

        let top_usage = res.get(0).unwrap();
        assert_eq!(top_usage.file_id, file_id);
        assert_eq!(
            top_usage.byte_secs,
            ((10000 * 24 * 3600 * 16) + (20000 * 24 * 3600 * 15))
        );
    }
}
