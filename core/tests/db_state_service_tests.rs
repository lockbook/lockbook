mod integration_test;

#[cfg(test)]
mod db_state_service_tests {
    use crate::integration_test::{generate_account, test_config};
    use lockbook_core::repo::db_version_repo::DbVersionRepo;
    use lockbook_core::service::code_version_service::{CodeVersion, CodeVersionImpl};
    use lockbook_core::service::db_state_service::DbStateService;
    use lockbook_core::service::db_state_service::State::{
        Empty, MigrationRequired, ReadyToUse, StateRequiresClearing,
    };
    use lockbook_core::storage::db_provider::Backend;
    use lockbook_core::{
        create_account, get_db_state, DefaultBackend, DefaultDbStateService, DefaultDbVersionRepo,
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

        let backend = DefaultBackend::connect_to_db(&cfg).unwrap();

        DefaultDbVersionRepo::set(&backend, "0.1.0").unwrap();
        assert_ne!(
            DefaultDbVersionRepo::get(&backend).unwrap().unwrap(),
            CodeVersionImpl::get_code_version()
        );

        assert_eq!(
            DefaultDbStateService::get_state(&backend).unwrap(),
            MigrationRequired
        );
        assert_eq!(
            DefaultDbStateService::get_state(&backend).unwrap(),
            MigrationRequired
        );

        assert!(DefaultDbStateService::perform_migration(&backend).is_err());
        assert_eq!(
            DefaultDbStateService::get_state(&backend).unwrap(),
            StateRequiresClearing
        );
        assert_eq!(
            DefaultDbStateService::get_state(&backend).unwrap(),
            StateRequiresClearing
        );
    }
}
