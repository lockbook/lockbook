use std::marker::PhantomData;

use crate::client::{
    ChangeFileContentRequest, Client, ClientError, CreateFileRequest, GetFileRequest,
    GetUpdatesRequest,
};
use crate::model::file_metadata::Status::Local;
use crate::model::file_metadata::{FileMetadata, Status};
use crate::repo;
use crate::repo::account_repo::AccountRepo;
use crate::repo::db_provider;
use crate::repo::file_metadata_repo::FileMetadataRepo;
use crate::repo::file_repo::FileRepo;
use crate::{client, debug, error, error_enum, info};
use sled::Db;
use std::borrow::Borrow;

error_enum! {
    enum Error {
        ConnectionFailure(db_provider::Error),
        RetrievalError(repo::account_repo::Error),
        ApiError(client::ClientError),
        MetadataRepoError(repo::file_metadata_repo::Error),
    }
}

pub trait FileMetadataService {
    fn sync(db: &Db) -> Result<Vec<FileMetadata>, Error>;
    fn create(db: &Db, name: String, path: String) -> Result<FileMetadata, Error>;
}

pub struct FileMetadataServiceImpl<
    FileMetadataDb: FileMetadataRepo,
    FileDb: FileRepo,
    AccountDb: AccountRepo,
    ApiClient: Client,
> {
    metadatas: PhantomData<FileMetadataDb>,
    files: PhantomData<FileDb>,
    accounts: PhantomData<AccountDb>,
    client: PhantomData<ApiClient>,
}

impl<
        FileMetadataDb: FileMetadataRepo,
        FileDb: FileRepo,
        AccountDb: AccountRepo,
        ApiClient: Client,
    > FileMetadataService
    for FileMetadataServiceImpl<FileMetadataDb, FileDb, AccountDb, ApiClient>
{
    fn sync(db: &Db) -> Result<Vec<FileMetadata>, Error> {
        // Load user's account
        let account = AccountDb::get_account(&db)?;
        // Get the last synced file
        let max_updated = match FileMetadataDb::last_updated(db) {
            Ok(max) => max,
            Err(_) => 0,
        };
        // Get remote updates from the last synced file onwards
        info(format!("Getting updates past {}", max_updated));
        let updates_remote = ApiClient::get_updates(&GetUpdatesRequest {
            username: account.username.to_string(),
            auth: "JUNK_AUTH".to_string(),
            since_version: max_updated,
        })?;
        debug(format!("Remote Updates {:?}", updates_remote));

        // Create all the "new" files
        let mut all_files = FileMetadataDb::get_all(&db)?;
        all_files.retain(|file| file.status == Status::New);
        all_files.iter().for_each(|file| {
            match ApiClient::create_file(&CreateFileRequest {
                username: account.username.to_string(),
                auth: "JUNK_AUTH".to_string(),
                file_id: file.id.to_string(),
                file_name: file.name.to_string(),
                file_path: file.path.to_string(),
                file_content: "".to_string(),
            }) {
                Ok(version) => {
                    // Mark as "synced" on success
                    FileMetadataDb::update(
                        db,
                        &FileMetadata {
                            id: file.id.to_string(),
                            name: file.name.to_string(),
                            path: file.path.to_string(),
                            updated_at: version,
                            version: version,
                            status: Status::Local,
                        },
                    )
                    .unwrap();
                    debug("CREATE -- SUCCESS".to_string())
                }
                Err(err) => error(format!("CREATE -- FAILURE: {:?}", err)),
            }
        });
        let mut updates_local = FileMetadataDb::get_all(db)?;
        updates_local.retain(|file| file.status == Status::Local);
        updates_local.iter().for_each(|file| {
            let content = FileDb::get(db, file.id.borrow()).unwrap().content;

            let new_version = ApiClient::change_file(&ChangeFileContentRequest {
                username: account.username.to_string(),
                auth: "JUNK_AUTH".to_string(),
                file_id: file.id.to_string(),
                old_file_version: file.version,
                new_file_content: content,
            })
            .unwrap();
            FileMetadataDb::update(
                db,
                &FileMetadata {
                    id: file.id.to_string(),
                    name: file.name.to_string(),
                    path: file.path.to_string(),
                    updated_at: file.updated_at,
                    version: new_version,
                    status: Status::Synced,
                },
            )
            .unwrap();
            ()
        });
        updates_remote.iter().for_each(|file| {
            let content = ApiClient::get_file(&GetFileRequest {
                file_id: file.file_id.to_string(),
            })
            .unwrap();
            FileDb::update(db, &content).unwrap();
            FileMetadataDb::update(
                db,
                &FileMetadata {
                    id: file.file_id.to_string(),
                    name: file.file_name.to_string(),
                    path: file.file_path.to_string(),
                    updated_at: file.file_metadata_version,
                    version: file.file_content_version,
                    status: Status::Synced,
                },
            )
            .unwrap();
            ()
        });
        Ok(FileMetadataDb::get_all(&db)?)
    }

    fn create(db: &Db, name: String, path: String) -> Result<FileMetadata, Error> {
        let meta = FileMetadataDb::insert(&db, &name, &path)?;
        Ok(meta)
    }
}

#[cfg(test)]
mod unit_tests {
    use crate::client::{
        ChangeFileContentRequest, Client, ClientError, CreateFileRequest, FileMetadata,
        GetFileRequest, GetUpdatesRequest, NewAccountRequest,
    };
    use crate::debug;
    use crate::model::account::Account;
    use crate::model::file::File;
    use crate::model::file_metadata;
    use crate::model::file_metadata::Status;
    use crate::model::state::Config;
    use crate::repo::account_repo::AccountRepo;
    use crate::repo::db_provider::{DbProvider, TempBackedDB};
    use crate::repo::file_metadata_repo::FileMetadataRepo;
    use crate::repo::file_repo::FileRepo;
    use crate::repo::{account_repo, file_metadata_repo, file_repo};
    use crate::service::crypto::{PubKeyCryptoService, RsaCryptoService};
    use crate::service::file_metadata_service::{FileMetadataService, FileMetadataServiceImpl};
    use sled::Db;

    struct FileMetaRepoFake;
    impl FileMetadataRepo for FileMetaRepoFake {
        fn insert(
            _db: &Db,
            _name: &String,
            _path: &String,
        ) -> Result<file_metadata::FileMetadata, file_metadata_repo::Error> {
            unimplemented!()
        }

        fn update(
            _db: &Db,
            file_metadata: &file_metadata::FileMetadata,
        ) -> Result<file_metadata::FileMetadata, file_metadata_repo::Error> {
            debug(format!("Updating in DB {:?}", file_metadata));
            Ok(file_metadata.clone())
        }

        fn get(
            _db: &Db,
            _id: &String,
        ) -> Result<file_metadata::FileMetadata, file_metadata_repo::Error> {
            unimplemented!()
        }

        fn last_updated(_db: &Db) -> Result<u64, file_metadata_repo::Error> {
            Ok(75)
        }

        fn get_all(
            _db: &Db,
        ) -> Result<Vec<file_metadata::FileMetadata>, file_metadata_repo::Error> {
            Ok(vec![
                file_metadata::FileMetadata {
                    id: "some_uuid_1".to_string(),
                    name: "First File".to_string(),
                    path: "/first".to_string(),
                    updated_at: 50,
                    version: 50,
                    status: Status::Synced,
                },
                file_metadata::FileMetadata {
                    id: "some_uuid_2".to_string(),
                    name: "Second File".to_string(),
                    path: "/second".to_string(),
                    updated_at: 75,
                    version: 75,
                    status: Status::Synced,
                },
                file_metadata::FileMetadata {
                    id: "some_uuid_3".to_string(),
                    name: "Third File".to_string(),
                    path: "/third".to_string(),
                    updated_at: 100,
                    version: 100,
                    status: Status::New,
                },
            ])
        }

        fn delete(_db: &Db, _id: &String) -> Result<u64, file_metadata_repo::Error> {
            unimplemented!()
        }
    }

    struct FileRepoFake;
    impl FileRepo for FileRepoFake {
        fn update(db: &Db, file: &File) -> Result<(), file_repo::Error> {
            Ok(())
        }

        fn get(db: &Db, id: &String) -> Result<File, file_repo::Error> {
            unimplemented!()
        }

        fn delete(db: &Db, id: &String) -> Result<(), file_repo::Error> {
            unimplemented!()
        }
    }

    struct AccountRepoFake;
    impl AccountRepo for AccountRepoFake {
        fn insert_account(_db: &Db, _account: &Account) -> Result<(), account_repo::Error> {
            unimplemented!()
        }

        fn get_account(_db: &Db) -> Result<Account, account_repo::Error> {
            Ok(Account {
                username: "lockbooker".to_string(),
                keys: RsaCryptoService::generate_key().expect("Key generation failure"),
            })
        }
    }

    struct ClientFake;
    impl Client for ClientFake {
        fn new_account(_params: &NewAccountRequest) -> Result<(), ClientError> {
            Ok(())
        }

        fn get_updates(params: &GetUpdatesRequest) -> Result<Vec<FileMetadata>, ClientError> {
            let mut metas = vec![
                FileMetadata {
                    file_id: "some_uuid_1".to_string(),
                    file_name: "First File".to_string(),
                    file_path: "/first".to_string(),
                    file_content_version: 0,
                    file_metadata_version: 50,
                    deleted: false,
                },
                FileMetadata {
                    file_id: "some_uuid_2".to_string(),
                    file_name: "Second File".to_string(),
                    file_path: "/second".to_string(),
                    file_content_version: 0,
                    file_metadata_version: 100,
                    deleted: false,
                },
                FileMetadata {
                    file_id: "some_uuid_4".to_string(),
                    file_name: "Fourth File".to_string(),
                    file_path: "/fourth".to_string(),
                    file_content_version: 0,
                    file_metadata_version: 150,
                    deleted: false,
                },
            ];
            metas.retain(|meta| meta.file_metadata_version > params.since_version);
            Ok(metas)
        }

        fn get_file(params: &GetFileRequest) -> Result<File, ClientError> {
            Ok(File {
                id: params.file_id.to_string(),
                content: "SOME CONTENT".to_string(),
            })
        }

        fn create_file(params: &CreateFileRequest) -> Result<u64, ClientError> {
            debug(format!("Uploading to server {:?}", params));
            Ok(1)
        }

        fn change_file(_params: &ChangeFileContentRequest) -> Result<u64, ClientError> {
            unimplemented!()
        }
    }

    type DefaultDbProvider = TempBackedDB;
    type DefaultFileMetadataService =
        FileMetadataServiceImpl<FileMetaRepoFake, FileRepoFake, AccountRepoFake, ClientFake>;

    #[test]
    fn test_sync() {
        let config = Config {
            writeable_path: "ignored".to_string(),
        };
        let db = &DefaultDbProvider::connect_to_db(&config).unwrap();

        let metas = DefaultFileMetadataService::sync(db).unwrap();
        print!("Metas: {:?}", metas)
    }
}
