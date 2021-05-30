mod integration_test;

#[cfg(test)]
mod db_state_service_tests {
    use lockbook_core::repo::db_version_repo;
    use lockbook_core::service::db_state_service;
    use lockbook_core::service::db_state_service::DbStateService;
    use lockbook_core::service::db_state_service::State::{
        Empty, MigrationRequired, ReadyToUse, StateRequiresClearing,
    };
    use lockbook_core::{create_account, get_db_state, DefaultDbStateService};
    use test_utils::{generate_account, test_config};

    #[test]
    fn initial_state() {
        let config = test_config();
        let generated_account = generate_account();
        assert_eq!(get_db_state(&config).unwrap(), Empty);
        assert_eq!(get_db_state(&config).unwrap(), Empty);
        create_account(
            &config,
            &generated_account.username,
            &generated_account.api_url,
        )
        .unwrap();
        assert_eq!(get_db_state(&config).unwrap(), ReadyToUse);

        db_version_repo::set(&config, "0.1.0").unwrap();
        assert_ne!(
            db_version_repo::get(&config).unwrap().unwrap(),
            db_state_service::get_code_version()
        );

        assert_eq!(
            DefaultDbStateService::get_state(&config).unwrap(),
            MigrationRequired
        );
        assert_eq!(
            DefaultDbStateService::get_state(&config).unwrap(),
            MigrationRequired
        );

        assert!(DefaultDbStateService::perform_migration(&config).is_err());
        assert_eq!(
            DefaultDbStateService::get_state(&config).unwrap(),
            StateRequiresClearing
        );
        assert_eq!(
            DefaultDbStateService::get_state(&config).unwrap(),
            StateRequiresClearing
        );
    }
}
