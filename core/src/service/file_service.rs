use sled::Db;

use crate::model::file::File;
use crate::model::file_metadata::{FileMetadata, Status};
use crate::repo::file_metadata_repo;
use crate::repo::file_metadata_repo::FileMetadataRepo;
use crate::repo::file_repo;
use crate::repo::file_repo::FileRepo;
use crate::{error_enum, info};
use serde::export::PhantomData;

error_enum! {
    enum Error {
        FileRepo(file_repo::Error),
        MetaRepo(file_metadata_repo::Error),
    }
}

pub trait FileService {
    fn update(db: &Db, id: String, content: String) -> Result<bool, Error>;
    fn get(db: &Db, id: String) -> Result<File, Error>;
}

pub struct FileServiceImpl<FileMetadataDb: FileMetadataRepo, FileDb: FileRepo> {
    metadatas: PhantomData<FileMetadataDb>,
    files: PhantomData<FileDb>,
}

impl<FileMetadataDb: FileMetadataRepo, FileDb: FileRepo> FileService
    for FileServiceImpl<FileMetadataDb, FileDb>
{
    fn update(db: &Db, id: String, content: String) -> Result<bool, Error> {
        FileDb::update(
            db,
            &File {
                id: id.clone(),
                content: content.clone(),
            },
        )?;
        let meta = FileMetadataDb::get(db, &id)?;
        FileMetadataDb::update(
            db,
            &FileMetadata {
                id: id.clone(),
                name: meta.name,
                path: meta.path,
                updated_at: 0,
                version: meta.version,
                status: if meta.status == Status::New {
                    Status::New
                } else {
                    Status::Local
                },
            },
        )?;
        info(format!("Updated file {:?} contents {:?}", &id, &content));
        Ok(true)
    }

    fn get(db: &Db, id: String) -> Result<File, Error> {
        info(format!("Getting file contents {:?}", &id));
        Ok(FileDb::get(db, &id)?)
    }
}
