use std::marker::PhantomData;

use rusqlite::Connection;

use crate::API_LOC;
use crate::client;
use crate::client::{Client, GetUpdatesRequest};
use crate::error;
use crate::error_enum;
use crate::model::file_metadata::FileMetadata;
use crate::repo;
use crate::repo::account_repo::AccountRepo;
use crate::repo::db_provider;
use crate::repo::file_metadata_repo::FileMetadataRepo;

error_enum! {
    enum Error {
        ConnectionFailure(db_provider::Error),
        RetrievalError(repo::account_repo::Error),
        ApiError(client::ClientError),
        MetadataRepoError(repo::file_metadata_repo::Error),
    }
}

pub trait FileMetadataService {
    fn update(db: &Connection) -> Result<Vec<FileMetadata>, Error>;
}

pub struct FileMetadataServiceImpl<
    FileMetadataDb: FileMetadataRepo,
    AccountDb: AccountRepo,
    C: Client,
> {
    metadatas: PhantomData<FileMetadataDb>,
    accounts: PhantomData<AccountDb>,
    client: PhantomData<C>,
}

impl<FileMetadataDb: FileMetadataRepo, AccountDb: AccountRepo, ApiClient: Client>
FileMetadataService for FileMetadataServiceImpl<FileMetadataDb, AccountDb, ApiClient>
{
    fn update(db: &Connection) -> Result<Vec<FileMetadata>, Error> {
        let account = AccountDb::get_account(&db)?;

        let max_updated = match FileMetadataDb::last_updated(db) {
            Ok(max) => max,
            Err(_) => 0,
        };

        let updates = ApiClient::get_updates(
            API_LOC.to_string(),
            &GetUpdatesRequest {
                username: account.username.to_string(),
                auth: "".to_string(),
                since_version: max_updated as u64,
            },
        )
            .map(|metadatas| {
                metadatas
                    .into_iter()
                    .map(|t| FileMetadata {
                        id: t.file_id,
                        name: t.file_name,
                        path: t.file_path,
                        updated_at: t.file_metadata_version as i64,
                        // TODO: Fix this so status is tracked accurately
                        status: "Remote".to_string(),
                    })
                    .collect::<Vec<FileMetadata>>()
            })?;

        updates
            .into_iter()
            .for_each(|meta| match FileMetadataDb::insert(&db, &meta) {
                Ok(_) => {}
                Err(err) => {
                    error(format!("Insert Error {:?}", err));
                }
            });

        let all_meta = FileMetadataDb::get_all(&db)?;
        Ok(all_meta)
    }
}

#[cfg(test)]
mod unit_tests {
    use crate::client::{Client, ClientError, FileMetadata, GetUpdatesRequest, NewAccountRequest};
    use crate::crypto::{PubKeyCryptoService, RsaCryptoService};
    use crate::debug;
    use crate::model::account::Account;
    use crate::model::state::Config;
    use crate::repo::account_repo::{AccountRepo, AccountRepoImpl};
    use crate::repo::db_provider::{DbProvider, RamBackedDB};
    use crate::repo::file_metadata_repo::FileMetadataRepoImpl;
    use crate::repo::schema::SchemaCreatorImpl;
    use crate::service::file_metadata_service::{FileMetadataService, FileMetadataServiceImpl};

    type DefaultDbProvider = RamBackedDB<SchemaCreatorImpl>;

    struct ClientFake;

    impl Client for ClientFake {
        fn new_account(
            api_location: String,
            params: &NewAccountRequest,
        ) -> Result<(), ClientError> {
            Ok(())
        }

        fn get_updates(
            api_location: String,
            params: &GetUpdatesRequest,
        ) -> Result<Vec<FileMetadata>, ClientError> {
            Ok(vec![])
        }
    }

    #[test]
    fn get_updates() {
        let config = &Config {
            writeable_path: "ignored".to_string(),
        };

        let db = DefaultDbProvider::connect_to_db(&config).unwrap();

        type DefaultFileMetadataService =
        FileMetadataServiceImpl<FileMetadataRepoImpl, AccountRepoImpl, ClientFake>;

        AccountRepoImpl::insert_account(
            &db,
            &Account {
                username: "jimmyjohn".to_string(),
                keys: RsaCryptoService::generate_key().expect("Key gen failed"),
            },
        )
            .unwrap();

        let all_files = DefaultFileMetadataService::update(&db).unwrap();

        assert_eq!(all_files, vec![]);
    }
}
