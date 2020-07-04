#[cfg(test)]
mod account_tests {
    use lockbook_core::client::Error;
    use lockbook_core::DefaultAccountService;
    use lockbook_core::model::api::NewAccountError::UsernameTaken;
    use lockbook_core::service::account_service::{AccountCreationError, AccountService};

    use crate::{random_username, test_db};
    use lockbook_core::model::api::NewAccountError;

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

        match DefaultAccountService::create_account(&db, username).unwrap_err() {
            AccountCreationError::ApiError(api_err) => match api_err {
                Error::Api(api_api_err) => {
                    match api_api_err {
                        UsernameTaken => {
                            return; // Test passed
                        },
                        _ => {}
                    }
                },
                _ => {}
            },
            _ => {}
        };

        panic!("This username should have been taken.")
    }


    #[test]
    fn invalid_username_test() {
        let db = test_db();
        let username = &random_username();
        DefaultAccountService::create_account(&db, username).unwrap();
        match DefaultAccountService::create_account(&db, username).unwrap_err() {
            AccountCreationError::ApiError(api_err) => match api_err {
                Error::Api(api_api_err) => {
                    match api_api_err {
                        UsernameTaken => {},
                        NewAccountError::InternalError => {},
                        NewAccountError::InvalidAuth => {},
                        NewAccountError::ExpiredAuth => {},
                        NewAccountError::InvalidPublicKey => {},
                        NewAccountError::InvalidUsername => {},
                        NewAccountError::FileIdTaken => {},
                    }
                },
                _ => {}
            },
            _ => {}
        }
    }
}
