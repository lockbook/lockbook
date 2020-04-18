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
use crate::{client, error};
use crate::{debug, error_enum, info};
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
    fn sync(db: &Db, sync: bool) -> Result<Vec<FileMetadata>, Error> {
        let account = AccountDb::get_account(&db)?;
        let max_updated = match FileMetadataDb::last_updated(db) {
            Ok(max) => max,
            Err(_) => 0,
        };
        debug(format!("Getting updates past {}", max_updated));
        if sync {
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
                        version: t.file_content_version,
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
                        version: meta.version,
                        status: Status::Synced,
                    },
                ) {
                    Ok(meta) => match ApiClient::get_file(&GetFileRequest { file_id: meta.id }) {
                        Ok(file) => {
                            info(format!("Retrieved file from bucket!"));
                            match FileDb::update(&db, &file) {
                                Ok(_) => info(format!("Saved file to db!")),
                                Err(err) => error(format!("Failed saving file to db! {:?}", err)),
                            }
                        }
                        Err(err) => error(format!(
                            "Failed to retrieve file from bucket! Error: {:?}",
                            err
                        )),
                    },
                    Err(err) => {
                        error(format!("Insert Error {:?}", err));
                    }
                }
            });
        }
        let mut all_meta = FileMetadataDb::get_all(&db)?;
        all_meta.retain(|f| (f.status == Status::New || f.status == Local));
        debug(format!("Local {:?}", all_meta));

        if sync {
            all_meta.into_iter().for_each(|meta| {
                let meta_copy = meta.clone();
                let meta_copy2 = meta.clone();
                let file_id = meta.id.clone();
                if meta.status == Status::New {
                    match ApiClient::create_file(&CreateFileRequest {
                        username: account.username.to_string(),
                        auth: "JUNKAUTH".to_string(),
                        file_id: meta.id,
                        file_name: meta.name,
                        file_path: meta.path,
                        file_content: "JUNKCONTENT".to_string(),
                    }) {
                        Ok(version) => {
                            info(format!("Uploaded file!"));
                            match FileMetadataDb::update(
                                &db,
                                &FileMetadata {
                                    id: meta_copy.id,
                                    name: meta_copy.name,
                                    path: meta_copy.path,
                                    updated_at: meta.updated_at,
                                    version,
                                    status: Status::Local,
                                },
                            ) {
                                Ok(_) => info(format!("Updated file locally")),
                                Err(err) => {
                                    error(format!("Failed to update file locally! Error {:?}", err))
                                }
                            }
                        }
                        Err(err) => error(format!("Upload meta error {:?}", err)),
                    }
                }
                match FileDb::get(&db, &file_id) {
                    Ok(file) => {
                        match ApiClient::change_file(&ChangeFileContentRequest {
                            username: account.username.to_string(),
                            auth: "JUNKAUTH".to_string(),
                            file_id: file_id.clone(),
                            old_file_version: meta.version,
                            new_file_content: file.content,
                        }) {
                            Ok(new_version) => {
                                info(format!("Uploaded file contents"));
                                match FileMetadataDb::update(
                                    &db,
                                    &FileMetadata {
                                        id: meta_copy2.id,
                                        name: meta_copy2.name,
                                        path: meta_copy2.path,
                                        updated_at: 0,
                                        version: new_version,
                                        status: Status::Synced,
                                    },
                                ) {
                                    Ok(_) => info(format!("Updated metadata version")),
                                    Err(_) => error(format!("Failed to update metadata version")),
                                }
                            }
                            Err(err) => error(format!("Upload contents error {:?}", err)),
                        }
                    }
                    Err(err) => error(format!("Failed getting file contents! Error: {:?}", err)),
                }
            });
        }

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
    use crate::repo::account_repo::AccountRepo;
    use crate::repo::file_metadata_repo::FileMetadataRepo;
    use crate::repo::{account_repo, file_metadata_repo};
    use crate::service::crypto::{PubKeyCryptoService, RsaCryptoService};
    use sled::Db;

    #[allow(dead_code)]
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

    #[allow(dead_code)]
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

    #[allow(dead_code)]
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
}
