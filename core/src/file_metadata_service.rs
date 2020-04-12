use std::marker::PhantomData;

use crate::account_repo;
use crate::account_repo::AccountRepo;
use crate::error_enum;
use crate::file_metadata::FileMetadata;
use crate::file_metadata_repo::FileMetadataRepo;
use crate::lockbook_api;
use crate::lockbook_api::GetUpdatesParams;
use crate::{db_provider, API_LOC};
use rusqlite::Connection;

error_enum! {
    enum Error {
        ConnectionFailure(db_provider::Error),
        RetrievalError(account_repo::Error),
        ApiError(lockbook_api::get_updates::GetUpdatesError),
    }
}

pub trait FileMetadataService {
    fn get_all_files(db: &Connection) -> Result<Vec<FileMetadata>, Error>;
}

pub struct FileMetadataServiceImpl<FileMetadataDb: FileMetadataRepo, AccountDb: AccountRepo> {
    metadatas: PhantomData<FileMetadataDb>,
    accounts: PhantomData<AccountDb>,
}

impl<FileMetadataDb: FileMetadataRepo, AccountDb: AccountRepo> FileMetadataService
    for FileMetadataServiceImpl<FileMetadataDb, AccountDb>
{
    fn get_all_files(db: &Connection) -> Result<Vec<FileMetadata>, Error> {
        let account = AccountDb::get_account(&db)?;

        let updates = lockbook_api::get_updates(
            API_LOC,
            &GetUpdatesParams {
                username: account.username.to_string(),
                auth: "".to_string(),
                since_version: 0,
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
                    status: "Remote".to_string(),
                })
                .collect::<Vec<FileMetadata>>()
        });

        Ok(updates?)
    }
}

#[cfg(test)]
mod unit_tests {
    use crate::account::Account;
    use crate::account_repo::{AccountRepo, AccountRepoImpl};
    use crate::crypto::{KeyPair, PrivateKey, PublicKey};
    use crate::db_provider::{DbProvider, RamBackedDB};
    use crate::file_metadata_repo::FileMetadataRepoImpl;
    use crate::file_metadata_service::{FileMetadataService, FileMetadataServiceImpl};
    use crate::schema::SchemaCreatorImpl;
    use crate::state::Config;

    type DefaultDbProvider = RamBackedDB<SchemaCreatorImpl>;

    #[test]
    fn get_all_files() {
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

        let all_files = DefaultFileMetadataService::get_all_files(&db).unwrap();

        println!("{:?}", serde_json::to_string(&all_files))
    }
}
