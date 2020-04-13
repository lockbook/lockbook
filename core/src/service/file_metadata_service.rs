use std::marker::PhantomData;

use crate::error;
use crate::error_enum;
use crate::lockbook_api;
use crate::lockbook_api::GetUpdatesRequest;
use crate::models::file_metadata::FileMetadata;
use crate::repo;
use crate::repo::account_repo::AccountRepo;
use crate::repo::file_metadata_repo::FileMetadataRepo;
use crate::{db_provider, API_LOC};
use rusqlite::Connection;

error_enum! {
    enum Error {
        ConnectionFailure(db_provider::Error),
        RetrievalError(repo::account_repo::Error),
        ApiError(lockbook_api::get_updates::GetUpdatesError),
        MetadataRepoError(repo::file_metadata_repo::Error),
    }
}

pub trait FileMetadataService {
    fn update(db: &Connection) -> Result<Vec<FileMetadata>, Error>;
}

pub struct FileMetadataServiceImpl<FileMetadataDb: FileMetadataRepo, AccountDb: AccountRepo> {
    metadatas: PhantomData<FileMetadataDb>,
    accounts: PhantomData<AccountDb>,
}

impl<FileMetadataDb: FileMetadataRepo, AccountDb: AccountRepo> FileMetadataService
    for FileMetadataServiceImpl<FileMetadataDb, AccountDb>
{
    fn update(db: &Connection) -> Result<Vec<FileMetadata>, Error> {
        let account = AccountDb::get_account(&db)?;

        let max_updated = match FileMetadataDb::last_updated(db) {
            Ok(max) => max,
            Err(_) => 0,
        };

        let updates = lockbook_api::get_updates(
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
    use crate::crypto::{KeyPair, PrivateKey, PublicKey};
    use crate::db_provider::{DbProvider, RamBackedDB};
    use crate::debug;
    use crate::models::account::Account;
    use crate::repo::account_repo::{AccountRepo, AccountRepoImpl};
    use crate::repo::file_metadata_repo::FileMetadataRepoImpl;
    use crate::schema::SchemaCreatorImpl;
    use crate::service::file_metadata_service::{FileMetadataService, FileMetadataServiceImpl};
    use crate::state::Config;

    type DefaultDbProvider = RamBackedDB<SchemaCreatorImpl>;

    #[test]
    fn get_updates() {
        let config = &Config {
            writeable_path: "ignored".to_string(),
        };

        let db = DefaultDbProvider::connect_to_db(&config).unwrap();

        type DefaultFileMetadataService =
            FileMetadataServiceImpl<FileMetadataRepoImpl, AccountRepoImpl>;

        AccountRepoImpl::insert_account(
            &db,
            &Account {
                username: "jimmyjohn".to_string(),
                keys: KeyPair {
                    public_key: PublicKey {
                        n: "a".to_string(),
                        e: "s".to_string(),
                    },
                    private_key: PrivateKey {
                        d: "d".to_string(),
                        p: "f".to_string(),
                        q: "g".to_string(),
                        dmp1: "h".to_string(),
                        dmq1: "j".to_string(),
                        iqmp: "k".to_string(),
                    },
                },
            },
        )
        .unwrap();

        let all_files = DefaultFileMetadataService::update(&db).unwrap();

        debug(format!("{:?}", serde_json::to_string(&all_files)))
    }
}
