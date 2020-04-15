use std::marker::PhantomData;
use std::time::SystemTime;

use crate::client::{Client, ClientError, CreateFileRequest, GetUpdatesRequest};
use crate::error_enum;
use crate::model::file_metadata::{FileMetadata, Status};
use crate::repo;
use crate::repo::account_repo::AccountRepo;
use crate::repo::db_provider;
use crate::repo::file_metadata_repo::FileMetadataRepo;
use crate::API_LOC;
use rusqlite::Connection;

error_enum! {
    enum Error {
        ConnectionFailure(db_provider::Error),
        RetrievalError(repo::account_repo::Error),
        ApiError(client::ClientError),
        MetadataRepoError(repo::file_metadata_repo::Error),
        SystemTimeError(std::time::SystemTimeError),
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
        debug(format!("Getting updates past {}", max_updated));
        if (sync) {
            let updates = ApiClient::get_updates(&GetUpdatesRequest {
                username: account.username.to_string(),
                // FIXME: Real auth...
                auth: "JUNKAUTH".to_string(),
                since_version: max_updated,
            })
            .map(|metadatas| {
                metadatas
                    .into_iter()
                    .map(|t| FileMetadata {
                        id: t.file_id,
                        name: t.file_name,
                        path: t.file_path,
                        updated_at: t.file_metadata_version,
                        // TODO: Fix this so status is tracked accurately
                        status: Status::Remote,
                    })
                    .collect::<Vec<FileMetadata>>()
            })?;
            debug(format!("Updates {:?}", updates));
            updates.into_iter().for_each(|meta| {
                match FileMetadataDb::update(
                    &db,
                    &FileMetadata {
                        id: meta.id,
                        name: meta.name,
                        path: meta.path,
                        updated_at: meta.updated_at,
                        status: Status::Synced,
                    },
                ) {
                    Ok(_) => {}
                    Err(err) => {
                        error(format!("Insert Error {:?}", err));
                    }
                }
            });
        }
        let mut all_meta = FileMetadataDb::get_all(&db)?;
        all_meta.retain(|f| f.status == Status::Local);
        debug(format!("Local {:?}", all_meta));

        if (sync) {
            all_meta.into_iter().for_each(|meta| {
                let meta_copy = meta.clone();
                match ApiClient::create_file(&CreateFileRequest {
                    username: account.username.to_string(),
                    auth: "JUNKAUTH".to_string(),
                    file_id: meta.id,
                    file_name: meta.name,
                    file_path: meta.path,
                    file_content: "JUNKCONTENT".to_string(),
                }) {
                    Ok(_) => {
                        info(format!("Uploaded file!"));
                        match FileMetadataDb::update(
                            &db,
                            &FileMetadata {
                                id: meta_copy.id,
                                name: meta_copy.name,
                                path: meta_copy.path,
                                updated_at: meta.updated_at,
                                status: Status::Synced,
                            },
                        ) {
                            Ok(_) => info(format!("Updated file locally")),
                            Err(err) => {
                                error(format!("Failed to update file locally! Error {:?}", err))
                            }
                        }
                    }
                    Err(err) => error(format!("Upload error {:?}", err)),
                }
            });
        }

        Ok(FileMetadataDb::get_all(&db)?)
    }

    fn create(db: &Db, name: String, path: String) -> Result<FileMetadata, Error> {
        let meta = FileMetadata {
            id: Uuid::new_v4().to_string(),
            name,
            path,
            updated_at: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)?
                .as_micros() as u64,
            status: Status::Local,
        };

        FileMetadataDb::insert(&db, &meta)?;
        Ok(meta)
    }
}

#[cfg(test)]
mod unit_tests {
    use crate::client::{Client, ClientError, FileMetadata, GetUpdatesRequest, NewAccountRequest};
    use crate::crypto::{PubKeyCryptoService, RsaCryptoService};
    use crate::debug;
    use crate::model::account::Account;
    use crate::model::file_metadata;
    use crate::model::file_metadata::Status;
    use crate::model::state::Config;
    use crate::repo::account_repo;
    use crate::repo::account_repo::{AccountRepo, AccountRepoImpl};
    use crate::repo::db_provider::{DbProvider, TempBackedDB};
    use crate::repo::file_metadata_repo::FileMetadataRepoImpl;
    use crate::service::file_metadata_service::{FileMetadataService, FileMetadataServiceImpl};
    use sled::Db;

    type DefaultDbProvider = TempBackedDB;

    type DefaultDbProvider = TempBackedDB;
    struct FileMetaRepoFake;
    impl FileMetadataRepo for FileMetaRepoFake {
        fn insert(
            db: &Db,
            file_metadata: &file_metadata::FileMetadata,
        ) -> Result<(), file_metadata_repo::Error> {
            unimplemented!()
        }

        fn update(
            db: &Db,
            file_metadata: &file_metadata::FileMetadata,
        ) -> Result<(), file_metadata_repo::Error> {
            debug(format!("Updating in DB {:?}", file_metadata));
            Ok(())
        }

        fn get(
            db: &Db,
            id: &String,
        ) -> Result<file_metadata::FileMetadata, file_metadata_repo::Error> {
            unimplemented!()
        }

        fn last_updated(db: &Db) -> Result<u64, file_metadata_repo::Error> {
            Ok(100)
        }

        fn get_all(db: &Db) -> Result<Vec<file_metadata::FileMetadata>, file_metadata_repo::Error> {
            Ok(vec![
                file_metadata::FileMetadata {
                    id: "a".to_string(),
                    name: "".to_string(),
                    path: "".to_string(),
                    updated_at: 50,
                    status: Status::Synced,
                },
                file_metadata::FileMetadata {
                    id: "n".to_string(),
                    name: "".to_string(),
                    path: "".to_string(),
                    updated_at: 75,
                    status: Status::Local,
                },
            ])
        }
    }

    struct ClientFake;
    impl Client for ClientFake {
        fn new_account(params: &NewAccountRequest) -> Result<(), ClientError> {
            Ok(())
        }

        fn get_updates(params: &GetUpdatesRequest) -> Result<Vec<FileMetadata>, ClientError> {
            Ok(vec![
                FileMetadata {
                    file_id: "a".to_string(),
                    file_name: "".to_string(),
                    file_path: "".to_string(),
                    file_content_version: 0,
                    file_metadata_version: 50,
                    deleted: false,
                },
                FileMetadata {
                    file_id: "b".to_string(),
                    file_name: "".to_string(),
                    file_path: "".to_string(),
                    file_content_version: 0,
                    file_metadata_version: 100,
                    deleted: false,
                },
                FileMetadata {
                    file_id: "c".to_string(),
                    file_name: "".to_string(),
                    file_path: "".to_string(),
                    file_content_version: 0,
                    file_metadata_version: 150,
                    deleted: false,
                },
            ])
        }

        fn create_file(params: &CreateFileRequest) -> Result<u64, ClientError> {
            debug(format!("Uploading to server {:?}", params));
            Ok(1)
        }
    }
    struct AccountRepoFake;
    impl AccountRepo for AccountRepoFake {
        fn insert_account(db: &Db, account: &Account) -> Result<(), account_repo::Error> {
            unimplemented!()
        }

        fn get_account(db: &Db) -> Result<Account, account_repo::Error> {
            Ok(Account {
                username: "jimmyjohn".to_string(),
                keys: RsaCryptoService::generate_key().expect("Key generation failure"),
            },
        )
        .unwrap();

        assert_eq!(FileMetadataRepoImpl::get_all(&db).unwrap().len(), 2);

        let all_files = DefaultFileMetadataService::update(&db, true).unwrap();

        assert_eq!(all_files.len(), 4);
    }
}
