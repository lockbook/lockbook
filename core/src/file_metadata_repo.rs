use std::option::NoneError;

use rusqlite::{params, Connection};

use crate::file_metadata::FileMetadata;
use crate::error_enum;
use std::ops::Try;

error_enum! {
    enum Error {
        DbError(rusqlite::Error),
        FileRowMissing(NoneError)
    }
}

pub trait FileMetadataRepo {
    fn insert_file_metadata(db: &Connection, file_metadata: &FileMetadata) -> Result<(), Error>;
    fn get_file_metadata(db: &Connection, id: &String) -> Result<FileMetadata, Error>;
}

pub struct FileMetadataRepoImpl;

impl FileMetadataRepo for FileMetadataRepoImpl {
    fn insert_file_metadata(db: &Connection, file_metadata: &FileMetadata) -> Result<(), Error> {
        db.execute(
            "insert into file_metadata (id, name, path, updated_at, status) values (?1, ?2, ?3, ?4, ?5)",
            params![
                &file_metadata.id,
                &file_metadata.name,
                &file_metadata.path,
                &file_metadata.updated_at,
                &file_metadata.status,
            ],
        )?;
        Ok(())
    }

    fn get_file_metadata(db: &Connection, id: &String) -> Result<FileMetadata, Error> {
        let mut stmt = db.prepare(
            "select * from file_metadata where id = ?1",
        )?;

        let mut file_iter = stmt.query_map(params![&id], |row| {
            Ok(FileMetadata {
                id: row.get(0)?,
                name: row.get(1)?,
                path: row.get(2)?,
                updated_at: row.get(3)?,
                status: row.get(4)?,
            }
            )
        })?;

        let maybe_row = file_iter.next().into_result()?;

        Ok(maybe_row?)
    }
}

#[cfg(test)]
mod unit_tests {
    use uuid::Uuid;
    use crate::file_metadata::FileMetadata;
    use crate::state::Config;
    use crate::db_provider::{RamBackedDB, DbProvider};
    use crate::schema::SchemaCreatorImpl;
    use crate::file_metadata_repo::{FileMetadataRepoImpl, FileMetadataRepo};

    type DefaultDbProvider = RamBackedDB<SchemaCreatorImpl>;

    #[test]
    fn insert_file_metadata() {
        let test_file_metadata = FileMetadata {
            id: Uuid::new_v4().to_string(),
            name: "test_file".to_string(),
            path: "a/b/c".to_string(),
            updated_at: 1234,
            status: "".to_string(),
        };

        let config = &Config {
            writeable_path: "ignored".to_string()
        };

        let db = DefaultDbProvider::connect_to_db(&config).unwrap();
        FileMetadataRepoImpl::insert_file_metadata(&db, &test_file_metadata).unwrap();

        let db_file_metadata = FileMetadataRepoImpl::get_file_metadata(&db, &test_file_metadata.id).unwrap();
        assert_eq!(test_file_metadata, db_file_metadata)
    }
}