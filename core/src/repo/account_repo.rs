use crate::model::account::{Account, ApiUrl};
use crate::repo::account_repo::AccountRepoError::NoAccount;
use crate::storage::db_provider::Backend;

#[derive(Debug)]
pub enum AccountRepoError<MyBackend: Backend> {
    BackendError(MyBackend::Error),
    SerdeError(serde_json::Error),
    NoAccount,
}

pub trait AccountRepo<MyBackend: Backend> {
    fn insert_account(
        backend: &MyBackend::Db,
        account: &Account,
    ) -> Result<(), AccountRepoError<MyBackend>>;
    fn maybe_get_account(
        backend: &MyBackend::Db,
    ) -> Result<Option<Account>, AccountRepoError<MyBackend>>;
    fn get_account(backend: &MyBackend::Db) -> Result<Account, AccountRepoError<MyBackend>>;
    fn get_api_url(backend: &MyBackend::Db) -> Result<ApiUrl, AccountRepoError<MyBackend>>;
}

pub struct AccountRepoImpl<MyBackend: Backend> {
    _backend: MyBackend,
}

static ACCOUNT: &str = "account";
static YOU: &str = "you";

impl<MyBackend: Backend> AccountRepo<MyBackend> for AccountRepoImpl<MyBackend> {
    fn insert_account(
        backend: &MyBackend::Db,
        account: &Account,
    ) -> Result<(), AccountRepoError<MyBackend>> {
        MyBackend::write(
            backend,
            ACCOUNT,
            YOU,
            serde_json::to_vec(account).map_err(AccountRepoError::SerdeError)?,
        )
        .map_err(AccountRepoError::BackendError)
    }

    fn maybe_get_account(
        backend: &MyBackend::Db,
    ) -> Result<Option<Account>, AccountRepoError<MyBackend>> {
        match Self::get_account(backend) {
            Ok(account) => Ok(Some(account)),
            Err(err) => match err {
                AccountRepoError::NoAccount => Ok(None),
                other => Err(other),
            },
        }
    }

    fn get_account(backend: &MyBackend::Db) -> Result<Account, AccountRepoError<MyBackend>> {
        let maybe_value: Option<Vec<u8>> =
            MyBackend::read(backend, ACCOUNT, YOU).map_err(AccountRepoError::BackendError)?;
        match maybe_value {
            None => Err(NoAccount),
            Some(account) => {
                Ok(serde_json::from_slice(account.as_ref())
                    .map_err(AccountRepoError::SerdeError)?)
            }
        }
    }

    fn get_api_url(backend: &MyBackend::Db) -> Result<ApiUrl, AccountRepoError<MyBackend>> {
        Self::get_account(backend).map(|account| account.api_url)
    }
}

#[cfg(test)]
mod unit_tests {
    use super::AccountRepoImpl;
    use crate::model::account::Account;
    use crate::model::state::temp_config;
    use crate::repo::account_repo::AccountRepo;
    use crate::service::clock_service::ClockImpl;
    use crate::service::crypto_service::{PubKeyCryptoService, RSAImpl};
    use crate::storage::db_provider::{Backend, FileBackend};

    type DefaultAccountRepo = AccountRepoImpl<FileBackend>;

    #[test]
    fn insert_account() {
        let test_account = Account {
            username: "parth".to_string(),
            api_url: "ftp://uranus.net".to_string(),
            private_key: RSAImpl::<ClockImpl>::generate_key().expect("Key generation failure"),
        };

        let config = temp_config();
        let backend = FileBackend::connect_to_db(&config).unwrap();
        let res = DefaultAccountRepo::get_account(backend);
        assert!(res.is_err());

        DefaultAccountRepo::insert_account(&backend, &test_account).unwrap();

        let db_account = DefaultAccountRepo::get_account(backend).unwrap();
        assert_eq!(test_account, db_account);
    }
}
