use std::marker::PhantomData;

use crate::client::{
    ChangeFileContentRequest, Client, CreateFileRequest, GetFileRequest, GetUpdatesRequest,
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

error_enum! {
    enum Error {
        ConnectionFailure(db_provider::Error),
        RetrievalError(repo::account_repo::Error),
        ApiError(client::ClientError),
        MetadataRepoError(repo::file_metadata_repo::Error),
    }
}

pub trait FileMetadataService {
    fn sync(db: &Db, sync: bool) -> Result<Vec<FileMetadata>, Error>;
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
    fn sync(db: &Db, _sync: bool) -> Result<Vec<FileMetadata>, Error> {
        let account = AccountDb::get_account(&db)?;
        let max_updated = match FileMetadataDb::last_updated(db) {
            Ok(max) => max,
            Err(_) => 0,
        };
        debug(format!(
            "Getting updates past {} for {:?}",
            max_updated, account
        ));

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
    use crate::repo::{account_repo, file_metadata_repo, file_repo};
    use crate::repo::account_repo::AccountRepo;
    use crate::repo::db_provider::{TempBackedDB, DbProvider};
    use crate::repo::file_repo::FileRepo;
    use crate::repo::file_metadata_repo::FileMetadataRepo;
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
            Ok(100)
        }

        fn get_all(
            _db: &Db,
        ) -> Result<Vec<file_metadata::FileMetadata>, file_metadata_repo::Error> {
            Ok(vec![
                file_metadata::FileMetadata {
                    id: "a".to_string(),
                    name: "".to_string(),
                    path: "".to_string(),
                    updated_at: 50,
                    version: 50,
                    status: Status::Synced,
                },
                file_metadata::FileMetadata {
                    id: "n".to_string(),
                    name: "".to_string(),
                    path: "".to_string(),
                    updated_at: 75,
                    version: 75,
                    status: Status::Local,
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
            unimplemented!()
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
                username: "jimmyjohn".to_string(),
                keys: RsaCryptoService::generate_key().expect("Key generation failure"),
            })
        }
    }

    struct ClientFake;
    impl Client for ClientFake {
        fn new_account(_params: &NewAccountRequest) -> Result<(), ClientError> {
            Ok(())
        }

        fn get_updates(_params: &GetUpdatesRequest) -> Result<Vec<FileMetadata>, ClientError> {
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

        fn get_file(_params: &GetFileRequest) -> Result<File, ClientError> {
            unimplemented!()
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

        fn get_account(_db: &Db) -> Result<Account, account_repo::Error> {
            Ok(Account {
                username: "jimmyjohn".to_string(),
                keys: RsaCryptoService::generate_key().expect("Key generation failure"),
            })
        }
    }
}
