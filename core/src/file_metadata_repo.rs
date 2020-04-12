use std::option::NoneError;

use rusqlite::{params, Connection, Row};

use crate::error_enum;
use crate::file_metadata::FileMetadata;
use std::ops::Try;

error_enum! {
    enum Error {
        DbError(rusqlite::Error),
        FileRowMissing(NoneError)
    }
}

pub trait FileMetadataRepo {
    fn insert(db: &Connection, file_metadata: &FileMetadata) -> Result<(), Error>;
    fn update(db: &Connection, file_metadata: &FileMetadata) -> Result<(), Error>;
    fn get(db: &Connection, id: &String) -> Result<FileMetadata, Error>;
    fn last_updated(db: &Connection) -> Result<i64, Error>;
    fn dump(db: &Connection) -> Result<Vec<FileMetadata>, Error>;
}

pub struct FileMetadataRepoImpl;

impl FileMetadataRepo for FileMetadataRepoImpl {
    fn insert(db: &Connection, file_metadata: &FileMetadata) -> Result<(), Error> {
        db.execute(
            "INSERT INTO file_metadata (id, name, path, updated_at, status) VALUES (?1, ?2, ?3, ?4, ?5)",
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

    fn update(db: &Connection, file_metadata: &FileMetadata) -> Result<(), Error> {
        db.execute(
            "UPDATE file_metadata SET updated_at = ?2 WHERE id = ?1",
            params![&file_metadata.id, &file_metadata.updated_at],
        )?;
        Ok(())
    }

    fn get(db: &Connection, id: &String) -> Result<FileMetadata, Error> {
        let mut stmt = db.prepare("SELECT * FROM file_metadata WHERE id = ?1")?;

        let mut file_iter = stmt.query_map(params![&id], to_metadata)?;

        let maybe_row = file_iter.next().into_result()?;

        Ok(maybe_row?)
    }

    fn last_updated(db: &Connection) -> Result<i64, Error> {
        let mut stmt = db.prepare("SELECT MAX(updated_at) FROM file_metadata")?;

        let mut file_iter = stmt.query_map(params![], |row| Ok(row.get(0)?))?;

        let maybe_max = file_iter.next().into_result()?;

        Ok(maybe_max?)
    }

    fn dump(db: &Connection) -> Result<Vec<FileMetadata>, Error> {
        let mut stmt = db.prepare("SELECT * FROM file_metadata")?;

        let file_iter = stmt.query_map(params![], to_metadata)?;

        let maybe_row = file_iter
            .filter_map(Result::ok)
            .collect::<Vec<FileMetadata>>();

        Ok(maybe_row)
    }
}

fn to_metadata(row: &Row) -> Result<FileMetadata, rusqlite::Error> {
    Ok(FileMetadata {
        id: row.get(0)?,
        name: row.get(1)?,
        path: row.get(2)?,
        updated_at: row.get(3)?,
        status: row.get(4)?,
    })
}

#[cfg(test)]
mod unit_tests {
    use crate::db_provider::{DbProvider, RamBackedDB};
    use crate::file_metadata::FileMetadata;
    use crate::file_metadata_repo::{FileMetadataRepo, FileMetadataRepoImpl};
    use crate::schema::SchemaCreatorImpl;
    use crate::state::Config;
    use uuid::Uuid;

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
            writeable_path: "ignored".to_string(),
        };

        let db = DefaultDbProvider::connect_to_db(&config).unwrap();
        FileMetadataRepoImpl::insert(&db, &test_file_metadata).unwrap();

        let db_file_metadata = FileMetadataRepoImpl::get(&db, &test_file_metadata.id).unwrap();
        assert_eq!(test_file_metadata, db_file_metadata)
    }

    #[test]
    fn update_file_metadata() {
        let test_meta = FileMetadata {
            id: "test".to_string(),
            name: "".to_string(),
            path: "".to_string(),
            updated_at: 0,
            status: "".to_string(),
        };
        let test_meta_updated = FileMetadata {
            id: "test".to_string(),
            name: "".to_string(),
            path: "".to_string(),
            updated_at: 1000,
            status: "".to_string(),
        };

        let config = &Config {
            writeable_path: "ignored".to_string(),
        };

        let db = DefaultDbProvider::connect_to_db(&config).unwrap();
        FileMetadataRepoImpl::insert(&db, &test_meta).unwrap();
        assert_eq!(
            test_meta,
            FileMetadataRepoImpl::get(&db, &test_meta.id).unwrap()
        );
        FileMetadataRepoImpl::update(&db, &test_meta_updated).unwrap();
        assert_eq!(
            test_meta_updated,
            FileMetadataRepoImpl::get(&db, &test_meta.id).unwrap()
        );
    }

    #[test]
    fn dump_repo_get_max() {
        let test_meta1 = FileMetadata {
            id: Uuid::new_v4().to_string(),
            name: "".to_string(),
            path: "".to_string(),
            updated_at: 0,
            status: "".to_string(),
        };
        let test_meta2 = FileMetadata {
            id: Uuid::new_v4().to_string(),
            name: "".to_string(),
            path: "".to_string(),
            updated_at: 100,
            status: "".to_string(),
        };
        let test_meta3 = FileMetadata {
            id: Uuid::new_v4().to_string(),
            name: "".to_string(),
            path: "".to_string(),
            updated_at: 9000,
            status: "".to_string(),
        };

        let config = &Config {
            writeable_path: "ignored".to_string(),
        };

        let db = DefaultDbProvider::connect_to_db(&config).unwrap();
        FileMetadataRepoImpl::insert(&db, &test_meta1).unwrap();
        FileMetadataRepoImpl::insert(&db, &test_meta2).unwrap();
        FileMetadataRepoImpl::insert(&db, &test_meta3).unwrap();

        let all_files = FileMetadataRepoImpl::dump(&db).unwrap();
        assert_eq!(all_files.len(), 3);

        let updated_max = FileMetadataRepoImpl::last_updated(&db).unwrap();
        assert_eq!(updated_max, 9000);
    }
}
