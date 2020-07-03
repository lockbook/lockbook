#[cfg(test)]
mod account_tests {
    use crate::{random_username, test_db};
    use lockbook_core::service::account_service::AccountService;
    use lockbook_core::DefaultAccountService;

    #[test]
    fn create_account_successfully() {
        let db = test_db();
        DefaultAccountService::create_account(&db, &random_username()).unwrap();
    }

    #[test]
    fn username_taken_test() {
        let db = test_db();
        let username = &random_username();
        DefaultAccountService::create_account(&db, username).unwrap();
        assert_eq!(DefaultAccountService::create_account(&db, username).unwrap_err(), )
    }
}
