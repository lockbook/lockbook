use sled::Db;
use uuid::Uuid;

use crate::model::crypto::*;

#[derive(Debug)]
pub enum Error {
    SledError(sled::Error),
    SerdeError(serde_json::Error),
    FileRowMissing(()), // TODO remove from insert
}

#[derive(Debug)]
pub enum DbError {
    SledError(sled::Error),
    SerdeError(serde_json::Error),
}

pub trait DocumentRepo {
    fn insert(db: &Db, id: Uuid, document: &EncryptedDocument) -> Result<(), Error>;
    fn get(db: &Db, id: Uuid) -> Result<EncryptedDocument, Error>;
    fn maybe_get(db: &Db, id: Uuid) -> Result<Option<EncryptedDocument>, DbError>;
    fn delete(db: &Db, id: Uuid) -> Result<(), Error>;
}

pub struct DocumentRepoImpl;

impl DocumentRepo for DocumentRepoImpl {
    fn insert(db: &Db, id: Uuid, document: &EncryptedDocument) -> Result<(), Error> {
        let tree = db.open_tree(b"documents").map_err(Error::SledError)?;
        tree.insert(
            id.as_bytes(),
            serde_json::to_vec(document).map_err(Error::SerdeError)?,
        )
        .map_err(Error::SledError)?;
        Ok(())
    }

    fn get(db: &Db, id: Uuid) -> Result<EncryptedDocument, Error> {
        let tree = db.open_tree(b"documents").map_err(Error::SledError)?;
        let maybe_value = tree.get(id.as_bytes()).map_err(Error::SledError)?;
        let value = maybe_value.ok_or(()).map_err(Error::FileRowMissing)?;
        let document: EncryptedDocument =
            serde_json::from_slice(value.as_ref()).map_err(Error::SerdeError)?;

        Ok(document)
    }

    fn maybe_get(db: &Db, id: Uuid) -> Result<Option<EncryptedDocument>, DbError> {
        let tree = db.open_tree(b"documents").map_err(DbError::SledError)?;
        match tree.get(id.as_bytes()).map_err(DbError::SledError)? {
            None => Ok(None),
            Some(file) => {
                let document: EncryptedDocument =
                    serde_json::from_slice(file.as_ref()).map_err(DbError::SerdeError)?;

                Ok(Some(document))
            }
        }
    }

    fn delete(db: &Db, id: Uuid) -> Result<(), Error> {
        let tree = db.open_tree(b"documents").map_err(Error::SledError)?;
        tree.remove(id.as_bytes()).map_err(Error::SledError)?;
        Ok(())
    }
}

#[cfg(test)]
mod unit_tests {
    use uuid::Uuid;

    use crate::model::crypto::*;
    use crate::model::state::dummy_config;
    use crate::repo::db_provider::{DbProvider, TempBackedDB};
    use crate::repo::document_repo::{DocumentRepo, DocumentRepoImpl};

    type DefaultDbProvider = TempBackedDB;

    #[test]
    fn update_document() {
        let test_document = EncryptedDocument::new("something", "nonce1");

        let config = dummy_config();
        let db = DefaultDbProvider::connect_to_db(&config).unwrap();
        let document_id = Uuid::new_v4();

        DocumentRepoImpl::insert(&db, document_id, &test_document).unwrap();

        let document = DocumentRepoImpl::get(&db, document_id).unwrap();
        assert_eq!(
            document,
            EncryptedDocument::new("something", "nonce1"),
        );

        DocumentRepoImpl::insert(
            &db,
            document_id,
            &EncryptedDocument::new("updated", "nonce2"),
        )
        .unwrap();

        let file_updated = DocumentRepoImpl::get(&db, document_id).unwrap();

        assert_eq!(
            file_updated,
            EncryptedDocument::new("updated", "nonce2")
        );
    }
}
