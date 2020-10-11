mod integration_test;

#[cfg(test)]
mod db_state_service_tests {
    use crate::integration_test::{generate_account, test_config};
    use lockbook_core::repo::db_version_repo::DbVersionRepo;
    use lockbook_core::service::db_state_service::DbStateService;
    use lockbook_core::service::db_state_service::State::{
        Empty, MigrationRequired, ReadyToUse, StateRequiresClearing,
    };
    use lockbook_core::CORE_CODE_VERSION;
    use lockbook_core::{
        connect_to_db, create_account, get_db_state, DefaultDbStateService, DefaultDbVersionRepo,
    };

    #[test]
    fn initial_state() {
        let cfg = test_config();
        let generated_account = generate_account();
        assert_eq!(get_db_state(&cfg).unwrap(), Empty);
        assert_eq!(get_db_state(&cfg).unwrap(), Empty);
        create_account(
            &cfg,
            &generated_account.username,
            &generated_account.api_url,
        )
        .unwrap();
        assert_eq!(get_db_state(&cfg).unwrap(), ReadyToUse);

        let db = connect_to_db(&cfg).unwrap();
        DefaultDbVersionRepo::set(&db, "0.1.0").unwrap();
        assert_ne!(
            DefaultDbVersionRepo::get(&db).unwrap().unwrap(),
            CORE_CODE_VERSION
        );

        assert_eq!(
            DefaultDbStateService::get_state(&db).unwrap(),
            MigrationRequired
        );
        assert_eq!(
            DefaultDbStateService::get_state(&db).unwrap(),
            MigrationRequired
        );

        assert!(DefaultDbStateService::perform_migration(&db).is_err());
        assert_eq!(
            DefaultDbStateService::get_state(&db).unwrap(),
            StateRequiresClearing
        );
        assert_eq!(
            DefaultDbStateService::get_state(&db).unwrap(),
            StateRequiresClearing
        );
    }
}
