#[cfg(test)]
mod account_tests {
    use crate::{random_username, test_db};
    use lockbook_core::service::account_service::AccountService;
    use lockbook_core::DefaultAccountService;

    #[test]
    fn create_account_successfully() {
        let db = test_db();
        let account = DefaultAccountService::create_account(&db, random_username()).unwrap();
    }
}
