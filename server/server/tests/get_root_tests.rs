mod integration_test;

#[cfg(test)]
mod get_root_tests {
    use crate::integration_test::{api_url, random_uuid, test_config};
    use libsecp256k1::PublicKey;
    use lockbook_models::file_metadata::FileMetadata;
    use lockbook_server_lib::config::Config;
    use lockbook_server_lib::file_index_repo;

    async fn get_user_root(pub_key: &PublicKey) -> FileMetadata {
        let fake_config = Config::from_env_vars().index_db;
        let pg_client = file_index_repo::connect(&fake_config).await.unwrap();
        let mut transaction = pg_client.begin().await.unwrap();
        file_index_repo::get_root(&mut transaction, &pub_key)
            .await
            .unwrap()
    }

    #[test]
    fn get_root() {
        let cfg1 = test_config();
        let account1 = lockbook_core::create_account(&cfg1, &random_uuid(), &api_url()).unwrap();
        lockbook_core::create_file_at_path(
            &cfg1,
            &format!("{}/a/b/c/d/test.txt", account1.username),
        )
        .unwrap();
        lockbook_core::sync_all(&cfg1, None).unwrap();

        let cfg2 = test_config();
        let account2 = lockbook_core::create_account(&cfg2, &random_uuid(), &api_url()).unwrap();
        lockbook_core::create_file_at_path(
            &cfg2,
            &format!("{}/a/b/c/d/test.txt", account2.username),
        )
        .unwrap();
        lockbook_core::sync_all(&cfg2, None).unwrap();

        let server_root = tokio_test::block_on(get_user_root(&account1.public_key()));
        let core_root = lockbook_core::get_root(&cfg1).unwrap();
        assert_eq!(server_root.id, core_root.id);

        let server_root = tokio_test::block_on(get_user_root(&account2.public_key()));
        let core_root = lockbook_core::get_root(&cfg2).unwrap();
        assert_eq!(server_root.id, core_root.id);
    }
}
