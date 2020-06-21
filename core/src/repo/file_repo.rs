use crate::error_enum;
use crate::model::crypto::*;
use sled::Db;
use uuid::Uuid;

error_enum! {
    enum Error {
        SledError(sled::Error),
        SerdeError(serde_json::Error),
        FileRowMissing(())
    }
}

pub trait FileRepo {
    fn update(db: &Db, id: Uuid, file: &EncryptedFile) -> Result<(), Error>;
    fn get(db: &Db, id: Uuid) -> Result<EncryptedFile, Error>;
    fn delete(db: &Db, id: Uuid) -> Result<(), Error>;
}

pub struct FileRepoImpl;

impl FileRepo for FileRepoImpl {
    fn update(db: &Db, id: Uuid, file: &EncryptedFile) -> Result<(), Error> {
        let tree = db.open_tree(b"files")?;
        tree.insert(id.as_bytes(), serde_json::to_vec(file)?)?;
        Ok(())
    }

    fn get(db: &Db, id: Uuid) -> Result<EncryptedFile, Error> {
        let tree = db.open_tree(b"files")?;
        let maybe_value = tree.get(id.as_bytes())?;
        let value = maybe_value.ok_or(())?;
        let file: EncryptedFile = serde_json::from_slice(value.as_ref())?;

        Ok(file)
    }

    fn delete(db: &Db, id: Uuid) -> Result<(), Error> {
        let tree = db.open_tree(b"files")?;
        tree.remove(id.as_bytes())?;
        Ok(())
    }
}

#[cfg(test)]
mod unit_tests {
    use crate::model::crypto::*;
    use crate::model::state::Config;
    use crate::repo::db_provider::{DbProvider, TempBackedDB};
    use crate::repo::file_repo::{FileRepo, FileRepoImpl};
    use uuid::Uuid;

    type DefaultDbProvider = TempBackedDB;

    #[test]
    fn update_file() {
        let test_file = EncryptedFile {
            content: EncryptedValueWithNonce {
                garbage: "something".to_string(),
                nonce: "nonce1".to_string(),
            },
        };

        let config = Config {
            writeable_path: "ignored".to_string(),
        };
        let db = DefaultDbProvider::connect_to_db(&config).unwrap();
        let file_id = Uuid::new_v4();

        FileRepoImpl::update(&db, file_id, &test_file).unwrap();

        let file = FileRepoImpl::get(&db, file_id).unwrap();
        assert_eq!(
            file.content,
            EncryptedValueWithNonce {
                garbage: "something".to_string(),
                nonce: "nonce1".to_string()
            }
        );

        FileRepoImpl::update(
            &db,
            file_id,
            &EncryptedFile {
                content: EncryptedValueWithNonce {
                    garbage: "updated".to_string(),
                    nonce: "nonce2".to_string(),
                },
            },
        )
        .unwrap();

        let file_updated = FileRepoImpl::get(&db, file_id).unwrap();

        assert_eq!(
            file_updated.content,
            EncryptedValueWithNonce {
                garbage: "updated".to_string(),
                nonce: "nonce2".to_string(),
            }
        );
    }
}
